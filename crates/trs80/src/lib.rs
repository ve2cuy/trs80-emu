//! Le TRS-80 Model 1 : ROM Level II, 48 Ko de RAM, clavier et vidéo 64 × 16.
//!
//! - `no_std`, sans allocation : la machine tient dans une seule structure.
//! - [`Trs80::run_frame`] exécute 1/60 de seconde de temps machine.
//! - [`Trs80::render`] dessine l'écran (384 × 192 pixels, RGBA).
//! - [`Trs80::load_cmd`] charge et lance un programme `.CMD`.
#![no_std]

mod cmd;
mod font;
mod keyboard;
mod video;

pub use cmd::CmdError;
pub use keyboard::Key;
pub use video::{SCREEN_HEIGHT, SCREEN_WIDTH};

use keyboard::Keyboard;
use z80::{Bus, Cpu};

/// Fréquence du Z80 du Model 1.
pub const CLOCK_HZ: u32 = 1_774_080;
/// T-states exécutés par image, à 60 images par seconde.
pub const CYCLES_PER_FRAME: u32 = CLOCK_HZ / 60;
/// Taille de la ROM Level II.
pub const ROM_SIZE: usize = 0x3000;

const VIDEO_START: u16 = 0x3C00;
const RAM_START: u16 = 0x4000;

/// Erreurs de la machine. Les messages sont en anglais : ils s'affichent dans l'interface web.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// La ROM fournie n'a pas la taille de la ROM Level II (12 Ko).
    BadRomSize(usize),
    /// Fichier `.CMD` invalide.
    Cmd(CmdError),
    /// Le BASIC n'a jamais atteint « READY » (impossible de charger un programme).
    NotReady,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::BadRomSize(n) => {
                write!(f, "a Level II ROM must be {ROM_SIZE} bytes (this file is {n} bytes)")
            }
            Error::Cmd(e) => write!(f, "{e}"),
            Error::NotReady => write!(f, "BASIC did not reach READY"),
        }
    }
}

/// Tout ce qui est branché sur le bus du Z80.
struct Board {
    rom: [u8; ROM_SIZE],
    video: [u8; 1024],
    ram: [u8; 0x10000 - RAM_START as usize],
    keyboard: Keyboard,
    /// Mode 32 caractères par ligne (bit 3 du port FF).
    wide: bool,
}

impl Bus for Board {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x2FFF => self.rom[addr as usize],
            0x3800..=0x3BFF => self.keyboard.read(addr as u8),
            0x3C00..=0x3FFF => self.video[(addr - VIDEO_START) as usize],
            0x4000..=0xFFFF => self.ram[(addr - RAM_START) as usize],
            // Interface d'expansion absente (37E0-37FF) et zones vides : bus flottant.
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x3C00..=0x3FFF => self.video[(addr - VIDEO_START) as usize] = val,
            0x4000..=0xFFFF => self.ram[(addr - RAM_START) as usize] = val,
            _ => {}
        }
    }

    fn output(&mut self, port: u16, val: u8) {
        if port as u8 == 0xFF {
            self.wide = val & 0x08 != 0;
        }
    }
}

/// Un TRS-80 Model 1 complet.
pub struct Trs80 {
    cpu: Cpu,
    board: Board,
    /// T-states exécutés en trop à l'image précédente (une instruction déborde souvent).
    overshoot: u32,
}

impl Trs80 {
    /// Crée la machine à partir de la ROM Level II (12 Ko) et la démarre.
    pub fn new(rom: &[u8]) -> Result<Self, Error> {
        if rom.len() != ROM_SIZE {
            return Err(Error::BadRomSize(rom.len()));
        }
        let mut board = Board {
            rom: [0; ROM_SIZE],
            video: [0x20; 1024],
            ram: [0; 0x10000 - RAM_START as usize],
            keyboard: Keyboard::new(),
            wide: false,
        };
        board.rom.copy_from_slice(rom);
        Ok(Trs80 { cpu: Cpu::new(), board, overshoot: 0 })
    }

    /// Bouton RESET : redémarre le processeur (la RAM est conservée, comme sur la vraie machine).
    pub fn reset(&mut self) {
        self.cpu.reset();
        self.board.wide = false;
        self.board.keyboard.release_all();
    }

    /// Exécute au moins `cycles` T-states.
    pub fn run_cycles(&mut self, cycles: u32) {
        let mut done = self.overshoot;
        while done < cycles {
            done += self.cpu.step(&mut self.board);
        }
        self.overshoot = done - cycles;
    }

    /// Exécute une image : 1/60 de seconde de temps machine.
    pub fn run_frame(&mut self) {
        self.run_cycles(CYCLES_PER_FRAME);
    }

    /// Touche enfoncée.
    pub fn key_down(&mut self, key: Key) {
        self.board.keyboard.press(key);
    }

    /// Touche relâchée.
    pub fn key_up(&mut self, key: Key) {
        self.board.keyboard.release(key);
    }

    /// État de la touche Majuscule du clavier hôte.
    pub fn set_shift(&mut self, down: bool) {
        self.board.keyboard.set_shift(down);
    }

    /// Relâche toutes les touches (ex. : la fenêtre perd le focus).
    pub fn release_all_keys(&mut self) {
        self.board.keyboard.release_all();
    }

    /// Mémoire vidéo : 16 lignes de 64 octets.
    pub fn video(&self) -> &[u8; 1024] {
        &self.board.video
    }

    /// Mode 32 caractères par ligne actif.
    pub fn wide(&self) -> bool {
        self.board.wide
    }

    /// Caractère affiché à une position (texte seulement; blocs graphiques : espace).
    pub fn char_at(&self, row: usize, col: usize) -> char {
        video::display_char(self.board.video[row * 64 + col])
    }

    /// Le texte `text` est-il affiché quelque part à l'écran ?
    pub fn screen_contains(&self, text: &str) -> bool {
        let text = text.as_bytes();
        (0..16).any(|row| {
            let line = &self.board.video[row * 64..row * 64 + 64];
            line.windows(text.len()).any(|w| {
                w.iter().zip(text).all(|(&b, &t)| video::display_char(b) as u8 == t)
            })
        })
    }

    /// Amène le BASIC jusqu'à « READY », en répondant ENTRÉE à l'invite de taille mémoire.
    fn boot_to_ready(&mut self) -> Result<(), Error> {
        let enter = Key::from_name("Enter").expect("touche ENTRÉE");
        for _ in 0..600 {
            if self.screen_contains("READY") {
                self.board.keyboard.release_all();
                return Ok(());
            }
            if self.screen_contains("SIZE?") {
                self.key_down(enter);
                for _ in 0..3 {
                    self.run_frame();
                }
                self.key_up(enter);
            }
            self.run_frame();
        }
        Err(Error::NotReady)
    }

    /// Remplace les points d'entrée des fichiers de TRSDOS (absent : pas de disquette) par
    /// des routines minimales. Beaucoup de programmes `.CMD` lisent ou écrivent un fichier
    /// (ex. : meilleurs scores) et prévoient l'échec : ouvrir et lire répondent « erreur »,
    /// les autres opérations « succès », sans rien enregistrer.
    fn install_dos_stubs(&mut self) {
        const FAIL: u16 = 0x4300; // LD A,1 ; OR A ; RET  (A ≠ 0 et Z = 0 : erreur)
        const OK: u16 = 0x4304; //   XOR A ; RET          (A = 0 et Z = 1 : succès)
        const STUBS: [u8; 6] = [0x3E, 0x01, 0xB7, 0xC9, 0xAF, 0xC9];
        // Points d'entrée TRSDOS 2.3 (Model 1) : un JP de 3 octets chacun.
        const VECTORS: [(u16, u16); 14] = [
            (0x4409, OK),   // @ERROR
            (0x4420, OK),   // @INIT  (créer / ouvrir)
            (0x4424, FAIL), // @OPEN  (fichier introuvable)
            (0x4428, OK),   // @CLOSE
            (0x442C, OK),   // @KILL
            (0x4430, FAIL), // @LOAD
            (0x4433, FAIL), // @RUN
            (0x4436, FAIL), // @READ
            (0x4439, OK),   // @WRITE
            (0x443C, OK),   // @VERF
            (0x443F, OK),   // @REW
            (0x4442, OK),   // @POSN
            (0x4448, OK),   // @PEOF
            (0x4467, OK),   // @DSPLY
        ];
        for (i, &b) in STUBS.iter().enumerate() {
            self.board.write(FAIL + i as u16, b);
        }
        for (vector, target) in VECTORS {
            let [lo, hi] = target.to_le_bytes();
            self.board.write(vector, 0xC3); // JP
            self.board.write(vector + 1, lo);
            self.board.write(vector + 2, hi);
        }
    }

    /// Charge un programme `.CMD` en mémoire et le lance; retourne son adresse de lancement.
    ///
    /// Si la machine vient de démarrer, le BASIC est d'abord amené jusqu'à « READY »
    /// pour que la ROM ait initialisé le système. Le fichier est validé avant tout
    /// chargement : en cas d'erreur, la mémoire n'est pas modifiée.
    pub fn load_cmd(&mut self, data: &[u8]) -> Result<u16, Error> {
        cmd::parse(data, |_, _| {}).map_err(Error::Cmd)?;
        self.boot_to_ready()?;
        self.install_dos_stubs();
        let board = &mut self.board;
        let entry = cmd::parse(data, |addr, bytes| {
            for (i, &b) in bytes.iter().enumerate() {
                board.write(addr.wrapping_add(i as u16), b);
            }
        })
        .map_err(Error::Cmd)?;
        self.cpu.halted = false;
        self.cpu.pc = entry;
        Ok(entry)
    }

    /// Le processeur (lecture seule).
    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    /// Dessine l'écran dans `out` : [`SCREEN_WIDTH`] × [`SCREEN_HEIGHT`] pixels RGBA.
    pub fn render(&self, out: &mut [u8]) {
        video::render(&self.board.video, self.board.wide, true, out);
    }

    /// Comme [`Trs80::render`], mais sans le texte : seulement le fond et les blocs
    /// semi-graphiques. L'hôte dessine alors le texte avec la police de son choix.
    pub fn render_graphics(&self, out: &mut [u8]) {
        video::render(&self.board.video, self.board.wide, false, out);
    }
}

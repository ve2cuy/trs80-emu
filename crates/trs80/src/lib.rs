//! Le TRS-80 Model 1 : ROM Level II, 48 Ko de RAM, clavier et vidéo 64 × 16.
//!
//! - `no_std`, sans allocation : la machine tient dans une seule structure.
//! - [`Trs80::run_frame`] exécute 1/60 de seconde de temps machine.
//! - [`Trs80::render`] dessine l'écran (384 × 192 pixels, RGBA).
//! - [`Trs80::load_cmd`] et [`Trs80::load_cas`] chargent un programme (`.CMD`, cassette `.CAS`).
//! - [`Trs80::type_text`] tape un texte au clavier (ex. : un programme BASIC collé).
//! - L'interface d'expansion fournit l'horloge à 40 Hz (interruption en mode 1).
#![no_std]

mod cas;
mod cmd;
mod font;
mod keyboard;
mod typer;
mod video;

pub use cas::{CasError, Tape};
pub use cmd::CmdError;
pub use keyboard::Key;
pub use video::{SCREEN_HEIGHT, SCREEN_WIDTH};

use keyboard::Keyboard;
use typer::Typer;
use z80::{Bus, Cpu};

/// Fréquence du Z80 du Model 1.
pub const CLOCK_HZ: u32 = 1_774_080;
/// T-states exécutés par image, à 60 images par seconde.
pub const CYCLES_PER_FRAME: u32 = CLOCK_HZ / 60;
/// Taille de la ROM Level II.
pub const ROM_SIZE: usize = 0x3000;
/// T-states entre deux interruptions de l'horloge de l'interface d'expansion (40 Hz).
const RTC_PERIOD: u64 = (CLOCK_HZ / 40) as u64;

/// Pointeurs du BASIC Level II en RAM : début du programme, puis fin du programme
/// (= début des variables), début des tableaux et fin des tableaux.
const BASIC_TXTTAB: u16 = 0x40A4;
const BASIC_VARTAB: u16 = 0x40F9;
const BASIC_ARYTAB: u16 = 0x40FB;
const BASIC_STREND: u16 = 0x40FD;

const VIDEO_START: u16 = 0x3C00;
const RAM_START: u16 = 0x4000;

/// Erreurs de la machine. Les messages sont en anglais : ils s'affichent dans l'interface web.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// La ROM fournie n'a pas la taille de la ROM Level II (12 Ko).
    BadRomSize(usize),
    /// Fichier `.CMD` invalide.
    Cmd(CmdError),
    /// Image cassette `.CAS` invalide.
    Cas(CasError),
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
            Error::Cas(e) => write!(f, "{e}"),
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
    /// Interface d'expansion branchée (horloge à 40 Hz en 37E0h).
    expansion: bool,
    /// Interruption d'horloge en attente : effacée par la lecture de 37E0h.
    rtc_pending: bool,
}

impl Bus for Board {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x2FFF => self.rom[addr as usize],
            0x3800..=0x3BFF => self.keyboard.read(addr as u8),
            // Interface d'expansion : verrou des interruptions (bit 7 = horloge), remis à zéro
            // par la lecture. 37E4h-37FFh (cassette, imprimante, disquettes) : absents.
            0x37E0..=0x37E3 if self.expansion => {
                let latch = if self.rtc_pending { 0x80 } else { 0x00 };
                self.rtc_pending = false;
                latch
            }
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
    /// Prochaine interruption de l'horloge à 40 Hz (en T-states).
    next_rtc: u64,
    typer: Typer,
}

/// Programme chargé depuis une cassette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loaded {
    /// Langage machine, lancé à cette adresse.
    System(u16),
    /// Programme BASIC, prêt pour RUN (taille en octets).
    Basic(usize),
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
            expansion: true,
            rtc_pending: false,
        };
        board.rom.copy_from_slice(rom);
        Ok(Trs80 { cpu: Cpu::new(), board, overshoot: 0, next_rtc: RTC_PERIOD, typer: Typer::new() })
    }

    /// Bouton RESET : redémarre le processeur (la RAM est conservée, comme sur la vraie machine).
    pub fn reset(&mut self) {
        self.cpu.reset();
        self.board.wide = false;
        self.board.rtc_pending = false;
        self.typer.cancel(&mut self.board.keyboard);
        self.board.keyboard.release_all();
    }

    /// Branche ou débranche l'interface d'expansion (horloge à 40 Hz). Branchée par défaut.
    pub fn set_expansion_interface(&mut self, present: bool) {
        self.board.expansion = present;
        self.board.rtc_pending = false;
    }

    /// Exécute au moins `cycles` T-states.
    pub fn run_cycles(&mut self, cycles: u32) {
        let mut done = self.overshoot;
        while done < cycles {
            done += self.cpu.step(&mut self.board);
            if self.cpu.cycles >= self.next_rtc {
                self.next_rtc += RTC_PERIOD;
                self.board.rtc_pending |= self.board.expansion;
            }
            // L'interruption reste demandée tant que 37E0h n'a pas été lu (niveau, pas front).
            if self.board.rtc_pending && self.cpu.iff1 {
                done += self.cpu.interrupt(&mut self.board, 0xFF);
            }
        }
        self.overshoot = done - cycles;
    }

    /// Exécute une image : 1/60 de seconde de temps machine.
    pub fn run_frame(&mut self) {
        self.typer.tick(&mut self.board.keyboard);
        self.run_cycles(CYCLES_PER_FRAME);
    }

    /// Tape `text` au clavier, touche par touche, au fil des images (les fins de ligne
    /// deviennent ENTRÉE). Retourne le nombre de caractères acceptés.
    pub fn type_text(&mut self, text: &str) -> usize {
        self.typer.push(text)
    }

    /// Une frappe automatique est-elle en cours ?
    pub fn typing(&self) -> bool {
        self.typer.busy()
    }

    /// Annule la frappe automatique en cours.
    pub fn cancel_typing(&mut self) {
        self.typer.cancel(&mut self.board.keyboard);
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

    /// Lit un octet de la mémoire, comme le ferait le processeur.
    pub fn peek(&mut self, addr: u16) -> u8 {
        self.board.read(addr)
    }

    fn read16(&mut self, addr: u16) -> u16 {
        u16::from_le_bytes([self.board.read(addr), self.board.read(addr.wrapping_add(1))])
    }

    fn write16(&mut self, addr: u16, v: u16) {
        let [lo, hi] = v.to_le_bytes();
        self.board.write(addr, lo);
        self.board.write(addr.wrapping_add(1), hi);
    }

    /// Charge une image cassette `.CAS` : un programme en langage machine est lancé; un
    /// programme BASIC est placé en mémoire, prêt pour RUN (comme après CLOAD).
    pub fn load_cas(&mut self, data: &[u8]) -> Result<Loaded, Error> {
        let tape = cas::parse(data).map_err(Error::Cas)?;
        self.boot_to_ready()?;
        match tape {
            Tape::System { entry, .. } => {
                let board = &mut self.board;
                cas::system_blocks(data, |addr, bytes| {
                    for (i, &b) in bytes.iter().enumerate() {
                        board.write(addr.wrapping_add(i as u16), b);
                    }
                })
                .map_err(Error::Cas)?;
                self.cpu.halted = false;
                self.cpu.pc = entry;
                Ok(Loaded::System(entry))
            }
            Tape::Basic { program, .. } => {
                let start = self.read16(BASIC_TXTTAB);
                for (i, &b) in program.iter().enumerate() {
                    self.board.write(start.wrapping_add(i as u16), b);
                }
                // Les pointeurs de ligne enregistrés valent pour l'adresse de la machine
                // d'origine : on les recalcule pour celle-ci.
                let mut line = start;
                while self.read16(line) != 0 {
                    let mut end = line.wrapping_add(4);
                    while self.board.read(end) != 0 {
                        end = end.wrapping_add(1);
                    }
                    let next = end.wrapping_add(1);
                    self.write16(line, next);
                    line = next;
                }
                let top = line.wrapping_add(2);
                for ptr in [BASIC_VARTAB, BASIC_ARYTAB, BASIC_STREND] {
                    self.write16(ptr, top);
                }
                Ok(Loaded::Basic(program.len()))
            }
        }
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

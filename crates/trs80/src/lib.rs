//! Le TRS-80 Model 1 : ROM Level II, 48 Ko de RAM, clavier et vidéo 64 × 16.
//!
//! - `no_std`, sans allocation : la machine tient dans une seule structure.
//! - [`Trs80::run_frame`] exécute 1/60 de seconde de temps machine.
//! - [`Trs80::render`] dessine l'écran (384 × 192 pixels, RGBA).
#![no_std]

mod font;
mod keyboard;
mod video;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// La ROM fournie n'a pas la taille de la ROM Level II (12 Ko).
    BadRomSize(usize),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::BadRomSize(n) => {
                write!(f, "la ROM Level II doit faire {ROM_SIZE} octets (fichier reçu : {n} octets)")
            }
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

    /// Le processeur (lecture seule).
    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    /// Dessine l'écran dans `out` : [`SCREEN_WIDTH`] × [`SCREEN_HEIGHT`] pixels RGBA.
    pub fn render(&self, out: &mut [u8]) {
        video::render(&self.board.video, self.board.wide, out);
    }
}

//! Le TRS-80 Model 1 : ROM Level II, 48 Ko de RAM, clavier et vidéo 64 × 16.
//!
//! - `no_std`, sans allocation : la machine tient dans une seule structure.
//! - [`Trs80::run_frame`] exécute 1/60 de seconde de temps machine.
//! - [`Trs80::render`] dessine l'écran (384 × 192 pixels, RGBA).
//! - [`Trs80::load_cmd`] et [`Trs80::load_cas`] chargent un programme (`.CMD`, cassette `.CAS`).
//! - [`Trs80::type_text`] tape un texte au clavier (ex. : un programme BASIC collé).
//! - L'interface d'expansion fournit l'horloge à 40 Hz (interruption en mode 1) et le
//!   contrôleur de disquettes WD1771 ([`Trs80::insert_disk`] : images JV1, JV3, DMK).
//!
//! La crate est `no_std`; les disquettes utilisent `alloc` (images de taille variable).
#![no_std]

extern crate alloc;

mod audio;
mod cas;
mod disk;
mod dma;
mod fdc;
mod cmd;
mod font;
mod hard;
mod serial;
mod sio;
mod keyboard;
pub mod ldosfs;
mod typer;
mod video;

pub use cas::{CasError, Tape};
pub use disk::{Disk, DiskError, Format};
pub use fdc::{DRIVES, FdcEvent};
pub use hard::{HARD_UNITS, HardDisk, HardError};
pub use cmd::CmdError;
pub use keyboard::Key;
pub use ldosfs::{DirEntry, FsError};
pub use video::{MAX_HEIGHT, MAX_WIDTH, MODE64, MODE80, Mode, SCREEN_HEIGHT, SCREEN_WIDTH};

use keyboard::Keyboard;
use audio::Audio;
pub use audio::AUDIO_CAPACITY;
use fdc::Fdc;
use typer::Typer;
use z80::{Bus, Cpu};

/// Fréquence du Z80 du Model 1.
pub const CLOCK_HZ: u32 = 1_774_080;
/// T-states exécutés par image, à 60 images par seconde (Model 1).
pub const CYCLES_PER_FRAME: u32 = CLOCK_HZ / 60;
/// Taille de la ROM Level II du Model 1 (12 Ko).
pub const ROM_SIZE: usize = 0x3000;
/// Taille de la ROM du Model III (14 Ko).
pub const ROM3_SIZE: usize = 0x3800;
/// Taille de la ROM d'amorçage du Model II (2 Ko).
pub const ROM2_SIZE: usize = 0x800;
/// Model II : durée d'une commande du disque dur avant son interruption (T-states, environ
/// 1 ms à 4 MHz).
const HD_LATENCY: u32 = 4_000;

/// Modèle de TRS-80 émulé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    /// Model I : ROM de 12 Ko, interface d'expansion (horloge 40 Hz, disquettes en 37E0h).
    I,
    /// Model III : ROM de 14 Ko, minuscules, disquettes et interruptions sur des ports
    /// (F0h-F4h, E0h, E4h, ECh), horloge à 30 Hz, contrôleur WD1793 en double densité.
    III,
    /// Model 4 : le Model III (même ROM) plus 128 Ko de RAM en banques, quatre plans de
    /// mémoire et l'écran de 80 × 24 (port 84h), 4 MHz et horloge à 60 Hz (port ECh, bit 6).
    IV,
    /// Model II : une autre machine. ROM d'amorçage de 2 Ko (le DOS se charge de la
    /// disquette de 8 pouces), 64 Ko de RAM, écran de 80 × 24 en F800h, horloge à 60 Hz par
    /// NMI, contrôleur FD1791 (ports E4h-E7h, choix en EFh) servi par un DMA Z80, clavier
    /// ASCII, interruptions en mode 2 (DMA, CTC, PIO), 4 MHz. Images de disquette IMD.
    II,
}

impl Model {
    /// Modèle d'après la taille de la ROM : 12 Ko (Model I), 14 Ko (Model III) ou 2 Ko (Model II).
    pub fn from_rom_size(len: usize) -> Option<Model> {
        match len {
            ROM_SIZE => Some(Model::I),
            ROM3_SIZE => Some(Model::III),
            ROM2_SIZE => Some(Model::II),
            _ => None,
        }
    }

    /// Fréquence du processeur au démarrage (le Model 4 passe à 4 MHz par le port ECh).
    pub fn clock_hz(self) -> u32 {
        match self {
            Model::I => CLOCK_HZ,
            Model::III | Model::IV => 2_027_520,
            Model::II => 4_000_000,
        }
    }

    /// T-states entre deux interruptions de l'horloge : 40 Hz sur le Model I, 30 Hz à 2 MHz
    /// (Model III et 4) et 60 Hz à 4 MHz (Model 4), soit le même nombre de T-states.
    fn rtc_period(self) -> u64 {
        match self {
            Model::I => (CLOCK_HZ / 40) as u64,
            Model::III | Model::IV => (2_027_520 / 30) as u64,
            Model::II => (4_000_000 / 60) as u64,
        }
    }

    fn rom_size(self) -> usize {
        match self {
            Model::I => ROM_SIZE,
            Model::III | Model::IV => ROM3_SIZE,
            Model::II => ROM2_SIZE,
        }
    }

    /// Périphériques du Model III (aussi sur le Model 4), sur des ports.
    fn ports(self) -> bool {
        matches!(self, Model::III | Model::IV)
    }
}

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
    /// Image de disquette invalide.
    Disk(DiskError),
    /// Image de disque dur invalide.
    Hard(HardError),
    /// Le BASIC n'a jamais atteint « READY » (impossible de charger un programme).
    NotReady,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::BadRomSize(n) => write!(
                f,
                "a ROM must be {ROM_SIZE} bytes (Model I), {ROM3_SIZE} bytes (Model III/4) or {ROM2_SIZE} bytes (Model II); this file is {n} bytes"
            ),
            Error::Cmd(e) => write!(f, "{e}"),
            Error::Cas(e) => write!(f, "{e}"),
            Error::Disk(e) => write!(f, "{e}"),
            Error::Hard(e) => write!(f, "{e}"),
            Error::NotReady => write!(f, "BASIC did not reach READY"),
        }
    }
}

/// Tout ce qui est branché sur le bus du Z80.
struct Board {
    model: Model,
    rom: [u8; ROM3_SIZE],
    /// Mémoire vidéo : 1 Ko (Model I et III), 2 Ko sur le Model 4 (deux pages de 64 × 16,
    /// ou un écran de 80 × 24).
    video: [u8; 2048],
    /// Model 4 : 128 Ko de RAM (quatre banques de 32 Ko); vide sur les autres modèles.
    ram4: alloc::vec::Vec<u8>,
    /// Model 4 : registre d'options (port 84h) : page vidéo (bit 7), banques (bits 4-6),
    /// vidéo inversée (bit 3), 80 × 24 (bit 2), plan de mémoire (bits 0-1).
    opreg: u8,
    /// Model 4 : contrôleur vidéo 68045 (registre choisi, adresse de début d'affichage).
    crtc_index: u8,
    crtc_start: u16,
    /// Model II : position du curseur (registres R14-R15 du 6845, relus par TRSDOS-II).
    crtc_cursor: u16,
    /// Model 4 : processeur à 4 MHz (port ECh, bit 6).
    fast: bool,
    /// Model II : ROM d'amorçage visible en 0000h-0FFFh (jusqu'à une écriture en F9h).
    rom_enabled: bool,
    /// Model II : port FFh (bit 7 : mémoire vidéo en F800h, bit 6 : écran éteint, bit 5 :
    /// horloge autorisée, bit 4 : 40 colonnes).
    ff_reg: u8,
    /// Model II : clavier (touche en attente de lecture en FCh, puis les suivantes).
    kbd_latch: Option<u8>,
    kbd_queue: alloc::collections::VecDeque<u8>,
    /// Model II : touche CAPS (lettres en majuscules), enfoncée au départ : TRSDOS-II
    /// n'accepte ses commandes qu'en majuscules (« dir » : ERROR 31).
    caps: bool,
    /// Model II : T-states depuis la dernière touche livrée (rythme du clavier).
    kbd_gap: u32,
    /// Model II : interruption du clavier (canal 3 du CTC) en attente.
    kbd_int: bool,
    ctc: dma::Ctc,
    /// Model II : CTC de l'interface du disque dur (C4h-C7h). Canal 0 : fin de commande du
    /// contrôleur (front montant de INTRQ), compté par `hard.completions`.
    hd_ctc: dma::Ctc,
    hd_completions: u32,
    /// T-states avant l'interruption de la prochaine fin de commande (le disque met environ
    /// une milliseconde; le DOS n'attend l'interruption qu'après avoir lancé la commande).
    hd_delay: u32,
    /// Model II : interruption d'horloge en attente (effacée par la lecture de FEh).
    rtc2: bool,
    /// Model II : DMA Z80 (port F8h) et PIO du contrôleur de disquettes (E0h-E3h).
    dma: dma::Dma,
    pio_a: dma::Pio,
    pio_b: dma::Pio,
    /// Model II : port B du PIO (sortie : relu tel qu'écrit).
    pio_b_data: u8,
    ram: [u8; 0x10000 - RAM_START as usize],
    keyboard: Keyboard,
    /// Mode 32 caractères par ligne (bit 3 du port FF).
    wide: bool,
    /// Interface d'expansion branchée (horloge à 40 Hz en 37E0h).
    expansion: bool,
    /// Sortie cassette (bits 0-1 du port FFh) : sert de haut-parleur.
    sound: u8,
    /// Contrôleur de disquettes (interface d'expansion).
    fdc: Fdc,
    /// Disque dur Radio Shack (WD1010, ports C0h-CFh).
    hard: hard::Controller,
    /// Port série RS-232-C (UART, ports E8h-EBh), Model I, III et 4.
    serial: serial::Serial,
    /// Model II : ports série (Z80 SIO, F4h-F7h); le canal A va au modem.
    sio: sio::Sio,
    /// Temps machine (T-states), pour l'impulsion d'index des disquettes.
    now: u64,
    /// Interruption d'horloge en attente : effacée par la lecture de 37E0h.
    rtc_pending: bool,
    /// Model III : interruptions en attente (bit 2 : horloge), autorisées (port E0h), NMI
    /// autorisées (port E4h, bit 7 : fin de commande du contrôleur), état de la ligne NMI.
    int_latch: u8,
    int_mask: u8,
    nmi_mask: u8,
    nmi_line: bool,
}

impl Board {
    /// Model II : mémoire.
    fn read2(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x0FFF if self.rom_enabled => self.rom[(addr & 0x7FF) as usize],
            0xF800..=0xFFFF if self.ff_reg & 0x80 != 0 => self.video[(addr - 0xF800) as usize],
            _ => self.ram4[addr as usize],
        }
    }

    fn write2(&mut self, addr: u16, val: u8) {
        match addr {
            0xF800..=0xFFFF if self.ff_reg & 0x80 != 0 => self.video[(addr - 0xF800) as usize] = val,
            // Sous la ROM d'amorçage, l'écriture va à la RAM (le secteur d'amorce y est lu).
            _ => self.ram4[addr as usize] = val,
        }
    }

    fn input2(&mut self, port: u8) -> u8 {
        match port {
            0xF4..=0xF7 => self.sio.read(port),
            0xC0..=0xCF => self.hard.read(port),
            // Fin de commande du contrôleur (bit 0); imprimante prête (bit 4 : pas de défaut).
            0xE0 => 0x10 | self.fdc.intrq as u8,
            0xE4..=0xE7 if self.fdc.present() => self.fdc.read(port as u16, self.now),
            0xE4 => 0x80, // pas de disquette : « pas prêt »
            0xE1 => self.pio_b_data,
            0xF8 => self.dma.status(),
            0xFC => {
                let key = self.kbd_latch.take().unwrap_or(0);
                self.kbd_gap = 0;
                key
            }
            // Seuls les registres du curseur se relisent.
            0xFD => match self.crtc_index {
                14 => (self.crtc_cursor >> 8) as u8,
                15 => self.crtc_cursor as u8,
                _ => 0,
            },
            0xFE => {
                self.rtc2 = false;
                0xFF
            }
            0xFF => (self.ff_reg & 0x70) | if self.kbd_latch.is_some() { 0x80 } else { 0 },
            _ => 0xFF,
        }
    }

    fn output2(&mut self, port: u8, val: u8) {
        match port {
            0xF4..=0xF7 => {
                let now = self.now;
                self.sio.write(port, val, now)
            }
            0xC4..=0xC7 => self.hd_ctc.write(port as usize & 3, val),
            0xC0..=0xCF => self.hard.write(port, val),
            0xE4..=0xE7 => {
                // Une commande abaisse INTRQ; ici elle peut se terminer aussitôt. Le PIO
                // doit voir ce passage à 0 pour interrompre sur la nouvelle fin de commande.
                if port == 0xE4 {
                    self.pio_a.update(0);
                }
                self.fdc.write(port as u16, val)
            }
            // Lecteur (bits 0-3, actifs à 0), face (bit 6 : 1 = face 0), double densité (bit 7).
            0xEF => {
                self.fdc.select(!val & 0x0F);
                self.fdc.side = if val & 0x40 != 0 { 0 } else { 1 };
                self.fdc.set_density(val & 0x80 != 0);
            }
            0xE1 => self.pio_b_data = val,
            0xE2 => self.pio_a.control(val),
            0xE3 => self.pio_b.control(val),
            0xF0..=0xF3 => self.ctc.write(port as usize & 3, val),
            0xF8 => self.dma.write(val),
            // Bit 0 : ROM d'amorçage visible.
            0xF9 => self.rom_enabled = val & 1 != 0,
            0xFC => self.crtc_index = val & 0x1F,
            0xFD => match self.crtc_index {
                12 => self.crtc_start = (self.crtc_start & 0x00FF) | ((val as u16 & 0x3F) << 8),
                13 => self.crtc_start = (self.crtc_start & 0xFF00) | val as u16,
                14 => self.crtc_cursor = (self.crtc_cursor & 0x00FF) | ((val as u16 & 0x3F) << 8),
                15 => self.crtc_cursor = (self.crtc_cursor & 0xFF00) | val as u16,
                _ => {}
            },
            0xFF => self.ff_reg = val,
            _ => {}
        }
    }

    /// Fréquence actuelle du processeur.
    fn clock_hz(&self) -> u32 {
        if self.fast { 4_055_040 } else { self.model.clock_hz() }
    }

    /// Model 4 : adresse dans les 128 Ko de RAM. Chaque moitié de l'espace (0000h-7FFFh,
    /// 8000h-FFFFh) montre une banque de 32 Ko selon les bits 4-6 du port 84h.
    fn phys4(&self, addr: u16) -> usize {
        let banks: [usize; 2] = match (self.opreg >> 4) & 7 {
            2 => [0, 2],
            3 => [0, 3],
            6 => [2, 1],
            7 => [3, 1],
            _ => [0, 1],
        };
        banks[(addr >> 15) as usize] * 0x8000 + (addr & 0x7FFF) as usize
    }

    /// Model 4 : fenêtre de 1 Ko sur la mémoire vidéo en 3C00h (page : bit 7 du port 84h).
    fn video4_index(&self, addr: u16) -> usize {
        (((self.opreg >> 7) as usize) << 10) | (addr - VIDEO_START) as usize
    }

    /// Model 4 : mémoire selon le plan choisi (bits 0-1 du port 84h).
    fn read4(&mut self, addr: u16) -> u8 {
        match (self.opreg & 3, addr) {
            (0, 0x37E8..=0x37E9) => 0x30, // état de l'imprimante (prête)
            (0, 0x0000..=0x37FF) => self.rom[addr as usize],
            (0 | 1, 0x3800..=0x3BFF) => self.keyboard.read(addr as u8),
            (0 | 1, 0x3C00..=0x3FFF) => self.video[self.video4_index(addr)],
            (2, 0xF400..=0xF7FF) => self.keyboard.read(addr as u8),
            (2, 0xF800..=0xFFFF) => self.video[(addr - 0xF800) as usize],
            _ => self.ram4[self.phys4(addr)],
        }
    }

    fn write4(&mut self, addr: u16, val: u8) {
        match (self.opreg & 3, addr) {
            (0, 0x0000..=0x37FF) | (0 | 1, 0x3800..=0x3BFF) | (2, 0xF400..=0xF7FF) => {}
            (0 | 1, 0x3C00..=0x3FFF) => self.video[self.video4_index(addr)] = val,
            (2, 0xF800..=0xFFFF) => self.video[(addr - 0xF800) as usize] = val,
            _ => {
                let i = self.phys4(addr);
                self.ram4[i] = val;
            }
        }
    }

    /// Model III : mémoire.
    fn read3(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x37FF => self.rom[addr as usize],
            0x3800..=0x3BFF => self.keyboard.read(addr as u8),
            0x3C00..=0x3FFF => self.video[(addr - VIDEO_START) as usize],
            _ => self.ram[(addr - RAM_START) as usize],
        }
    }

    /// Model III : ports. Les verrous d'interruption se lisent inversés (0 = en attente).
    fn input3(&mut self, port: u8) -> u8 {
        match port {
            // Interruptions en attente (inversées) : horloge, et RS-232 (bit 4 : émission, bit 5 : réception).
            0xE0..=0xE3 => {
                let pending = self.int_latch | self.serial.interrupts();
                self.serial.acknowledge();
                !pending
            }
            // Bit 7 : fin de commande du contrôleur; bit 5 : bouton RESET (relâché).
            0xE4..=0xE7 => !(if self.fdc.intrq { 0x80 } else { 0 }),
            // Lire ECh acquitte l'interruption d'horloge.
            0xEC..=0xEF => {
                self.int_latch &= !0x04;
                0xFF
            }
            // Sans disquette, le contrôleur répond FFh : la ROM démarre aussitôt en BASIC.
            0xF0..=0xF3 if self.fdc.present() => self.fdc.read(port as u16, self.now),
            // Imprimante : prête (sélectionnée, pas occupée).
            0xF8..=0xFB => 0x30,
            _ => 0xFF,
        }
    }

    fn output3(&mut self, port: u8, val: u8) {
        let m4 = self.model == Model::IV;
        match port {
            0x84..=0x87 if m4 => self.opreg = val,
            // Contrôleur vidéo : registre choisi (pair), puis sa valeur (impair). Seule
            // l'adresse de début (registres 12 et 13) change l'affichage.
            0x88..=0x8B if m4 => {
                if port & 1 == 0 {
                    self.crtc_index = val & 0x1F;
                } else if self.crtc_index == 12 {
                    self.crtc_start = (self.crtc_start & 0x00FF) | ((val as u16 & 0x3F) << 8);
                } else if self.crtc_index == 13 {
                    self.crtc_start = (self.crtc_start & 0xFF00) | val as u16;
                }
            }
            // Haut-parleur du Model 4 (bit 0).
            0x90..=0x93 if m4 => self.sound = if val & 1 != 0 { 0x01 } else { 0x02 },
            0xE0..=0xE3 => self.int_mask = val,
            0xE4..=0xE7 => self.nmi_mask = val,
            0xEC..=0xEF => {
                self.wide = val & 0x04 != 0;
                if m4 {
                    self.fast = val & 0x40 != 0;
                }
            }
            0xF0..=0xF3 => self.fdc.write(port as u16, val),
            // Lecteur (bits 0-3), face (bit 4), double densité (bit 7).
            0xF4..=0xF7 => {
                self.fdc.select(val & 0x0F);
                self.fdc.side = (val >> 4) & 1;
                self.fdc.set_density(val & 0x80 != 0);
            }
            0xFF => self.sound = val & 0x03,
            _ => {}
        }
    }
}

impl dma::DmaBus for Board {
    fn mem_read(&mut self, addr: u16) -> u8 {
        self.read2(addr)
    }

    fn mem_write(&mut self, addr: u16, val: u8) {
        self.write2(addr, val)
    }

    fn io_read(&mut self, port: u16) -> u8 {
        self.input2(port as u8)
    }

    fn io_write(&mut self, port: u16, val: u8) {
        self.output2(port as u8, val)
    }

    /// Les registres de données des contrôleurs (E7h : disquettes, C8h : disque dur) ne sont
    /// prêts que pendant un transfert.
    fn io_ready(&mut self, port: u16) -> bool {
        match port as u8 {
            0xE7 => self.fdc.drq(),
            0xC8 => self.hard.drq(),
            _ => true,
        }
    }
}

impl Bus for Board {
    fn read(&mut self, addr: u16) -> u8 {
        match self.model {
            Model::III => return self.read3(addr),
            Model::IV => return self.read4(addr),
            Model::II => return self.read2(addr),
            Model::I => {}
        }
        match addr {
            0x0000..=0x2FFF => self.rom[addr as usize],
            0x3800..=0x3BFF => self.keyboard.read(addr as u8),
            // Interface d'expansion : verrou des interruptions (bit 7 = horloge, remis à zéro
            // par la lecture; bit 6 = disquettes, remis à zéro par la lecture de l'état).
            0x37E0..=0x37E3 if self.expansion => {
                let mut latch = if self.rtc_pending { 0x80 } else { 0x00 };
                if self.fdc.intrq && self.fdc.present() {
                    latch |= 0x40;
                }
                self.rtc_pending = false;
                latch
            }
            // Contrôleur de disquettes : absent (FFh) tant qu'aucune disquette n'est insérée.
            0x37EC..=0x37EF if self.expansion && self.fdc.present() => self.fdc.read(addr, self.now),
            0x3C00..=0x3FFF => self.video[(addr - VIDEO_START) as usize],
            0x4000..=0xFFFF => self.ram[(addr - RAM_START) as usize],
            // Interface d'expansion absente (37E0-37FF) et zones vides : bus flottant.
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match self.model {
            Model::IV => return self.write4(addr, val),
            Model::II => return self.write2(addr, val),
            _ => {}
        }
        match addr {
            0x3C00..=0x3FFF => self.video[(addr - VIDEO_START) as usize] = val,
            0x4000..=0xFFFF => self.ram[(addr - RAM_START) as usize] = val,
            _ if self.model == Model::III => {}
            0x37E0..=0x37E3 if self.expansion => self.fdc.select(val),
            0x37EC..=0x37EF if self.expansion => self.fdc.write(addr, val),
            _ => {}
        }
    }

    fn input(&mut self, port: u16) -> u8 {
        match self.model {
            Model::II => self.input2(port as u8),
            _ if (0xC0..=0xCF).contains(&(port as u8)) => self.hard.read(port as u8),
            _ if (0xE8..=0xEB).contains(&(port as u8)) => self.serial.read(port as u8),
            m if m.ports() => self.input3(port as u8),
            _ => 0xFF,
        }
    }

    fn output(&mut self, port: u16, val: u8) {
        if self.model == Model::II {
            return self.output2(port as u8, val);
        }
        if (0xC0..=0xCF).contains(&(port as u8)) {
            return self.hard.write(port as u8, val);
        }
        if (0xE8..=0xEB).contains(&(port as u8)) {
            let (now, hz) = (self.now, self.clock_hz());
            return self.serial.write(port as u8, val, now, hz);
        }
        if self.model.ports() {
            return self.output3(port as u8, val);
        }
        if port as u8 == 0xFF {
            self.wide = val & 0x08 != 0;
            self.sound = val & 0x03;
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
    /// Somme des compteurs d'activité des disques à l'image précédente (voir `run_frame`).
    disk_activity: u32,
    audio: Audio,
    debug: Debug,
}

/// Raison d'un arrêt du débogueur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Point d'arrêt atteint.
    Breakpoint,
    /// Retour dans la plage surveillée (pas à pas : fin d'un appel au système).
    Range,
    /// Adresse de sortie atteinte (retour à LDOS ou au BASIC : le programme est terminé).
    Exit,
}

/// Arrêts demandés par le débogueur; vérifiés avant chaque instruction quand l'un d'eux
/// est actif.
#[derive(Default)]
struct Debug {
    breakpoints: alloc::vec::Vec<u16>,
    range: Option<(u16, u16)>,
    exits: alloc::vec::Vec<u16>,
    stop: Option<Stop>,
    /// Exécuter la prochaine instruction sans vérifier (reprise sur un point d'arrêt).
    skip: bool,
}

impl Debug {
    fn active(&self) -> bool {
        !self.breakpoints.is_empty() || self.range.is_some() || !self.exits.is_empty()
    }

    fn check(&self, pc: u16) -> Option<Stop> {
        if self.exits.contains(&pc) {
            Some(Stop::Exit)
        } else if self.breakpoints.contains(&pc) {
            Some(Stop::Breakpoint)
        } else if self.range.is_some_and(|(lo, hi)| (lo..=hi).contains(&pc)) {
            Some(Stop::Range)
        } else {
            None
        }
    }
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
    /// Crée la machine à partir de sa ROM et la démarre : 12 Ko pour le Model I (Level II),
    /// 14 Ko pour le Model III.
    pub fn new(rom: &[u8]) -> Result<Self, Error> {
        let model = Model::from_rom_size(rom.len()).ok_or(Error::BadRomSize(rom.len()))?;
        Self::with_model(rom, model)
    }

    /// Crée un modèle donné (la ROM doit lui correspondre).
    pub fn with_model(rom: &[u8], model: Model) -> Result<Self, Error> {
        if rom.len() != model.rom_size() {
            return Err(Error::BadRomSize(rom.len()));
        }
        let mut fdc = Fdc::new();
        fdc.doubler = model == Model::I;
        fdc.lost_data = model == Model::II;
        let mut board = Board {
            model,
            rom: [0xFF; ROM3_SIZE],
            video: [0x20; 2048],
            ram4: match model {
                Model::IV => alloc::vec![0; 0x20000],
                Model::II => alloc::vec![0; 0x10000],
                _ => alloc::vec::Vec::new(),
            },
            rom_enabled: true,
            ff_reg: 0,
            kbd_latch: None,
            kbd_queue: alloc::collections::VecDeque::new(),
            caps: true,
            kbd_gap: 0,
            kbd_int: false,
            ctc: dma::Ctc::default(),
            hd_ctc: dma::Ctc::default(),
            hd_completions: 0,
            hd_delay: 0,
            rtc2: false,
            dma: dma::Dma::default(),
            pio_a: dma::Pio::default(),
            pio_b: dma::Pio::default(),
            pio_b_data: 0,
            opreg: 0,
            crtc_index: 0,
            crtc_start: 0,
            crtc_cursor: 0,
            fast: false,
            ram: [0; 0x10000 - RAM_START as usize],
            keyboard: Keyboard::new(),
            wide: false,
            expansion: true,
            rtc_pending: false,
            sound: 0,
            fdc,
            hard: hard::Controller::new(model == Model::II),
            serial: serial::Serial::new(),
            sio: sio::Sio::default(),
            now: 0,
            int_latch: 0,
            int_mask: 0,
            nmi_mask: 0,
            nmi_line: false,
        };
        board.rom[..rom.len()].copy_from_slice(rom);
        let mut audio = Audio::new();
        audio.set_clock(model.clock_hz());
        let mut typer = Typer::new();
        if model.ports() {
            typer.set_timing(5, 3);
        }
        Ok(Trs80 {
            cpu: Cpu::new(),
            board,
            overshoot: 0,
            next_rtc: model.rtc_period(),
            typer,
            disk_activity: 0,
            audio,
            debug: Debug::default(),
        })
    }

    /// Modèle émulé.
    pub fn model(&self) -> Model {
        self.board.model
    }

    /// Fréquence actuelle du processeur (le Model 4 passe à 4 MHz).
    pub fn clock_hz(&self) -> u32 {
        self.board.clock_hz()
    }

    /// Bouton RESET : redémarre le processeur (la RAM est conservée, comme sur la vraie machine).
    pub fn reset(&mut self) {
        self.cpu.reset();
        self.board.wide = false;
        self.board.rtc_pending = false;
        self.board.int_latch = 0;
        self.board.int_mask = 0;
        self.board.nmi_mask = 0;
        self.board.nmi_line = false;
        self.board.opreg = 0;
        self.board.crtc_start = 0;
        self.board.fast = false;
        self.board.rom_enabled = true;
        self.board.ff_reg = 0;
        self.board.rtc2 = false;
        self.board.kbd_latch = None;
        self.board.kbd_int = false;
        self.board.ctc = dma::Ctc::default();
        self.board.sio.reset();
        self.board.hd_ctc = dma::Ctc::default();
        self.board.kbd_queue.clear();
        self.board.fdc.reset();
        self.board.hard.reset();
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
            if self.debug.stop.is_some() {
                self.overshoot = 0;
                return;
            }
            if self.debug.active() {
                if self.debug.skip {
                    self.debug.skip = false;
                } else if let Some(stop) = self.debug.check(self.cpu.pc) {
                    self.debug.stop = Some(stop);
                    self.overshoot = 0;
                    return;
                }
            }
            done += self.instruction();
        }
        self.overshoot = done - cycles;
    }

    /// Une instruction, puis l'interruption en attente s'il y en a une; retourne les T-states.
    fn instruction(&mut self) -> u32 {
        let mut done = 0;
        {
            let level = Audio::level(self.board.sound);
            self.board.now = self.cpu.cycles;
            self.board.fdc.tick(self.cpu.cycles);
            let hz = self.board.clock_hz();
            if self.board.model == Model::II {
                let clocks = [self.board.ctc.timer_period(0), self.board.ctc.timer_period(1)];
                self.board.sio.set_clocks(clocks, hz);
                self.board.sio.tick(self.cpu.cycles);
            } else {
                self.board.serial.tick(self.cpu.cycles, hz);
            }
            let t = self.cpu.step(&mut self.board);
            self.audio.advance(level, t);
            done += t;
            let model = self.board.model;
            if self.cpu.cycles >= self.next_rtc {
                self.next_rtc += model.rtc_period();
                match model {
                    Model::I => self.board.rtc_pending |= self.board.expansion,
                    Model::III | Model::IV => self.board.int_latch |= 0x04,
                    Model::II => self.board.rtc2 |= self.board.ff_reg & 0x20 != 0,
                }
            }
            if model == Model::II {
                // DMA : transfert dès que le périphérique est prêt; PIO : fin de commande.
                let mut dma = core::mem::take(&mut self.board.dma);
                dma.run(&mut self.board);
                self.board.dma = dma;
                let intrq = self.board.fdc.intrq as u8;
                self.board.pio_a.update(intrq);
                // Clavier : une touche à la fois, à environ 2 ms d'intervalle; chaque touche
                // déclenche le canal 3 du CTC.
                self.board.kbd_gap = self.board.kbd_gap.saturating_add(done);
                if self.board.kbd_latch.is_none() && self.board.kbd_gap > 8_000 {
                    self.board.kbd_latch = self.board.kbd_queue.pop_front();
                    self.board.kbd_int = self.board.kbd_latch.is_some() && self.board.ctc.enabled(3);
                }
                self.board.ctc.tick(done);
                self.board.hd_ctc.tick(done);
                // Disque dur : chaque fin de commande déclenche le canal 0 du CTC de
                // l'interface, après le temps de la commande.
                if self.board.hd_completions != self.board.hard.completions {
                    if self.board.hd_delay == 0 {
                        self.board.hd_delay = HD_LATENCY;
                    } else if self.board.hd_delay > done {
                        self.board.hd_delay -= done;
                    } else {
                        self.board.hd_delay = 0;
                        self.board.hd_completions = self.board.hd_completions.wrapping_add(1);
                        self.board.hd_ctc.trigger(0);
                    }
                }
                // Interruptions en mode 2, par ordre de priorité : DMA, temporisateurs du CTC
                // (canaux 0 à 2), clavier (canal 3), PIO, SIO, puis le CTC du disque dur
                // (carte d'interface, plus loin sur la chaîne). La demande reste en attente tant que
                // le processeur ne l'a pas acceptée (interruptions masquées, ou instruction
                // qui suit un EI).
                let timer = (0..3).find(|&ch| self.board.ctc.pending[ch]);
                let request = if self.board.dma.int_pending {
                    Some((0, self.board.dma.int_vector()))
                } else if let Some(ch) = timer {
                    Some((3 + ch, self.board.ctc.vector(ch)))
                } else if self.board.kbd_int {
                    Some((1, self.board.ctc.vector(3)))
                } else if self.board.pio_a.int_pending {
                    Some((2, self.board.pio_a.vector()))
                } else if let Some((source, vector)) = self.board.sio.request() {
                    Some((20 + source, vector))
                } else if let Some(ch) = (0..4).find(|&ch| self.board.hd_ctc.pending[ch]) {
                    Some((10 + ch, self.board.hd_ctc.vector(ch)))
                } else {
                    None
                };
                if let Some((source, vector)) = request {
                    let t = self.cpu.interrupt(&mut self.board, vector);
                    if t > 0 {
                        match source {
                            0 => self.board.dma.int_pending = false,
                            1 => self.board.kbd_int = false,
                            2 => self.board.pio_a.int_pending = false,
                            ch @ 3..=5 => self.board.ctc.pending[ch - 3] = false,
                            ch @ 10..=13 => self.board.hd_ctc.pending[ch - 10] = false,
                            source => self.board.sio.acknowledge(source - 20),
                        }
                        done += t;
                    }
                }
                // Horloge (si autorisée) : NMI, sur le front montant.
                let nmi = self.board.rtc2;
                if nmi && !self.board.nmi_line {
                    done += self.cpu.nmi(&mut self.board);
                }
                self.board.nmi_line = nmi;
            }
            if model.ports() {
                // Fin de commande du contrôleur : NMI (front montant), si le port E4h l'autorise.
                let nmi = self.board.fdc.intrq && self.board.nmi_mask & 0x80 != 0;
                if nmi && !self.board.nmi_line {
                    done += self.cpu.nmi(&mut self.board);
                }
                self.board.nmi_line = nmi;
            }
            // L'interruption reste demandée tant que sa source n'a pas été lue (niveau, pas
            // front) : 37E0h (Model I) ou ECh (Model III) pour l'horloge, l'état du contrôleur
            // pour les disquettes du Model I.
            let pending = match model {
                Model::I => {
                    self.board.rtc_pending
                        || (self.board.fdc.intrq && self.board.fdc.present() && self.board.expansion)
                }
                Model::III | Model::IV => (self.board.int_latch | self.board.serial.interrupts()) & self.board.int_mask != 0,
                Model::II => false,
            };
            if pending && self.cpu.iff1 {
                let t = self.cpu.interrupt(&mut self.board, 0xFF);
                self.audio.advance(Audio::level(self.board.sound), t);
                done += t;
            }
        }
        done
    }

    // ------------------------------------------------------------ débogueur

    /// Exécute une seule instruction (et l'interruption qui la suit éventuellement), sans
    /// tenir compte des points d'arrêt.
    pub fn step_instruction(&mut self) {
        self.instruction();
    }

    /// Points d'arrêt (adresses d'instructions).
    pub fn set_breakpoints(&mut self, addresses: &[u16]) {
        self.debug.breakpoints = addresses.to_vec();
    }

    /// Arrêt dès que PC entre dans `lo..=hi` (pas à pas qui passe par-dessus les appels au
    /// système et les interruptions). `None` : pas de surveillance.
    pub fn set_stop_range(&mut self, range: Option<(u16, u16)>) {
        self.debug.range = range;
    }

    /// Adresses de sortie : le programme est terminé quand PC les atteint.
    pub fn set_exit_points(&mut self, addresses: &[u16]) {
        self.debug.exits = addresses.to_vec();
    }

    /// Arrêt survenu depuis le dernier appel (l'exécution reste suspendue tant qu'il n'est
    /// pas lu).
    pub fn take_stop(&mut self) -> Option<Stop> {
        self.debug.stop.take()
    }

    /// Un arrêt attend-il d'être lu ?
    pub fn stopped(&self) -> bool {
        self.debug.stop.is_some()
    }

    /// Reprend après un arrêt : l'instruction courante s'exécute même si c'est un point d'arrêt.
    pub fn resume(&mut self) {
        self.debug.stop = None;
        self.debug.skip = true;
    }

    /// Écrit des octets en mémoire (la ROM n'est pas modifiable).
    pub fn poke(&mut self, addr: u16, bytes: &[u8]) {
        for (i, &b) in bytes.iter().enumerate() {
            self.board.write(addr.wrapping_add(i as u16), b);
        }
    }

    /// Lance un programme déjà en mémoire comme le ferait le système : son RET (ou @EXIT)
    /// ramène à LDOS si une disquette est dans le lecteur 0, sinon au BASIC.
    ///
    /// Sous LDOS, on attend un moment où les interruptions sont permises (hors de la routine
    /// d'interruption). Sans DOS, le BASIC est amené jusqu'à « READY » et les points d'entrée
    /// de LDOS les plus simples sont remplacés : @DSPLY affiche vraiment son message, @EXIT
    /// et @ABORT reviennent au BASIC. Retourne l'adresse de retour (402DH ou 1A19H).
    pub fn launch(&mut self, entry: u16) -> Result<u16, Error> {
        let dos = self.board.fdc.drives[0].is_some();
        let back = if dos {
            for _ in 0..200_000 {
                if self.cpu.iff1 && !self.cpu.halted {
                    break;
                }
                self.step_instruction();
            }
            0x402D
        } else {
            self.boot_to_ready()?;
            self.install_dos_stubs();
            // @DSPLY : affiche jusqu'à 0DH (inclus) ou 03H, avec la routine de la ROM (0033H).
            const DSPLY: u16 = 0x4310;
            self.poke(DSPLY, &[
                0x7E, 0xFE, 0x03, 0xC8, 0xE5, 0xCD, 0x33, 0x00, 0xE1, 0x7E, 0x23, 0xFE, 0x0D, 0x20, 0xF1, 0xC9,
            ]);
            self.poke(0x4467, &[0xC3, DSPLY as u8, (DSPLY >> 8) as u8]);
            self.poke(0x402D, &[0xC3, 0x19, 0x1A]); // @EXIT  : JP 1A19H (« READY »)
            self.poke(0x4030, &[0xC3, 0x19, 0x1A]); // @ABORT
            0x1A19
        };
        self.cpu.sp = self.cpu.sp.wrapping_sub(2);
        let sp = self.cpu.sp;
        self.write16(sp, back);
        self.cpu.halted = false;
        self.cpu.pc = entry;
        Ok(back)
    }

    /// Fichiers de la disquette du lecteur `drive` (LDOS).
    pub fn disk_files(&self, drive: usize) -> Result<alloc::vec::Vec<DirEntry>, FsError> {
        ldosfs::list(self.disk(drive).ok_or(FsError::NoDisk)?)
    }

    /// Contenu d'un fichier de la disquette du lecteur `drive`.
    pub fn read_disk_file(&self, drive: usize, name: &str) -> Result<alloc::vec::Vec<u8>, FsError> {
        ldosfs::read_file(self.disk(drive).ok_or(FsError::NoDisk)?, name)
    }

    /// Écrit un fichier sur la disquette du lecteur `drive` (le remplace s'il existe).
    pub fn write_disk_file(&mut self, drive: usize, name: &str, data: &[u8]) -> Result<(), FsError> {
        let disk = self.board.fdc.drives[drive % DRIVES].as_mut().ok_or(FsError::NoDisk)?;
        ldosfs::write_file(disk, name, data)
    }

    /// Espace libre de la disquette du lecteur `drive`, en octets.
    pub fn disk_free(&self, drive: usize) -> Result<usize, FsError> {
        let (granules, size) = ldosfs::free_space(self.disk(drive).ok_or(FsError::NoDisk)?)?;
        Ok(granules * size)
    }

    /// Exécute une image : 1/60 de seconde de temps machine.
    pub fn run_frame(&mut self) {
        // Activité des disques pendant l'image précédente (la frappe attend leur silence).
        let activity = self.board.fdc.accesses.wrapping_add(self.board.fdc.steps).wrapping_add(self.board.hard.accesses);
        let disk_busy = activity != self.disk_activity;
        self.disk_activity = activity;
        self.typer.tick(&mut self.board.keyboard, disk_busy);
        let hz = self.board.clock_hz();
        self.audio.set_clock(hz);
        self.run_cycles(hz / 60);
    }

    /// Tape `text` au clavier, touche par touche, au fil des images (les fins de ligne
    /// deviennent ENTRÉE). Retourne le nombre de caractères acceptés.
    pub fn type_text(&mut self, text: &str) -> usize {
        if self.board.model == Model::II {
            // Clavier ASCII : les caractères vont directement dans la file du clavier.
            let mut n = 0;
            for c in text.chars().filter(|c| c.is_ascii()) {
                self.key_code(if c == '\n' { 0x0D } else { c as u8 });
                n += 1;
            }
            return n;
        }
        self.typer.push(text)
    }

    /// Une frappe automatique est-elle en cours ?
    pub fn typing(&self) -> bool {
        self.typer.busy() || !self.board.kbd_queue.is_empty()
    }

    /// Annule la frappe automatique en cours.
    /// Insère une image de disquette (JV1, JV3 ou DMK) dans le lecteur `drive` (0 à 3).
    /// Pour démarrer sur un DOS : disquette système dans le lecteur 0, puis [`Trs80::reset`].
    pub fn insert_disk(&mut self, drive: usize, image: alloc::vec::Vec<u8>) -> Result<&Disk, Error> {
        let disk = Disk::open(image).map_err(Error::Disk)?;
        let slot = &mut self.board.fdc.drives[drive % DRIVES];
        *slot = Some(disk);
        Ok(slot.as_ref().unwrap())
    }

    /// Insère une disquette vierge (à formater par le DOS) dans le lecteur `drive`.
    pub fn insert_blank_disk(&mut self, drive: usize) {
        self.board.fdc.drives[drive % DRIVES] = Some(Disk::blank());
    }

    /// Retire la disquette du lecteur `drive`.
    pub fn eject_disk(&mut self, drive: usize) -> Option<Disk> {
        self.board.fdc.drives[drive % DRIVES].take()
    }

    /// Port série RS-232 : octets venus de l'autre bout (modem, Internet), que le TRS-80
    /// recevra au rythme de la vitesse choisie. Model II : canal A du SIO.
    pub fn serial_send(&mut self, bytes: &[u8]) {
        match self.board.model {
            Model::II => self.board.sio.send(bytes),
            _ => self.board.serial.send(bytes),
        }
    }

    /// Port série RS-232 : octets émis par le TRS-80 depuis le dernier appel.
    pub fn serial_take(&mut self) -> alloc::vec::Vec<u8> {
        match self.board.model {
            Model::II => self.board.sio.take_output(),
            _ => self.board.serial.take_output(),
        }
    }

    /// Port série RS-232 : octets reçus pas encore lus par le TRS-80 (contrôle de flux).
    pub fn serial_pending(&self) -> usize {
        match self.board.model {
            Model::II => self.board.sio.pending_input(),
            _ => self.board.serial.pending_input(),
        }
    }

    /// Port série RS-232 : vitesse choisie par le programme (bauds).
    pub fn serial_baud(&self) -> u32 {
        match self.board.model {
            Model::II => self.board.sio.baud(),
            _ => self.board.serial.baud(),
        }
    }

    /// Branche une image de disque dur (format Reed / HDV) sur l'unité `unit` (0 à 3) du
    /// contrôleur Radio Shack.
    pub fn insert_hard_disk(&mut self, unit: usize, image: alloc::vec::Vec<u8>) -> Result<&HardDisk, Error> {
        let disk = HardDisk::open(image).map_err(Error::Hard)?;
        let slot = &mut self.board.hard.units[unit % HARD_UNITS];
        *slot = Some(disk);
        Ok(slot.as_ref().unwrap())
    }

    /// Débranche le disque dur de l'unité `unit`.
    pub fn eject_hard_disk(&mut self, unit: usize) -> Option<HardDisk> {
        self.board.hard.units[unit % HARD_UNITS].take()
    }

    /// Le disque dur de l'unité `unit`.
    pub fn hard_disk(&self, unit: usize) -> Option<&HardDisk> {
        self.board.hard.units[unit % HARD_UNITS].as_ref()
    }

    /// Dernières commandes reçues par le contrôleur de disquettes (diagnostic).
    pub fn fdc_trace(&self) -> &[FdcEvent] {
        &self.board.fdc.trace
    }

    /// Activité des lecteurs, pour en imiter le bruit : (pas de la tête, accès), deux
    /// compteurs cumulatifs qui reviennent à zéro après `u32::MAX`. Un accès (commande ou
    /// sélection d'un lecteur garni) démarre le moteur.
    pub fn disk_activity(&self) -> (u32, u32) {
        (self.board.fdc.steps, self.board.fdc.accesses)
    }

    /// Le contrôleur de disquettes est-il en double densité (WD1791 d'un doubleur) ?
    pub fn double_density(&self) -> bool {
        self.board.fdc.double_density()
    }

    /// La disquette du lecteur `drive`.
    pub fn disk(&self, drive: usize) -> Option<&Disk> {
        self.board.fdc.drives[drive % DRIVES].as_ref()
    }

    /// Active le son à la fréquence d'échantillonnage `rate` (Hz); 0 le désactive.
    pub fn set_audio_rate(&mut self, rate: u32) {
        self.audio.set_rate(rate);
    }

    /// Échantillons produits depuis le dernier [`Trs80::clear_audio`] (au plus
    /// [`AUDIO_CAPACITY`]; au-delà, ils sont perdus).
    pub fn audio_samples(&self) -> &[f32] {
        self.audio.samples()
    }

    pub fn clear_audio(&mut self) {
        self.audio.clear();
    }

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
    pub fn video(&self) -> &[u8; 2048] {
        &self.board.video
    }

    /// Mode d'affichage : 64 × 16, ou 80 × 24 (Model 4, bit 2 du port 84h).
    pub fn text_mode(&self) -> video::Mode {
        match self.board.model {
            Model::II => video::MODE80_II,
            Model::IV if self.board.opreg & 0x04 != 0 => video::MODE80,
            _ => video::MODE64,
        }
    }

    /// Taille de l'image produite par [`Trs80::render`] (pixels).
    pub fn screen_size(&self) -> (usize, usize) {
        let m = self.text_mode();
        (m.width(), m.height())
    }

    /// Caractères affichés, ligne par ligne (`cols × rows` octets).
    pub fn display(&self) -> alloc::vec::Vec<u8> {
        let m = self.text_mode();
        // Model 4 : l'affichage commence à l'adresse du contrôleur vidéo (page de 64 × 16).
        let start = if matches!(self.board.model, Model::IV | Model::II) { self.board.crtc_start as usize & 0x7FF } else { 0 };
        (0..m.cols * m.rows).map(|i| self.board.video[(start + i) & 0x7FF]).collect()
    }

    fn lowercase(&self) -> bool {
        self.board.model != Model::I
    }

    fn inverse(&self) -> bool {
        match self.board.model {
            Model::II => true,
            Model::IV => self.board.opreg & 0x08 != 0,
            _ => false,
        }
    }

    /// Model II : envoie un code de touche (ASCII) au clavier; il arrive au programme par une interruption (canal 3 du CTC).
    /// Avec CAPS, les lettres arrivent en majuscules.
    pub fn key_code(&mut self, code: u8) {
        let code = if self.board.caps { code.to_ascii_uppercase() } else { code };
        if self.board.kbd_queue.len() < 256 {
            self.board.kbd_queue.push_back(code);
        }
    }

    /// Model II : bascule la touche CAPS; retourne son nouvel état.
    pub fn toggle_caps(&mut self) -> bool {
        self.board.caps = !self.board.caps;
        self.board.caps
    }

    /// Model II : touche CAPS enfoncée (lettres en majuscules).
    pub fn caps(&self) -> bool {
        self.board.caps
    }

    /// Mode 32 caractères par ligne actif.
    pub fn wide(&self) -> bool {
        self.board.wide
    }

    /// Caractère affiché à une position (texte seulement; blocs graphiques : espace).
    pub fn char_at(&self, row: usize, col: usize) -> char {
        let m = self.text_mode();
        let code = self.display().get(row * m.cols + col).copied().unwrap_or(0x20);
        if self.board.model == Model::II && code & 0x7C == 0 {
            return ['◢', '◣', '◤', '◥'][code as usize & 3];
        }
        video::display_char(code, self.lowercase(), self.inverse())
    }

    /// Le texte `text` est-il affiché quelque part à l'écran ?
    pub fn screen_contains(&self, text: &str) -> bool {
        let text = text.as_bytes();
        let m = self.text_mode();
        let shown = self.display();
        let (lower, inverse) = (self.lowercase(), self.inverse());
        shown.chunks(m.cols).any(|line| {
            line.windows(text.len()).any(|w| {
                w.iter().zip(text).all(|(&b, &t)| video::display_char(b, lower, inverse) as u8 == t)
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
            // Model III : « Cass? » puis « Memory Size? » : ENTRÉE aux deux.
            if self.screen_contains("SIZE?") || self.screen_contains("Size?") || self.screen_contains("Cass?") {
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
        let m = self.text_mode();
        video::render(&self.display(), &m, self.board.wide, true, self.lowercase(), self.inverse(), out);
    }

    /// Comme [`Trs80::render`], mais sans le texte : seulement le fond et les blocs
    /// semi-graphiques. L'hôte dessine alors le texte avec la police de son choix.
    pub fn render_graphics(&self, out: &mut [u8]) {
        let m = self.text_mode();
        video::render(&self.display(), &m, self.board.wide, false, self.lowercase(), self.inverse(), out);
    }
}

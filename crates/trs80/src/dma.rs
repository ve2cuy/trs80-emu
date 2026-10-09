//! Contrôleur DMA Z80 (Model II) et PIO Z80 du contrôleur de disquettes.
//!
//! DMA : programmé octet par octet sur un seul port (F8h). Le premier octet choisit un
//! registre (WR0 à WR6) et annonce les octets qui suivent (adresses, longueur, vecteur...);
//! WR6 porte les commandes (RESET, LOAD, ENABLE...). Le transfert va du port A vers le port
//! B, ou l'inverse; chaque port est la mémoire ou un port d'E/S, à adresse croissante,
//! décroissante ou fixe. Ici, le transfert se fait d'un bloc quand le périphérique est prêt
//! (DRQ du contrôleur de disquettes, ou READY forcé).
//!
//! PIO : seul le port A, en mode « bits » (mode 3), est utilisé par le Model II pour
//! signaler la fin de commande du contrôleur de disquettes (bit 0, port E0h).

use alloc::vec::Vec;

/// Registres qui suivent l'octet de base d'un WR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Follow {
    PortALo,
    PortAHi,
    LenLo,
    LenHi,
    PortATiming,
    PortBTiming,
    Mask,
    Match,
    PortBLo,
    PortBHi,
    IntCtrl,
    Pulse,
    Vector,
    ReadMask,
}

#[derive(Default)]
pub(crate) struct Dma {
    wr0: u8,
    wr1: u8,
    wr2: u8,
    wr4: u8,
    wr5: u8,
    port_a: u16,
    port_b: u16,
    len: u16,
    int_ctrl: u8,
    pub(crate) vector: u8,
    follow: Vec<Follow>,
    /// Compteurs du transfert en cours (chargés par LOAD).
    addr_a: u16,
    addr_b: u16,
    count: u32,
    pub(crate) enabled: bool,
    force_ready: bool,
    int_enabled: bool,
    /// Fin de bloc atteinte (état), et interruption en attente.
    end_of_block: bool,
    pub(crate) int_pending: bool,
}

/// Accès aux deux côtés d'un transfert (mémoire et ports du Model II).
pub(crate) trait DmaBus {
    fn mem_read(&mut self, addr: u16) -> u8;
    fn mem_write(&mut self, addr: u16, val: u8);
    fn io_read(&mut self, port: u16) -> u8;
    fn io_write(&mut self, port: u16, val: u8);
    /// Le périphérique (port d'E/S) est-il prêt à donner ou recevoir un octet ?
    fn io_ready(&mut self, port: u16) -> bool;
}

impl Dma {
    pub(crate) fn write(&mut self, data: u8) {
        if let Some(reg) = (!self.follow.is_empty()).then(|| self.follow.remove(0)) {
            match reg {
                Follow::PortALo => self.port_a = (self.port_a & 0xFF00) | data as u16,
                Follow::PortAHi => self.port_a = (self.port_a & 0x00FF) | (data as u16) << 8,
                Follow::LenLo => self.len = (self.len & 0xFF00) | data as u16,
                Follow::LenHi => self.len = (self.len & 0x00FF) | (data as u16) << 8,
                Follow::PortBLo => self.port_b = (self.port_b & 0xFF00) | data as u16,
                Follow::PortBHi => self.port_b = (self.port_b & 0x00FF) | (data as u16) << 8,
                Follow::IntCtrl => {
                    self.int_ctrl = data;
                    // Octet d'impulsions (bit 3) puis vecteur (bit 4).
                    if data & 0x08 != 0 {
                        self.follow.insert(0, Follow::Pulse);
                    }
                    if data & 0x10 != 0 {
                        let at = self.follow.iter().take_while(|f| **f == Follow::Pulse).count();
                        self.follow.insert(at, Follow::Vector);
                    }
                }
                Follow::Vector => self.vector = data,
                Follow::PortATiming | Follow::PortBTiming | Follow::Mask | Follow::Match | Follow::Pulse | Follow::ReadMask => {}
            }
            return;
        }
        if data & 0x87 == 0x00 {
            // WR2 : port B (bit 3 : E/S), mode d'adresse (bits 4-5), timing (bit 6).
            self.wr2 = data;
            if data & 0x40 != 0 {
                self.follow.push(Follow::PortBTiming);
            }
        } else if data & 0x87 == 0x04 {
            // WR1 : port A.
            self.wr1 = data;
            if data & 0x40 != 0 {
                self.follow.push(Follow::PortATiming);
            }
        } else if data & 0x80 == 0 {
            // WR0 : opération (bits 1-0 : 01 transfert, 10 recherche, 11 les deux), sens
            // (bit 2), puis adresse du port A et longueur.
            self.wr0 = data;
            for (bit, f) in [(0x08, Follow::PortALo), (0x10, Follow::PortAHi), (0x20, Follow::LenLo), (0x40, Follow::LenHi)] {
                if data & bit != 0 {
                    self.follow.push(f);
                }
            }
        } else if data & 0x83 == 0x80 {
            // WR3 : masque et comparaison (recherche), interruptions (bit 5), DMA (bit 6).
            if data & 0x08 != 0 {
                self.follow.push(Follow::Mask);
            }
            if data & 0x10 != 0 {
                self.follow.push(Follow::Match);
            }
            if data & 0x20 != 0 {
                self.int_enabled = true;
            }
            if data & 0x40 != 0 {
                self.enabled = true;
            }
        } else if data & 0x83 == 0x81 {
            // WR4 : mode (bits 5-6), adresse du port B, contrôle des interruptions.
            self.wr4 = data;
            for (bit, f) in [(0x04, Follow::PortBLo), (0x08, Follow::PortBHi), (0x10, Follow::IntCtrl)] {
                if data & bit != 0 {
                    self.follow.push(f);
                }
            }
        } else if data & 0xC7 == 0x82 {
            self.wr5 = data;
        } else {
            self.command(data);
        }
    }

    /// WR6 : commandes.
    fn command(&mut self, cmd: u8) {
        match cmd {
            0xC3 => {
                // RESET
                self.enabled = false;
                self.force_ready = false;
                self.int_enabled = false;
                self.int_pending = false;
                self.end_of_block = false;
                self.follow.clear();
            }
            0xCF => {
                // LOAD : adresses de départ dans les compteurs.
                self.addr_a = self.port_a;
                self.addr_b = self.port_b;
                self.count = 0;
                self.end_of_block = false;
            }
            0xD3 => {
                // CONTINUE : même adresses, compteur remis à zéro.
                self.count = 0;
                self.end_of_block = false;
            }
            0x87 => self.enabled = true,
            0x83 => self.enabled = false,
            0xAB => self.int_enabled = true,
            0xAF => self.int_enabled = false,
            0xA3 => {
                self.int_enabled = false;
                self.int_pending = false;
            }
            0xB7 => self.int_enabled = true,
            0xB3 => self.force_ready = true,
            0x8B => self.end_of_block = false,
            0xBB => self.follow.push(Follow::ReadMask),
            _ => {}
        }
    }

    /// Octet d'état (lecture du port) : bit 5 à 0 = fin de bloc atteinte.
    pub(crate) fn status(&self) -> u8 {
        0x1A | if self.end_of_block { 0 } else { 0x20 } | if self.int_pending { 0 } else { 0x08 }
    }

    fn step_addr(addr: u16, wr: u8) -> u16 {
        match (wr >> 4) & 3 {
            0 => addr.wrapping_sub(1),
            1 => addr.wrapping_add(1),
            _ => addr,
        }
    }

    /// Exécute le transfert autant que le périphérique le permet.
    pub(crate) fn run(&mut self, bus: &mut impl DmaBus) {
        if !self.enabled || self.end_of_block {
            return;
        }
        // Le DMA du Zilog transfère « longueur + 1 » octets.
        let total = self.len as u32 + 1;
        let a_io = self.wr1 & 0x08 != 0;
        let b_io = self.wr2 & 0x08 != 0;
        let a_to_b = self.wr0 & 0x04 != 0;
        while self.count < total {
            let io_port = if a_io { Some(self.addr_a) } else if b_io { Some(self.addr_b) } else { None };
            let ready = self.force_ready || io_port.is_none_or(|p| bus.io_ready(p));
            if !ready {
                return;
            }
            let (src, src_io, dst, dst_io) =
                if a_to_b { (self.addr_a, a_io, self.addr_b, b_io) } else { (self.addr_b, b_io, self.addr_a, a_io) };
            let v = if src_io { bus.io_read(src) } else { bus.mem_read(src) };
            // Recherche seule (WR0 bits 1-0 = 10) : la source est lue, rien n'est écrit
            // (TRSDOS-II s'en sert pour vérifier un secteur sans le garder).
            if self.wr0 & 0x01 == 0 {
            } else if dst_io {
                bus.io_write(dst, v);
            } else {
                bus.mem_write(dst, v);
            }
            self.addr_a = Self::step_addr(self.addr_a, self.wr1);
            self.addr_b = Self::step_addr(self.addr_b, self.wr2);
            self.count += 1;
            // Mode octet (WR4 bits 5-6 = 00) : un octet à la fois.
            if (self.wr4 >> 5) & 3 == 0 {
                break;
            }
        }
        if self.count >= total {
            self.end_of_block = true;
            // Redémarrage automatique (WR5 bit 5), sinon arrêt.
            if self.wr5 & 0x20 != 0 {
                self.addr_a = self.port_a;
                self.addr_b = self.port_b;
                self.count = 0;
                self.end_of_block = false;
            } else {
                self.enabled = false;
            }
            if self.int_enabled && self.int_ctrl & 0x02 != 0 {
                self.int_pending = true;
            }
        }
    }

    /// Vecteur d'interruption. Avec « l'état modifie le vecteur » (bit 5 de l'octet de
    /// contrôle), les bits 2-1 donnent la cause : 10 = fin de bloc.
    pub(crate) fn int_vector(&self) -> u8 {
        if self.int_ctrl & 0x20 != 0 { (self.vector & 0xF9) | 0x04 } else { self.vector }
    }
}

/// PIO Z80, port A (port B : sortie, sans interruption).
#[derive(Default)]
pub(crate) struct Pio {
    vector: u8,
    mode: u8,
    /// Mode 3 : bits en entrée, puis mot de contrôle d'interruption et masque.
    expect_dir: bool,
    expect_mask: bool,
    int_word: u8,
    mask: u8,
    int_enabled: bool,
    /// Condition d'interruption au dernier examen (on interrompt sur son apparition).
    active: bool,
    pub(crate) int_pending: bool,
}

impl Pio {
    pub(crate) fn control(&mut self, data: u8) {
        if self.expect_dir {
            self.expect_dir = false;
            return;
        }
        if self.expect_mask {
            self.expect_mask = false;
            self.mask = data;
            return;
        }
        if data & 1 == 0 {
            self.vector = data;
        } else if data & 0x0F == 0x0F {
            self.mode = data >> 6;
            self.expect_dir = self.mode == 3;
        } else if data & 0x0F == 0x07 {
            self.int_word = data;
            self.int_enabled = data & 0x80 != 0;
            self.expect_mask = data & 0x10 != 0;
            if !self.int_enabled {
                self.int_pending = false;
            }
        } else if data & 0x0F == 0x03 {
            self.int_enabled = data & 0x80 != 0;
            if !self.int_enabled {
                self.int_pending = false;
            }
        }
    }

    /// Mode 3 : examine les entrées (`input`); interrompt quand la condition apparaît.
    pub(crate) fn update(&mut self, input: u8) {
        if self.mode != 3 {
            return;
        }
        let watched = !self.mask;
        let high = self.int_word & 0x20 != 0;
        let and = self.int_word & 0x40 != 0;
        let bits = (if high { input } else { !input }) & watched;
        let cond = if and { watched != 0 && bits == watched } else { bits != 0 };
        if cond && !self.active && self.int_enabled {
            self.int_pending = true;
        }
        self.active = cond;
    }

    pub(crate) fn vector(&self) -> u8 {
        self.vector
    }
}

/// CTC Z80 (F0h-F3h), réduit à ce que demande le Model II : le vecteur et l'autorisation
/// d'interruption de chaque canal. Le clavier déclenche le canal 3 (vecteur de base + 6).
#[derive(Default)]
pub(crate) struct Ctc {
    vector: u8,
    control: [u8; 4],
    /// Le prochain octet du canal est sa constante de temps.
    expect_tc: [bool; 4],
}

impl Ctc {
    pub(crate) fn write(&mut self, channel: usize, data: u8) {
        if self.expect_tc[channel] {
            self.expect_tc[channel] = false;
        } else if data & 1 == 0 {
            // Vecteur (écrit sur le canal 0) : bits 2-1 = numéro du canal.
            if channel == 0 {
                self.vector = data & 0xF8;
            }
        } else {
            self.control[channel] = data;
            self.expect_tc[channel] = data & 0x04 != 0;
        }
    }

    /// Le canal peut-il interrompre ?
    pub(crate) fn enabled(&self, channel: usize) -> bool {
        self.control[channel] & 0x80 != 0
    }

    pub(crate) fn vector(&self, channel: usize) -> u8 {
        self.vector | (channel as u8) << 1
    }
}

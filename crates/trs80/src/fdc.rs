//! Contrôleur de disquettes WD1771 de l'interface d'expansion du Model I.
//!
//! ```text
//! 37E0h-37E3h  écriture : sélection du lecteur (bits 0-3 = lecteurs 0-3)
//! 37ECh        écriture : commande      lecture : état (efface la demande d'interruption)
//! 37EDh        registre de piste
//! 37EEh        registre de secteur
//! 37EFh        registre de données
//! ```
//!
//! Simplifications : les transferts sont instantanés (les données sont prêtes dès que le
//! DOS interroge DRQ), le formatage (« écrire la piste ») n'est pas émulé. L'impulsion
//! d'index est simulée à 300 tours par minute, car les DOS s'en servent pour savoir si
//! une disquette tourne.

use crate::CLOCK_HZ;
use crate::disk::Disk;
use alloc::vec::Vec;

pub const DRIVES: usize = 4;

// Bits d'état.
const BUSY: u8 = 0x01;
const INDEX: u8 = 0x02; // commandes de type I
const DRQ: u8 = 0x02; // commandes de type II et III
const TRACK0: u8 = 0x04;
const NOT_FOUND: u8 = 0x10; // « record not found » (type II) ou erreur de positionnement (type I)
const HEAD_LOADED: u8 = 0x20;
const WRITE_PROTECT: u8 = 0x40;
const NOT_READY: u8 = 0x80;

/// Une rotation (300 tr/min) et la durée de l'impulsion d'index, en T-states.
const ROTATION: u64 = CLOCK_HZ as u64 / 5;
const INDEX_PULSE: u64 = 7_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Transfer {
    None,
    Read { pos: usize, multiple: bool },
    Write { pos: usize, dam: u8 },
    Address { pos: usize },
}

pub(crate) struct Fdc {
    pub(crate) drives: [Option<Disk>; DRIVES],
    selected: usize,
    head: [u8; DRIVES],
    track: u8,
    sector: u8,
    data: u8,
    status: u8,
    /// Dernière commande de type I (l'état affiche alors index et piste 0).
    type1: bool,
    step_out: bool,
    transfer: Transfer,
    /// Secteur en cours de transfert (indice dans la disquette).
    current: usize,
    buffer: Vec<u8>,
    /// Pour « lire l'adresse » : prochain secteur rencontré sur la piste.
    address_turn: usize,
    /// Demande d'interruption (bit 6 du verrou 37E0h).
    pub(crate) intrq: bool,
}

impl Fdc {
    pub(crate) fn new() -> Self {
        Fdc {
            drives: [None, None, None, None],
            selected: 0,
            head: [0; DRIVES],
            track: 0,
            sector: 0,
            data: 0,
            status: 0,
            type1: true,
            step_out: false,
            transfer: Transfer::None,
            current: 0,
            buffer: Vec::new(),
            address_turn: 0,
            intrq: false,
        }
    }

    /// Une disquette au moins est insérée : le contrôleur est « présent ». Sinon il répond
    /// FFh partout, comme une machine sans lecteur, et la ROM démarre en BASIC.
    pub(crate) fn present(&self) -> bool {
        self.drives.iter().any(Option::is_some)
    }

    fn disk(&self) -> Option<&Disk> {
        self.drives[self.selected].as_ref()
    }

    pub(crate) fn select(&mut self, val: u8) {
        if let Some(d) = (0..DRIVES).find(|d| val & (1 << d) != 0) {
            self.selected = d;
        }
    }

    pub(crate) fn read(&mut self, reg: u16, now: u64) -> u8 {
        match reg & 3 {
            0 => {
                self.intrq = false;
                self.status_now(now)
            }
            1 => self.track,
            2 => self.sector,
            _ => self.read_data(),
        }
    }

    pub(crate) fn write(&mut self, reg: u16, val: u8) {
        match reg & 3 {
            0 => self.command(val),
            1 => self.track = val,
            2 => self.sector = val,
            _ => self.write_data(val),
        }
    }

    fn status_now(&self, now: u64) -> u8 {
        let mut s = self.status;
        if self.disk().is_none() {
            s |= NOT_READY;
        }
        if self.type1 {
            s &= !(INDEX | TRACK0);
            if self.disk().is_some() && now % ROTATION < INDEX_PULSE {
                s |= INDEX;
            }
            if self.head[self.selected] == 0 {
                s |= TRACK0;
            }
        }
        s
    }

    fn finish(&mut self, status: u8) {
        self.status = status;
        self.transfer = Transfer::None;
        self.intrq = true;
    }

    fn command(&mut self, cmd: u8) {
        self.intrq = false;
        match cmd >> 4 {
            0x0..=0x7 => self.type_one(cmd),
            0x8..=0x9 => self.read_sector(cmd & 0x10 != 0),
            0xA..=0xB => self.write_sector(cmd),
            0xC => self.read_address(),
            0xD => {
                // Interruption forcée : arrête la commande en cours.
                self.type1 = true;
                self.transfer = Transfer::None;
                self.status &= !(BUSY | DRQ);
                self.intrq = cmd & 0x0F != 0;
            }
            // Lire ou écrire une piste : non émulé. FEh/FFh sont aussi les commandes de
            // changement de densité de certains « doubleurs » : sans effet ici.
            _ => {
                self.type1 = false;
                self.finish(if cmd >= 0xFE { 0 } else { NOT_FOUND });
            }
        }
    }

    /// Positionnement de la tête : restore, seek, step, step in, step out.
    fn type_one(&mut self, cmd: u8) {
        self.type1 = true;
        let d = self.selected;
        let update = cmd & 0x10 != 0;
        match cmd >> 4 {
            0x0 => {
                self.head[d] = 0;
                self.track = 0;
            }
            0x1 => {
                let target = self.data;
                let delta = target as i16 - self.track as i16;
                self.head[d] = (self.head[d] as i16 + delta).clamp(0, 95) as u8;
                self.step_out = delta < 0;
                self.track = target;
            }
            n => {
                let out = match n {
                    0x2 | 0x3 => self.step_out,
                    0x4 | 0x5 => false,
                    _ => true,
                };
                self.step_out = out;
                self.head[d] = if out { self.head[d].saturating_sub(1) } else { (self.head[d] + 1).min(95) };
                if update || n <= 0x1 {
                    self.track = if out { self.track.wrapping_sub(1) } else { self.track.wrapping_add(1) };
                }
            }
        }
        let mut status = if cmd & 0x08 != 0 { HEAD_LOADED } else { 0 };
        if let Some(disk) = self.disk() {
            if disk.write_protected() {
                status |= WRITE_PROTECT;
            }
            // Vérification : un secteur de la piste porte-t-il le bon numéro de piste ?
            if cmd & 0x04 != 0 && disk.on_track(self.head[d], 0).next().is_none_or(|(_, s)| s.track != self.track) {
                status |= NOT_FOUND;
            }
        }
        self.finish(status);
    }

    /// Bits 5-6 de l'état après une lecture : le type de marque de données.
    fn record_type(dam: u8) -> u8 {
        match dam {
            0xFB => 0x00,
            0xFA => 0x20,
            0xF9 => 0x40,
            _ => 0x60,
        }
    }

    fn locate(&self) -> Option<usize> {
        let disk = self.disk()?;
        disk.find(self.head[self.selected], 0, self.track, self.sector)
    }

    fn read_sector(&mut self, multiple: bool) {
        self.type1 = false;
        let Some(disk) = self.disk() else {
            return self.finish(NOT_READY);
        };
        let Some(index) = self.locate() else {
            return self.finish(NOT_FOUND);
        };
        let bytes = disk.sector_data(index).to_vec();
        let record_type = Self::record_type(disk.sectors[index].dam);
        self.buffer = bytes;
        self.current = index;
        self.status = BUSY | DRQ | record_type;
        self.transfer = Transfer::Read { pos: 0, multiple };
    }

    fn read_data(&mut self) -> u8 {
        match self.transfer {
            Transfer::Read { pos, multiple } => {
                self.data = self.buffer[pos];
                if pos + 1 < self.buffer.len() {
                    self.transfer = Transfer::Read { pos: pos + 1, multiple };
                } else if multiple {
                    // Lecture de plusieurs secteurs : le suivant, jusqu'à « non trouvé ».
                    self.sector = self.sector.wrapping_add(1);
                    self.read_sector(true);
                } else {
                    let rt = self.status & 0x60;
                    self.finish(rt);
                }
            }
            Transfer::Address { pos } => {
                self.data = self.buffer[pos];
                if pos + 1 < self.buffer.len() {
                    self.transfer = Transfer::Address { pos: pos + 1 };
                } else {
                    self.finish(0);
                }
            }
            _ => {}
        }
        self.data
    }

    fn write_sector(&mut self, cmd: u8) {
        self.type1 = false;
        let Some(disk) = self.disk() else {
            return self.finish(NOT_READY);
        };
        if disk.write_protected() {
            return self.finish(WRITE_PROTECT);
        }
        let Some(index) = self.locate() else {
            return self.finish(NOT_FOUND);
        };
        let len = disk.sectors[index].len;
        self.buffer.clear();
        self.buffer.resize(len, 0);
        self.current = index;
        // Bits 0-1 de la commande : marque de données à écrire.
        let dam = [0xFB, 0xFA, 0xF9, 0xF8][(cmd & 3) as usize];
        self.status = BUSY | DRQ;
        self.transfer = Transfer::Write { pos: 0, dam };
    }

    fn write_data(&mut self, val: u8) {
        self.data = val;
        if let Transfer::Write { pos, dam } = self.transfer {
            self.buffer[pos] = val;
            if pos + 1 < self.buffer.len() {
                self.transfer = Transfer::Write { pos: pos + 1, dam };
            } else {
                let (index, bytes) = (self.current, core::mem::take(&mut self.buffer));
                if let Some(disk) = self.drives[self.selected].as_mut() {
                    disk.write_sector(index, &bytes, dam);
                }
                self.buffer = bytes;
                self.finish(0);
            }
        }
    }

    /// Lire l'adresse : les 6 octets du champ d'identification du prochain secteur.
    fn read_address(&mut self) {
        self.type1 = false;
        let Some(disk) = self.disk() else {
            return self.finish(NOT_READY);
        };
        let head = self.head[self.selected];
        let on_track: Vec<_> = disk.on_track(head, 0).map(|(_, s)| *s).collect();
        if on_track.is_empty() {
            return self.finish(NOT_FOUND);
        }
        let s = on_track[self.address_turn % on_track.len()];
        self.address_turn = self.address_turn.wrapping_add(1);
        self.buffer.clear();
        self.buffer.extend_from_slice(&[s.track, s.side, s.sector, s.size_code, 0, 0]);
        // Le WD1771 recopie le numéro de piste lu dans le registre de secteur.
        self.sector = s.track;
        self.status = BUSY | DRQ;
        self.transfer = Transfer::Address { pos: 0 };
    }
}

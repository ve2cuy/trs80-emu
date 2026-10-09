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
//! Doubleurs de densité : le contrôleur d'origine (WD1771) ne lit que la simple densité.
//! Les « doubleurs » ajoutent un WD1791 pour la double densité; les deux modèles courants
//! sont émulés à la fois, comme dans xtrs :
//!
//! - **Percom** : commandes FEh (WD1771, simple densité) et FFh (WD1791, double densité);
//! - **Radio Shack** : valeurs écrites dans le registre de secteur, 3 bits du haut à 100
//!   (80h : WD1791, double densité) ou 101 (A0h : WD1771, simple densité). La valeur est
//!   aussi rangée dans le registre de secteur. 010/011 (face) et 110/111 (précompensation)
//!   n'ont pas d'effet.
//!
//! Le contrôleur actif décide de la densité des secteurs trouvés et du codage du type de
//! secteur dans l'état (WD1771 : FB/FA/F9/F8 en bits 5-6; WD1791 : F8 « effacé » en bit 5).
//!
//! Les déplacements de la tête durent le temps réel (6 à 20 ms par piste) : certains DOS
//! lancent la commande puis attendent l'interruption de fin, qui doit donc venir après.
//!
//! Simplifications : les transferts sont instantanés (les données sont prêtes dès que le
//! DOS interroge DRQ), le formatage (« écrire la piste ») n'est pas émulé. L'impulsion
//! d'index est simulée à 300 tours par minute, car les DOS s'en servent pour savoir si
//! une disquette tourne.

use crate::CLOCK_HZ;
use crate::disk::Disk;
use alloc::vec::Vec;

pub const DRIVES: usize = 4;

/// Une commande reçue par le contrôleur, pour le diagnostic ([`crate::Trs80::fdc_trace`]).
#[derive(Clone, Copy, Debug)]
pub struct FdcEvent {
    pub command: u8,
    pub drive: u8,
    pub track: u8,
    pub sector: u8,
    /// Position de la tête du lecteur.
    pub head: u8,
    /// WD1791 (double densité) actif.
    pub double_density: bool,
    /// État juste après la commande.
    pub status: u8,
}

/// Commandes conservées dans le journal.
const TRACE_LEN: usize = 64;

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
/// Durée d'un pas de la tête selon les bits 0-1 des commandes de type I (6, 6, 10, 20 ms),
/// et temps de chargement de la tête, en T-states.
const STEP_TIME: [u64; 4] = [6, 6, 10, 20];
const SETTLE: u64 = 2_000;
const MS: u64 = CLOCK_HZ as u64 / 1000;

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
    /// WD1791 du doubleur actif (double densité); sinon le WD1771 d'origine.
    wd1791: bool,
    /// Dernières commandes (journal circulaire).
    pub(crate) trace: Vec<FdcEvent>,
    /// Temps machine courant (T-states).
    now: u64,
    /// Commande de type I en cours : fin prévue et état final.
    pending: Option<(u64, u8)>,
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
            wd1791: false,
            trace: Vec::new(),
            now: 0,
            pending: None,
        }
    }

    /// Avance le temps : termine la commande de type I en cours si son heure est venue.
    #[inline]
    pub(crate) fn tick(&mut self, now: u64) {
        self.now = now;
        if let Some((end, status)) = self.pending
            && now >= end
        {
            self.pending = None;
            self.finish(status);
        }
    }

    /// RESET : le doubleur revient au WD1771 (simple densité), comme à la mise sous tension.
    pub(crate) fn reset(&mut self) {
        self.wd1791 = false;
        self.transfer = Transfer::None;
        self.pending = None;
        self.status = 0;
        self.intrq = false;
    }

    /// Double densité active ?
    pub(crate) fn double_density(&self) -> bool {
        self.wd1791
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
            2 => {
                // Doubleur Radio Shack : sélection du contrôleur par le registre de secteur.
                match val & 0xE0 {
                    0x80 => self.wd1791 = true,
                    0xA0 => self.wd1791 = false,
                    _ => {}
                }
                self.sector = val;
            }
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
        self.execute(cmd);
        if self.trace.len() == TRACE_LEN {
            self.trace.remove(0);
        }
        self.trace.push(FdcEvent {
            command: cmd,
            drive: self.selected as u8,
            track: self.track,
            sector: self.sector,
            head: self.head[self.selected],
            double_density: self.wd1791,
            status: self.status,
        });
    }

    fn execute(&mut self, cmd: u8) {
        self.intrq = false;
        self.pending = None;
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
            // Doubleur Percom : FEh = WD1771, FFh = WD1791. Se comporte comme une
            // interruption forcée, sans demande d'interruption.
            _ if cmd >= 0xFE => {
                self.wd1791 = cmd == 0xFF;
                self.type1 = true;
                self.transfer = Transfer::None;
                self.status &= !(BUSY | DRQ);
            }
            // Lire ou écrire une piste : non émulé.
            _ => {
                self.type1 = false;
                self.finish(NOT_FOUND);
            }
        }
    }

    /// Positionnement de la tête : restore, seek, step, step in, step out.
    fn type_one(&mut self, cmd: u8) {
        self.type1 = true;
        let d = self.selected;
        let start = self.head[d];
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
            if cmd & 0x04 != 0
                && disk.on_track(self.head[d], 0, self.wd1791).next().is_none_or(|(_, s)| s.track != self.track)
            {
                status |= NOT_FOUND;
            }
        }
        // La tête met un certain temps à se déplacer : occupé jusque-là, puis interruption.
        let steps = start.abs_diff(self.head[d]) as u64;
        let delay = SETTLE + steps * STEP_TIME[(cmd & 3) as usize] * MS;
        self.status = BUSY | (status & HEAD_LOADED);
        self.transfer = Transfer::None;
        self.pending = Some((self.now + delay, status));
    }

    /// Type de marque de données dans l'état après une lecture : bits 5-6 sur le WD1771
    /// (FB, FA, F9, F8), bit 5 sur le WD1791 (F8 « effacé » seulement).
    fn record_type(&self, dam: u8) -> u8 {
        if self.wd1791 {
            return if dam == 0xF8 { 0x20 } else { 0x00 };
        }
        match dam {
            0xFB => 0x00,
            0xFA => 0x20,
            0xF9 => 0x40,
            _ => 0x60,
        }
    }

    /// Le secteur demandé, sur la piste où se trouve la tête et dans la densité active.
    fn locate(&self) -> Option<usize> {
        let disk = self.disk()?;
        disk.find(self.track, 0, self.sector, self.wd1791)
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
        let record_type = self.record_type(disk.sectors[index].dam);
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
        // Marque de données à écrire : bits 0-1 de la commande sur le WD1771, bit 0 sur le
        // WD1791 (0 = FB, 1 = F8).
        let dam = if self.wd1791 {
            if cmd & 1 != 0 { 0xF8 } else { 0xFB }
        } else {
            [0xFB, 0xFA, 0xF9, 0xF8][(cmd & 3) as usize]
        };
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
        let on_track: Vec<_> = disk.on_track(head, 0, self.wd1791).map(|(_, s)| *s).collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Disquette JV3 : piste 0, secteur 0 en simple densité (octets 11h, marque F8h) et en
    /// double densité (octets 22h, marque F8h).
    fn two_density_disk() -> Disk {
        let header = 2901 * 3 + 1;
        let mut image = vec![0xFFu8; header];
        image[0..3].copy_from_slice(&[0, 0, 0x60]); // SD, F8h, 256 octets
        image[3..6].copy_from_slice(&[0, 0, 0xA0]); // DD, F8h, 256 octets
        image.extend(vec![0x11; 256]);
        image.extend(vec![0x22; 256]);
        Disk::open(image).unwrap()
    }

    fn fdc() -> Fdc {
        let mut f = Fdc::new();
        f.drives[0] = Some(two_density_disk());
        f.select(1);
        f
    }

    /// Lit le secteur 0 de la piste 0 : premier octet et état après la commande.
    fn read(f: &mut Fdc) -> (u8, u8) {
        f.write(1, 0); // piste
        f.write(2, 0); // secteur
        f.write(0, 0x88); // lire un secteur
        let status = f.status;
        (f.read(3, 0), status)
    }

    #[test]
    fn wd1771_reads_single_density() {
        let mut f = fdc();
        let (byte, status) = read(&mut f);
        assert_eq!(byte, 0x11);
        assert_eq!(status & 0x60, 0x60, "WD1771 : F8h codé dans les bits 5-6");
    }

    #[test]
    fn percom_doubler_selects_density() {
        let mut f = fdc();
        f.write(0, 0xFF); // Percom : WD1791
        let (byte, status) = read(&mut f);
        assert_eq!(byte, 0x22);
        assert_eq!(status & 0x60, 0x20, "WD1791 : F8h signalé par le bit 5");
        f.write(0, 0xFE); // Percom : WD1771
        assert_eq!(read(&mut f).0, 0x11);
    }

    #[test]
    fn radio_shack_doubler_selects_density() {
        let mut f = fdc();
        f.write(2, 0x80); // Radio Shack : WD1791, par le registre de secteur
        assert!(f.double_density());
        assert_eq!(read(&mut f).0, 0x22);
        f.write(2, 0xA0); // Radio Shack : WD1771
        assert_eq!(read(&mut f).0, 0x11);
    }

    #[test]
    fn reset_returns_to_single_density() {
        let mut f = fdc();
        f.write(0, 0xFF);
        f.reset();
        assert!(!f.double_density());
    }

    #[test]
    fn seek_takes_time_then_interrupts() {
        let mut f = fdc();
        f.tick(1000);
        f.write(3, 10); // registre de données : piste visée
        f.write(0, 0x10); // seek, 6 ms par piste
        assert_eq!(f.status & BUSY, BUSY);
        assert!(!f.intrq);
        f.tick(1000 + 10 * 6 * MS + SETTLE);
        assert_eq!(f.status & BUSY, 0);
        assert!(f.intrq, "interruption à la fin du déplacement");
        assert_eq!(f.track, 10);
    }
}

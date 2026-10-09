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
//! Formatage (« écrire la piste ») : le contrôleur reçoit le flux d'octets de la piste
//! (intervalles, marques d'adresse FEh, identifiants, marques de données F8h-FBh, données)
//! et en tire les secteurs de la piste. Les octets F7h (« écrire le CRC ») et F5h/F6h (en
//! double densité) sont des codes de commande du contrôleur, ignorés dans les données.
//!
//! Simplifications : les transferts sont instantanés (les données sont prêtes dès que le
//! DOS interroge DRQ), la lecture d'une piste entière n'est pas émulée. L'impulsion
//! d'index est simulée à 300 tours par minute, car les DOS s'en servent pour savoir si
//! une disquette tourne.

use crate::CLOCK_HZ;
use crate::disk::{Disk, NewSector};
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
const LOST_DATA: u8 = 0x04; // type II et III
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
/// Octets d'une piste (une rotation) : simple densité 3125, double densité 6250. Le
/// formatage se termine quand ce nombre d'octets a été écrit.
const TRACK_BYTES_SD: usize = 3_125;
const TRACK_BYTES_DD: usize = 6_250;
const MS: u64 = CLOCK_HZ as u64 / 1000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Transfer {
    None,
    Read { pos: usize, multiple: bool },
    Write { pos: usize, dam: u8 },
    Address { pos: usize },
    Track,
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
    /// Doubleurs de densité du Model I (Percom : commandes FEh/FFh; Radio Shack : registre
    /// de secteur). Sur le Model III, la densité vient du port F4h (`set_density`).
    pub(crate) doubler: bool,
    /// Face choisie (Model III et 4 : bit 4 du port F4h).
    pub(crate) side: u8,
    /// Données perdues : si l'ordinateur cesse de lire, le contrôleur finit seul le secteur
    /// (Model II : le DMA peut ne prendre que le début d'un secteur). Sur les autres
    /// modèles, le contrôleur attend indéfiniment, ce qui tolère les lectures lentes.
    pub(crate) lost_data: bool,
    /// Heure du dernier octet transféré (ou du début de la commande).
    last_byte: u64,
    /// Compteurs cumulatifs (qui reviennent à zéro après u32::MAX) pour le bruit des
    /// lecteurs : pas de la tête, et accès (commandes, sélections qui démarrent le moteur).
    pub(crate) steps: u32,
    pub(crate) accesses: u32,
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
            doubler: true,
            side: 0,
            lost_data: false,
            last_byte: 0,
            steps: 0,
            accesses: 0,
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
        // Un secteur double densité défile en 8 ms environ : au-delà de ce délai sans
        // lecture, le reste du secteur est perdu.
        if self.lost_data && matches!(self.transfer, Transfer::Read { .. }) && now > self.last_byte + 10 * MS {
            let rt = self.status & 0x60;
            self.finish(rt | LOST_DATA);
        }
    }

    /// RESET : le doubleur revient au WD1771 (simple densité), comme à la mise sous tension,
    /// et le contrôleur exécute de lui-même une commande Restore (comme un vrai WD1771 à la
    /// réinitialisation) : la tête revient à la piste 0 et l'état indique « occupé ».
    ///
    /// C'est indispensable : la ROM (0696h) démarre en BASIC si l'état vaut 00h ou FFh. Après
    /// un DOS, la tête n'est plus sur la piste 0; sans ce Restore, l'état valait 00h et le
    /// RESET suivant démarrait en BASIC au lieu de la disquette.
    pub(crate) fn reset(&mut self) {
        self.wd1791 = false;
        self.transfer = Transfer::None;
        self.pending = None;
        self.intrq = false;
        self.type_one(0x03);
        self.intrq = false;
    }

    /// Un octet de données est-il attendu ou disponible (DRQ) ? Sert de « prêt » au DMA.
    pub(crate) fn drq(&self) -> bool {
        !matches!(self.transfer, Transfer::None)
    }

    /// Densité choisie par la machine (Model III : bit 7 du port F4h).
    pub(crate) fn set_density(&mut self, dd: bool) {
        self.wd1791 = dd;
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
            if self.drives[d].is_some() {
                self.accesses = self.accesses.wrapping_add(1);
            }
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
                    0x80 if self.doubler => self.wd1791 = true,
                    0xA0 if self.doubler => self.wd1791 = false,
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
        if self.disk().is_some() {
            self.accesses = self.accesses.wrapping_add(1);
        }
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
        self.last_byte = self.now;
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
            _ if cmd >= 0xFE && self.doubler => {
                self.wd1791 = cmd == 0xFF;
                self.type1 = true;
                self.transfer = Transfer::None;
                self.status &= !(BUSY | DRQ);
            }
            // Écrire une piste (formatage).
            0xF => self.write_track(),
            // Lire une piste : non émulé.
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
                && disk.on_track(self.head[d], self.side, self.wd1791).next().is_none_or(|(_, s)| s.track != self.track)
            {
                status |= NOT_FOUND;
            }
        }
        // La tête met un certain temps à se déplacer : occupé jusque-là, puis interruption.
        let steps = start.abs_diff(self.head[d]) as u64;
        if self.drives[d].is_some() {
            self.steps = self.steps.wrapping_add(steps as u32);
        }
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
        disk.find(self.track, self.side, self.sector, self.wd1791)
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
        self.last_byte = self.now;
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

    fn write_track(&mut self) {
        self.type1 = false;
        let Some(disk) = self.disk() else {
            return self.finish(NOT_READY);
        };
        if disk.write_protected() {
            return self.finish(WRITE_PROTECT);
        }
        self.buffer.clear();
        self.status = BUSY | DRQ;
        self.transfer = Transfer::Track;
    }

    /// Tire les secteurs du flux d'une piste formatée : « FEh piste face secteur taille »,
    /// puis, un peu plus loin, une marque de données F8h-FBh suivie des données.
    fn parse_track(stream: &[u8], dd: bool) -> Vec<NewSector> {
        let mut sectors = Vec::new();
        let mut i = 0;
        while i + 5 < stream.len() {
            if stream[i] != 0xFE {
                i += 1;
                continue;
            }
            let (track, side, sector, size_code) = (stream[i + 1], stream[i + 2], stream[i + 3], stream[i + 4]);
            let len = 128usize << (size_code & 3);
            let search = i + 5..(i + 5 + 64).min(stream.len());
            let Some(mark) = search.clone().find(|&j| matches!(stream[j], 0xF8..=0xFB)) else {
                i += 1;
                continue;
            };
            let data = stream.get(mark + 1..mark + 1 + len).map(<[u8]>::to_vec).unwrap_or_default();
            if data.len() == len {
                sectors.push(NewSector { track, side, sector, size_code, dam: stream[mark], dd, data });
            }
            i = mark + 1 + len;
        }
        sectors
    }

    fn write_data(&mut self, val: u8) {
        self.data = val;
        if self.transfer == Transfer::Track {
            self.buffer.push(val);
            let limit = if self.wd1791 { TRACK_BYTES_DD } else { TRACK_BYTES_SD };
            if self.buffer.len() >= limit {
                let sectors = Self::parse_track(&self.buffer, self.wd1791);
                let head = self.head[self.selected];
                if let Some(disk) = self.drives[self.selected].as_mut() {
                    disk.format_track(head, self.side, sectors);
                }
                self.finish(0);
            }
            return;
        }
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
        let on_track: Vec<_> = disk.on_track(head, self.side, self.wd1791).map(|(_, s)| *s).collect();
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

#[cfg(test)]
mod format_tests {
    use super::*;

    /// Flux d'une piste en simple densité : deux secteurs de 256 octets (ID FEh, CRC F7h,
    /// intervalle, marque de données FBh, données E5h, CRC F7h).
    fn sd_track(track: u8) -> Vec<u8> {
        let mut t = alloc::vec![0xFFu8; 40];
        for sector in 0..2u8 {
            t.extend_from_slice(&[0xFE, track, 0, sector, 1, 0xF7]);
            t.extend_from_slice(&[0xFF; 11]);
            t.extend_from_slice(&[0x00; 6]);
            t.push(0xFB);
            t.extend_from_slice(&[0xE5; 256]);
            t.push(0xF7);
            t.extend_from_slice(&[0xFF; 20]);
        }
        t
    }

    #[test]
    fn parse_track_finds_sectors() {
        let s = Fdc::parse_track(&sd_track(5), false);
        assert_eq!(s.len(), 2);
        assert_eq!((s[1].track, s[1].sector, s[1].size_code, s[1].dam), (5, 1, 1, 0xFB));
        assert!(s.iter().all(|x| x.data.len() == 256 && x.data[0] == 0xE5 && !x.dd));
    }

    #[test]
    fn write_track_formats_a_blank_disk() {
        let mut f = Fdc::new();
        f.drives[0] = Some(Disk::blank());
        f.select(1);
        f.write(0, 0xF4); // écrire la piste (tête sur la piste 0)
        assert_eq!(f.status & (BUSY | DRQ), BUSY | DRQ);
        let mut stream = sd_track(0);
        stream.resize(TRACK_BYTES_SD, 0xFF);
        for b in stream {
            f.write(3, b);
        }
        assert_eq!(f.status & BUSY, 0, "formatage terminé après une piste");
        // Le secteur 1 de la piste 0 existe maintenant et se lit.
        f.write(1, 0);
        f.write(2, 1);
        f.write(0, 0x88);
        assert_eq!(f.read(3, 0), 0xE5);
        let disk = f.drives[0].as_ref().unwrap();
        assert!(disk.modified());
        assert_eq!(disk.image_format(), crate::disk::Format::Jv3);
        assert_eq!(disk.image().len(), 2901 * 3 + 1 + 2 * 256);
    }
}

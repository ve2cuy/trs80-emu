//! Disque dur Radio Shack : contrôleur Western Digital WD1010 sur les ports C0h-CFh, utilisé
//! par les pilotes RSHARD5 (LDOS 5.3) et RSHARD6 (LS-DOS 6.3) de MISOSYS, par les cartes
//! FreHD, et sur le Model II par la ROM d'amorçage et TRSDOS-HD (secteurs de 512 octets).
//!
//! Images au format « Reed » (.hdv), comme xtrs, trs80gp et FreHD : un en-tête de 256 octets
//! puis les secteurs, cylindre par cylindre, tête par tête, 32 secteurs par piste. La taille
//! des secteurs (256 octets, ou celle que le DOS choisit dans le registre CEh) fixe leur
//! position dans l'image. L'en-tête donne le nombre de secteurs par cylindre (octet 29), d'où le nombre de
//! têtes. Comme xtrs, le nombre de cylindres n'est pas une limite : l'image s'allonge quand
//! le DOS écrit plus loin, et un secteur jamais écrit se lit plein de zéros. Sur le Model II,
//! une image vierge (en-tête seul) est un disque non formaté, où aucun secteur n'est trouvé
//! avant la première commande FORMAT (la ROM d'amorçage passe alors aux disquettes au lieu de
//! lire le disque sans fin).
//!
//! Ports (comme xtrs) :
//! - C0h : lecture : protection en écriture (bit 7 - unité, bit 1 : au moins une),
//!   interruption du contrôleur (bit 0 : fin de commande, effacée par la lecture de CFh).
//! - C1h : contrôle (bit 4 : RESET, bit 3 : contrôleur actif).
//! - C8h : données; C9h : erreur (lecture) / précompensation (écriture);
//! - CAh : nombre de secteurs; CBh : secteur; CCh-CDh : cylindre;
//! - CEh : taille (bits 6-5 : 256, 512, 1024, 128 octets) / unité (bits 4-3) / tête (bits 2-0);
//! - CFh : état (lecture) / commande (écriture).

use alloc::vec;
use alloc::vec::Vec;

/// Unités par contrôleur.
pub const HARD_UNITS: usize = 4;
const SECTOR: usize = 256;
const MAX_SECTOR: usize = 1024;
const HEADER: usize = 256;
const SECTORS_PER_TRACK: usize = 32;
const MAX_HEADS: usize = 8;

// État.
const READY: u8 = 0x40;
const SEEK_DONE: u8 = 0x10;
const DRQ: u8 = 0x08;
const ERR: u8 = 0x01;
// Erreurs.
const NOT_FOUND: u8 = 0x10;
const ABORTED: u8 = 0x04;

/// Erreur à l'ouverture d'une image de disque dur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardError {
    /// Pas un en-tête Reed (56h CBh).
    NotReed,
    /// Géométrie inutilisable (secteurs par cylindre pas multiple de 32, ou plus de 8 têtes).
    Geometry,
}

impl core::fmt::Display for HardError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HardError::NotReed => write!(f, "not a hard disk image (Reed/HDV header 56h CBh expected)"),
            HardError::Geometry => write!(f, "unusable hard disk geometry (sectors per cylinder must be a multiple of 32, 8 heads at most)"),
        }
    }
}

/// Une image de disque dur.
pub struct HardDisk {
    data: Vec<u8>,
    heads: usize,
    write_protected: bool,
    modified: bool,
    writes: u32,
}

impl HardDisk {
    /// Ouvre une image Reed.
    pub fn open(data: Vec<u8>) -> Result<HardDisk, HardError> {
        if data.len() < HEADER || data[0] != 0x56 || data[1] != 0xCB || data[2] >= 0x1F {
            return Err(HardError::NotReed);
        }
        let per_cyl = if data[29] == 0 { 256 } else { data[29] as usize };
        let heads = per_cyl / SECTORS_PER_TRACK;
        if per_cyl % SECTORS_PER_TRACK != 0 || heads == 0 || heads > MAX_HEADS {
            return Err(HardError::Geometry);
        }
        Ok(HardDisk { write_protected: data[7] & 0x80 != 0, data, heads, modified: false, writes: 0 })
    }

    /// Image vierge (non formatée) : `cylinders` cylindres de `heads` têtes. L'en-tête seul
    /// est écrit; les secteurs s'ajoutent quand le DOS les écrit.
    pub fn blank(cylinders: usize, heads: usize) -> Vec<u8> {
        let mut h = vec![0u8; HEADER];
        h[0] = 0x56;
        h[1] = 0xCB;
        h[2] = 0x10; // version 1.0
        h[4] = 1; // un bloc d'en-tête
        h[5] = 4;
        h[10] = 0x42; // créé comme par « mkdisk » de xtrs
        h[28] = cylinders.min(256) as u8; // 256 : 0
        h[29] = (heads.clamp(1, MAX_HEADS) * SECTORS_PER_TRACK) as u8;
        h[30] = h[29];
        h[31] = 1;
        let sum = h.iter().take(32).enumerate().filter(|(i, _)| *i != 3).fold(0u8, |s, (_, b)| s.wrapping_add(*b));
        h[3] = sum ^ 0x4C;
        h
    }

    pub fn heads(&self) -> usize {
        self.heads
    }

    /// Cylindres annoncés par l'en-tête (0 : 256).
    pub fn cylinders(&self) -> usize {
        if self.data[28] == 0 { 256 } else { self.data[28] as usize }
    }

    /// Taille (Mo) annoncée par la géométrie.
    pub fn megabytes(&self) -> f64 {
        (self.cylinders() * self.heads * SECTORS_PER_TRACK * SECTOR) as f64 / 1_048_576.0
    }

    pub fn write_protected(&self) -> bool {
        self.write_protected
    }

    /// Le DOS a-t-il écrit sur le disque depuis son insertion ?
    pub fn modified(&self) -> bool {
        self.modified
    }

    /// Nombre d'écritures depuis l'insertion (revient à zéro après `u32::MAX`).
    pub fn writes(&self) -> u32 {
        self.writes
    }

    /// L'image, avec les écritures du DOS.
    pub fn image(&self) -> &[u8] {
        &self.data
    }

    fn offset(&self, cyl: u16, head: u8, sector: u8, size: usize) -> usize {
        let index = (cyl as usize * self.heads + head as usize) * SECTORS_PER_TRACK + sector as usize % SECTORS_PER_TRACK;
        HEADER + index * size
    }

    fn read(&self, at: usize, out: &mut [u8]) {
        for (i, b) in out.iter_mut().enumerate() {
            *b = self.data.get(at + i).copied().unwrap_or(0);
        }
    }

    fn write(&mut self, at: usize, data: &[u8]) {
        if self.data.len() < at + data.len() {
            self.data.resize(at + data.len(), 0);
        }
        self.data[at..at + data.len()].copy_from_slice(data);
        self.modified = true;
        self.writes = self.writes.wrapping_add(1);
    }
}

/// Dernière commande du contrôleur (barre des lecteurs de la page).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HardEvent {
    pub command: u8,
    pub unit: u8,
    pub cylinder: u16,
    pub head: u8,
    pub sector: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Transfer {
    None,
    Read,
    Write,
}

/// Le contrôleur et ses unités.
pub(crate) struct Controller {
    pub(crate) units: [Option<HardDisk>; HARD_UNITS],
    control: u8,
    status: u8,
    error: u8,
    sector_count: u8,
    sector: u8,
    cylinder: u16,
    unit: usize,
    head: u8,
    /// Taille des secteurs (registre CEh).
    size: usize,
    transfer: Transfer,
    /// Secteur en cours de transfert, et sa position dans l'image.
    buffer: [u8; MAX_SECTOR],
    pos: usize,
    at: usize,
    /// Compteur cumulatif d'accès (bruit des lecteurs).
    pub(crate) accesses: u32,
    /// Fin de commande (INTRQ du WD1010) : données prêtes pour une lecture, secteur écrit
    /// pour une écriture. Sur le Model II, déclenche le canal 0 du CTC de l'interface.
    pub(crate) intrq: bool,
    /// Nombre de fins de commande (chaque une est un front montant de INTRQ).
    pub(crate) completions: u32,
    /// Dernière commande, et nombre de commandes (compteur cumulatif).
    pub(crate) last: HardEvent,
    pub(crate) commands: u32,
    /// Model II : une unité absente n'est pas prête (TRSDOS-HD cherche ainsi les unités), et
    /// une image vierge est un disque non formaté. Ailleurs, comme xtrs : « prête » même sans
    /// unité, et un secteur jamais écrit se lit plein de zéros même sur une image vierge.
    strict: bool,
}

impl Controller {
    pub(crate) fn new(strict: bool) -> Self {
        Controller {
            units: [None, None, None, None],
            control: 0,
            status: READY | SEEK_DONE,
            error: 0,
            sector_count: 0,
            sector: 0,
            cylinder: 0,
            unit: 0,
            head: 0,
            size: SECTOR,
            transfer: Transfer::None,
            buffer: [0; MAX_SECTOR],
            pos: 0,
            at: 0,
            accesses: 0,
            intrq: false,
            completions: 0,
            last: HardEvent::default(),
            commands: 0,
            strict,
        }
    }

    /// Un disque dur au moins : le contrôleur répond; sinon les ports lisent FFh.
    pub(crate) fn present(&self) -> bool {
        self.units.iter().any(Option::is_some)
    }

    pub(crate) fn reset(&mut self) {
        self.status = READY | SEEK_DONE;
        self.error = 0;
        self.transfer = Transfer::None;
        self.cylinder = 0;
    }

    pub(crate) fn read(&mut self, port: u8) -> u8 {
        if !self.present() {
            return 0xFF;
        }
        match port {
            0xC0 => self
                .units
                .iter()
                .enumerate()
                .filter(|(_, u)| u.as_ref().is_some_and(HardDisk::write_protected))
                .fold(self.intrq as u8, |v, (i, _)| v | (0x80 >> i) | 0x02),
            0xC1 => self.control,
            0xC8 => self.read_data(),
            0xC9 => self.error,
            0xCA => self.sector_count,
            0xCB => self.sector,
            0xCC => self.cylinder as u8,
            0xCD => (self.cylinder >> 8) as u8,
            0xCE => Self::size_bits(self.size) | (self.unit as u8) << 3 | self.head,
            0xCF => {
                self.intrq = false;
                if self.strict && self.units[self.unit].is_none() {
                    self.status & !READY
                } else {
                    self.status
                }
            }
            _ => 0xFF,
        }
    }

    pub(crate) fn write(&mut self, port: u8, val: u8) {
        match port {
            0xC1 => {
                if val & 0x10 != 0 {
                    self.reset();
                }
                self.control = val;
            }
            0xC8 => self.write_data(val),
            0xCA => self.sector_count = val,
            0xCB => self.sector = val,
            0xCC => self.cylinder = (self.cylinder & 0xFF00) | val as u16,
            0xCD => self.cylinder = (self.cylinder & 0x00FF) | (val as u16) << 8,
            0xCE => {
                self.unit = ((val >> 3) & 3) as usize;
                self.head = val & 7;
                self.size = [256, 512, 1024, 128][(val >> 5) as usize & 3];
                // « Prêt » même sans unité (comme xtrs) : la commande signalera l'erreur.
                self.status = READY | SEEK_DONE;
            }
            0xCF => self.command(val),
            _ => {}
        }
    }

    fn fail(&mut self, error: u8) {
        self.transfer = Transfer::None;
        self.status = READY | SEEK_DONE | ERR;
        self.error = error;
    }

    /// Le secteur demandé existe-t-il ? Si oui, sa position dans l'image.
    fn find(&mut self) -> Option<usize> {
        let (cyl, head, sector) = (self.cylinder, self.head, self.sector);
        let Some(disk) = self.units[self.unit].as_ref() else {
            self.fail(NOT_FOUND);
            return None;
        };
        let unformatted = self.strict && disk.data.len() <= HEADER;
        if head as usize >= disk.heads || sector as usize > SECTORS_PER_TRACK || unformatted {
            self.fail(NOT_FOUND);
            return None;
        }
        let at = disk.offset(cyl, head, sector, self.size);
        self.accesses = self.accesses.wrapping_add(1);
        self.error = 0;
        Some(at)
    }

    fn command(&mut self, cmd: u8) {
        self.last = HardEvent { command: cmd, unit: self.unit as u8, cylinder: self.cylinder, head: self.head, sector: self.sector };
        self.commands = self.commands.wrapping_add(1);
        self.transfer = Transfer::None;
        self.intrq = false;
        match cmd & 0xF0 {
            // RESTORE
            0x10 => {
                self.cylinder = 0;
                self.error = 0;
                self.status = READY | SEEK_DONE;
            }
            // READ / WRITE (un secteur; les lectures multiples ne sont pas émulées).
            0x20 | 0x30 if cmd & 0x04 != 0 => self.fail(ABORTED),
            0x20 => {
                if let Some(at) = self.find() {
                    let size = self.size;
                    self.units[self.unit].as_ref().unwrap().read(at, &mut self.buffer[..size]);
                    self.start(Transfer::Read, at);
                }
            }
            0x30 => {
                if let Some(at) = self.find() {
                    if self.units[self.unit].as_ref().unwrap().write_protected() {
                        self.fail(ABORTED);
                    } else {
                        self.start(Transfer::Write, at);
                    }
                }
            }
            // VERIFY, SEEK : le secteur doit exister.
            0x40 | 0x70 => {
                if self.find().is_some() {
                    self.status = READY | SEEK_DONE;
                }
            }
            // FORMAT (la piste reste lisible : rien à effacer), INIT.
            0x50 | 0x60 => {
                // Disque non formaté (Model II) : le voici formaté.
                if let Some(disk) = self.units[self.unit].as_mut().filter(|_| self.strict) {
                    if disk.data.len() <= HEADER && !disk.write_protected {
                        let at = disk.offset(self.cylinder, self.head, 0, self.size);
                        disk.write(at, &[0]);
                    }
                }
                self.error = 0;
                self.status = READY | SEEK_DONE;
            }
            _ => self.fail(ABORTED),
        }
        if self.transfer != Transfer::Write {
            self.complete();
        }
    }

    fn complete(&mut self) {
        self.intrq = true;
        self.completions = self.completions.wrapping_add(1);
    }

    /// Données prêtes à lire ou à écrire (registre C8h), pour le DMA du Model II.
    pub(crate) fn drq(&self) -> bool {
        self.transfer != Transfer::None
    }

    fn size_bits(size: usize) -> u8 {
        match size {
            512 => 0x20,
            1024 => 0x40,
            128 => 0x60,
            _ => 0,
        }
    }

    fn start(&mut self, transfer: Transfer, at: usize) {
        self.transfer = transfer;
        self.pos = 0;
        self.at = at;
        self.status = READY | SEEK_DONE | DRQ;
    }

    fn read_data(&mut self) -> u8 {
        if self.transfer != Transfer::Read {
            return 0xFF;
        }
        let v = self.buffer[self.pos];
        self.pos += 1;
        if self.pos == self.size {
            self.transfer = Transfer::None;
            self.status = READY | SEEK_DONE;
        }
        v
    }

    fn write_data(&mut self, val: u8) {
        if self.transfer != Transfer::Write {
            return;
        }
        self.buffer[self.pos] = val;
        self.pos += 1;
        if self.pos == self.size {
            let (at, buffer) = (self.at, self.buffer);
            self.units[self.unit].as_mut().unwrap().write(at, &buffer[..self.size]);
            self.transfer = Transfer::None;
            self.status = READY | SEEK_DONE;
            self.complete();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_image_has_a_valid_reed_header() {
        let disk = HardDisk::open(HardDisk::blank(306, 4)).unwrap();
        assert_eq!(disk.heads(), 4);
        assert_eq!(disk.cylinders(), 256, "un octet : 306 cylindres annoncés comme 256 (xtrs ignore cette limite)");
        assert!(!disk.write_protected());
        assert_eq!(HardDisk::open(vec![0; 256]).err(), Some(HardError::NotReed));
    }

    #[test]
    fn write_then_read_a_sector() {
        let mut c = Controller::new(true);
        c.units[0] = Some(HardDisk::open(HardDisk::blank(153, 2)).unwrap());
        // Cylindre 100, tête 1, secteur 5 : introuvable tant que le disque n'est pas formaté.
        c.write(0xCC, 100);
        c.write(0xCD, 0);
        c.write(0xCE, 0x01);
        c.write(0xCB, 5);
        c.write(0xCF, 0x20);
        assert_eq!(c.read(0xCF) & ERR, ERR);
        c.write(0xCF, 0x50);
        c.write(0xCB, 5);
        c.write(0xCF, 0x30);
        assert_eq!(c.read(0xCF) & DRQ, DRQ);
        (0..=255u8).for_each(|b| c.write(0xC8, b));
        assert_eq!(c.read(0xCF), READY | SEEK_DONE);
        let disk = c.units[0].as_ref().unwrap();
        assert!(disk.modified());
        assert_eq!(disk.image().len(), HEADER + ((100 * 2 + 1) * 32 + 5 + 1) * SECTOR);
        c.write(0xCF, 0x20);
        let back: Vec<u8> = (0..256).map(|_| c.read(0xC8)).collect();
        assert_eq!(back, (0..=255u8).collect::<Vec<_>>());
        // Un secteur jamais écrit se lit plein de zéros; une tête de trop : « non trouvé ».
        c.write(0xCB, 6);
        c.write(0xCF, 0x20);
        assert_eq!(c.read(0xC8), 0);
        c.write(0xCE, 0x02);
        c.write(0xCF, 0x20);
        assert_eq!(c.read(0xCF) & ERR, ERR);
        assert_eq!(c.read(0xC9), NOT_FOUND);
    }

    #[test]
    fn absent_controller_reads_ff() {
        let mut c = Controller::new(false);
        assert_eq!(c.read(0xCF), 0xFF);
    }
}

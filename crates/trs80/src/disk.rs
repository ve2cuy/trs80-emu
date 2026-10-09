//! Images de disquette : JV1, JV3 et DMK, ramenées à une liste de secteurs.
//!
//! - **JV1** : 10 secteurs de 256 octets par piste, simple face, sans en-tête. Par
//!   convention, les secteurs de la piste 17 (répertoire) portent la marque FAh.
//! - **JV3** : en-tête de 2901 descripteurs (piste, secteur, indicateurs) puis les données;
//!   accepte simple et double densité, une ou deux faces, plusieurs blocs d'en-tête.
//! - **DMK** : pistes brutes avec table des marques d'adresse (IDAM); les secteurs
//!   en simple densité y sont souvent enregistrés octet par octet en double.
//!
//! Chaque secteur garde sa densité : le contrôleur ne trouve que les secteurs de la
//! densité choisie (simple avec le WD1771, double avec le WD1791 d'un « doubleur »).

use alloc::vec::Vec;

/// Erreurs d'ouverture d'une image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskError {
    /// Format non reconnu (ni JV1, ni JV3, ni DMK).
    UnknownFormat,
    /// Image tronquée ou incohérente.
    Corrupt,
}

impl core::fmt::Display for DiskError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DiskError::UnknownFormat => write!(f, "unknown disk image format (JV1, JV3 or DMK expected)"),
            DiskError::Corrupt => write!(f, "corrupt or truncated disk image"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Jv1,
    Jv3,
    Dmk,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Format::Jv1 => "JV1",
            Format::Jv3 => "JV3",
            Format::Dmk => "DMK",
        }
    }
}

/// Un secteur : son identité et l'emplacement de ses données.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sector {
    pub track: u8,
    pub side: u8,
    pub sector: u8,
    /// Code de taille du champ d'identification (0 = 128, 1 = 256, 2 = 512, 3 = 1024).
    pub size_code: u8,
    /// Marque de données : FBh (normale), FAh, F9h ou F8h (« effacée »).
    pub dam: u8,
    /// Double densité (MFM).
    pub dd: bool,
    /// Position des données dans `Disk::data`.
    pub offset: usize,
    pub len: usize,
    /// Position du descripteur JV3 (pour enregistrer une nouvelle marque de données).
    pub jv3_entry: Option<usize>,
}

/// Une disquette en mémoire.
pub struct Disk {
    pub(crate) format: Format,
    /// Octets des secteurs. JV1 et JV3 : l'image elle-même (les écritures y vont
    /// directement, l'image modifiée peut être réenregistrée). DMK : données extraites.
    pub(crate) data: Vec<u8>,
    pub(crate) sectors: Vec<Sector>,
    pub(crate) write_protected: bool,
    pub(crate) modified: bool,
}

const JV3_ENTRIES: usize = 2901;
const JV3_HEADER: usize = JV3_ENTRIES * 3 + 1;

fn size_of_code(code: u8) -> usize {
    128 << (code & 3)
}

impl Disk {
    /// Ouvre une image (format détecté automatiquement).
    pub fn open(image: Vec<u8>) -> Result<Disk, DiskError> {
        if let Some(disk) = Self::open_dmk(&image) {
            return disk;
        }
        if let Some(disk) = Self::open_jv3(&image) {
            return Ok(disk.with_data(image));
        }
        if !image.is_empty() && image.len() % 2560 == 0 {
            return Ok(Self::open_jv1(image));
        }
        Err(DiskError::UnknownFormat)
    }

    fn with_data(mut self, data: Vec<u8>) -> Disk {
        self.data = data;
        self
    }

    fn open_jv1(image: Vec<u8>) -> Disk {
        let sectors = (0..image.len() / 256)
            .map(|i| {
                let track = (i / 10) as u8;
                Sector {
                    track,
                    side: 0,
                    sector: (i % 10) as u8,
                    size_code: 1,
                    dam: if track == 17 { 0xFA } else { 0xFB },
                    dd: false,
                    offset: i * 256,
                    len: 256,
                    jv3_entry: None,
                }
            })
            .collect();
        Disk { format: Format::Jv1, data: image, sectors, write_protected: false, modified: false }
    }

    /// JV3 : vérifie que les descripteurs sont plausibles et que les tailles concordent.
    fn open_jv3(image: &[u8]) -> Option<Disk> {
        let mut sectors = Vec::new();
        let mut block = 0;
        let mut write_protected = false;
        loop {
            let header = image.get(block..block + JV3_HEADER)?;
            if block == 0 {
                write_protected = header[JV3_HEADER - 1] != 0xFF;
            }
            let mut offset = block + JV3_HEADER;
            let mut used = 0;
            for i in 0..JV3_ENTRIES {
                let (track, sector, flags) = (header[i * 3], header[i * 3 + 1], header[i * 3 + 2]);
                if track == 0xFF && flags == 0xFF {
                    continue; // emplacement inutilisé, sans données
                }
                // Taille JV3 : 0 = 256, 1 = 128, 2 = 1024, 3 = 512 (code IBM xor 1).
                let size_code = (flags & 3) ^ 1;
                let len = size_of_code(size_code);
                if track != 0xFF {
                    if track > 95 {
                        return None;
                    }
                    let dd = flags & 0x80 != 0;
                    let dam = match (dd, (flags >> 5) & 3) {
                        (false, 0) => 0xFB,
                        (false, 1) => 0xFA,
                        (false, 2) => 0xF9,
                        (false, _) => 0xF8,
                        (true, 0) => 0xFB,
                        (true, _) => 0xF8,
                    };
                    sectors.push(Sector {
                        track,
                        side: (flags >> 4) & 1,
                        sector,
                        size_code,
                        dam,
                        dd,
                        offset,
                        len,
                        jv3_entry: Some(block + i * 3),
                    });
                    used += 1;
                }
                offset += len;
            }
            if offset > image.len() || (block == 0 && used == 0) {
                return None;
            }
            // Un autre bloc d'en-tête suit si l'image continue.
            if offset + JV3_HEADER <= image.len() && used == JV3_ENTRIES {
                block = offset;
            } else {
                break;
            }
        }
        Some(Disk { format: Format::Jv3, data: Vec::new(), sectors, write_protected, modified: false })
    }

    /// DMK : en-tête de 16 octets, puis les pistes (table IDAM de 128 octets + données brutes).
    ///
    /// Tolérant comme un vrai lecteur : une image un peu plus courte que ne l'annonce son
    /// en-tête garde ses pistes complètes, et un secteur illisible (champ ou données qui
    /// débordent de la piste) est simplement absent, sans rejeter toute la disquette.
    fn open_dmk(image: &[u8]) -> Option<Result<Disk, DiskError>> {
        if image.len() < 16 {
            return None;
        }
        let tracks = image[1] as usize;
        let track_len = u16::from_le_bytes([image[2], image[3]]) as usize;
        let options = image[4];
        let sides = if options & 0x10 != 0 { 1 } else { 2 };
        if tracks == 0 || tracks > 96 || track_len < 128 || track_len > 0x4000 || image.len() < 16 + track_len {
            return None;
        }
        // Pistes réellement présentes dans le fichier (au plus celles de l'en-tête).
        let slots = ((image.len() - 16) / track_len).min(tracks * sides);
        // Un fichier beaucoup plus long n'est pas une image DMK; un fichier plus court l'est
        // s'il contient un nombre entier de pistes, ou au moins les trois quarts annoncés.
        let expected = tracks * sides;
        let whole_tracks = (image.len() - 16) % track_len == 0;
        if image.len() > 16 + (expected + 1) * track_len || (!whole_tracks && slots * 4 < expected * 3) {
            return None;
        }
        // Les octets 12-15 sont nuls dans une image DMK (réservés au « vrai lecteur »).
        if image[5..12].iter().any(|&b| b != 0) && image[12..16] != [0x12, 0x34, 0x56, 0x78] {
            return None;
        }
        let ignore_density = options & 0xC0 != 0;
        let mut data = Vec::new();
        let mut sectors = Vec::new();
        for t in 0..slots {
            let start = 16 + t * track_len;
            let raw = &image[start..start + track_len];
            for k in 0..64 {
                let ptr = u16::from_le_bytes([raw[k * 2], raw[k * 2 + 1]]);
                if ptr == 0 {
                    break;
                }
                let double = ptr & 0x8000 != 0;
                let step = if double || ignore_density { 1 } else { 2 };
                let pos = (ptr & 0x3FFF) as usize;
                let get = |i: usize| raw.get(pos + i * step).copied();
                if get(0) != Some(0xFE) {
                    continue;
                }
                let (Some(track), Some(side), Some(sector), Some(size_code)) = (get(1), get(2), get(3), get(4)) else {
                    continue; // champ d'identification tronqué
                };
                // Marque de données dans les octets qui suivent le champ d'identification.
                let Some(mark) = (7..7 + 60).find(|&i| matches!(get(i), Some(0xF8..=0xFB))) else {
                    continue;
                };
                let len = size_of_code(size_code);
                if get(mark + len).is_none() {
                    continue; // données qui débordent de la piste
                }
                let offset = data.len();
                data.extend((0..len).map(|i| get(mark + 1 + i).unwrap_or(0)));
                sectors.push(Sector {
                    track,
                    side: side & 1,
                    sector,
                    size_code: size_code & 3,
                    dam: get(mark).unwrap(),
                    dd: double,
                    offset,
                    len,
                    jv3_entry: None,
                });
            }
        }
        if sectors.is_empty() {
            return Some(Err(DiskError::Corrupt));
        }
        Some(Ok(Disk { format: Format::Dmk, data, sectors, write_protected: image[0] == 0xFF, modified: false }))
    }

    pub fn format(&self) -> Format {
        self.format
    }

    pub fn sector_count(&self) -> usize {
        self.sectors.len()
    }

    pub fn write_protected(&self) -> bool {
        self.write_protected
    }

    /// La disquette a-t-elle été modifiée depuis son ouverture ?
    pub fn modified(&self) -> bool {
        self.modified
    }

    /// Image à enregistrer (JV1 et JV3 seulement; les DMK modifiés ne sont pas réécrits).
    pub fn image(&self) -> Option<&[u8]> {
        matches!(self.format, Format::Jv1 | Format::Jv3).then_some(self.data.as_slice())
    }

    /// Indice du secteur `sector` sur la piste `track`, dans la densité demandée.
    /// (JV1, JV3 et DMK numérotent les pistes comme le champ d'identification.)
    pub(crate) fn find(&self, track: u8, side: u8, sector: u8, dd: bool) -> Option<usize> {
        self.sectors
            .iter()
            .position(|s| s.track == track && s.side == side && s.sector == sector && s.dd == dd)
    }

    /// Secteurs d'une piste lisibles dans la densité demandée (« lire l'adresse », vérification).
    pub(crate) fn on_track(&self, track: u8, side: u8, dd: bool) -> impl Iterator<Item = (usize, &Sector)> {
        self.sectors
            .iter()
            .enumerate()
            .filter(move |(_, s)| s.track == track && s.side == side && s.dd == dd)
    }

    pub(crate) fn sector_data(&self, index: usize) -> &[u8] {
        let s = &self.sectors[index];
        &self.data[s.offset..s.offset + s.len]
    }

    /// Écrit un secteur et sa marque de données.
    pub(crate) fn write_sector(&mut self, index: usize, bytes: &[u8], dam: u8) {
        let s = self.sectors[index];
        let n = bytes.len().min(s.len);
        self.data[s.offset..s.offset + n].copy_from_slice(&bytes[..n]);
        self.sectors[index].dam = dam;
        if let Some(entry) = s.jv3_entry {
            // JV3 : marque de données dans les bits 5-6 de l'indicateur (simple densité :
            // FB, FA, F9, F8) ou dans le bit 5 (double densité : FB ou F8).
            let flags = &mut self.data[entry + 2];
            let code = match (*flags & 0x80 != 0, dam) {
                (true, 0xF8) => 1,
                (true, _) => 0,
                (false, 0xFB) => 0,
                (false, 0xFA) => 1,
                (false, 0xF9) => 2,
                (false, _) => 3,
            };
            *flags = (*flags & !0x60) | (code << 5);
        }
        self.modified = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn jv1_layout() {
        let mut image = vec![0u8; 35 * 2560];
        image[17 * 2560 + 2 * 256] = 0x42; // piste 17, secteur 2
        let d = Disk::open(image).unwrap();
        assert_eq!(d.format(), Format::Jv1);
        assert_eq!(d.sector_count(), 350);
        let i = d.find(17, 0, 2, false).unwrap();
        assert!(d.find(17, 0, 2, true).is_none(), "JV1 : simple densité seulement");
        assert_eq!(d.sector_data(i)[0], 0x42);
        assert_eq!(d.sectors[i].dam, 0xFA, "répertoire : marque FAh");
    }

    #[test]
    fn jv3_two_sectors() {
        let mut image = vec![0xFFu8; JV3_HEADER];
        image[0..3].copy_from_slice(&[0, 0, 0x00]); // piste 0, secteur 0, 256 octets, FBh
        image[3..6].copy_from_slice(&[1, 5, 0x21]); // piste 1, secteur 5, 128 octets, FAh
        image.extend(vec![0x11; 256]);
        image.extend(vec![0x22; 128]);
        let d = Disk::open(image).unwrap();
        assert_eq!(d.format(), Format::Jv3);
        assert_eq!(d.sector_count(), 2);
        let i = d.find(1, 0, 5, false).unwrap();
        assert_eq!((d.sector_data(i).len(), d.sector_data(i)[0], d.sectors[i].dam), (128, 0x22, 0xFA));
    }

    #[test]
    fn dmk_single_density_doubled() {
        // 1 piste, simple face, longueur 0x200 : un secteur 0 de 128 octets en simple densité.
        let track_len = 0x200usize;
        let mut image = vec![0u8; 16 + track_len];
        image[1] = 1;
        image[2..4].copy_from_slice(&(track_len as u16).to_le_bytes());
        image[4] = 0x10; // simple face
        let t = 16;
        let idam = 0x80usize; // juste après la table IDAM
        image[t..t + 2].copy_from_slice(&(idam as u16).to_le_bytes());
        let mut pos = t + idam;
        let mut put = |b: u8| {
            image[pos] = b;
            image[pos + 1] = b; // simple densité : chaque octet en double
            pos += 2;
        };
        for b in [0xFE, 0, 0, 0, 0, 0xAA, 0xBB] {
            put(b);
        }
        for _ in 0..11 {
            put(0xFF);
        }
        put(0xFB);
        for i in 0..128 {
            put(i as u8);
        }
        let d = Disk::open(image).unwrap();
        assert_eq!(d.format(), Format::Dmk);
        let i = d.find(0, 0, 0, false).unwrap();
        assert_eq!(d.sector_data(i)[..4], [0, 1, 2, 3]);
        assert_eq!(d.sectors[i].dam, 0xFB);
    }

    #[test]
    fn unknown_format() {
        assert!(Disk::open(vec![1, 2, 3]).is_err());
    }
}

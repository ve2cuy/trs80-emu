//! Fichiers d'une disquette LDOS 5 (Model I) : liste, lecture, écriture.
//!
//! Organisation (vérifiée sur les disquettes de LDOS 5.3.1) :
//! - piste 0, secteur 0 (amorce) : l'octet 2 donne la piste du répertoire;
//! - répertoire, secteur 0 : GAT, un octet par piste (bit = granule occupé), puis en 60h la
//!   table des granules inexistants (lockout);
//! - secteur 1 : HIT, une empreinte du nom par entrée (0 = libre). La position d'une entrée
//!   (DEC) vaut (rang dans le secteur << 5) | (secteur - 2);
//! - secteurs 2 et suivants : 8 entrées de 32 octets. Attributs (bit 4 : utilisée, bit 6 :
//!   système), date, octet de fin (EOF), longueur d'enregistrement, nom (8), extension (3),
//!   mots de passe, nombre de secteurs (ERN), puis 5 extensions (piste, granule de départ
//!   << 5 | nombre de granules - 1), FFh à la fin.
//!
//! Taille d'un fichier : (ERN - 1) × 256 + EOF, ou ERN × 256 si EOF vaut 0. Un granule
//! compte 5 secteurs en simple densité (2 par piste de 10) et 6 en double densité (3 par
//! piste de 18).

use crate::disk::Disk;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// Erreur d'accès aux fichiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FsError {
    /// Pas de disquette dans le lecteur.
    NoDisk,
    /// La disquette n'a pas de répertoire LDOS lisible (pas formatée, autre DOS).
    NotLdos,
    /// Géométrie non prise en charge (double face, nombre de secteurs inattendu).
    Unsupported,
    /// Nom de fichier invalide (NOM/EXT : lettres et chiffres, 8 et 3 au plus).
    BadName,
    NoSuchFile,
    /// Plus de granule libre.
    DiskFull,
    /// Plus d'entrée libre dans le répertoire.
    DirectoryFull,
    /// Le fichier demanderait plus de 5 extensions (disquette trop fragmentée).
    Fragmented,
    /// La disquette est protégée en écriture.
    WriteProtected,
    /// Fichier système : on ne le remplace pas.
    SystemFile,
}

impl FsError {
    /// Code de l'erreur (clé de traduction de la page).
    pub fn code(self) -> &'static str {
        match self {
            FsError::NoDisk => "noDisk",
            FsError::NotLdos => "notLdos",
            FsError::Unsupported => "unsupported",
            FsError::BadName => "badName",
            FsError::NoSuchFile => "noSuchFile",
            FsError::DiskFull => "diskFull",
            FsError::DirectoryFull => "directoryFull",
            FsError::Fragmented => "fragmented",
            FsError::WriteProtected => "writeProtected",
            FsError::SystemFile => "systemFile",
        }
    }
}

impl core::fmt::Display for FsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.code())
    }
}

/// Un fichier du répertoire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry {
    /// NOM/EXT
    pub name: String,
    pub size: usize,
    /// Fichier système (/SYS) ou invisible.
    pub system: bool,
}

const ENTRY: usize = 32;
const BLANK_PASSWORD: [u8; 2] = [0x96, 0x42];

struct Geometry {
    dir_track: u8,
    sectors: u8,
    grans_per_track: u8,
    gran_sectors: u8,
    tracks: u8,
}

fn find(disk: &Disk, track: u8, sector: u8) -> Option<usize> {
    disk.find(track, 0, sector, false).or_else(|| disk.find(track, 0, sector, true))
}

fn read(disk: &Disk, track: u8, sector: u8) -> Result<Vec<u8>, FsError> {
    let i = find(disk, track, sector).ok_or(FsError::NotLdos)?;
    let mut data = disk.sector_data(i).to_vec();
    data.resize(256, 0);
    Ok(data)
}

/// Écrit un secteur en gardant sa marque de données (le répertoire en a une particulière).
fn write(disk: &mut Disk, track: u8, sector: u8, bytes: &[u8]) -> Result<(), FsError> {
    let i = find(disk, track, sector).ok_or(FsError::NotLdos)?;
    let dam = disk.sectors[i].dam;
    disk.write_sector(i, bytes, dam);
    Ok(())
}

fn geometry(disk: &Disk) -> Result<Geometry, FsError> {
    let boot = read(disk, 0, 0)?;
    let dir_track = boot[2];
    let sectors = (0..32u8).filter(|&s| find(disk, dir_track, s).is_some()).count() as u8;
    let (grans_per_track, gran_sectors) = match sectors {
        10 => (2, 5),
        18 => (3, 6),
        0 => return Err(FsError::NotLdos),
        _ => return Err(FsError::Unsupported),
    };
    if disk.sectors.iter().any(|s| s.side != 0) {
        return Err(FsError::Unsupported);
    }
    let tracks = disk.sectors.iter().map(|s| s.track).max().unwrap_or(0).saturating_add(1);
    let geo = Geometry { dir_track, sectors, grans_per_track, gran_sectors, tracks };
    if dir_track == 0 || dir_track >= tracks || find(disk, dir_track, 1).is_none() {
        return Err(FsError::NotLdos);
    }
    // Un répertoire LDOS commence par BOOT/SYS et DIR/SYS (aussi sur les disquettes de données).
    let first = read(disk, dir_track, 2)?;
    if &first[5..16] != b"BOOT    SYS" {
        return Err(FsError::NotLdos);
    }
    Ok(geo)
}

/// Empreinte du nom (8 + 3 caractères) rangée dans la HIT.
fn hash(name11: &[u8]) -> u8 {
    let h = name11.iter().fold(0u8, |a, &c| (a ^ c).rotate_left(1));
    if h == 0 { 1 } else { h }
}

/// « NOM/EXT » → 11 octets (nom et extension complétés d'espaces), en majuscules.
fn name11(name: &str) -> Result<[u8; 11], FsError> {
    let upper = name.trim().to_ascii_uppercase();
    let (n, e) = upper.split_once(['/', '.']).unwrap_or((&upper, ""));
    let valid = |s: &str, max: usize, required: bool| {
        (!required || !s.is_empty())
            && s.len() <= max
            && s.chars().all(|c| c.is_ascii_alphanumeric())
            && s.chars().next().is_none_or(|c| c.is_ascii_alphabetic())
    };
    if !valid(n, 8, true) || !valid(e, 3, false) {
        return Err(FsError::BadName);
    }
    let mut out = [b' '; 11];
    out[..n.len()].copy_from_slice(n.as_bytes());
    out[8..8 + e.len()].copy_from_slice(e.as_bytes());
    Ok(out)
}

fn display_name(entry: &[u8]) -> String {
    let n = String::from_utf8_lossy(&entry[5..13]);
    let e = String::from_utf8_lossy(&entry[13..16]);
    let (n, e) = (n.trim_end(), e.trim_end());
    if e.is_empty() { n.into() } else { alloc::format!("{n}/{e}") }
}

fn file_size(entry: &[u8]) -> usize {
    let ern = u16::from_le_bytes([entry[20], entry[21]]) as usize;
    match (ern, entry[3]) {
        (0, _) => 0,
        (n, 0) => n * 256,
        (n, eof) => (n - 1) * 256 + eof as usize,
    }
}

/// Toutes les entrées utilisées : (DEC, 32 octets). Les entrées d'extension (FXDE) sont omises.
fn entries(disk: &Disk, geo: &Geometry) -> Result<Vec<(u8, Vec<u8>)>, FsError> {
    let mut out = Vec::new();
    for s in 2..geo.sectors {
        let data = read(disk, geo.dir_track, s)?;
        for e in 0..8 {
            let entry = &data[e * ENTRY..(e + 1) * ENTRY];
            if entry[0] & 0x10 != 0 && entry[0] & 0x80 == 0 {
                out.push((((e as u8) << 5) | (s - 2), entry.to_vec()));
            }
        }
    }
    Ok(out)
}

fn dec_location(dec: u8) -> (u8, usize) {
    ((dec & 0x1F) + 2, (dec >> 5) as usize * ENTRY)
}

fn read_entry(disk: &Disk, geo: &Geometry, dec: u8) -> Result<Vec<u8>, FsError> {
    let (sector, offset) = dec_location(dec);
    if sector >= geo.sectors {
        return Err(FsError::NotLdos);
    }
    Ok(read(disk, geo.dir_track, sector)?[offset..offset + ENTRY].to_vec())
}

/// Granules d'un fichier, dans l'ordre (numéros linéaires : piste × granules par piste + rang).
fn granules(disk: &Disk, geo: &Geometry, entry: &[u8]) -> Result<Vec<u32>, FsError> {
    let mut out = Vec::new();
    let mut entry = entry.to_vec();
    // Au plus quelques entrées d'extension enchaînées (protection contre une boucle).
    for _ in 0..16 {
        let mut next = None;
        for i in (22..32).step_by(2) {
            let (track, g) = (entry[i], entry[i + 1]);
            match track {
                0xFF => break,
                0xFE => {
                    next = Some(g);
                    break;
                }
                _ => {
                    let start = track as u32 * geo.grans_per_track as u32 + (g >> 5) as u32;
                    out.extend(start..start + (g & 0x1F) as u32 + 1);
                }
            }
        }
        match next {
            Some(dec) => entry = read_entry(disk, geo, dec)?,
            None => return Ok(out),
        }
    }
    Err(FsError::NotLdos)
}

/// Piste et premier secteur d'un granule.
fn granule_position(geo: &Geometry, g: u32) -> (u8, u8) {
    ((g / geo.grans_per_track as u32) as u8, ((g % geo.grans_per_track as u32) * geo.gran_sectors as u32) as u8)
}

/// Fichiers de la disquette (ceux du système compris, marqués comme tels).
pub fn list(disk: &Disk) -> Result<Vec<DirEntry>, FsError> {
    let geo = geometry(disk)?;
    Ok(entries(disk, &geo)?
        .iter()
        .map(|(_, e)| DirEntry { name: display_name(e), size: file_size(e), system: e[0] & 0x48 != 0 })
        .collect())
}

/// Contenu d'un fichier.
pub fn read_file(disk: &Disk, name: &str) -> Result<Vec<u8>, FsError> {
    let geo = geometry(disk)?;
    let wanted = name11(name)?;
    let (_, entry) = entries(disk, &geo)?.into_iter().find(|(_, e)| e[5..16] == wanted).ok_or(FsError::NoSuchFile)?;
    let size = file_size(&entry);
    let mut out = Vec::with_capacity(size + 256);
    for g in granules(disk, &geo, &entry)? {
        let (track, first) = granule_position(&geo, g);
        for s in 0..geo.gran_sectors {
            if out.len() >= size {
                break;
            }
            out.extend_from_slice(&read(disk, track, first + s)?);
        }
    }
    out.truncate(size);
    Ok(out)
}

/// Écrit (crée ou remplace) un fichier, sans mot de passe.
pub fn write_file(disk: &mut Disk, name: &str, data: &[u8]) -> Result<(), FsError> {
    if disk.write_protected() {
        return Err(FsError::WriteProtected);
    }
    let geo = geometry(disk)?;
    let wanted = name11(name)?;
    let mut gat = read(disk, geo.dir_track, 0)?;
    let mut hit = read(disk, geo.dir_track, 1)?;
    let gpt = geo.grans_per_track as u32;

    // Fichier existant : ses granules sont libérés et son entrée réutilisée.
    let existing = entries(disk, &geo)?.into_iter().find(|(_, e)| e[5..16] == wanted);
    if let Some((_, e)) = &existing {
        if e[0] & 0x40 != 0 {
            return Err(FsError::SystemFile);
        }
        for g in granules(disk, &geo, e)? {
            gat[(g / gpt) as usize] &= !(1 << (g % gpt));
        }
    }
    let dec = match &existing {
        Some((dec, _)) => *dec,
        None => (0..=255u8)
            .find(|&d| {
                let (sector, _) = dec_location(d);
                hit[d as usize] == 0 && sector < geo.sectors
            })
            .ok_or(FsError::DirectoryFull)?,
    };

    // Granules libres, en commençant par le début de la disquette.
    let sectors = data.len().div_ceil(256);
    let needed = sectors.div_ceil(geo.gran_sectors as usize);
    let mut chosen: Vec<u32> = Vec::with_capacity(needed);
    for g in gpt..geo.tracks as u32 * gpt {
        if chosen.len() == needed {
            break;
        }
        let (track, bit) = ((g / gpt) as usize, 1u8 << (g % gpt));
        let locked = gat.get(0x60 + track).is_some_and(|l| l & bit != 0);
        if track != geo.dir_track as usize && gat[track] & bit == 0 && !locked {
            chosen.push(g);
        }
    }
    if chosen.len() < needed {
        return Err(FsError::DiskFull);
    }
    // Extensions : suites de granules consécutifs (32 au plus chacune).
    let mut extents: Vec<(u32, u32)> = Vec::new();
    for &g in &chosen {
        match extents.last_mut() {
            Some((start, count)) if *start + *count == g && *count < 32 => *count += 1,
            _ => extents.push((g, 1)),
        }
    }
    if extents.len() > 5 {
        return Err(FsError::Fragmented);
    }

    // Données.
    for (k, chunk) in data.chunks(256).enumerate() {
        let g = chosen[k / geo.gran_sectors as usize];
        let (track, first) = granule_position(&geo, g);
        let mut sector = vec![0u8; 256];
        sector[..chunk.len()].copy_from_slice(chunk);
        write(disk, track, first + (k % geo.gran_sectors as usize) as u8, &sector)?;
    }
    for &g in &chosen {
        gat[(g / gpt) as usize] |= 1 << (g % gpt);
    }

    // Entrée du répertoire.
    let mut entry = [0u8; ENTRY];
    entry[0] = 0x10; // utilisée, visible, accès complet
    entry[3] = (data.len() % 256) as u8;
    entry[5..16].copy_from_slice(&wanted);
    entry[16..18].copy_from_slice(&BLANK_PASSWORD);
    entry[18..20].copy_from_slice(&BLANK_PASSWORD);
    entry[20..22].copy_from_slice(&(sectors as u16).to_le_bytes());
    entry[22..32].fill(0xFF);
    for (i, (start, count)) in extents.iter().enumerate() {
        entry[22 + i * 2] = (start / gpt) as u8;
        entry[23 + i * 2] = (((start % gpt) << 5) | (count - 1)) as u8;
    }
    let (sector, offset) = dec_location(dec);
    let mut dir = read(disk, geo.dir_track, sector)?;
    dir[offset..offset + ENTRY].copy_from_slice(&entry);
    write(disk, geo.dir_track, sector, &dir)?;
    hit[dec as usize] = hash(&wanted);
    write(disk, geo.dir_track, 1, &hit)?;
    write(disk, geo.dir_track, 0, &gat)?;
    Ok(())
}

/// Espace libre : nombre de granules libres et taille d'un granule (octets).
pub fn free_space(disk: &Disk) -> Result<(usize, usize), FsError> {
    let geo = geometry(disk)?;
    let gat = read(disk, geo.dir_track, 0)?;
    let gpt = geo.grans_per_track as u32;
    let free = (gpt..geo.tracks as u32 * gpt)
        .filter(|g| {
            let (track, bit) = ((g / gpt) as usize, 1u8 << (g % gpt));
            track != geo.dir_track as usize && gat[track] & bit == 0 && gat.get(0x60 + track).is_none_or(|l| l & bit == 0)
        })
        .count();
    Ok((free, geo.gran_sectors as usize * 256))
}

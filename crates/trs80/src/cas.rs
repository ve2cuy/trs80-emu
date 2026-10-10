//! Images de cassette `.CAS` du Model I (500 bauds) : les octets enregistrés sur la bande.
//!
//! Une amorce d'octets 00h, l'octet de synchronisation A5h, puis l'un de deux formats :
//!
//! - **SYSTEM** (langage machine, commande `SYSTEM`) : 55h, nom de 6 caractères, blocs
//!   « 3Ch, n (0 = 256), adresse, n octets, somme de contrôle », puis « 78h, adresse de lancement ».
//!   La somme de contrôle est la somme, sur 8 bits, des deux octets d'adresse et des données.
//! - **BASIC** (commande `CLOAD`) : D3h D3h D3h, nom d'un caractère, puis le programme tokenisé
//!   tel qu'en mémoire : chaque ligne commence par le pointeur vers la suivante (0000h à la fin).
//!
//! - **Level I** (BASIC Level I, 250 bauds) : adresses de début et de fin (octet fort en
//!   premier), puis les octets de la mémoire. Seule la ROM Level I sait les lire : le
//!   magnétophone les lui joue ([`level1`] les reconnaît).
//!
//! Les images Model III à 1500 bauds (amorce 55h, synchronisation 7Fh) sont aussi acceptées :
//! [`normalize`] les ramène au format à 500 bauds, y compris celles qui gardent le train de
//! bits brut (un bit de départ à 0 devant chaque octet).
//!
//! Comme la ROM, la lecture tolère une somme de contrôle fausse (la ROM affiche « C » et
//! continue) et les octets parasites entre les blocs.

/// Erreurs de lecture d'une image `.CAS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CasError {
    /// Pas d'octet de synchronisation après l'amorce (image vide, ou bits décalés).
    NoSync,
    /// Ni SYSTEM ni BASIC après la synchronisation.
    UnknownFormat,
    /// Le fichier se termine au milieu d'un bloc.
    Truncated,
    /// Somme de contrôle d'un bloc SYSTEM invalide (adresse du bloc).
    BadChecksum(u16),
    /// Cassette Level I : il faut la ROM du BASIC Level I.
    NeedsLevel1,
    /// Cassette Level II : la ROM Level I ne sait pas la lire.
    NeedsLevel2,
}

impl core::fmt::Display for CasError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CasError::NoSync => write!(f, "no tape sync byte found (unsupported CAS image)"),
            CasError::UnknownFormat => write!(f, "neither a SYSTEM nor a BASIC tape"),
            CasError::Truncated => write!(f, "truncated CAS file"),
            CasError::BadChecksum(addr) => write!(f, "bad checksum in block at {addr:04X}h"),
            CasError::NeedsLevel1 => write!(f, "Level I tape: it needs the Level I BASIC ROM (4 KB)"),
            CasError::NeedsLevel2 => write!(f, "Level II tape: the Level I BASIC ROM cannot read it"),
        }
    }
}

/// Contenu d'une image cassette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tape<'a> {
    /// Programme en langage machine : blocs (à relire avec [`system_blocks`]), lancement
    /// (`None` : la cassette s'arrête après les blocs, une image d'écran par exemple) et
    /// nombre de blocs dont la somme de contrôle est fausse.
    System { name: [u8; 6], entry: Option<u16>, bad_blocks: u16 },
    /// Programme BASIC tokenisé, jusqu'au pointeur final 0000h inclus.
    Basic { name: u8, program: &'a [u8] },
}

/// Position du premier octet après la synchronisation.
fn after_sync(data: &[u8]) -> Result<usize, CasError> {
    let mut i = 0;
    // Amorce : octets 00h (500 bauds) ou 55h (1500 bauds, jusqu'à la synchronisation 7Fh).
    while i < data.len() && (data[i] == 0x00 || data[i] == 0x55 && matches!(data.get(i + 1), Some(0x55 | 0x7F))) {
        i += 1;
    }
    match data.get(i) {
        Some(0xA5) | Some(0x7F) => Ok(i + 1),
        _ => Err(CasError::NoSync),
    }
}

/// Image au format à 500 bauds (amorce de 00h, synchronisation A5h) : une cassette à
/// 1500 bauds est convertie (le magnétophone ne joue que du 500 bauds), et un en-tête BASIC
/// abîmé (53h D3h D3h : le bit 7 du premier octet perdu) est réparé.
pub fn normalize(data: &[u8]) -> alloc::vec::Vec<u8> {
    let Ok(start) = after_sync(data) else { return data.to_vec() };
    let body = &data[start..];
    let known = |b: &[u8]| matches!(b.first(), Some(0x55 | 0xD3));
    if data[start - 1] == 0x7F {
        // Train de bits brut : 9 bits par octet (bit de départ, puis l'octet).
        let (mut raw, missing) = unframe(body);
        let body = if known(body) || !known(&raw) {
            body.to_vec()
        } else {
            if missing > 0 {
                complete_entry(&mut raw, missing);
            }
            // Les derniers bits manquent parfois : fin de programme BASIC ajoutée.
            raw.extend([0, 0, 0]);
            raw
        };
        let mut out = alloc::vec![0u8; 256];
        out.push(0xA5);
        out.extend(body);
        return out;
    }
    let mut out = data.to_vec();
    if body.starts_with(&[0x53, 0xD3, 0xD3]) {
        out[start] = 0xD3;
    }
    out
}

/// Octets d'un train de bits où chaque octet est précédé d'un bit de départ, et nombre de
/// bits manquants du dernier octet (complétés par des 0).
fn unframe(bits: &[u8]) -> (alloc::vec::Vec<u8>, u32) {
    let total = bits.len() * 8;
    let bit = |k: usize| if k < total { (bits[k / 8] >> (7 - k % 8)) & 1 } else { 0 };
    let mut out = alloc::vec::Vec::with_capacity(total / 9 + 1);
    let mut k = 0;
    let mut missing = 0;
    while k + 2 <= total {
        out.push((1..9).fold(0u8, |acc, j| acc << 1 | bit(k + j)));
        missing = (k + 9).saturating_sub(total) as u32;
        k += 9;
    }
    (out, missing)
}

/// Cassette SYSTEM dont le dernier octet (celui de l'adresse de lancement) a perdu ses
/// derniers bits : parmi les valeurs possibles, celle qui tombe au début d'un bloc chargé.
fn complete_entry(body: &mut [u8], missing: u32) {
    if body.first() != Some(&0x55) || body.len() < 3 {
        return;
    }
    let last = body.len() - 1;
    if body[last - 2] != 0x78 {
        return;
    }
    let mut starts = alloc::vec::Vec::new();
    let _ = walk_system(&body[7..], |addr, _| starts.push(addr));
    let lo = body[last - 1];
    if let Some(hi) = (0..1u16 << missing)
        .map(|m| body[last] | m as u8)
        .find(|&hi| starts.contains(&u16::from_le_bytes([lo, hi])))
    {
        body[last] = hi;
    }
}

/// Cassette du BASIC Level I : après la synchronisation, ni SYSTEM ni BASIC Level II, mais
/// deux adresses plausibles (début ≤ fin, en RAM).
pub fn level1(data: &[u8]) -> bool {
    let Ok(start) = after_sync(data) else { return false };
    match data.get(start..start + 4) {
        Some(&[a, b, c, d]) if a != 0x55 && a != 0xD3 => {
            let (from, to) = (u16::from_be_bytes([a, b]), u16::from_be_bytes([c, d]));
            from >= 0x4000 && from <= to
        }
        _ => false,
    }
}

/// Lit une image `.CAS` (format et validation complète).
pub fn parse(data: &[u8]) -> Result<Tape<'_>, CasError> {
    let start = after_sync(data)?;
    let body = &data[start..];
    match body {
        [0x55, ..] => {
            let name: [u8; 6] = body.get(1..7).ok_or(CasError::Truncated)?.try_into().unwrap();
            let walk = walk_system(&body[7..], |_, _| {})?;
            Ok(Tape::System { name, entry: walk.entry, bad_blocks: walk.bad_blocks })
        }
        [0xD3, 0xD3, 0xD3, name, rest @ ..] => {
            let len = basic_length(rest)?;
            Ok(Tape::Basic { name: *name, program: &rest[..len] })
        }
        _ => Err(CasError::UnknownFormat),
    }
}

/// Appelle `load(adresse, données)` pour chaque bloc d'une cassette SYSTEM déjà validée.
pub fn system_blocks(data: &[u8], load: impl FnMut(u16, &[u8])) -> Result<(), CasError> {
    let start = after_sync(data)? + 7;
    walk_system(&data[start..], load).map(|_| ())
}

/// Position de l'octet qui suit le programme SYSTEM (après son adresse de lancement) : la
/// suite de la cassette, que certains chargeurs lisent eux-mêmes.
pub fn system_end(data: &[u8]) -> Result<usize, CasError> {
    let start = after_sync(data)? + 7;
    walk_system(&data[start..], |_, _| {}).map(|w| start + w.end)
}

/// Résultat du parcours d'une cassette SYSTEM.
struct Walk {
    entry: Option<u16>,
    /// Position qui suit l'adresse de lancement (ou la fin de l'image).
    end: usize,
    bad_blocks: u16,
}

/// Parcourt les blocs. Comme la ROM, saute les octets qui ne commencent pas un bloc
/// (certaines cassettes ont un nom de 7 caractères, par exemple) et charge un bloc dont la
/// somme de contrôle est fausse. Une image qui s'arrête après au moins un bloc complet n'a
/// pas d'adresse de lancement.
fn walk_system(data: &[u8], mut load: impl FnMut(u16, &[u8])) -> Result<Walk, CasError> {
    let mut i = 0;
    let mut blocks = 0;
    let mut bad_blocks = 0;
    loop {
        match data.get(i) {
            Some(0x3C) => {
                let header = data.get(i + 1..i + 4).ok_or(CasError::Truncated)?;
                let n = if header[0] == 0 { 256 } else { header[0] as usize };
                let addr = u16::from_le_bytes([header[1], header[2]]);
                let bytes = data.get(i + 4..i + 4 + n).ok_or(CasError::Truncated)?;
                let checksum = *data.get(i + 4 + n).ok_or(CasError::Truncated)?;
                let sum = bytes
                    .iter()
                    .fold(header[1].wrapping_add(header[2]), |acc, &b| acc.wrapping_add(b));
                if sum != checksum {
                    bad_blocks += 1;
                }
                load(addr, bytes);
                blocks += 1;
                i += 5 + n;
            }
            Some(0x78) => {
                let addr = data.get(i + 1..i + 3).ok_or(CasError::Truncated)?;
                let entry = Some(u16::from_le_bytes([addr[0], addr[1]]));
                return Ok(Walk { entry, end: i + 3, bad_blocks });
            }
            Some(_) => i += 1,
            None if blocks > 0 => return Ok(Walk { entry: None, end: i, bad_blocks }),
            None => return Err(CasError::Truncated),
        }
    }
}

/// Longueur d'un programme BASIC tokenisé, pointeur final 0000h compris.
fn basic_length(data: &[u8]) -> Result<usize, CasError> {
    let mut i = 0;
    loop {
        let link = data.get(i..i + 2).ok_or(CasError::Truncated)?;
        if link == [0, 0] {
            return Ok(i + 2);
        }
        // Pointeur (2), numéro de ligne (2), texte tokenisé terminé par 00h.
        let text = data.get(i + 4..).ok_or(CasError::Truncated)?;
        let end = text.iter().position(|&b| b == 0).ok_or(CasError::Truncated)?;
        i += 4 + end + 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system_tape() -> [u8; 24] {
        let mut t = [0u8; 24];
        // Amorce, synchronisation, en-tête « TEST  »
        t[..2].copy_from_slice(&[0x00, 0xA5]);
        t[2] = 0x55;
        t[3..9].copy_from_slice(b"TEST  ");
        // Bloc de 3 octets en 7000h : somme = 00 + 70 + 01 + 02 + 03 = 76h
        t[9..17].copy_from_slice(&[0x3C, 3, 0x00, 0x70, 1, 2, 3, 0x76]);
        // Lancement en 7000h
        t[17..20].copy_from_slice(&[0x78, 0x00, 0x70]);
        t
    }

    #[test]
    fn system_tape_blocks_and_entry() {
        let t = system_tape();
        assert_eq!(parse(&t), Ok(Tape::System { name: *b"TEST  ", entry: Some(0x7000), bad_blocks: 0 }));
        let mut seen = (0, 0);
        system_blocks(&t, |addr, bytes| seen = (addr, bytes.len())).unwrap();
        assert_eq!(seen, (0x7000, 3));
    }

    #[test]
    fn stray_bytes_between_records_are_skipped() {
        // Nom de 7 caractères (MICROCHESS 1.5), puis la suite de la cassette.
        let mut t = alloc::vec![0x00, 0xA5, 0x55];
        t.extend(b"CHESS  ");
        t.extend([0x3C, 1, 0x00, 0x70, 0xC9, 0x39, 0x78, 0x00, 0x70, 0x00, 0xA5, 0x12]);
        assert_eq!(parse(&t), Ok(Tape::System { name: *b"CHESS ", entry: Some(0x7000), bad_blocks: 0 }));
        assert_eq!(system_end(&t), Ok(t.len() - 3));
    }

    #[test]
    fn system_tape_bad_checksum_still_loads() {
        let mut t = system_tape();
        t[16] = 0x00;
        assert_eq!(parse(&t), Ok(Tape::System { name: *b"TEST  ", entry: Some(0x7000), bad_blocks: 1 }));
    }

    #[test]
    fn system_tape_without_entry() {
        let t = &system_tape()[..17];
        assert_eq!(parse(t), Ok(Tape::System { name: *b"TEST  ", entry: None, bad_blocks: 0 }));
        assert_eq!(system_end(t), Ok(17));
    }

    #[test]
    fn raw_1500_baud_bits_are_unframed() {
        // Amorce 55h, synchronisation 7Fh, puis D3h D3h D3h « A » avec un bit 0 devant chacun.
        let mut bits = alloc::string::String::new();
        for b in [0xD3u8, 0xD3, 0xD3, b'A', 0, 0] {
            bits += &alloc::format!("0{b:08b}");
        }
        while bits.len() % 8 != 0 {
            bits.push('0');
        }
        let mut t = alloc::vec![0x55u8; 4];
        t.push(0x7F);
        t.extend((0..bits.len() / 8).map(|k| u8::from_str_radix(&bits[k * 8..k * 8 + 8], 2).unwrap()));
        let n = normalize(&t);
        assert_eq!(&n[255..262], &[0x00, 0xA5, 0xD3, 0xD3, 0xD3, b'A', 0]);
    }

    #[test]
    fn incomplete_entry_byte_points_to_a_block() {
        // Bloc en 42F0h, lancement « 78 F0 4? » : 2 bits manquants, 40h devient 42h.
        let mut body = alloc::vec![0x55];
        body.extend(b"QUEST ");
        body.extend([0x3C, 1, 0xF0, 0x42, 0xC9, 0xFB, 0x78, 0xF0, 0x40]);
        complete_entry(&mut body, 2);
        assert_eq!(body[body.len() - 1], 0x42);
    }

    #[test]
    fn damaged_basic_header_is_repaired() {
        let t = [0x00, 0xA5, 0x53, 0xD3, 0xD3, b'A', 0x00, 0x00];
        assert_eq!(normalize(&t)[2], 0xD3);
    }

    #[test]
    fn basic_tape() {
        // 10 PRINT : ligne en 42E9h, pointeur vers 42EFh, token PRINT = B2h
        let t = [0x00, 0xA5, 0xD3, 0xD3, 0xD3, b'A', 0xEF, 0x42, 10, 0, 0xB2, 0x00, 0x00, 0x00];
        assert_eq!(parse(&t), Ok(Tape::Basic { name: b'A', program: &t[6..14] }));
    }

    #[test]
    fn no_sync() {
        assert_eq!(parse(&[0x00, 0x00, 0x12]), Err(CasError::NoSync));
    }
}

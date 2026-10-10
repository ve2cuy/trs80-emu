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
//! Les images Model III à 1500 bauds (amorce 55h, synchronisation 7Fh) sont aussi acceptées.

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
}

impl core::fmt::Display for CasError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CasError::NoSync => write!(f, "no tape sync byte found (unsupported CAS image)"),
            CasError::UnknownFormat => write!(f, "neither a SYSTEM nor a BASIC tape"),
            CasError::Truncated => write!(f, "truncated CAS file"),
            CasError::BadChecksum(addr) => write!(f, "bad checksum in block at {addr:04X}h"),
        }
    }
}

/// Contenu d'une image cassette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tape<'a> {
    /// Programme en langage machine : blocs (à relire avec [`system_blocks`]) et lancement.
    System { name: [u8; 6], entry: u16 },
    /// Programme BASIC tokenisé, jusqu'au pointeur final 0000h inclus.
    Basic { name: u8, program: &'a [u8] },
}

/// Position du premier octet après la synchronisation.
fn after_sync(data: &[u8]) -> Result<usize, CasError> {
    let mut i = 0;
    // Amorce : octets 00h (500 bauds) ou 55h (1500 bauds).
    while i < data.len() && (data[i] == 0x00 || data[i] == 0x55 && data.get(i + 1) == Some(&0x55)) {
        i += 1;
    }
    match data.get(i) {
        Some(0xA5) | Some(0x7F) => Ok(i + 1),
        _ => Err(CasError::NoSync),
    }
}

/// Lit une image `.CAS` (format et validation complète).
pub fn parse(data: &[u8]) -> Result<Tape<'_>, CasError> {
    let start = after_sync(data)?;
    let body = &data[start..];
    match body {
        [0x55, ..] => {
            let name: [u8; 6] = body.get(1..7).ok_or(CasError::Truncated)?.try_into().unwrap();
            let entry = walk_system(&body[7..], |_, _| {})?;
            Ok(Tape::System { name, entry })
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
    let body = &data[start..];
    let mut i = 0;
    loop {
        match body.get(i) {
            Some(0x3C) => {
                let n = match body.get(i + 1) {
                    Some(0) => 256,
                    Some(&n) => n as usize,
                    None => return Err(CasError::Truncated),
                };
                i += 5 + n;
            }
            Some(0x78) => return Ok(start + i + 3),
            _ => return Err(CasError::Truncated),
        }
    }
}

fn walk_system(data: &[u8], mut load: impl FnMut(u16, &[u8])) -> Result<u16, CasError> {
    let mut i = 0;
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
                    return Err(CasError::BadChecksum(addr));
                }
                load(addr, bytes);
                i += 5 + n;
            }
            Some(0x78) => {
                let addr = data.get(i + 1..i + 3).ok_or(CasError::Truncated)?;
                return Ok(u16::from_le_bytes([addr[0], addr[1]]));
            }
            _ => return Err(CasError::Truncated),
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
        assert_eq!(parse(&t), Ok(Tape::System { name: *b"TEST  ", entry: 0x7000 }));
        let mut seen = (0, 0);
        system_blocks(&t, |addr, bytes| seen = (addr, bytes.len())).unwrap();
        assert_eq!(seen, (0x7000, 3));
    }

    #[test]
    fn system_tape_bad_checksum() {
        let mut t = system_tape();
        t[16] = 0x00;
        assert_eq!(parse(&t), Err(CasError::BadChecksum(0x7000)));
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

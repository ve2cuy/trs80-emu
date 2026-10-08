//! Format `.CMD` (exécutable TRS-80) : une suite d'enregistrements « type, longueur, données ».
//!
//! - 01h : bloc à charger. Longueur = 2 (adresse) + n octets de données; une longueur
//!   de 0, 1 ou 2 signifie 256, 257 ou 258 (le bloc fait alors 254 à 256 octets).
//! - 02h : adresse de lancement (fin du fichier).
//! - Autres (05h nom du module, 1Fh copyright, ...) : commentaires, ignorés.

/// Erreurs de lecture d'un fichier `.CMD`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmdError {
    /// Le fichier se termine au milieu d'un enregistrement.
    Truncated,
    /// Aucune adresse de lancement (enregistrement 02h).
    NoEntryPoint,
}

impl core::fmt::Display for CmdError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CmdError::Truncated => write!(f, "truncated CMD file"),
            CmdError::NoEntryPoint => write!(f, "CMD file has no entry point"),
        }
    }
}

/// Parcourt un fichier `.CMD` : appelle `load(adresse, données)` pour chaque bloc et
/// retourne l'adresse de lancement. Le fichier est entièrement validé avant le
/// premier appel à `load`, pour ne jamais charger un programme à moitié.
pub fn parse(data: &[u8], mut load: impl FnMut(u16, &[u8])) -> Result<u16, CmdError> {
    let entry = walk(data, |_, _| {})?;
    walk(data, &mut load)?;
    Ok(entry)
}

fn walk(data: &[u8], mut load: impl FnMut(u16, &[u8])) -> Result<u16, CmdError> {
    let mut i = 0;
    while i < data.len() {
        let kind = data[i];
        let len = *data.get(i + 1).ok_or(CmdError::Truncated)? as usize;
        match kind {
            0x01 => {
                let len = if len < 3 { len + 256 } else { len };
                let block = data.get(i + 2..i + 2 + len).ok_or(CmdError::Truncated)?;
                load(u16::from_le_bytes([block[0], block[1]]), &block[2..]);
                i += 2 + len;
            }
            0x02 => {
                let addr = data.get(i + 2..i + 4).ok_or(CmdError::Truncated)?;
                return Ok(u16::from_le_bytes([addr[0], addr[1]]));
            }
            _ => {
                let len = if len == 0 { 256 } else { len };
                if i + 2 + len > data.len() {
                    return Err(CmdError::Truncated);
                }
                i += 2 + len;
            }
        }
    }
    Err(CmdError::NoEntryPoint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_and_entry_point() {
        // Copyright (1Fh), bloc de 3 octets en 5200h, bloc de 256 octets en 6000h, lancement en 5200h.
        let mut file = [0x1F, 0x03, b'(', b'C', b')', 0x01, 0x05, 0x00, 0x52, 1, 2, 3].to_vec();
        file.extend_from_slice(&[0x01, 0x02, 0x00, 0x60]);
        file.extend_from_slice(&[0xAA; 256]);
        file.extend_from_slice(&[0x02, 0x02, 0x00, 0x52]);

        let mut seen = [(0u16, 0usize); 2];
        let mut n = 0;
        let entry = parse(&file, |addr, bytes| {
            seen[n] = (addr, bytes.len());
            n += 1;
        });
        assert_eq!(entry, Ok(0x5200));
        assert_eq!(seen, [(0x5200, 3), (0x6000, 256)]);
    }

    #[test]
    fn truncated_file_loads_nothing() {
        let file = [0x01, 0x05, 0x00, 0x52, 1, 2, 3, 0x01, 0x10, 0x00];
        let mut called = false;
        assert_eq!(parse(&file, |_, _| called = true), Err(CmdError::Truncated));
        assert!(!called);
    }

    #[test]
    fn missing_entry_point() {
        assert_eq!(parse(&[0x01, 0x03, 0x00, 0x52, 0xC9], |_, _| {}), Err(CmdError::NoEntryPoint));
    }
}

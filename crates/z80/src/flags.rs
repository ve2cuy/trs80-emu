//! Bits du registre F et tables précalculées.

/// Signe (bit 7 du résultat).
pub const S: u8 = 0x80;
/// Zéro.
pub const Z: u8 = 0x40;
/// Non documenté : copie du bit 5 (selon l'instruction).
pub const Y: u8 = 0x20;
/// Demi-retenue (entre les bits 3 et 4).
pub const H: u8 = 0x10;
/// Non documenté : copie du bit 3 (selon l'instruction).
pub const X: u8 = 0x08;
/// Parité / dépassement (overflow).
pub const PV: u8 = 0x04;
/// Soustraction (utilisé par DAA).
pub const N: u8 = 0x02;
/// Retenue.
pub const C: u8 = 0x01;

/// S, Z, Y et X pour chaque valeur 8 bits.
pub(crate) static SZ: [u8; 256] = build_sz();
/// S, Z, Y, X et parité (P/V) pour chaque valeur 8 bits.
pub(crate) static SZP: [u8; 256] = build_szp();

const fn build_sz() -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let v = i as u8;
        t[i] = (v & (S | Y | X)) | if v == 0 { Z } else { 0 };
        i += 1;
    }
    t
}

const fn build_szp() -> [u8; 256] {
    let sz = build_sz();
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let parity_even = (i as u8).count_ones() % 2 == 0;
        t[i] = sz[i] | if parity_even { PV } else { 0 };
        i += 1;
    }
    t
}

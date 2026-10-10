//! Affichage : 16 lignes de 64 caractères (cellules de 6 × 12 pixels), ou 24 lignes de 80
//! caractères sur le Model 4 (cellules de 8 × 10 pixels).
//!
//! - Codes 00h-7Fh : caractères. Sans la modification minuscules, la mémoire vidéo du
//!   Model I n'a pas de bit 6 : 00h-1Fh et 60h-7Fh s'affichent comme les majuscules 40h-5Fh.
//!   Les Model III et 4 affichent les minuscules (60h-7Fh).
//! - Codes 80h-FFh : blocs semi-graphiques 2 × 3 (bits 0-5). Model 4 en vidéo inversée :
//!   le caractère 00h-7Fh correspondant, en inverse. (Sur les Model III et 4, C0h-FFh sont
//!   des caractères spéciaux; ils s'affichent ici comme des blocs.)
//! - Model II : 80 × 24, bit 7 = vidéo inversée; 00h-03h sont des triangles (moitiés de
//!   cellule coupées en diagonale) qui dessinent notamment le logo de TRSDOS-II.
//! - Mode 32 caractères : seules les colonnes paires s'affichent, en double largeur.

use crate::font::{ASCII_5B, FONT, LOWER};

/// Taille de l'image en 64 × 16 (et taille maximale : voir `MAX_WIDTH`).
pub const SCREEN_WIDTH: usize = 64 * 6;
pub const SCREEN_HEIGHT: usize = 16 * 12;
/// Image la plus grande (80 × 24 du Model 4) : taille du tampon d'affichage à prévoir.
pub const MAX_WIDTH: usize = 80 * 8;
pub const MAX_HEIGHT: usize = 24 * 10;

/// Géométrie d'un mode texte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mode {
    pub cols: usize,
    pub rows: usize,
    cell_w: usize,
    cell_h: usize,
    /// Position du caractère de 5 × 7 dans sa cellule.
    glyph_x: usize,
    glyph_y: usize,
    /// Model II : codes 00h-03h en triangles, et 5Bh-5Eh en [ \ ] ^ (jeu ASCII) au lieu
    /// des flèches des Model I et III.
    triangles: bool,
}

impl Mode {
    pub fn width(&self) -> usize {
        self.cols * self.cell_w
    }

    pub fn height(&self) -> usize {
        self.rows * self.cell_h
    }

    /// Rangée (0 à 2) d'un bloc semi-graphique pour la ligne `y` de la cellule.
    fn block_row(&self, y: usize) -> usize {
        if self.cell_h == 12 { y / 4 } else { [0, 0, 0, 1, 1, 1, 1, 2, 2, 2][y] }
    }
}

pub const MODE64: Mode = Mode { cols: 64, rows: 16, cell_w: 6, cell_h: 12, glyph_x: 0, glyph_y: 2, triangles: false };
pub const MODE80: Mode = Mode { cols: 80, rows: 24, cell_w: 8, cell_h: 10, glyph_x: 1, glyph_y: 1, triangles: false };
/// Model II : 80 × 24 avec les triangles 00h-03h.
pub const MODE80_II: Mode = Mode { triangles: true, ..MODE80 };

/// Phosphore blanc légèrement bleuté sur fond noir.
const FG: [u8; 4] = [0xE6, 0xEE, 0xFF, 0xFF];
const BG: [u8; 4] = [0x08, 0x08, 0x0A, 0xFF];

/// Code ASCII (20h-7Fh) réellement affiché pour un code de caractère.
fn ascii(code: u8, lowercase: bool) -> u8 {
    match code & 0x7F {
        c @ 0x00..=0x1F => c + 0x40,
        c @ 0x60..=0x7F if !lowercase => c - 0x20,
        c => c,
    }
}

/// Caractère affiché pour un octet de la mémoire vidéo (blocs graphiques : espace).
pub(crate) fn display_char(code: u8, lowercase: bool, inverse: bool) -> char {
    if code & 0x80 != 0 && !inverse { ' ' } else { ascii(code, lowercase) as char }
}

/// Le pixel (x, y) du caractère `c` (20h-7Fh), relatif au coin de son dessin de 5 × 8.
/// `ascii` : [ \ ] ^ en 5Bh-5Eh (Model II) au lieu des flèches.
fn glyph_pixel(c: u8, x: usize, y: usize, ascii: bool) -> bool {
    if x >= 5 {
        return false;
    }
    let row = if ascii && (0x5B..=0x5E).contains(&c) {
        ASCII_5B[(c - 0x5B) as usize].get(y).copied().unwrap_or(0)
    } else if c >= 0x60 {
        LOWER[(c - 0x60) as usize].get(y).copied().unwrap_or(0)
    } else {
        FONT[(c - 0x20) as usize].get(y).copied().unwrap_or(0)
    };
    row & (0x10 >> x) != 0
}

/// Le pixel (x, y) d'une cellule est-il allumé ? Avec `text` faux, les caractères ne sont
/// pas dessinés (seulement les blocs graphiques et le fond des caractères inversés).
fn cell_pixel(code: u8, x: usize, y: usize, m: &Mode, text: bool, lowercase: bool, inverse: bool) -> bool {
    if code & 0x80 != 0 && !inverse {
        let bit = m.block_row(y) * 2 + x * 2 / m.cell_w;
        return code & (1 << bit) != 0;
    }
    let inverted = code & 0x80 != 0;
    if m.triangles && code & 0x7C == 0 {
        // Moitié allumée : 00h en bas à droite, 01h en bas à gauche, 02h en haut à
        // gauche, 03h en haut à droite (proportions de la cellule).
        // Centre du pixel, ramené à la même échelle sur les deux axes.
        let (u, v) = ((2 * x + 1) * m.cell_h, (2 * y + 1) * m.cell_w);
        let full = 2 * m.cell_w * m.cell_h;
        let lit = match code & 3 {
            0 => u + v >= full,
            1 => v >= u,
            2 => u + v < full,
            _ => u > v,
        };
        return lit != inverted;
    }
    let lit = text
        && x >= m.glyph_x
        && y >= m.glyph_y
        && glyph_pixel(ascii(code, lowercase), x - m.glyph_x, y - m.glyph_y, m.triangles);
    lit != inverted
}

/// Dessine l'écran dans `out` (RGBA, `m.width()` × `m.height()`). `video` contient les
/// `m.cols × m.rows` caractères affichés, ligne par ligne.
pub(crate) fn render(video: &[u8], m: &Mode, wide: bool, text: bool, lowercase: bool, inverse: bool, out: &mut [u8]) {
    let (w, h) = (m.width(), m.height());
    assert!(out.len() >= w * h * 4, "tampon d'affichage trop petit");
    for y in 0..h {
        let row = y / m.cell_h;
        let cy = y % m.cell_h;
        for x in 0..w {
            let (col, cx) = if wide {
                // Double largeur : une cellule de deux largeurs par colonne paire.
                ((x / (2 * m.cell_w)) * 2, (x % (2 * m.cell_w)) / 2)
            } else {
                (x / m.cell_w, x % m.cell_w)
            };
            let code = video.get(row * m.cols + col).copied().unwrap_or(0x20);
            let on = cell_pixel(code, cx, cy, m, text, lowercase, inverse);
            let i = (y * w + x) * 4;
            out[i..i + 4].copy_from_slice(if on { &FG } else { &BG });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(c: u8, ascii: bool) -> [u8; 7] {
        core::array::from_fn(|y| (0..5).fold(0, |r, x| r | (glyph_pixel(c, x, y, ascii) as u8) << (4 - x)))
    }

    #[test]
    fn model2_shows_brackets_where_model1_shows_arrows() {
        // 5Bh : ↑ sur les Model I et III, [ sur le Model II (jeu ASCII).
        assert_eq!(rows(0x5B, false), [0x04, 0x0E, 0x15, 0x04, 0x04, 0x04, 0x04]);
        assert_eq!(rows(0x5B, true), [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E]);
        assert_eq!(rows(0x5D, true), [0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E]);
        // Les autres caractères ne changent pas.
        assert_eq!(rows(b'A', true), rows(b'A', false));
    }
}

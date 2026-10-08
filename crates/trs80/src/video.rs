//! Affichage : 16 lignes de 64 caractères, chaque cellule fait 6 × 12 pixels.
//!
//! - Codes 00h-7Fh : caractères. Sans la modification minuscules, la mémoire vidéo
//!   n'a pas de bit 6 : 00h-1Fh et 60h-7Fh s'affichent comme les majuscules 40h-5Fh.
//! - Codes 80h-FFh : blocs semi-graphiques 2 × 3 (bits 0-5), 3 × 4 pixels chacun.
//! - Mode 32 caractères : seules les colonnes paires s'affichent, en double largeur.

use crate::font::FONT;

pub const SCREEN_WIDTH: usize = 64 * CELL_W;
pub const SCREEN_HEIGHT: usize = 16 * CELL_H;

const CELL_W: usize = 6;
const CELL_H: usize = 12;
/// Ligne où commence le caractère de 5 × 7 dans sa cellule.
const GLYPH_TOP: usize = 2;

/// Phosphore blanc légèrement bleuté sur fond noir.
const FG: [u8; 4] = [0xE6, 0xEE, 0xFF, 0xFF];
const BG: [u8; 4] = [0x08, 0x08, 0x0A, 0xFF];

/// Code ASCII (20h-5Fh) réellement affiché pour un code de caractère.
fn ascii(code: u8) -> u8 {
    match code & 0x7F {
        c @ 0x00..=0x1F => c + 0x40,
        c @ 0x60..=0x7F => c - 0x20,
        c => c,
    }
}

/// Caractère affiché pour un octet de la mémoire vidéo (blocs graphiques : espace).
pub(crate) fn display_char(code: u8) -> char {
    if code & 0x80 != 0 { ' ' } else { ascii(code) as char }
}

/// Le pixel (x, y) d'une cellule (6 × 12) est-il allumé ?
fn cell_pixel(code: u8, x: usize, y: usize) -> bool {
    if code & 0x80 != 0 {
        let bit = (y / 4) * 2 + x / 3;
        code & (1 << bit) != 0
    } else if x < 5 && (GLYPH_TOP..GLYPH_TOP + 7).contains(&y) {
        let row = FONT[(ascii(code) - 0x20) as usize][y - GLYPH_TOP];
        row & (0x10 >> x) != 0
    } else {
        false
    }
}

pub(crate) fn render(video: &[u8; 1024], wide: bool, out: &mut [u8]) {
    assert!(out.len() >= SCREEN_WIDTH * SCREEN_HEIGHT * 4, "tampon d'affichage trop petit");
    for y in 0..SCREEN_HEIGHT {
        let row = y / CELL_H;
        let cy = y % CELL_H;
        for x in 0..SCREEN_WIDTH {
            let (col, cx) = if wide {
                // Double largeur : une cellule de 12 pixels par colonne paire.
                ((x / (2 * CELL_W)) * 2, (x % (2 * CELL_W)) / 2)
            } else {
                (x / CELL_W, x % CELL_W)
            };
            let on = cell_pixel(video[row * 64 + col], cx, cy);
            let i = (y * SCREEN_WIDTH + x) * 4;
            out[i..i + 4].copy_from_slice(if on { &FG } else { &BG });
        }
    }
}

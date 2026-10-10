//! Carte graphique haute résolution Radio Shack des Model III (26-1125) et 4 (26-1126) :
//! 640 × 240 points, superposés au texte. Comme dans xtrs (MIT, Tim Mann), qui l'émule avec
//! la Grafyx Solution de Micro Labs.
//!
//! La mémoire fait 32 Ko : 256 lignes de 128 octets, dont 240 lignes de 80 octets (8 points
//! par octet, bit 7 à gauche) sont affichées.
//!
//! Ports :
//! - 80h : X, l'octet dans la ligne (0-127); 81h : Y, la ligne (0-255);
//! - 82h : l'octet en (X, Y), en lecture et en écriture; après l'accès, X et Y avancent (ou
//!   reculent) selon le registre de mode;
//! - 83h : mode : bit 0 affichage du graphique, bit 2 X recule, bit 3 Y recule, bits 4-5 X
//!   et Y ne bougent pas après une lecture, bits 6-7 après une écriture;
//! - Model 4 : 8Ch et 8Dh, le point de départ de l'affichage (X, Y : défilement); 8Eh bit 0,
//!   le texte superposé (comme xtrs).

use alloc::vec;
use alloc::vec::Vec;

/// Octets par ligne de la mémoire, lignes de la mémoire.
const XSIZE: usize = 128;
const YSIZE: usize = 256;
/// Partie affichée : 80 octets (640 points) sur 240 lignes.
pub const HIRES_WIDTH: usize = 640;
pub const HIRES_HEIGHT: usize = 240;

const ENABLE: u8 = 0x01;
const XDEC: u8 = 0x04;
const YDEC: u8 = 0x08;
const XNOCLK_READ: u8 = 0x10;
const YNOCLK_READ: u8 = 0x20;
const XNOCLK_WRITE: u8 = 0x40;
const YNOCLK_WRITE: u8 = 0x80;

pub(crate) struct HiRes {
    mem: Vec<u8>,
    x: u8,
    y: u8,
    mode: u8,
    xoffset: u8,
    yoffset: u8,
    /// Changements de l'image (écritures, mode, défilement) : compteur cumulatif, pour que
    /// la page ne redessine que si l'image a changé.
    pub(crate) changes: u32,
}

impl HiRes {
    pub(crate) fn new() -> Self {
        HiRes { mem: vec![0; XSIZE * YSIZE], x: 0, y: 0, mode: 0, xoffset: 0, yoffset: 0, changes: 0 }
    }

    /// RESET : l'affichage du graphique s'éteint (la mémoire reste).
    pub(crate) fn reset(&mut self) {
        self.mode = 0;
        self.xoffset = 0;
        self.yoffset = 0;
        self.changes = self.changes.wrapping_add(1);
    }

    /// Le graphique est-il affiché ?
    pub(crate) fn enabled(&self) -> bool {
        self.mode & ENABLE != 0
    }

    fn index(&self) -> usize {
        self.y as usize * XSIZE + self.x as usize % XSIZE
    }

    /// Avance (ou recule) X et Y après un accès, sauf si le mode le bloque.
    fn step(&mut self, x_stays: u8, y_stays: u8) {
        if self.mode & x_stays == 0 {
            self.x = if self.mode & XDEC != 0 { self.x.wrapping_sub(1) } else { self.x.wrapping_add(1) };
        }
        if self.mode & y_stays == 0 {
            self.y = if self.mode & YDEC != 0 { self.y.wrapping_sub(1) } else { self.y.wrapping_add(1) };
        }
    }

    /// Lecture d'un port (80h-83h, 8Ch-8Eh); `None` : pas un port de la carte.
    pub(crate) fn read(&mut self, port: u8) -> Option<u8> {
        match port {
            0x82 => {
                let v = self.mem[self.index()];
                self.step(XNOCLK_READ, YNOCLK_READ);
                Some(v)
            }
            _ => None,
        }
    }

    /// Écriture d'un port; faux si ce n'est pas un port de la carte. `model4` : ports
    /// 8Ch-8Eh.
    pub(crate) fn write(&mut self, port: u8, val: u8, model4: bool) -> bool {
        match port {
            0x80 => self.x = val,
            0x81 => self.y = val,
            0x82 => {
                let i = self.index();
                self.mem[i] = val;
                self.step(XNOCLK_WRITE, YNOCLK_WRITE);
            }
            0x83 => self.mode = val,
            0x8C if model4 => self.xoffset = val % XSIZE as u8,
            0x8D if model4 => self.yoffset = val,
            0x8E if model4 => {}
            _ => return false,
        }
        if port >= 0x82 {
            self.changes = self.changes.wrapping_add(1);
        }
        true
    }

    /// Le point (x, y) de l'écran (0-639, 0-239) est-il allumé ?
    pub(crate) fn pixel(&self, x: usize, y: usize) -> bool {
        let col = (x / 8 + self.xoffset as usize) % XSIZE;
        let row = (y + self.yoffset as usize) % YSIZE;
        self.mem[row * XSIZE + col] & (0x80 >> (x % 8)) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_advance_and_show_on_screen() {
        let mut h = HiRes::new();
        // Mode : graphique affiché, X avance à l'écriture, Y ne bouge pas.
        assert!(h.write(0x83, ENABLE | YNOCLK_WRITE | XNOCLK_READ | YNOCLK_READ, true));
        h.write(0x80, 10, true);
        h.write(0x81, 5, true);
        h.write(0x82, 0x80, true);
        h.write(0x82, 0x01, true);
        assert!(h.enabled());
        assert!(h.pixel(80, 5), "bit 7 de l'octet 10 : point 80");
        assert!(h.pixel(95, 5), "bit 0 de l'octet 11 : point 95");
        assert!(!h.pixel(81, 5));
        // Relecture : X et Y restent en place (bits 4-5).
        h.write(0x80, 11, true);
        assert_eq!(h.read(0x82), Some(0x01));
        assert_eq!(h.read(0x82), Some(0x01));
        // Défilement (Model 4) : l'octet 11 passe au début de la ligne.
        h.write(0x8C, 11, true);
        assert!(h.pixel(7, 5));
        assert!(!h.write(0x8C, 0, false), "8Ch : Model 4 seulement");
    }
}

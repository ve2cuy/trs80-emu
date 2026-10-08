//! Clavier du Model 1 : une matrice de 8 rangées × 8 touches, lue en mémoire de 3800h à 3BFFh.
//!
//! Chaque bit de l'octet bas de l'adresse sélectionne une rangée; la lecture retourne
//! le OU des rangées sélectionnées (1 = touche enfoncée).
//!
//! ```text
//!         bit 0   1     2     3     4     5     6     7
//! 3801  0  @     A     B     C     D     E     F     G
//! 3802  1  H     I     J     K     L     M     N     O
//! 3804  2  P     Q     R     S     T     U     V     W
//! 3808  3  X     Y     Z
//! 3810  4  0     1     2     3     4     5     6     7
//! 3820  5  8     9     :     ;     ,     -     .     /
//! 3840  6  ENTER CLEAR BREAK ↑     ↓     ←     →     ESPACE
//! 3880  7  SHIFT
//! ```
//!
//! Les symboles ne sont pas aux mêmes endroits que sur un clavier de PC (sur le TRS-80,
//! `"` est MAJ+2 et `*` est MAJ+:). Chaque touche indique donc si elle exige la touche
//! MAJ du TRS-80, l'interdit, ou suit celle du clavier hôte.

/// Exigence d'une touche envers la touche MAJ du TRS-80.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shift {
    /// Suit la touche Majuscule du clavier hôte.
    Host,
    /// Exige MAJ (ex. : `"` = MAJ+2).
    On,
    /// Interdit MAJ (ex. : `:` est MAJ+; sur un PC, mais sans MAJ sur le TRS-80).
    Off,
}

/// Une touche du TRS-80 : sa position dans la matrice et son exigence envers MAJ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    row: u8,
    bit: u8,
    shift: Shift,
}

const fn key(row: u8, bit: u8, shift: Shift) -> Option<Key> {
    Some(Key { row, bit, shift })
}

impl Key {
    /// Touche correspondant à un nom de touche du DOM (`KeyboardEvent.key`) :
    /// un caractère (`"a"`, `"\""`, `"+"`) ou un nom (`"Enter"`, `"ArrowUp"`, ...).
    pub fn from_name(name: &str) -> Option<Key> {
        use Shift::{Host, Off, On};
        let mut chars = name.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            return match c.to_ascii_uppercase() {
                '@' => key(0, 0, Off),
                c @ 'A'..='G' => key(0, c as u8 - b'A' + 1, Host),
                c @ 'H'..='O' => key(1, c as u8 - b'H', Host),
                c @ 'P'..='W' => key(2, c as u8 - b'P', Host),
                c @ 'X'..='Z' => key(3, c as u8 - b'X', Host),
                c @ '0'..='7' => key(4, c as u8 - b'0', Off),
                '8' => key(5, 0, Off),
                '9' => key(5, 1, Off),
                ':' => key(5, 2, Off),
                ';' => key(5, 3, Off),
                ',' => key(5, 4, Off),
                '-' => key(5, 5, Off),
                '.' => key(5, 6, Off),
                '/' => key(5, 7, Off),
                // Symboles obtenus avec MAJ sur le TRS-80
                '!' => key(4, 1, On),
                '"' => key(4, 2, On),
                '#' => key(4, 3, On),
                '$' => key(4, 4, On),
                '%' => key(4, 5, On),
                '&' => key(4, 6, On),
                '\'' => key(4, 7, On),
                '(' => key(5, 0, On),
                ')' => key(5, 1, On),
                '*' => key(5, 2, On),
                '+' => key(5, 3, On),
                '<' => key(5, 4, On),
                '=' => key(5, 5, On),
                '>' => key(5, 6, On),
                '?' => key(5, 7, On),
                ' ' => key(6, 7, Host),
                _ => None,
            };
        }
        match name {
            "Enter" => key(6, 0, Host),
            "Home" | "Delete" => key(6, 1, Host),  // CLEAR
            "Escape" | "End" => key(6, 2, Host),   // BREAK
            "ArrowUp" => key(6, 3, Host),
            "ArrowDown" => key(6, 4, Host),
            "ArrowLeft" | "Backspace" => key(6, 5, Host),
            "ArrowRight" | "Tab" => key(6, 6, Host),
            _ => None,
        }
    }
}

/// Nombre maximal de touches enfoncées en même temps.
const MAX_PRESSED: usize = 8;

pub(crate) struct Keyboard {
    pressed: [Option<Key>; MAX_PRESSED],
    host_shift: bool,
}

impl Keyboard {
    pub(crate) fn new() -> Self {
        Keyboard { pressed: [None; MAX_PRESSED], host_shift: false }
    }

    pub(crate) fn press(&mut self, key: Key) {
        if self.pressed.contains(&Some(key)) {
            return;
        }
        // Retire la touche la plus ancienne si toutes les places sont prises.
        if let Some(slot) = self.pressed.iter_mut().find(|k| k.is_none()) {
            *slot = Some(key);
        } else {
            self.pressed.rotate_left(1);
            self.pressed[MAX_PRESSED - 1] = Some(key);
        }
    }

    pub(crate) fn release(&mut self, key: Key) {
        for slot in &mut self.pressed {
            if *slot == Some(key) {
                *slot = None;
            }
        }
    }

    pub(crate) fn release_all(&mut self) {
        self.pressed = [None; MAX_PRESSED];
        self.host_shift = false;
    }

    pub(crate) fn set_shift(&mut self, down: bool) {
        self.host_shift = down;
    }

    /// État de MAJ vu par le TRS-80 : la dernière touche qui l'impose l'emporte,
    /// sinon c'est la touche Majuscule du clavier hôte.
    fn shift(&self) -> bool {
        self.pressed
            .iter()
            .flatten()
            .rev()
            .find_map(|k| match k.shift {
                Shift::On => Some(true),
                Shift::Off => Some(false),
                Shift::Host => None,
            })
            .unwrap_or(self.host_shift)
    }

    fn row(&self, row: u8) -> u8 {
        let mut v = self
            .pressed
            .iter()
            .flatten()
            .filter(|k| k.row == row)
            .fold(0, |acc, k| acc | (1 << k.bit));
        if row == 7 && self.shift() {
            v |= 0x01;
        }
        v
    }

    /// Lecture à l'adresse 3800h + `select` : OU des rangées sélectionnées.
    pub(crate) fn read(&self, select: u8) -> u8 {
        (0..8).filter(|r| select & (1 << r) != 0).fold(0, |acc, r| acc | self.row(r))
    }
}

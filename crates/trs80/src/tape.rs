//! Magnétophone : lecture d'une image `.CAS` à 500 bauds par l'entrée cassette, comme xtrs
//! (MIT, Tim Mann, trs_cassette.c).
//!
//! Chaque bit commence par une impulsion d'horloge; un 1 ajoute une impulsion de données au
//! milieu de la cellule (2 ms). Une impulsion met à 1 une bascule, lue au bit 7 du port FFh;
//! une écriture sur FFh la remet à 0. La bande ne défile que moteur en marche (Model I :
//! bit 2 du port FFh; Model III et 4 : bit 1 du port ECh).
//!
//! Le Model III signale aussi chaque front par une interruption (port E0h, bit 0 : montant,
//! bit 1 : descendant), et le bit 0 du port FFh donne le niveau du signal.
//!
//! « Lancer » une cassette en charge le programme directement en mémoire; le magnétophone
//! reprend alors juste après, pour les programmes qui lisent la suite eux-mêmes (chargeurs à
//! plusieurs étapes, comme FROGGER).

use alloc::vec::Vec;

/// Forme des impulsions à 500 bauds (durée en µs, niveau suivant : 0 repos, 1 haut, 2 bas),
/// pour un bit à 0 puis à 1. `None` termine la cellule.
const ZERO: [(u32, u8); 4] = [(0, 1), (128, 2), (128, 0), (1871, 0)];
const ONE: [(u32, u8); 7] = [(0, 1), (128, 2), (128, 0), (748, 1), (128, 2), (128, 0), (860, 0)];
/// Pause ajoutée après chaque bit de l'octet de synchronisation A5h (le BASIC y exécute CLEAR).
const SYNC_PAUSE: u32 = 1034;

#[derive(Default)]
pub(crate) struct Tape {
    data: Vec<u8>,
    /// Prochain octet à lire.
    pos: usize,
    /// Octet en cours (100h : l'octet nul ajouté à la fin) et son bit (7 à 0).
    byte: u16,
    bit: i8,
    pulse: usize,
    pub(crate) motor: bool,
    value: u8,
    next: u8,
    /// Dernière transition (T-states) et durée jusqu'à la suivante (`None` : fin de bande).
    transition: u64,
    delta: Option<u64>,
    flipflop: u8,
    /// Fronts vus depuis la dernière lecture (bit 0 : montant, bit 1 : descendant).
    edges: u8,
    hz: u64,
}

impl Tape {
    /// Met une cassette, prête à lire à partir de l'octet `pos`.
    pub(crate) fn insert(&mut self, data: Vec<u8>, pos: usize) {
        *self = Tape { pos: pos.min(data.len()), data, hz: self.hz, ..Tape::default() };
    }

    pub(crate) fn eject(&mut self) {
        *self = Tape { hz: self.hz, ..Tape::default() };
    }

    pub(crate) fn loaded(&self) -> bool {
        !self.data.is_empty()
    }

    /// Octets lus et taille de la cassette.
    pub(crate) fn progress(&self) -> (usize, usize) {
        (self.pos, self.data.len())
    }

    /// Moteur en marche ou arrêté (`hz` : fréquence du processeur).
    pub(crate) fn set_motor(&mut self, on: bool, now: u64, hz: u32) {
        self.hz = hz as u64;
        if on && !self.motor {
            self.transition = now;
            self.value = 0;
            self.next = 0;
            self.delta = Some(0);
            self.flipflop = 0;
            self.byte = 0;
            self.bit = 0;
            self.pulse = 0;
        }
        self.motor = on;
    }

    /// Prochaine transition du signal (forme des impulsions du bit en cours).
    fn transition_in(&mut self) {
        if self.pulse == 0 {
            self.bit -= 1;
        }
        if self.bit < 0 {
            if self.pos < self.data.len() {
                self.byte = self.data[self.pos] as u16;
                self.pos += 1;
            } else if self.byte != 0x100 {
                self.byte = 0x100;
            } else {
                self.delta = None;
                return;
            }
            self.bit = 7;
        }
        let one = (self.byte >> self.bit) & 1 != 0;
        let shape: &[(u32, u8)] = if one { &ONE } else { &ZERO };
        let (mut us, next) = shape[self.pulse];
        self.next = next;
        self.pulse += 1;
        if self.pulse == shape.len() {
            self.pulse = 0;
            if self.byte == 0xA5 {
                us += SYNC_PAUSE;
            }
        }
        self.delta = Some(us as u64 * self.hz / 1_000_000);
    }

    /// Fait défiler la bande jusqu'à `now`.
    pub(crate) fn update(&mut self, now: u64) {
        if !self.motor || self.data.is_empty() {
            return;
        }
        while let Some(delta) = self.delta {
            if now.saturating_sub(self.transition) < delta {
                break;
            }
            // Impulsion (passage du repos au signal) : la bascule se met à 1.
            if self.next != 0 && self.value == 0 {
                self.flipflop = 0x80;
            }
            if self.next == 1 && self.value != 1 {
                self.edges |= 0x01;
            } else if self.value == 1 && self.next != 1 {
                self.edges |= 0x02;
            }
            self.value = self.next;
            self.transition += delta;
            self.transition_in();
        }
    }

    /// Bit 7 du port FFh : la bascule d'entrée.
    pub(crate) fn read(&mut self, now: u64) -> u8 {
        self.update(now);
        self.flipflop
    }

    /// Fronts vus depuis le dernier appel (bit 0 : montant, bit 1 : descendant).
    pub(crate) fn take_edges(&mut self) -> u8 {
        core::mem::take(&mut self.edges)
    }

    /// Model III, bit 0 du port FFh : le signal est haut.
    pub(crate) fn level(&self) -> u8 {
        (self.motor && self.value == 1) as u8
    }

    /// Écriture sur le port FFh : la bascule revient à 0.
    pub(crate) fn clear(&mut self, now: u64) {
        if self.motor {
            self.update(now);
            self.flipflop = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lit un octet comme la ROM Level II : pour chaque bit, attend l'impulsion d'horloge,
    /// remet la bascule à 0, attend environ 1 ms et regarde s'il y a eu une impulsion de
    /// données.
    fn read_byte(t: &mut Tape, now: &mut u64, hz: u64) -> u8 {
        let mut v = 0u8;
        for _ in 0..8 {
            while t.read(*now) == 0 {
                *now += 20;
            }
            t.clear(*now);
            *now += hz * 13 / 10_000; // 1,3 ms
            let bit = t.read(*now) != 0;
            t.clear(*now);
            *now += hz * 7 / 10_000;
            v = v << 1 | bit as u8;
        }
        v
    }

    #[test]
    fn bytes_come_back_from_the_pulses() {
        let hz = 1_774_080;
        let mut t = Tape::default();
        t.insert(alloc::vec![0x00, 0xA5, 0x3C, 0x5A], 0);
        let mut now = 1000;
        t.set_motor(true, now, hz as u32);
        assert_eq!(read_byte(&mut t, &mut now, hz), 0x00);
        assert_eq!(read_byte(&mut t, &mut now, hz), 0xA5);
        assert_eq!(read_byte(&mut t, &mut now, hz), 0x3C);
        assert_eq!(read_byte(&mut t, &mut now, hz), 0x5A);
        assert_eq!(t.progress(), (4, 4));
        // Moteur arrêté : plus d'impulsions.
        t.set_motor(false, now, hz as u32);
        let f = t.read(now + hz);
        assert_eq!(f, 0);
    }
}

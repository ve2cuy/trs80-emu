//! Frappe automatique : tape un texte (ex. : un programme BASIC collé) touche par touche,
//! au rythme où la ROM lit le clavier. Chaque caractère est enfoncé quelques images puis
//! relâché; après ENTRÉE, une pause plus longue laisse le BASIC traiter la ligne, et la
//! frappe attend que les disques se taisent : une commande du DOS qui charge un programme
//! (SET, LCOMM...) ne lit pas le clavier pendant ce temps, et les touches seraient perdues.

use crate::keyboard::{Key, Keyboard};

/// Images pendant lesquelles une touche reste enfoncée (au moins deux lectures du clavier
/// par les DOS, qui le lisent à chaque interruption de 25 ms).
const HOLD: u8 = 3;
/// Images de pause après une touche.
const GAP: u8 = 2;
/// Images de pause après ENTRÉE (le BASIC analyse et range la ligne).
const ENTER_GAP: u8 = 10;
/// Après ENTRÉE : images sans aucune activité des disques avant de reprendre la frappe, et
/// attente maximale (un programme qui accède sans cesse au disque ne bloque pas la frappe).
const DISK_QUIET: u8 = 20;
const DISK_WAIT_MAX: u16 = 600;
/// Capacité de la file : un long programme BASIC.
const CAPACITY: usize = 16 * 1024;

#[derive(Clone, Copy)]
enum State {
    Idle,
    Down { key: Key, frames: u8, enter: bool },
    Gap { frames: u8 },
    /// Après ENTRÉE : images écoulées, images calmes depuis la dernière activité des
    /// disques, et activité vue depuis ENTRÉE.
    Settle { waited: u16, quiet: u8, seen: bool },
}

pub(crate) struct Typer {
    /// Images pendant lesquelles une touche reste enfoncée, puis pause (voir HOLD et GAP).
    hold: u8,
    gap: u8,
    queue: [u8; CAPACITY],
    head: usize,
    len: usize,
    state: State,
}

impl Typer {
    pub(crate) fn new() -> Self {
        Typer { hold: HOLD, gap: GAP, queue: [0; CAPACITY], head: 0, len: 0, state: State::Idle }
    }

    /// Ajoute du texte à taper; retourne le nombre de caractères acceptés. Les fins de ligne
    /// deviennent ENTRÉE, `\x08` la touche ←; les caractères sans touche sur le TRS-80 sont
    /// ignorés.
    pub(crate) fn push(&mut self, text: &str) -> usize {
        let mut accepted = 0;
        for c in text.chars() {
            let byte = match c {
                '\n' => b'\n',
                BACKSPACE => BACKSPACE as u8,
                '\t' => b' ',
                c if c.is_ascii() && key_for(c as u8).is_some() => c as u8,
                _ => continue,
            };
            if self.len == CAPACITY {
                break;
            }
            self.queue[(self.head + self.len) % CAPACITY] = byte;
            self.len += 1;
            accepted += 1;
        }
        accepted
    }

    /// Durées plus longues pour les systèmes qui lisent le clavier moins souvent (le Model III
    /// le lit à chaque interruption d'horloge, 30 fois par seconde).
    pub(crate) fn set_timing(&mut self, hold: u8, gap: u8) {
        self.hold = hold;
        self.gap = gap;
    }

    pub(crate) fn busy(&self) -> bool {
        self.len > 0 || !matches!(self.state, State::Idle)
    }

    pub(crate) fn cancel(&mut self, keyboard: &mut Keyboard) {
        if let State::Down { key, .. } = self.state {
            keyboard.release(key);
        }
        self.len = 0;
        self.state = State::Idle;
    }

    fn pop(&mut self) -> Option<u8> {
        (self.len > 0).then(|| {
            let b = self.queue[self.head];
            self.head = (self.head + 1) % CAPACITY;
            self.len -= 1;
            b
        })
    }

    /// Avance d'une image. `disk_busy` : les disques ont été actifs pendant l'image précédente.
    pub(crate) fn tick(&mut self, keyboard: &mut Keyboard, disk_busy: bool) {
        self.state = match self.state {
            State::Idle => match self.pop() {
                Some(b) => {
                    let key = key_for(b).expect("caractère validé par push");
                    keyboard.press(key);
                    State::Down { key, frames: self.hold, enter: b == b'\n' }
                }
                None => State::Idle,
            },
            State::Down { key, frames, enter } if frames > 1 => {
                State::Down { key, frames: frames - 1, enter }
            }
            State::Down { key, enter, .. } => {
                keyboard.release(key);
                if enter { State::Settle { waited: 0, quiet: 0, seen: false } } else { State::Gap { frames: self.gap } }
            }
            State::Gap { frames } if frames > 1 => State::Gap { frames: frames - 1 },
            State::Gap { .. } => State::Idle,
            State::Settle { waited, quiet, seen } => {
                let waited = waited + 1;
                let seen = seen || disk_busy;
                let quiet = if disk_busy { 0 } else { quiet.saturating_add(1) };
                // Sans disque : la pause habituelle. Avec : jusqu'au silence des disques.
                let done = waited >= ENTER_GAP as u16 && (!seen || quiet >= DISK_QUIET);
                if done || waited >= DISK_WAIT_MAX { State::Idle } else { State::Settle { waited, quiet, seen } }
            }
        };
    }
}

/// Caractère « retour arrière » : la touche ← du TRS-80 (clavier virtuel d'une tablette).
const BACKSPACE: char = '\x08';

fn key_for(b: u8) -> Option<Key> {
    if b == b'\n' {
        return Key::from_name("Enter");
    }
    if b == BACKSPACE as u8 {
        return Key::from_name("Backspace");
    }
    let buf = [b];
    Key::from_name(core::str::from_utf8(&buf).ok()?)
}

//! Frappe automatique : tape un texte (ex. : un programme BASIC collé) touche par touche,
//! au rythme où la ROM lit le clavier. Chaque caractère est enfoncé quelques images puis
//! relâché; après ENTRÉE, une pause plus longue laisse le BASIC traiter la ligne.

use crate::keyboard::{Key, Keyboard};

/// Images pendant lesquelles une touche reste enfoncée.
const HOLD: u8 = 2;
/// Images de pause après une touche.
const GAP: u8 = 2;
/// Images de pause après ENTRÉE (le BASIC analyse et range la ligne).
const ENTER_GAP: u8 = 10;
/// Capacité de la file : un long programme BASIC.
const CAPACITY: usize = 16 * 1024;

#[derive(Clone, Copy)]
enum State {
    Idle,
    Down { key: Key, frames: u8, enter: bool },
    Gap { frames: u8 },
}

pub(crate) struct Typer {
    queue: [u8; CAPACITY],
    head: usize,
    len: usize,
    state: State,
}

impl Typer {
    pub(crate) fn new() -> Self {
        Typer { queue: [0; CAPACITY], head: 0, len: 0, state: State::Idle }
    }

    /// Ajoute du texte à taper; retourne le nombre de caractères acceptés. Les fins de ligne
    /// deviennent ENTRÉE; les caractères sans touche sur le TRS-80 sont ignorés.
    pub(crate) fn push(&mut self, text: &str) -> usize {
        let mut accepted = 0;
        for c in text.chars() {
            let byte = match c {
                '\n' => b'\n',
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

    /// Avance d'une image.
    pub(crate) fn tick(&mut self, keyboard: &mut Keyboard) {
        self.state = match self.state {
            State::Idle => match self.pop() {
                Some(b) => {
                    let key = key_for(b).expect("caractère validé par push");
                    keyboard.press(key);
                    State::Down { key, frames: HOLD, enter: b == b'\n' }
                }
                None => State::Idle,
            },
            State::Down { key, frames, enter } if frames > 1 => {
                State::Down { key, frames: frames - 1, enter }
            }
            State::Down { key, enter, .. } => {
                keyboard.release(key);
                State::Gap { frames: if enter { ENTER_GAP } else { GAP } }
            }
            State::Gap { frames } if frames > 1 => State::Gap { frames: frames - 1 },
            State::Gap { .. } => State::Idle,
        };
    }
}

fn key_for(b: u8) -> Option<Key> {
    if b == b'\n' {
        return Key::from_name("Enter");
    }
    let buf = [b];
    Key::from_name(core::str::from_utf8(&buf).ok()?)
}

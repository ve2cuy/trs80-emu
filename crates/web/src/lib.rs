//! Liaison WebAssembly : expose la machine TRS-80 à JavaScript.
//!
//! JavaScript s'occupe de la page (canvas, clavier, fichiers); Rust fait tout le reste.
//! L'image de l'écran est lue directement dans la mémoire Wasm, sans copie
//! (voir `framebuffer_ptr`).

use trs80::{Key, Loaded, SCREEN_HEIGHT, SCREEN_WIDTH, Trs80};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Emulator {
    machine: Box<Trs80>,
    framebuffer: Vec<u8>,
}

#[wasm_bindgen]
impl Emulator {
    /// Crée l'émulateur à partir du contenu du fichier ROM Level II.
    #[wasm_bindgen(constructor)]
    pub fn new(rom: &[u8]) -> Result<Emulator, JsError> {
        let machine = Trs80::new(rom).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Emulator {
            machine: Box::new(machine),
            framebuffer: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
        })
    }

    /// Exécute `count` images de 1/60 s (plus d'une en mode turbo).
    pub fn run_frames(&mut self, count: u32) {
        for _ in 0..count {
            self.machine.run_frame();
        }
    }

    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// Charge et lance un programme `.CMD`; retourne son adresse de lancement.
    pub fn load_cmd(&mut self, data: &[u8]) -> Result<u16, JsError> {
        self.machine.load_cmd(data).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Charge une cassette `.CAS`. Un programme BASIC est aussitôt lancé (RUN tapé au
    /// clavier). Retourne une description pour la barre d'état (en anglais).
    pub fn load_cas(&mut self, data: &[u8]) -> Result<String, JsError> {
        match self.machine.load_cas(data).map_err(|e| JsError::new(&e.to_string()))? {
            Loaded::System(entry) => Ok(format!("machine-language tape, started at {entry:04X}h")),
            Loaded::Basic(size) => {
                self.machine.type_text("RUN\n");
                Ok(format!("BASIC tape ({size} bytes), running"))
            }
        }
    }

    /// Tape un texte au clavier (ex. : programme BASIC collé); retourne le nombre de
    /// caractères acceptés (ceux qui n'existent pas sur le TRS-80 sont ignorés).
    pub fn type_text(&mut self, text: &str) -> u32 {
        self.machine.type_text(text) as u32
    }

    /// Une frappe automatique est-elle en cours ?
    pub fn typing(&self) -> bool {
        self.machine.typing()
    }

    pub fn cancel_typing(&mut self) {
        self.machine.cancel_typing();
    }

    /// Interface d'expansion (horloge à 40 Hz).
    pub fn set_expansion_interface(&mut self, present: bool) {
        self.machine.set_expansion_interface(present);
    }

    /// Touche enfoncée, selon `KeyboardEvent.key`. Retourne `true` si la touche
    /// existe sur le TRS-80 (la page annule alors l'action par défaut du fureteur).
    pub fn key_down(&mut self, name: &str) -> bool {
        if name == "Shift" {
            self.machine.set_shift(true);
            return true;
        }
        match Key::from_name(name) {
            Some(key) => {
                self.machine.key_down(key);
                true
            }
            None => false,
        }
    }

    pub fn key_up(&mut self, name: &str) {
        if name == "Shift" {
            self.machine.set_shift(false);
        } else if let Some(key) = Key::from_name(name) {
            self.machine.key_up(key);
        }
    }

    pub fn release_all_keys(&mut self) {
        self.machine.release_all_keys();
    }

    /// Dessine l'écran et retourne l'adresse de l'image RGBA dans la mémoire Wasm.
    pub fn render(&mut self) -> *const u8 {
        self.machine.render(&mut self.framebuffer);
        self.framebuffer.as_ptr()
    }

    /// Comme `render`, sans le texte (la page le dessine avec une autre police).
    pub fn render_graphics(&mut self) -> *const u8 {
        self.machine.render_graphics(&mut self.framebuffer);
        self.framebuffer.as_ptr()
    }

    /// Adresse de la mémoire vidéo (1024 octets : 16 lignes de 64) dans la mémoire Wasm.
    pub fn video_ptr(&self) -> *const u8 {
        self.machine.video().as_ptr()
    }

    /// Mode 32 caractères par ligne actif.
    pub fn wide(&self) -> bool {
        self.machine.wide()
    }

    pub fn width() -> u32 {
        SCREEN_WIDTH as u32
    }

    pub fn height() -> u32 {
        SCREEN_HEIGHT as u32
    }

    /// Fréquence du Z80 (pour l'affichage d'informations).
    pub fn clock_hz() -> u32 {
        trs80::CLOCK_HZ
    }
}

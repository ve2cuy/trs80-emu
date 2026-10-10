//! Liaison WebAssembly : expose la machine TRS-80 à JavaScript.
//!
//! JavaScript s'occupe de la page (canvas, clavier, fichiers); Rust fait tout le reste.
//! L'image de l'écran est lue directement dans la mémoire Wasm, sans copie
//! (voir `framebuffer_ptr`).

use trs80::{Key, Loaded, MAX_HEIGHT, MAX_WIDTH, Model, Trs80};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Emulator {
    machine: Box<Trs80>,
    framebuffer: Vec<u8>,
    /// Caractères affichés (copie, lue par la page : voir `video_ptr`).
    display: Vec<u8>,
}

#[wasm_bindgen]
impl Emulator {
    /// Crée l'émulateur à partir d'une ROM : 12 Ko (Model I), 14 Ko (Model III) ou 2 Ko (Model II).
    #[wasm_bindgen(constructor)]
    pub fn new(rom: &[u8]) -> Result<Emulator, JsError> {
        let machine = Trs80::new(rom).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Self::from_machine(machine))
    }

    /// Crée un modèle donné : 1, 2, 3 ou 4 (le Model 4 utilise la ROM du Model III).
    pub fn with_model(rom: &[u8], model: u8) -> Result<Emulator, JsError> {
        let model = match model {
            2 => Model::II,
            3 => Model::III,
            4 => Model::IV,
            _ => Model::I,
        };
        let machine = Trs80::with_model(rom, model).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Self::from_machine(machine))
    }

    fn from_machine(machine: Trs80) -> Emulator {
        Emulator { machine: Box::new(machine), framebuffer: vec![0; MAX_WIDTH * MAX_HEIGHT * 4], display: Vec::new() }
    }

    /// Modèle émulé : 1, 2, 3 ou 4.
    pub fn model(&self) -> u8 {
        match self.machine.model() {
            Model::I => 1,
            Model::II => 2,
            Model::III => 3,
            Model::IV => 4,
        }
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

    /// Insère une image de disquette (JV1, JV3 ou DMK) dans le lecteur `drive` (0 à 3).
    /// Retourne une description (format, nombre de secteurs) pour la page.
    pub fn insert_disk(&mut self, drive: u32, image: Vec<u8>) -> Result<String, JsError> {
        let disk = self
            .machine
            .insert_disk(drive as usize, image)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(format!(
            "{}, {} sectors{}",
            disk.format().name(),
            disk.sector_count(),
            if disk.write_protected() { ", write-protected" } else { "" }
        ))
    }

    pub fn eject_disk(&mut self, drive: u32) {
        self.machine.eject_disk(drive as usize);
    }

    /// La disquette du lecteur a-t-elle été modifiée par le TRS-80 ?
    /// Pas de la tête des lecteurs depuis le démarrage (compteur cumulatif).
    pub fn disk_steps(&self) -> u32 {
        self.machine.disk_activity().0
    }

    /// Accès aux lecteurs (commandes, sélections) depuis le démarrage : le moteur tourne.
    pub fn disk_accesses(&self) -> u32 {
        self.machine.disk_activity().1
    }

    pub fn disk_modified(&self, drive: u32) -> bool {
        self.machine.disk(drive as usize).is_some_and(|d| d.modified())
    }

    /// Image (modifiée) à enregistrer : le fichier d'origine (JV1, JV3) ou, après un
    /// formatage ou pour une image DMK, une image JV3.
    pub fn disk_image(&self, drive: u32) -> Option<Vec<u8>> {
        Some(self.machine.disk(drive as usize)?.image())
    }

    /// Format de l'image enregistrée par `disk_image` (« JV1 » ou « JV3 »).
    pub fn disk_image_format(&self, drive: u32) -> Option<String> {
        Some(self.machine.disk(drive as usize)?.image_format().name().to_string())
    }

    /// Disquette vierge dans le lecteur (à formater par le DOS, ex. FORMAT :1).
    pub fn insert_blank_disk(&mut self, drive: u32) {
        self.machine.insert_blank_disk(drive as usize);
    }

    /// Branche une image de disque dur (Reed / HDV) sur l'unité `unit` (0 à 3) du contrôleur
    /// Radio Shack. Retourne sa description (taille, têtes).
    pub fn insert_hard_disk(&mut self, unit: u32, image: Vec<u8>) -> Result<String, JsError> {
        let disk = self
            .machine
            .insert_hard_disk(unit as usize, image)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(format!(
            "{:.1} MB, {} cylinders, {} heads{}",
            disk.megabytes(),
            disk.cylinders(),
            disk.heads(),
            if disk.write_protected() { ", write-protected" } else { "" }
        ))
    }

    /// Image vierge de disque dur : `cylinders` × `heads` (les valeurs proposées par le
    /// pilote RSHARD : 306 × 4, 10 Mo). À formater par RSFORM.
    pub fn blank_hard_disk(cylinders: u32, heads: u32) -> Vec<u8> {
        trs80::HardDisk::blank(cylinders as usize, heads as usize)
    }

    pub fn eject_hard_disk(&mut self, unit: u32) {
        self.machine.eject_hard_disk(unit as usize);
    }

    /// RS-232 : octets venus d'Internet (par le modem de la page), reçus par le TRS-80.
    pub fn serial_send(&mut self, bytes: &[u8]) {
        self.machine.serial_send(bytes);
    }

    /// RS-232 : octets émis par le TRS-80 depuis le dernier appel.
    pub fn serial_take(&mut self) -> Vec<u8> {
        self.machine.serial_take()
    }

    /// RS-232 : octets reçus pas encore lus par le TRS-80.
    pub fn serial_pending(&self) -> u32 {
        self.machine.serial_pending() as u32
    }

    /// RS-232 : vitesse choisie par le programme (bauds).
    pub fn serial_baud(&self) -> u32 {
        self.machine.serial_baud()
    }

    /// Écritures sur la disquette du lecteur depuis son insertion (pour l'enregistrer).
    pub fn disk_writes(&self, drive: u32) -> u32 {
        self.machine.disk(drive as usize).map_or(0, |d| d.writes())
    }

    /// Écritures sur le disque dur depuis son insertion (pour l'enregistrer).
    pub fn hard_disk_writes(&self, unit: u32) -> u32 {
        self.machine.hard_disk(unit as usize).map_or(0, |d| d.writes())
    }

    /// Le DOS a-t-il écrit sur le disque dur ?
    pub fn hard_disk_modified(&self, unit: u32) -> bool {
        self.machine.hard_disk(unit as usize).is_some_and(|d| d.modified())
    }

    /// Image du disque dur, avec les écritures du DOS.
    pub fn hard_disk_image(&self, unit: u32) -> Option<Vec<u8>> {
        Some(self.machine.hard_disk(unit as usize)?.image().to_vec())
    }

    /// Géométrie de la disquette du lecteur, en JSON (vide sans disquette) : pistes, faces,
    /// puis [secteurs, taille, double densité, premier secteur] de la piste 0 et des autres pistes.
    pub fn disk_geometry(&self, drive: u32) -> String {
        let Some(g) = self.machine.disk_geometry(drive as usize) else { return String::new() };
        let shape = |t: trs80::TrackShape| format!("[{},{},{},{}]", t.sectors, t.size, t.dd, t.first);
        format!(r#"{{"tracks":{},"sides":{},"t0":{},"t":{}}}"#, g.tracks, g.sides, shape(g.track0), shape(g.track))
    }

    /// Géométrie du disque dur : [cylindres, têtes] (vide sans disque).
    pub fn hard_geometry(&self, unit: u32) -> Vec<u32> {
        self.machine.hard_disk(unit as usize).map_or_else(Vec::new, |d| vec![d.cylinders() as u32, d.heads() as u32])
    }

    /// Dernier accès aux disquettes : [n° de commande, lecteur, piste (tête), secteur,
    /// commande] (vide avant le premier). Un nouveau n° signale un nouvel accès.
    pub fn disk_position(&self) -> Vec<u32> {
        self.machine.disk_position().map_or_else(Vec::new, |(n, e)| {
            vec![n, e.drive as u32, e.head as u32, e.sector as u32, e.command as u32]
        })
    }

    /// Dernier accès au disque dur : [n° de commande, unité, cylindre, tête, secteur, commande].
    pub fn hard_position(&self) -> Vec<u32> {
        self.machine.hard_position().map_or_else(Vec::new, |(n, e)| {
            vec![n, e.unit as u32, e.cylinder as u32, e.head as u32, e.sector as u32, e.command as u32]
        })
    }

    /// Active le son à la fréquence `rate` (celle de l'AudioContext); 0 le désactive.
    pub fn set_audio_rate(&mut self, rate: u32) {
        self.machine.set_audio_rate(rate);
    }

    /// Adresse et nombre des échantillons audio produits depuis `clear_audio`
    /// (f32 dans la mémoire Wasm, lus sans copie par la page).
    pub fn audio_ptr(&self) -> *const f32 {
        self.machine.audio_samples().as_ptr()
    }

    pub fn audio_len(&self) -> u32 {
        self.machine.audio_samples().len() as u32
    }

    pub fn clear_audio(&mut self) {
        self.machine.clear_audio();
    }

    /// Model II : touche CAPS enfoncée (lettres en majuscules).
    pub fn caps(&self) -> bool {
        self.machine.caps()
    }

    /// Interface d'expansion (horloge à 40 Hz).
    pub fn set_expansion_interface(&mut self, present: bool) {
        self.machine.set_expansion_interface(present);
    }

    /// Touche enfoncée, selon `KeyboardEvent.key`. Retourne `true` si la touche
    /// existe sur le TRS-80 (la page annule alors l'action par défaut du fureteur).
    pub fn key_down(&mut self, name: &str) -> bool {
        if self.machine.model() == Model::II {
            // Clavier ASCII : la touche donne directement son code (MAJ déjà appliquée).
            // Verr. Maj bascule la touche CAPS (majuscules), enfoncée au départ.
            if name == "CapsLock" {
                self.machine.toggle_caps();
                return true;
            }
            return match model2_code(name) {
                Some(code) => {
                    self.machine.key_code(code);
                    true
                }
                None => name == "Shift",
            };
        }
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
        if self.machine.model() == Model::II {
            return;
        }
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

    /// Copie les caractères affichés (`text_cols × text_rows` octets) et retourne leur
    /// adresse dans la mémoire Wasm.
    pub fn video_ptr(&mut self) -> *const u8 {
        self.display = self.machine.display();
        self.display.as_ptr()
    }

    /// Nombre de caractères par ligne et de lignes affichés (64 × 16, ou 80 × 24).
    /// Texte de l'écran (lignes sans espaces de fin), pour le copier dans le presse-papiers.
    /// Les blocs graphiques deviennent des espaces.
    pub fn screen_text(&self) -> String {
        let mode = self.machine.text_mode();
        let lines: Vec<String> = (0..mode.rows)
            .map(|r| (0..mode.cols).map(|c| self.machine.char_at(r, c)).collect::<String>().trim_end().to_string())
            .collect();
        let used = lines.iter().rposition(|l| !l.is_empty()).map_or(0, |i| i + 1);
        lines[..used].join("\n")
    }

    pub fn text_cols(&self) -> u32 {
        self.machine.text_mode().cols as u32
    }

    pub fn text_rows(&self) -> u32 {
        self.machine.text_mode().rows as u32
    }

    /// Taille de l'image produite par `render` (pixels).
    pub fn screen_width(&self) -> u32 {
        self.machine.screen_size().0 as u32
    }

    pub fn screen_height(&self) -> u32 {
        self.machine.screen_size().1 as u32
    }

    /// Fréquence actuelle du processeur (pour l'affichage de la vitesse).
    pub fn current_hz(&self) -> u32 {
        self.machine.clock_hz()
    }

    /// Mode 32 caractères par ligne actif.
    pub fn wide(&self) -> bool {
        self.machine.wide()
    }

    /// Fréquence du Z80 (pour l'affichage d'informations).
    pub fn clock_hz() -> u32 {
        trs80::CLOCK_HZ
    }

    // ------------------------------------------------------------ débogueur

    /// Exécute une seule instruction (sans tenir compte des points d'arrêt).
    pub fn step_instruction(&mut self) {
        self.machine.step_instruction();
    }

    pub fn set_breakpoints(&mut self, addresses: &[u16]) {
        self.machine.set_breakpoints(addresses);
    }

    /// Arrêt dès que PC revient dans `lo..=hi` (pas à pas); `lo > hi` : plus de surveillance.
    pub fn set_stop_range(&mut self, lo: u16, hi: u16) {
        self.machine.set_stop_range((lo <= hi).then_some((lo, hi)));
    }

    pub fn set_exit_points(&mut self, addresses: &[u16]) {
        self.machine.set_exit_points(addresses);
    }

    /// Arrêt survenu : 0 aucun, 1 point d'arrêt, 2 retour dans la plage, 3 fin du programme.
    pub fn take_stop(&mut self) -> u8 {
        match self.machine.take_stop() {
            None => 0,
            Some(trs80::Stop::Breakpoint) => 1,
            Some(trs80::Stop::Range) => 2,
            Some(trs80::Stop::Exit) => 3,
        }
    }

    pub fn stopped(&self) -> bool {
        self.machine.stopped()
    }

    /// Reprend après un arrêt (l'instruction courante s'exécute même sur un point d'arrêt).
    pub fn resume(&mut self) {
        self.machine.resume();
    }

    /// Registres : AF BC DE HL IX IY SP PC AF' BC' DE' HL' I R IFF1 IM, arrêté (HALT).
    pub fn registers(&self) -> Vec<u16> {
        let c = self.machine.cpu();
        let pair = |h: u8, l: u8| u16::from_be_bytes([h, l]);
        vec![
            pair(c.a, c.f), pair(c.b, c.c), pair(c.d, c.e), pair(c.h, c.l), c.ix, c.iy, c.sp, c.pc,
            pair(c.a_, c.f_), pair(c.b_, c.c_), pair(c.d_, c.e_), pair(c.h_, c.l_),
            c.i as u16, c.r as u16, c.iff1 as u16, c.im as u16, c.halted as u16,
        ]
    }

    /// `len` octets de la mémoire à partir de `addr`.
    pub fn peek_range(&mut self, addr: u16, len: u16) -> Vec<u8> {
        (0..len).map(|i| self.machine.peek(addr.wrapping_add(i))).collect()
    }

    pub fn poke(&mut self, addr: u16, bytes: &[u8]) {
        self.machine.poke(addr, bytes);
    }

    /// Lance un programme déjà en mémoire (retour à LDOS ou au BASIC); retourne l'adresse
    /// de retour (402DH ou 1A19H).
    pub fn launch(&mut self, entry: u16) -> Result<u16, JsError> {
        self.machine.launch(entry).map_err(|e| JsError::new(&e.to_string()))
    }

    // ------------------------------------------------------------ fichiers LDOS

    /// Fichiers de la disquette : JSON [{"name","size","system"}]. Erreur : son code.
    pub fn disk_files(&self, drive: u32) -> Result<String, JsError> {
        let files = self.machine.disk_files(drive as usize).map_err(|e| JsError::new(e.code()))?;
        let items: Vec<String> = files
            .iter()
            .map(|f| format!("{{\"name\":{},\"size\":{},\"system\":{}}}", json_string(&f.name), f.size, f.system))
            .collect();
        Ok(format!("[{}]", items.join(",")))
    }

    pub fn read_disk_file(&self, drive: u32, name: &str) -> Result<Vec<u8>, JsError> {
        self.machine.read_disk_file(drive as usize, name).map_err(|e| JsError::new(e.code()))
    }

    pub fn write_disk_file(&mut self, drive: u32, name: &str, data: &[u8]) -> Result<(), JsError> {
        self.machine.write_disk_file(drive as usize, name, data).map_err(|e| JsError::new(e.code()))
    }

    /// Espace libre (octets) de la disquette.
    pub fn disk_free(&self, drive: u32) -> Result<u32, JsError> {
        self.machine.disk_free(drive as usize).map(|n| n as u32).map_err(|e| JsError::new(e.code()))
    }
}

// ---------------------------------------------------------------- assembleur

/// Chaîne JSON (guillemets et caractères spéciaux échappés).
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Résultat de [`assemble`].
#[wasm_bindgen]
pub struct Assembled {
    asm: z80asm::Assembly,
}

#[wasm_bindgen]
impl Assembled {
    pub fn ok(&self) -> bool {
        self.asm.ok()
    }

    pub fn entry(&self) -> u16 {
        self.asm.entry
    }

    pub fn size(&self) -> u32 {
        self.asm.size() as u32
    }

    /// Le programme appelle-t-il des services de LDOS ?
    pub fn uses_ldos(&self) -> bool {
        self.asm.uses_ldos()
    }

    /// Symboles prédéfinis utilisés : JSON ["@DSPLY", ...].
    pub fn builtins_used_json(&self) -> String {
        let items: Vec<String> = self.asm.builtins_used.iter().map(|n| json_string(n)).collect();
        format!("[{}]", items.join(","))
    }

    /// Fichier .CMD.
    pub fn cmd(&self) -> Vec<u8> {
        self.asm.cmd()
    }

    /// Première et dernière adresse ([] si le programme est vide).
    pub fn bounds(&self) -> Vec<u16> {
        self.asm.bounds().map(|(lo, hi)| vec![lo, hi]).unwrap_or_default()
    }

    /// Blocs : JSON [[adresse, [octets...]], ...].
    pub fn blocks_json(&self) -> String {
        let items: Vec<String> = self
            .asm
            .blocks
            .iter()
            .map(|(a, b)| format!("[{a},[{}]]", b.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")))
            .collect();
        format!("[{}]", items.join(","))
    }

    /// Pour chaque ligne : adresse et nombre d'octets, à plat ([a0, n0, a1, n1, ...]).
    pub fn lines(&self) -> Vec<u16> {
        self.asm.lines.iter().flat_map(|l| [l.addr, l.len]).collect()
    }

    /// Diagnostics : JSON [{"line","warning","code","args","fix","insert"}].
    pub fn diagnostics_json(&self) -> String {
        let items: Vec<String> = self
            .asm
            .diagnostics
            .iter()
            .map(|d| {
                let args: Vec<String> = d.args.iter().map(|a| json_string(a)).collect();
                format!(
                    "{{\"line\":{},\"warning\":{},\"code\":{},\"args\":[{}],\"fix\":{},\"insert\":{}}}",
                    d.line,
                    d.warning,
                    json_string(d.code),
                    args.join(","),
                    d.fix.as_deref().map(json_string).unwrap_or_else(|| "null".into()),
                    d.insert
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    }

    /// Symboles du programme : JSON [["NOM", valeur], ...].
    pub fn symbols_json(&self) -> String {
        let items: Vec<String> = self.asm.symbols.iter().map(|(n, v)| format!("[{},{v}]", json_string(n))).collect();
        format!("[{}]", items.join(","))
    }
}

/// Assemble un source Z80 (syntaxe des assembleurs TRS-80).
#[wasm_bindgen]
pub fn assemble(source: &str) -> Assembled {
    Assembled { asm: z80asm::assemble(source) }
}

/// Symboles prédéfinis (services de LDOS, routines de la ROM) : JSON [["@DSPLY", 17511, true], ...].
#[wasm_bindgen]
pub fn asm_builtins_json() -> String {
    let items: Vec<String> = z80asm::builtins::BUILTINS
        .iter()
        .map(|b| format!("[{},{},{}]", json_string(b.name), b.value, b.ldos))
        .collect();
    format!("[{}]", items.join(","))
}

/// Model II : code envoyé par le clavier pour `KeyboardEvent.key` (ou « Ctrl+x » pour une
/// touche de contrôle).
fn model2_code(name: &str) -> Option<u8> {
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return c.is_ascii().then_some(c as u8);
    }
    if let Some(letter) = name.strip_prefix("Ctrl+") {
        let &[c] = letter.as_bytes() else { return None };
        return (c.is_ascii_alphabetic() || br"@[\]^_".contains(&c)).then_some(c & 0x1F);
    }
    Some(match name {
        "Enter" => 0x0D,
        "Escape" => 0x1B,
        "Backspace" => 0x08,
        "Tab" => 0x09,
        // BREAK
        "End" | "Pause" => 0x03,
        "ArrowLeft" => 0x1C,
        "ArrowRight" => 0x1D,
        "ArrowUp" => 0x1E,
        "ArrowDown" => 0x1F,
        "F1" => 0x01,
        "F2" => 0x02,
        _ => return None,
    })
}

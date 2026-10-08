//! Le jeu VE2CUY INVADERS (asm/invaders.asm, assemblé dans www/programs/) : choix de la
//! langue, menu, partie. Les captures d'écran sont écrites dans `target/screens/`.
//!
//! Exige la ROM Level II dans `crates/trs80/tests/roms/` (non fournie); sinon le test est ignoré.

use trs80::{Key, SCREEN_HEIGHT, SCREEN_WIDTH, Trs80};

fn load_rom() -> Option<Vec<u8>> {
    let dir = format!("{}/tests/roms", env!("CARGO_MANIFEST_DIR"));
    ["M1L2_1.3.bin", "level2.rom"]
        .iter()
        .find_map(|name| std::fs::read(format!("{dir}/{name}")).ok())
}

fn game() -> Vec<u8> {
    std::fs::read(format!("{}/../../www/programs/ve2cuy-invaders.cmd", env!("CARGO_MANIFEST_DIR")))
        .unwrap()
}

fn screen(m: &Trs80) -> String {
    (0..16)
        .map(|r| (0..64).map(|c| m.char_at(r, c)).collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn save_ppm(m: &Trs80, name: &str) {
    let mut rgba = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
    m.render(&mut rgba);
    let mut ppm = format!("P6\n{SCREEN_WIDTH} {SCREEN_HEIGHT}\n255\n").into_bytes();
    rgba.chunks(4).for_each(|px| ppm.extend_from_slice(&px[..3]));
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screens");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{name}.ppm")), ppm).unwrap();
}

/// Écrit des échantillons en WAV mono 16 bits à 44,1 kHz (pour écouter le résultat).
fn save_wav(samples: &[f32], name: &str) {
    let data: Vec<u8> = samples
        .iter()
        .flat_map(|&x| ((x.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
        .collect();
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&44_100u32.to_le_bytes());
    wav.extend_from_slice(&(44_100u32 * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wav.extend_from_slice(&data);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screens");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{name}.wav")), wav).unwrap();
}

fn frames(m: &mut Trs80, n: usize) {
    (0..n).for_each(|_| m.run_frame());
}

fn tap(m: &mut Trs80, name: &str) {
    let key = Key::from_name(name).unwrap();
    m.key_down(key);
    frames(m, 4);
    m.key_up(key);
    frames(m, 4);
}

/// Score affiché en haut à gauche (colonnes 7 à 11).
fn score(m: &Trs80) -> u32 {
    (7..12).map(|c| m.char_at(0, c)).collect::<String>().parse().unwrap_or(0)
}

fn start(rom: &[u8], language: &str) -> Trs80 {
    let mut m = Trs80::new(rom).unwrap();
    m.load_cmd(&game()).unwrap();
    frames(&mut m, 30);
    let title = screen(&m);
    assert!(title.contains("V E 2 C U Y    I N V A D E R S"), "écran d'accueil :\n{title}");
    assert!(title.contains("PRESS  E  FOR ENGLISH"), "écran d'accueil :\n{title}");
    assert!(title.contains("APPUYEZ SUR  F  POUR LE FRANCAIS"), "écran d'accueil :\n{title}");
    save_ppm(&m, "invaders-title");
    tap(&mut m, language);
    frames(&mut m, 10);
    m
}

#[test]
fn english_game() {
    let Some(rom) = load_rom() else { return };
    let mut m = start(&rom, "e");
    let menu = screen(&m);
    assert!(menu.contains("PRESS SPACE TO START"), "menu :\n{menu}");
    assert!(menu.contains("BY VE2CUY"), "menu :\n{menu}");
    save_ppm(&m, "invaders-menu-en");

    tap(&mut m, " ");
    frames(&mut m, 60);
    let s = screen(&m);
    assert!(s.contains("SCORE") && s.contains("HI-SCORE") && s.contains("LIVES"), "partie :\n{s}");

    // Dix secondes de jeu : on tire sans arrêt en se déplaçant de gauche à droite,
    // en gardant le son produit (tirs, explosions, pas de la formation).
    m.set_audio_rate(44_100);
    let mut sound: Vec<f32> = Vec::new();
    let mut play = |m: &mut Trs80, n: usize| {
        for _ in 0..n {
            m.run_frame();
            sound.extend_from_slice(m.audio_samples());
            m.clear_audio();
        }
    };
    let fire = Key::from_name(" ").unwrap();
    let left = Key::from_name("ArrowLeft").unwrap();
    let right = Key::from_name("ArrowRight").unwrap();
    for step in 0..20 {
        let dir = if step % 4 < 2 { left } else { right };
        m.key_down(dir);
        m.key_down(fire);
        play(&mut m, 15);
        m.key_up(fire);
        m.key_up(dir);
        play(&mut m, 15);
    }
    save_ppm(&m, "invaders-game-en");
    let peak = sound.iter().fold(0f32, |a, &x| a.max(x.abs()));
    assert!(peak > 0.05, "aucun son produit (crête {peak})");
    assert!(sound.len() > 400_000, "environ 44 100 échantillons par seconde : {}", sound.len());
    save_wav(&sound, "invaders-sound");
    // Cadence du jeu : le compteur d'images FRAME (adresse du listage zmac, si présente).
    if let Ok(frame_addr) = std::env::var("INVADERS_FRAME_ADDR") {
        let addr = u16::from_str_radix(&frame_addr, 16).unwrap();
        let before = m.peek(addr);
        frames(&mut m, 60);
        eprintln!("images de jeu par seconde : {}", m.peek(addr).wrapping_sub(before));
    }
    assert!(score(&m) > 0, "aucun envahisseur touché en 10 secondes :\n{}", screen(&m));
}

#[test]
fn french_texts() {
    let Some(rom) = load_rom() else { return };
    let mut m = start(&rom, "f");
    let menu = screen(&m);
    assert!(menu.contains("APPUYEZ SUR ESPACE POUR JOUER"), "menu :\n{menu}");
    assert!(menu.contains("PAR VE2CUY"), "menu :\n{menu}");
    save_ppm(&m, "invaders-menu-fr");
    tap(&mut m, " ");
    frames(&mut m, 60);
    let s = screen(&m);
    assert!(s.contains("RECORD") && s.contains("VIES"), "partie :\n{s}");
    save_ppm(&m, "invaders-game-fr");

    // BREAK revient au menu (une fois la bannière « VAGUE 1 » passée), puis au choix de la langue.
    frames(&mut m, 60);
    tap(&mut m, "Escape");
    frames(&mut m, 10);
    assert!(screen(&m).contains("APPUYEZ SUR ESPACE POUR JOUER"), "après BREAK :\n{}", screen(&m));
    tap(&mut m, "Escape");
    frames(&mut m, 10);
    assert!(screen(&m).contains("PRESS  E  FOR ENGLISH"), "après BREAK :\n{}", screen(&m));
}

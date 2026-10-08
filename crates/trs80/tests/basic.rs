//! Session BASIC complète : démarrage, réponse à « MEM SIZE? », puis PRINT 2+2.
//! Valide ensemble le Z80, la carte mémoire, le clavier (matrice et MAJ) et la vidéo.
//!
//! Exige la ROM Level II dans `crates/z80/tests/roms/` (non fournie); sinon le test est ignoré.

use trs80::{Key, SCREEN_HEIGHT, SCREEN_WIDTH, Trs80};

fn load_rom() -> Option<Vec<u8>> {
    let dir = format!("{}/../z80/tests/roms", env!("CARGO_MANIFEST_DIR"));
    ["M1L2_1.3.bin", "level2.rom"]
        .iter()
        .find_map(|name| std::fs::read(format!("{dir}/{name}")).ok())
}

fn screen(m: &Trs80) -> String {
    (0..16)
        .map(|r| (0..64).map(|c| m.char_at(r, c)).collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn frames(m: &mut Trs80, n: usize) {
    for _ in 0..n {
        m.run_frame();
    }
}

/// Exécute jusqu'à ce que `text` apparaisse à l'écran (au plus 5 secondes de temps machine).
fn wait_for(m: &mut Trs80, text: &str) {
    for _ in 0..300 {
        if screen(m).contains(text) {
            return;
        }
        m.run_frame();
    }
    panic!("« {text} » jamais affiché. Écran :\n{}", screen(m));
}

/// Tape une touche : enfoncée pendant 3 images, puis relâchée pendant 3 images.
fn tap(m: &mut Trs80, name: &str) {
    let key = Key::from_name(name).unwrap_or_else(|| panic!("touche inconnue : {name}"));
    m.key_down(key);
    frames(m, 3);
    m.key_up(key);
    frames(m, 3);
}

fn type_line(m: &mut Trs80, text: &str) {
    for c in text.chars() {
        tap(m, &c.to_string());
    }
    tap(m, "Enter");
}

#[test]
fn basic_session() {
    let Some(rom) = load_rom() else {
        eprintln!("ROM Level II absente : test ignoré");
        return;
    };
    let mut m = Trs80::new(&rom).unwrap();

    wait_for(&mut m, "SIZE?");
    tap(&mut m, "Enter");
    wait_for(&mut m, "READY");

    // « + » et « ( » exigent MAJ sur le TRS-80, pas sur le clavier hôte.
    type_line(&mut m, "PRINT (2+2)*3");
    wait_for(&mut m, " 12");
    eprintln!("Écran final ({} T-states) :\n{}", m.cpu().cycles, screen(&m));

    let mut pixels = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
    m.render(&mut pixels);
    assert!(pixels.chunks(4).any(|p| p[0] > 0x80), "l'écran rendu contient du texte");
}

#[test]
fn rejects_wrong_rom_size() {
    assert!(Trs80::new(&[0u8; 100]).is_err());
}

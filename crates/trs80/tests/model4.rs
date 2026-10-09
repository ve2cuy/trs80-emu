//! Model 4 : ROM du Model III, 128 Ko de RAM en banques, plans de mémoire et écran de
//! 80 × 24 (port 84h), 4 MHz. TRSDOS 6 (LS-DOS) passe lui-même en mode Model 4.
//!
//! Exige la ROM du Model III (`M3_REVC.bin`) et la disquette TRSDOS 6.2.1
//! (`m4-trsdos621.dsk`) dans `crates/trs80/tests/roms/` (non fournies : test ignoré sinon).

use trs80::{Model, Trs80};

fn load(name: &str) -> Option<Vec<u8>> {
    std::fs::read(format!("{}/tests/roms/{name}", env!("CARGO_MANIFEST_DIR"))).ok()
}

fn screen(m: &Trs80) -> String {
    let mode = m.text_mode();
    (0..mode.rows)
        .map(|r| (0..mode.cols).map(|c| m.char_at(r, c)).collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn wait_for(m: &mut Trs80, text: &str, seconds: usize) -> bool {
    for _ in 0..seconds * 60 {
        if m.screen_contains(text) {
            return true;
        }
        m.run_frame();
    }
    m.screen_contains(text)
}

fn type_line(m: &mut Trs80, text: &str) {
    m.type_text(text);
    m.type_text("\n");
    for _ in 0..3000 {
        if !m.typing() {
            break;
        }
        m.run_frame();
    }
}

#[test]
fn model4_basic_like_model3() {
    let Some(rom) = load("M3_REVC.bin") else { return };
    let mut m = Trs80::with_model(&rom, Model::IV).unwrap();
    assert!(wait_for(&mut m, "Cass?", 3), "Écran :\n{}", screen(&m));
}

#[test]
fn model4_boots_trsdos6() {
    let (Some(rom), Some(disk)) = (load("M3_REVC.bin"), load("m4-trsdos621.dsk")) else { return };
    let mut m = Trs80::with_model(&rom, Model::IV).unwrap();
    m.insert_disk(0, disk).unwrap();
    assert!(wait_for(&mut m, "TRSDOS Ready", 10), "Écran :\n{}", screen(&m));
    // TRSDOS 6 finit de charger sa configuration : une touche tapée avant serait perdue.
    (0..300).for_each(|_| m.run_frame());
    assert_eq!(m.text_mode().cols, 80, "TRSDOS 6 passe en 80 × 24");
    // Banques de 32 Ko : MEMORY affiche la mémoire du Model 4.
    type_line(&mut m, "MEMORY");
    (0..120).for_each(|_| m.run_frame());
    let s = screen(&m);
    eprintln!("--- MEMORY :\n{s}");
    // Clavier (en F400h dans le plan de mémoire du Model 4) et répertoire du lecteur 0.
    // (DIR seul passerait aux lecteurs vides, avec une pause en fin d'écran.)
    type_line(&mut m, "DIR :0");
    (0..180).for_each(|_| m.run_frame());
    let s = screen(&m);
    eprintln!("--- DIR :\n{s}");
    assert!(s.contains("DOS/HLP") && s.contains("Free ="), "Écran :\n{s}");
}

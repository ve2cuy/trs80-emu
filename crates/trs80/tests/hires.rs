//! Carte graphique haute résolution Radio Shack (ports 80h-83h) avec BASICG sous TRSDOS 6
//! (Model 4) : SCREEN 0 affiche le graphique, CIRCLE et LINE y dessinent.
//!
//! Exige la ROM du Model III `M3_REVC.bin`, TRSDOS 6.2.1 `m4-trsdos621.dsk` et la disquette
//! BASICG 01.01.00 `m4-basicg-sys.dsk` dans `crates/trs80/tests/roms/` (non fournies : test
//! ignoré sinon).

use trs80::{Model, Trs80};

fn load(name: &str) -> Option<Vec<u8>> {
    std::fs::read(format!("{}/tests/roms/{name}", env!("CARGO_MANIFEST_DIR"))).ok()
}

fn run(m: &mut Trs80, seconds: u32) {
    (0..seconds * 60).for_each(|_| m.run_frame());
}

#[test]
fn model4_basicg_draws_on_the_graphics_board() {
    let (Some(rom), Some(dos), Some(basicg)) = (load("M3_REVC.bin"), load("m4-trsdos621.dsk"), load("m4-basicg-sys.dsk"))
    else {
        return;
    };
    let mut m = Trs80::with_model(&rom, Model::IV).unwrap();
    m.insert_disk(0, dos).unwrap();
    m.insert_disk(1, basicg).unwrap();
    run(&mut m, 15);
    m.type_text("BASICG\n");
    run(&mut m, 10);
    assert!(m.screen_contains("BASICG 01.01.00"));
    assert!(!m.hires_active());
    for line in ["SCREEN 0", "CLR", "CIRCLE (320,120),100", "LINE (0,0)-(639,239)"] {
        m.type_text(&format!("{line}\n"));
        run(&mut m, 3);
    }
    run(&mut m, 3);
    assert!(m.hires_active());
    assert_eq!(m.screen_size(), (640, 240));
    let mut out = vec![0u8; 640 * 240 * 4];
    m.render(&mut out);
    let lit = |x: usize, y: usize| out[(y * 640 + x) * 4] > 0x80;
    // Le cercle (centre 320,120, rayon 100) et la diagonale, qui passe par son centre.
    assert!(lit(420, 120) || lit(419, 120), "bord droit du cercle");
    assert!(lit(320, 120) || lit(321, 120), "diagonale au centre");
    assert!(!lit(360, 120), "intérieur du cercle");
    // Sans la carte, les ports lisent FFh et rien n'est affiché.
    m.set_graphics_board(false);
    assert!(!m.hires_active());
}

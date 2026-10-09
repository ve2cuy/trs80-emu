//! Model II : ROM d'amorçage de 2 Ko, disquettes de 8 pouces, horloge par NMI, clavier,
//! disquettes et DMA par interruptions en mode 2.
//!
//! Exige la ROM d'amorçage (`m2_boot_v5.bin`) et une disquette TRSDOS-II
//! (`m2-trsdos20a-64k.imd`, TRSDOS 2.0a) dans `crates/trs80/tests/roms/` (non fournies : test ignoré sinon).

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

fn run(m: &mut Trs80, seconds: u32) {
    (0..seconds * 60).for_each(|_| m.run_frame());
}

#[test]
fn model2_rom_asks_for_a_disk() {
    let Some(rom) = load("m2_boot_v5.bin") else { return };
    let mut m = Trs80::new(&rom).unwrap();
    assert_eq!(m.model(), Model::II);
    run(&mut m, 3);
    let s = screen(&m);
    assert!(s.to_uppercase().contains("INSERT"), "Écran :\n{s}");
}

#[test]
fn model2_boots_trsdos2() {
    let (Some(rom), Some(disk)) = (load("m2_boot_v5.bin"), load("m2-trsdos20a-64k.imd")) else { return };
    let mut m = Trs80::new(&rom).unwrap();
    m.insert_disk(0, disk).unwrap();
    run(&mut m, 20);
    let s = screen(&m);
    assert!(s.contains("TRSDOS version 2.0a") && s.contains("Enter Date"), "Écran :\n{s}");
    // Logo en vidéo inversée : triangles (80h-83h) et blocs pleins (A0h).
    assert!(m.video().contains(&0x81) && m.video().contains(&0xA0));
    // Clavier (canal 3 du CTC); TRSDOS 2.0a n'accepte que les années 1980 à 1999.
    m.type_text("10/09/1986\n");
    run(&mut m, 5);
    m.type_text("12.00.00\n");
    run(&mut m, 5);
    assert!(m.screen_contains("TRSDOS READY"), "Écran :\n{}", screen(&m));
    // Répertoire : lectures par DMA, dont des lectures partielles (données perdues).
    m.type_text("DIR\n");
    run(&mut m, 10);
    let s = screen(&m);
    assert!(s.contains("DISK NAME:TRSDOS") && s.contains("FREE GRANULES"), "Écran :\n{s}");
}

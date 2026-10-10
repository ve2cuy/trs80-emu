//! Model II : ROM d'amorçage de 2 Ko, disquettes de 8 pouces, horloge par NMI, clavier,
//! disquettes et DMA par interruptions en mode 2.
//!
//! Exige la ROM d'amorçage (`m2_boot_v5.bin`) et des disquettes TRSDOS-II dans
//! `crates/trs80/tests/roms/` (non fournies : test ignoré sinon) : `m2-trsdos20a-64k.imd`
//! (TRSDOS 2.0a), `m2-trsdos42.imd` (TRSDOS-II 4.2), `m2-omniterm-tdos4.imd` (OMNITERM sur
//! TRSDOS-II 4.2).

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

/// Date et heure de TRSDOS-II 4.x (et TRSDOS-HD), puis l'invite « Ready ».
fn enter_date(m: &mut Trs80) {
    m.type_text("10/09/1986\n");
    run(m, 5);
    m.type_text("12.00.00\n");
    run(m, 5);
}

#[test]
fn model2_boots_trsdos42() {
    // TRSDOS-II 4.2 : avant de démarrer, son amorce teste la mémoire, le DMA, le PIO et les
    // trois temporisateurs du CTC (une interruption par canal, sinon « BOOT ERROR CT »).
    let (Some(rom), Some(disk)) = (load("m2_boot_v5.bin"), load("m2-trsdos42.imd")) else { return };
    let mut m = Trs80::new(&rom).unwrap();
    m.insert_disk(0, disk).unwrap();
    run(&mut m, 20);
    enter_date(&mut m);
    // Touche CAPS enfoncée au départ : « dir » arrive en majuscules (sinon : ERROR 31).
    m.type_text("dir\n");
    run(&mut m, 10);
    let s = screen(&m);
    assert!(s.contains("Files Displayed") && s.contains("TRSDOS-II Ready"), "Écran :\n{s}");
    m.toggle_caps();
    m.type_text("dir\n");
    run(&mut m, 5);
    assert!(m.screen_contains("ERROR 31"), "Écran :\n{}", screen(&m));
}

#[test]
fn model2_omniterm_over_rs232() {
    // Port série : canal A du SIO (F4h-F7h), relié au modem de la page.
    let (Some(rom), Some(disk)) = (load("m2_boot_v5.bin"), load("m2-omniterm-tdos4.imd")) else { return };
    let mut m = Trs80::new(&rom).unwrap();
    m.insert_disk(0, disk).unwrap();
    run(&mut m, 20);
    enter_date(&mut m);
    m.type_text("OMNITERM\n");
    run(&mut m, 15);
    assert!(m.screen_contains("OMNITERM"), "Écran :\n{}", screen(&m));
    m.serial_take();
    m.type_text("ATDT BBS\r");
    run(&mut m, 5);
    assert_eq!(m.serial_take(), b"ATDT BBS\r");
    m.serial_send(b"\r\nCONNECT 9600\r\nWELCOME TO THE BBS\r\n");
    run(&mut m, 5);
    let s = screen(&m);
    assert!(s.contains("CONNECT 9600") && s.contains("WELCOME TO THE BBS"), "Écran :\n{s}");
    assert_eq!(m.serial_pending(), 0);
}

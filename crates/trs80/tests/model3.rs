//! Model III : ROM de 14 Ko, BASIC, disquettes (WD1793 sur les ports F0h-F4h, NMI).
//!
//! Exige la ROM du Model III (`M3_REVC.bin`) dans `crates/trs80/tests/roms/` (non fournie).
//! Les disquettes de système (TRSDOS 1.3, LDOS) ne sont pas publiées : les tests qui les
//! démarrent sont ignorés si elles ne sont pas dans ce même dossier.

use trs80::{Model, Trs80};

fn roms() -> String {
    format!("{}/tests/roms", env!("CARGO_MANIFEST_DIR"))
}

fn load(name: &str) -> Option<Vec<u8>> {
    std::fs::read(format!("{}/{name}", roms())).ok()
}

fn screen(m: &Trs80) -> String {
    (0..16)
        .map(|r| (0..64).map(|c| m.char_at(r, c)).collect::<String>().trim_end().to_string())
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
fn model3_rom_boots_basic() {
    let Some(rom) = load("M3_REVC.bin") else { return };
    let mut m = Trs80::new(&rom).unwrap();
    assert_eq!(m.model(), Model::III);
    assert!(wait_for(&mut m, "Cass?", 3), "Écran :\n{}", screen(&m));
    type_line(&mut m, "");
    assert!(wait_for(&mut m, "Memory Size?", 3), "Écran :\n{}", screen(&m));
    type_line(&mut m, "");
    assert!(wait_for(&mut m, "READY", 3), "Écran :\n{}", screen(&m));
    type_line(&mut m, "PRINT 6*7");
    (0..30).for_each(|_| m.run_frame());
    assert!(screen(&m).lines().any(|l| l.trim() == "42"), "Écran :\n{}", screen(&m));
    // Les minuscules s'affichent (Model III).
    type_line(&mut m, "PRINT CHR$(97);CHR$(98)");
    (0..30).for_each(|_| m.run_frame());
    assert!(screen(&m).contains("ab"), "Écran :\n{}", screen(&m));
}

/// Démarre `disk` sur le Model III et rend l'écran après `seconds` secondes.
fn boot(disk: &str, seconds: usize) -> Option<(Trs80, String)> {
    let rom = load("M3_REVC.bin")?;
    let image = load(disk)?;
    let mut m = Trs80::new(&rom).unwrap();
    m.insert_disk(0, image).unwrap();
    (0..seconds * 60).for_each(|_| m.run_frame());
    let s = screen(&m);
    eprintln!("{disk} après {seconds} s :\n{s}\n");
    Some((m, s))
}

#[test]
fn model3_boots_trsdos13() {
    let Some((mut m, s)) = boot("m3-trsdos13.dsk", 6) else { return };
    assert!(s.contains("TRSDOS version 1.3"), "Écran :\n{s}");
    assert!(wait_for(&mut m, "TRSDOS Ready", 10), "Écran :\n{}", screen(&m));
    type_line(&mut m, "DIR");
    (0..240).for_each(|_| m.run_frame());
    eprintln!("DIR :\n{}", screen(&m));
    let s = screen(&m);
    assert!(s.contains("MEMTEST/CMD") && s.contains("Free Granules"), "Écran :\n{s}");
}

#[test]
fn model3_boots_ldos() {
    // Disquette de données : le secteur d'amorce est lu et répond qu'il n'y a pas de système.
    // (Message affiché en mode 32 colonnes : une lettre sur deux colonnes.)
    let Some((_, s)) = boot("m3-ldos-util.dmk", 6) else { return };
    assert!(s.replace(' ', "").contains("Nosystem"), "Écran :\n{s}");
}

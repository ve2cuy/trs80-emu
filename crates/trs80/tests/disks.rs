//! Disquettes : démarrage de LDOS 5.3.1 (www/disks/, redistribuable) et commandes du DOS.
//!
//! Exige la ROM Level II dans `crates/trs80/tests/roms/` (non fournie); sinon le test est ignoré.
//! Des images supplémentaires (non publiées, ex. TRSDOS) peuvent être essayées avec
//! `TRS80_EXTRA_DISK=<chemin>` : le test affiche alors l'écran après démarrage.

use trs80::{Format, Key, Trs80};

fn load_rom() -> Option<Vec<u8>> {
    let dir = format!("{}/tests/roms", env!("CARGO_MANIFEST_DIR"));
    ["M1L2_1.3.bin", "level2.rom"]
        .iter()
        .find_map(|name| std::fs::read(format!("{dir}/{name}")).ok())
}

fn ldos() -> Vec<u8> {
    std::fs::read(format!("{}/../../www/disks/ldos-531.dsk", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn screen(m: &Trs80) -> String {
    (0..16)
        .map(|r| (0..64).map(|c| m.char_at(r, c)).collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Exécute jusqu'à ce que `text` apparaisse (au plus `seconds` secondes de temps machine).
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
    for _ in 0..2000 {
        if !m.typing() {
            break;
        }
        m.run_frame();
    }
}

#[test]
fn ldos_boots_and_lists_directory() {
    let Some(rom) = load_rom() else { return };
    let mut m = Trs80::new(&rom).unwrap();
    let disk = m.insert_disk(0, ldos()).unwrap();
    assert_eq!(disk.format(), Format::Jv1);

    // LDOS demande la date puis l'heure au démarrage (en majuscules : pas de minuscules).
    assert!(wait_for(&mut m, "DATE", 20), "LDOS n'a pas démarré. Écran :\n{}", screen(&m));
    // LDOS 5.3.1 (1991) refuse les années trop lointaines : une date de son époque.
    type_line(&mut m, "10/08/91");
    assert!(wait_for(&mut m, "TIME", 5), "pas de demande d'heure. Écran :\n{}", screen(&m));
    type_line(&mut m, "12:00:00");
    assert!(wait_for(&mut m, "READY", 10), "pas d'invite LDOS. Écran :\n{}", screen(&m));

    // DIR : le répertoire de la disquette système.
    type_line(&mut m, "DIR");
    (0..300).for_each(|_| m.run_frame());
    let s = screen(&m);
    assert!(s.contains("DRIVE :0 LDOS531") && s.contains("PATCH/CMD"), "DIR n'affiche pas les fichiers. Écran :\n{s}");
    eprintln!("Écran LDOS :\n{}", screen(&m));
}

#[test]
#[ignore = "diagnostic"]
fn ldos_keyboard_debug() {
    let Some(rom) = load_rom() else { return };
    let mut m = Trs80::new(&rom).unwrap();
    m.insert_disk(0, ldos()).unwrap();
    wait_for(&mut m, "DATE", 20);
    let c = m.cpu();
    eprintln!("à DATE ? : iff1={} im={} pc={:04X} i={:02X}", c.iff1, c.im, c.pc, c.i);
    for text in ["2", "34"] {
        m.type_text(text);
        for f in 0..30 {
            m.run_frame();
            if f % 3 == 0 {
                eprintln!("  frappe auto {text:?}, image {f} : typing={} ligne 12 = {:?}",
                    m.typing(), (0..64).map(|x| m.char_at(12, x)).collect::<String>().trim());
            }
        }
    }
    for hold in [3, 10, 30] {
        let k = Key::from_name("1").unwrap();
        m.key_down(k);
        (0..hold).for_each(|_| m.run_frame());
        m.key_up(k);
        (0..20).for_each(|_| m.run_frame());
        let c = m.cpu();
        eprintln!("touche 1 maintenue {hold} images : ligne 12 = {:?}  iff1={} pc={:04X}",
            (0..64).map(|x| m.char_at(12, x)).collect::<String>().trim(), c.iff1, c.pc);
    }
}

/// Scénario de la page : la machine tourne en BASIC, on insère la disquette, puis RESET.
#[test]
fn insert_then_reset_boots_the_disk() {
    let Some(rom) = load_rom() else { return };
    let mut m = Trs80::new(&rom).unwrap();
    assert!(wait_for(&mut m, "SIZE?", 2));
    m.insert_disk(0, ldos()).unwrap();
    m.reset();
    assert!(wait_for(&mut m, "DATE", 20), "RESET avec disquette : LDOS attendu. Écran :\n{}", screen(&m));
}

#[test]
fn extra_disk_insert_then_reset() {
    let (Some(rom), Ok(path)) = (load_rom(), std::env::var("TRS80_EXTRA_DISK")) else { return };
    let mut m = Trs80::new(&rom).unwrap();
    wait_for(&mut m, "SIZE?", 2);
    m.insert_disk(0, std::fs::read(&path).unwrap()).unwrap();
    m.reset();
    (0..600).for_each(|_| m.run_frame());
    eprintln!("Après RESET :\n{}", screen(&m));
    for e in m.fdc_trace().iter().take(12) {
        eprintln!("  cmd {:02X} piste {} secteur {} état {:02X}", e.command, e.track, e.sector, e.status);
    }
}

#[test]
fn no_disk_still_boots_basic() {
    let Some(rom) = load_rom() else { return };
    let mut m = Trs80::new(&rom).unwrap();
    assert!(wait_for(&mut m, "SIZE?", 2), "sans disquette, la ROM doit démarrer en BASIC");
}

#[test]
fn extra_disk_from_environment() {
    let (Some(rom), Ok(path)) = (load_rom(), std::env::var("TRS80_EXTRA_DISK")) else { return };
    let mut m = Trs80::new(&rom).unwrap();
    let disk = m.insert_disk(0, std::fs::read(&path).unwrap()).unwrap();
    eprintln!("{path} : {} secteurs, format {}", disk.sector_count(), disk.format().name());
    for _ in 0..600 {
        m.run_frame();
    }
    let enter = Key::from_name("Enter").unwrap();
    m.key_down(enter);
    (0..5).for_each(|_| m.run_frame());
    m.key_up(enter);
    (0..300).for_each(|_| m.run_frame());
    eprintln!("Écran :\n{}", screen(&m));
    if std::env::var("TRS80_FDC_TRACE").is_ok() {
        for e in m.fdc_trace() {
            eprintln!(
                "  cmd {:02X}  lecteur {} piste {:3} secteur {:3} tête {:3} {}  état {:02X}",
                e.command, e.drive, e.track, e.sector, e.head,
                if e.double_density { "DD" } else { "SD" }, e.status
            );
        }
    }
}

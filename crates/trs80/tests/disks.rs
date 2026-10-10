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

/// Le bruit des lecteurs suit leur activité : le démarrage déplace la tête et lit la disquette.
#[test]
fn disk_activity_counts_steps_and_accesses() {
    let Some(rom) = load_rom() else { return };
    let mut m = Trs80::new(&rom).unwrap();
    assert_eq!(m.disk_activity(), (0, 0));
    m.insert_disk(0, ldos()).unwrap();
    assert!(wait_for(&mut m, "DATE", 20), "LDOS n'a pas démarré. Écran :\n{}", screen(&m));
    let (steps, accesses) = m.disk_activity();
    eprintln!("démarrage de LDOS : {steps} pas, {accesses} accès");
    assert!(steps > 0 && accesses > 10, "pas = {steps}, accès = {accesses}");
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

/// Changer de disquette pendant qu'un DOS tourne, puis RESET : le nouveau DOS doit démarrer.
/// LDOS, à l'invite de la date, a laissé la tête loin de la piste 0 : la ROM (0696h) démarre
/// en BASIC si l'état du contrôleur vaut 00h; le RESET doit donc lancer un Restore, comme
/// le WD1771 réinitialisé (bogue corrigé : tout changement de disquette démarrait en BASIC).
///
/// L'échec dépendait de l'instant du RESET : pendant l'impulsion d'index, l'état n'est pas
/// nul. Une rotation dure exactement 12 images : on essaie les 12 instants d'un tour.
#[test]
fn switch_disk_after_dos_then_reset() {
    let Some(rom) = load_rom() else { return };
    for offset in 0..12 {
        // Comme « Boot a disk » : insertion puis RESET, deux fois de suite.
        let mut m = Trs80::new(&rom).unwrap();
        m.insert_disk(0, ldos()).unwrap();
        m.reset();
        // LDOS a fini de démarrer et attend la date (tête loin de la piste 0).
        (0..600 + offset).for_each(|_| m.run_frame());
        assert!(m.screen_contains("DATE"), "LDOS attendu. Écran :\n{}", screen(&m));

        // Une deuxième fois LDOS (seule disquette publiée), comme « Boot a disk » dans la page.
        m.insert_disk(0, ldos()).unwrap();
        m.reset();
        // Le RESET n'efface pas l'écran : on laisse démarrer, puis on vérifie que la ROM
        // n'a pas affiché l'invite du BASIC (MEM SIZE?) et que LDOS demande la date.
        (0..600).for_each(|_| m.run_frame());
        let s = screen(&m);
        assert!(
            !s.contains("SIZE?") && s.contains("DATE"),
            "RESET à l'image {offset} du tour : LDOS attendu. Écran :\n{s}"
        );
    }
}

/// Suite de disquettes (TRS80_DISK_SEQUENCE = chemins séparés par « ; ») : chacune est
/// insérée dans le lecteur 0 puis RESET, comme « Boot a disk » dans la page.
#[test]
fn disk_sequence_from_environment() {
    let (Some(rom), Ok(list)) = (load_rom(), std::env::var("TRS80_DISK_SEQUENCE")) else { return };
    let mut m = Trs80::new(&rom).unwrap();
    for path in list.split(';').filter(|p| !p.is_empty()) {
        m.insert_disk(0, std::fs::read(path).unwrap()).unwrap();
        m.reset();
        (0..600).for_each(|_| m.run_frame());
        let first: Vec<String> = screen(&m).lines().filter(|l| !l.trim().is_empty()).take(3).map(String::from).collect();
        eprintln!("{} -> {:?}", path.rsplit(['/', '\\']).next().unwrap(), first);
        if std::env::var("TRS80_FDC_TRACE").is_ok() {
            for e in m.fdc_trace().iter().rev().take(6).collect::<Vec<_>>().into_iter().rev() {
                eprintln!("    cmd {:02X} piste {} secteur {} tête {} état {:02X}", e.command, e.track, e.sector, e.head, e.status);
            }
            eprintln!("    PC={:04X} iff1={} im={}", m.cpu().pc, m.cpu().iff1, m.cpu().im);
        }
    }
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

/// LDOS double densité publié (www/disks/ldos-531-dd.dsk) : piste 0 en simple densité pour
/// la ROM, puis le reste en double densité par le doubleur Radio Shack.
#[test]
fn ldos_double_density_boots() {
    let Some(rom) = load_rom() else { return };
    let dd = std::fs::read(format!("{}/../../www/disks/ldos-531-dd.dsk", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut m = Trs80::new(&rom).unwrap();
    assert_eq!(m.insert_disk(0, dd).unwrap().format(), Format::Jv3);
    assert!(wait_for(&mut m, "DATE", 20), "LDOS double densité n'a pas démarré. Écran :\n{}", screen(&m));
    type_line(&mut m, "10/08/91");
    type_line(&mut m, "12:00:00");
    // La configuration enregistrée (SYSGEN : pilote du doubleur) se charge avant l'invite.
    assert!(wait_for(&mut m, "READY", 10), "pas d'invite LDOS. Écran :\n{}", screen(&m));
    type_line(&mut m, "DIR");
    (0..300).for_each(|_| m.run_frame());
    let s = screen(&m);
    assert!(s.contains("40D1") && s.contains("DOS/HLP"), "DIR du disque double densité. Écran :\n{s}");
    assert!(m.double_density(), "le doubleur doit être en double densité");
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

#[test]
fn geometry_of_published_disks() {
    use trs80::{Disk, Geometry, TrackShape};
    let open = |name: &str| {
        Disk::open(std::fs::read(format!("{}/../../www/disks/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap()
    };
    // LDOS 5.3.1 (Model I) : JV1, 35 pistes de 10 secteurs de 256 octets en simple densité.
    let sd = TrackShape { sectors: 10, size: 256, dd: false };
    assert_eq!(open("ldos-531.dsk").geometry(), Geometry { tracks: 35, sides: 1, track0: sd, track: sd });
    // TRSDOS-II 2.0a (Model II) : 77 pistes de 8 pouces; piste 0 en simple densité
    // (26 × 128), les autres en double densité (26 × 256).
    let g = open("trsdos20a-m2.imd").geometry();
    assert_eq!((g.tracks, g.sides), (77, 1));
    assert_eq!(g.track0, TrackShape { sectors: 26, size: 128, dd: false });
    assert_eq!(g.track, TrackShape { sectors: 26, size: 256, dd: true });
    assert_eq!(Disk::blank().geometry().tracks, 0);
}

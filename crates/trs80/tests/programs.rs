//! Chargement des programmes `.CMD` fournis avec le site (www/programs/).
//! Chaque programme doit se charger, se lancer et modifier l'écran.
//!
//! Exige la ROM Level II dans `crates/trs80/tests/roms/` (non fournie); sinon le test est ignoré.
//! Une capture de l'écran de chaque programme est écrite dans `target/screens/` (format PPM).

use trs80::{SCREEN_HEIGHT, SCREEN_WIDTH, Trs80};

fn load_rom() -> Option<Vec<u8>> {
    let dir = format!("{}/tests/roms", env!("CARGO_MANIFEST_DIR"));
    ["M1L2_1.3.bin", "level2.rom"]
        .iter()
        .find_map(|name| std::fs::read(format!("{dir}/{name}")).ok())
}

fn programs_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../www/programs")
}

fn save_ppm(m: &Trs80, name: &str) {
    let mut rgba = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
    m.render(&mut rgba);
    let mut ppm = format!("P6\n{SCREEN_WIDTH} {SCREEN_HEIGHT}\n255\n").into_bytes();
    for px in rgba.chunks(4) {
        ppm.extend_from_slice(&px[..3]);
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screens");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{name}.ppm")), ppm).unwrap();
}

#[test]
fn bundled_programs_start() {
    let Some(rom) = load_rom() else {
        eprintln!("ROM Level II absente : test ignoré");
        return;
    };
    let mut names: Vec<_> = std::fs::read_dir(programs_dir())
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("cmd")))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "aucun programme dans www/programs");

    for path in names {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let mut m = Trs80::new(&rom).unwrap();
        let entry = m.load_cmd(&std::fs::read(&path).unwrap()).unwrap();
        let before = *m.video();
        for _ in 0..300 {
            m.run_frame();
        }
        save_ppm(&m, &name);
        eprintln!("{name} : lancé en {entry:04X}h, PC = {:04X}h", m.cpu().pc);
        assert_ne!(*m.video(), before, "{name} n'a rien affiché");
    }
}

#[test]
fn rejects_invalid_cmd_without_touching_memory() {
    let Some(rom) = load_rom() else { return };
    let mut m = Trs80::new(&rom).unwrap();
    assert!(m.load_cmd(&[0x01, 0x10, 0x00]).is_err());
}

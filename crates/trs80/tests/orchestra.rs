//! Carte son Orchestra-90 (Software Affair) : deux convertisseurs 8 bits sur les ports 79h
//! (gauche) et 75h (droite) des Model III et 4. ORCH90 sous LDOS 5.3.1 joue un morceau à
//! quatre voix.
//!
//! Exige la ROM `M3_REVC.bin`, LDOS 5.3.1 `m3-ldos531.dsk` et la disquette d'ORCHESTRA-90
//! `orchutil.dsk` (ORCH90/CMD et GYPSY/ORC) dans `crates/trs80/tests/roms/` (non fournies :
//! test ignoré sinon).

use trs80::{Key, Model, Trs80};

fn load(name: &str) -> Option<Vec<u8>> {
    std::fs::read(format!("{}/tests/roms/{name}", env!("CARGO_MANIFEST_DIR"))).ok()
}

fn run(m: &mut Trs80, seconds: u32) {
    (0..seconds * 60).for_each(|_| m.run_frame());
}

#[test]
fn model3_orchestra90_plays_music() {
    let (Some(rom), Some(dos), Some(orch)) = (load("M3_REVC.bin"), load("m3-ldos531.dsk"), load("orchutil.dsk")) else {
        return;
    };
    let mut m = Trs80::with_model(&rom, Model::III).unwrap();
    m.insert_disk(0, dos).unwrap();
    m.insert_disk(1, orch).unwrap();
    m.set_audio_rate(22050);
    run(&mut m, 8);
    // Date, ORCH90 : horloge normale (N), 4 voix, sans enregistrer le programme (N).
    for line in ["10/10/86", "", "ORCH90", "N", "4", "N"] {
        m.type_text(&format!("{line}\n"));
        run(&mut m, 6);
    }
    assert!(m.screen_contains("ORCHESTRA-90"));
    // BREAK : ligne de commande; G lit, compile et joue le morceau.
    let brk = Key::from_name("Escape").unwrap();
    m.key_down(brk);
    run(&mut m, 1);
    m.key_up(brk);
    run(&mut m, 6);
    m.type_text("G GYPSY:1\n");
    run(&mut m, 12);
    assert!(m.screen_contains("GYPSY RONDO"));
    let mut peak = 0f32;
    for _ in 0..15 * 60 {
        m.run_frame();
        peak = m.audio_samples().iter().fold(peak, |a, &b| a.max(b.abs()));
        m.clear_audio();
    }
    assert!(m.orchestra_writes() > 50_000, "écritures : {}", m.orchestra_writes());
    assert!(peak > 0.05, "crête du son : {peak}");
}

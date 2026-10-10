//! Magnétophone : CLOAD lit une cassette `.CAS` à 500 bauds par l'entrée cassette.
//!
//! Exige les ROM dans `crates/trs80/tests/roms/` (non fournies); sinon les tests sont ignorés.

use trs80::{Model, Trs80};

fn rom(name: &str) -> Option<Vec<u8>> {
    std::fs::read(format!("{}/tests/roms/{name}", env!("CARGO_MANIFEST_DIR"))).ok()
}

/// Cassette BASIC : 10 PRINT "TAPE";6*7
fn basic_tape() -> Vec<u8> {
    let mut t = vec![0u8; 256];
    t.extend([0xA5, 0xD3, 0xD3, 0xD3, b'A']);
    let line: &[u8] = &[0xB2, b' ', b'"', b'T', b'A', b'P', b'E', b'"', b';', b'6', 0xCF, b'7', 0];
    let next = 0x42E9u16 + 4 + line.len() as u16;
    t.extend(next.to_le_bytes());
    t.extend(10u16.to_le_bytes());
    t.extend(line);
    t.extend([0, 0, 0, 0]);
    t
}

fn finish_typing(m: &mut Trs80, extra: usize) {
    for _ in 0..20_000 {
        if !m.typing() {
            break;
        }
        m.run_frame();
    }
    (0..extra).for_each(|_| m.run_frame());
}

#[test]
fn cload_reads_the_tape_on_model_i_and_iii() {
    for (name, model) in [("M1L2_1.3.bin", Model::I), ("M3_REVC.bin", Model::III)] {
        let Some(rom) = rom(name) else { continue };
        let mut m = Trs80::with_model(&rom, model).unwrap();
        let tape = basic_tape();
        // Charger la cassette directement redémarre la machine en BASIC; on efface le
        // programme pour le relire par le magnétophone.
        m.load_cas(&tape).unwrap();
        m.type_text("NEW\n");
        finish_typing(&mut m, 30);
        m.insert_tape(tape.clone());
        assert_eq!(m.tape_progress(), Some((0, tape.len(), false)));
        m.type_text("CLOAD\n");
        finish_typing(&mut m, 0);
        for _ in 0..600 {
            m.run_frame();
            if m.tape_progress().is_some_and(|(pos, _, motor)| pos > 0 && !motor) {
                break;
            }
        }
        // Le BASIC arrête le moteur après les zéros de fin du programme.
        let (pos, len, motor) = m.tape_progress().unwrap();
        assert!(!motor && pos + 4 >= len, "{model:?} : {pos}/{len}");
        m.type_text("RUN\n");
        finish_typing(&mut m, 30);
        assert!(m.screen_contains("TAPE 42"), "{model:?} : le programme lu ne s'exécute pas");
    }
}

/// BASIC Level I (ROM de 4 Ko) : une cassette Level I est lue par CLOAD à 250 bauds; une
/// cassette Level II est refusée, et la ROM Level II refuse une cassette Level I.
#[test]
fn level1_tapes_go_through_cload() {
    let (Some(l1), Some(l2)) = (rom("level1.bin"), rom("M1L2_1.3.bin")) else { return };
    // Adresses de début et de fin (octet fort en premier), puis les octets.
    let mut tape = vec![0u8; 64];
    tape.extend([0xA5, 0x70, 0x00, 0x70, 0x03, 1, 2, 3, 4, 10]);
    let mut m = Trs80::new(&l1).unwrap();
    assert_eq!(m.model(), Model::I);
    assert!(matches!(m.load_cas(&tape), Ok(trs80::Loaded::Level1)));
    for _ in 0..600 {
        m.run_frame();
        if m.tape_progress().is_some_and(|(pos, _, motor)| pos > 0 && !motor) {
            break;
        }
    }
    let (pos, len, motor) = m.tape_progress().unwrap();
    assert!(!motor && pos + 2 >= len, "{pos}/{len}");
    assert_eq!((0x7000..0x7003).map(|a| m.peek(a)).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(m.load_cas(&basic_tape()).unwrap_err().to_string(), "Level II tape: the Level I BASIC ROM cannot read it");
    let mut m = Trs80::new(&l2).unwrap();
    assert_eq!(m.load_cas(&tape).unwrap_err().to_string(), "Level I tape: it needs the Level I BASIC ROM (4 KB)");
}

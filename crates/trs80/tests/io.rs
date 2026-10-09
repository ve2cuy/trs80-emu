//! Collage de texte (frappe automatique), cassettes `.CAS` et horloge à 40 Hz.
//!
//! Exige la ROM Level II dans `crates/trs80/tests/roms/` (non fournie); sinon les tests sont ignorés.

use trs80::{Loaded, Trs80};

fn load_rom() -> Option<Vec<u8>> {
    let dir = format!("{}/tests/roms", env!("CARGO_MANIFEST_DIR"));
    ["M1L2_1.3.bin", "level2.rom"]
        .iter()
        .find_map(|name| std::fs::read(format!("{dir}/{name}")).ok())
}

fn screen(m: &Trs80) -> String {
    (0..16)
        .map(|r| (0..64).map(|c| m.char_at(r, c)).collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Exécute jusqu'à la fin de la frappe automatique, puis encore `extra` images.
fn finish_typing(m: &mut Trs80, extra: usize) {
    for _ in 0..20_000 {
        if !m.typing() {
            break;
        }
        m.run_frame();
    }
    assert!(!m.typing(), "frappe automatique jamais terminée");
    (0..extra).for_each(|_| m.run_frame());
}

const PROGRAM: &str = "10 FOR I=1 TO 3\n20 PRINT \"LINE\";I*11\n30 NEXT I\n";

/// Machine au BASIC, avec PROGRAM tapé (sans RUN).
fn machine_with_program(rom: &[u8]) -> Trs80 {
    let mut m = Trs80::new(rom).unwrap();
    // Réponse à MEM SIZE?, une fois l'invite affichée (la ROM met environ 0,3 s à démarrer).
    while !m.screen_contains("SIZE?") {
        m.run_frame();
    }
    m.type_text("\n");
    // Puis la ROM teste la mémoire avant d'afficher READY.
    for _ in 0..300 {
        if m.screen_contains("READY") {
            break;
        }
        m.run_frame();
    }
    assert!(m.screen_contains("READY"), "Écran :\n{}", screen(&m));
    assert_eq!(m.type_text(PROGRAM), PROGRAM.len());
    finish_typing(&mut m, 10);
    m
}

#[test]
fn pasted_program_runs() {
    let Some(rom) = load_rom() else { return };
    let mut m = machine_with_program(&rom);
    m.type_text("RUN\n");
    finish_typing(&mut m, 60);
    let s = screen(&m);
    for expected in ["LINE 11", "LINE 22", "LINE 33"] {
        assert!(s.contains(expected), "« {expected} » absent. Écran :\n{s}");
    }
}

#[test]
fn typed_backspace_erases() {
    let Some(rom) = load_rom() else { return };
    let mut m = machine_with_program(&rom);
    m.type_text("PRINT 12\x083\n");
    finish_typing(&mut m, 30);
    let s = screen(&m);
    assert!(s.lines().any(|l| l.trim() == "13"), "Écran :\n{s}");
}

#[test]
fn typing_skips_characters_without_a_key() {
    let Some(rom) = load_rom() else { return };
    let mut m = Trs80::new(&rom).unwrap();
    assert_eq!(m.type_text("A{é}B"), 2);
    m.cancel_typing();
    assert!(!m.typing());
}

/// Image .CAS BASIC fabriquée à partir de la mémoire d'une machine où PROGRAM a été tapé.
fn basic_cas(m: &mut Trs80) -> Vec<u8> {
    let peek = |m: &mut Trs80, a: u16| m.peek(a);
    let txttab = u16::from_le_bytes([peek(m, 0x40A4), peek(m, 0x40A5)]);
    let vartab = u16::from_le_bytes([peek(m, 0x40F9), peek(m, 0x40FA)]);
    let mut cas = vec![0u8; 255];
    cas.extend_from_slice(&[0xA5, 0xD3, 0xD3, 0xD3, b'P']);
    cas.extend((txttab..vartab).map(|a| m.peek(a)));
    cas
}

#[test]
fn basic_cassette_loads_and_runs() {
    let Some(rom) = load_rom() else { return };
    let mut source = machine_with_program(&rom);
    let cas = basic_cas(&mut source);

    let mut m = Trs80::new(&rom).unwrap();
    assert!(matches!(m.load_cas(&cas), Ok(Loaded::Basic(_))));
    m.type_text("RUN\n");
    finish_typing(&mut m, 60);
    assert!(screen(&m).contains("LINE 33"), "Écran :\n{}", screen(&m));
}

#[test]
fn system_cassette_loads_and_runs() {
    let Some(rom) = load_rom() else { return };
    // Programme en 7000h : écrit « OK » en haut à gauche de l'écran, puis boucle.
    //   LD HL,3C00h ; LD (HL),'O' ; INC HL ; LD (HL),'K' ; JR $
    let code = [0x21, 0x00, 0x3C, 0x36, b'O', 0x23, 0x36, b'K', 0x18, 0xFE];
    let mut cas = vec![0u8; 255];
    cas.extend_from_slice(&[0xA5, 0x55]);
    cas.extend_from_slice(b"OKTEST");
    cas.extend_from_slice(&[0x3C, code.len() as u8, 0x00, 0x70]);
    cas.extend_from_slice(&code);
    cas.push(code.iter().fold(0x70u8, |a, &b| a.wrapping_add(b)));
    cas.extend_from_slice(&[0x78, 0x00, 0x70]);

    let mut m = Trs80::new(&rom).unwrap();
    assert_eq!(m.load_cas(&cas), Ok(Loaded::System(0x7000)));
    m.run_frame();
    assert_eq!((m.char_at(0, 0), m.char_at(0, 1)), ('O', 'K'));
}

/// Fichier .CMD à partir de blocs (adresse, octets) et d'une adresse de lancement.
fn cmd(blocks: &[(u16, &[u8])], entry: u16) -> Vec<u8> {
    let mut out = Vec::new();
    for (addr, bytes) in blocks {
        out.push(0x01);
        out.push(bytes.len() as u8 + 2);
        out.extend_from_slice(&addr.to_le_bytes());
        out.extend_from_slice(bytes);
    }
    out.extend_from_slice(&[0x02, 0x02]);
    out.extend_from_slice(&entry.to_le_bytes());
    out
}

/// Compte les interruptions d'horloge pendant une seconde (60 images).
fn rtc_ticks(rom: &[u8], expansion: bool) -> u8 {
    // Gestionnaire en 7000h, branché sur le vecteur RST 38h de la ROM (JP 4012h) :
    //   PUSH AF ; PUSH HL ; LD A,(37E0h) ; LD HL,7100h ; INC (HL) ; POP HL ; POP AF ; EI ; RET
    let handler = [0xF5, 0xE5, 0x3A, 0xE0, 0x37, 0x21, 0x00, 0x71, 0x34, 0xE1, 0xF1, 0xFB, 0xC9];
    // Programme en 7020h : IM 1 ; EI ; JR $
    let main = [0xED, 0x56, 0xFB, 0x18, 0xFE];
    let file = cmd(
        &[(0x7000, &handler), (0x7020, &main), (0x7100, &[0]), (0x4012, &[0xC3, 0x00, 0x70])],
        0x7020,
    );
    let mut m = Trs80::new(rom).unwrap();
    m.set_expansion_interface(expansion);
    m.load_cmd(&file).unwrap();
    (0..60).for_each(|_| m.run_frame());
    m.peek(0x7100)
}

#[test]
fn clock_interrupts_at_40_hz() {
    let Some(rom) = load_rom() else { return };
    let ticks = rtc_ticks(&rom, true);
    assert!((39..=41).contains(&ticks), "{ticks} interruptions en une seconde");
    assert_eq!(rtc_ticks(&rom, false), 0, "sans interface d'expansion, pas d'horloge");
}

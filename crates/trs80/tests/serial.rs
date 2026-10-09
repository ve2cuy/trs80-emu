//! Port série RS-232 (UART, ports E8h-EBh) avec le programme de communication LCOMM de LDOS :
//! ce qu'on tape part sur la ligne, ce qui arrive de la ligne s'affiche.
//!
//! Exige la ROM du modèle dans `crates/trs80/tests/roms/` (non fournie : test ignoré sinon).

use trs80::{Model, Trs80};

fn local(name: &str) -> Option<Vec<u8>> {
    std::fs::read(format!("{}/tests/roms/{name}", env!("CARGO_MANIFEST_DIR"))).ok()
}

fn published(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/../../www/disks/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn screen(m: &Trs80) -> String {
    let mode = m.text_mode();
    (0..mode.rows)
        .map(|r| (0..mode.cols).map(|c| m.char_at(r, c)).collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn shows(m: &Trs80, text: &str) -> bool {
    screen(m).to_uppercase().contains(&text.to_uppercase())
}

fn wait_for(m: &mut Trs80, text: &str, seconds: usize) -> bool {
    for _ in 0..seconds * 60 {
        if shows(m, text) {
            return true;
        }
        m.run_frame();
    }
    shows(m, text)
}

fn type_line(m: &mut Trs80, text: &str) {
    (0..120).for_each(|_| m.run_frame());
    m.type_text(text);
    m.type_text("\n");
    for _ in 0..3000 {
        if !m.typing() {
            break;
        }
        m.run_frame();
    }
}

fn answer(m: &mut Trs80, prompt: &str, answer: &str) {
    assert!(wait_for(m, prompt, 30), "invite « {prompt} » absente. Écran :\n{}", screen(m));
    type_line(m, answer);
}

/// LDOS démarré, pilote série installé par `set`, puis LCOMM : la frappe part sur la ligne,
/// la ligne s'affiche.
fn lcomm_talks(m: &mut Trs80, set: &str) {
    answer(m, "DATE", "10/08/91");
    answer(m, "TIME", "12:00:00");
    assert!(wait_for(m, "READY", 10), "Écran :\n{}", screen(m));
    // LCOMM exige le pilote de clavier de LDOS.
    type_line(m, "SET *KI KI");
    type_line(m, set);
    // L'installation du pilote se termine après l'invite : une frappe trop tôt serait perdue.
    (0..240).for_each(|_| m.run_frame());
    type_line(m, "LCOMM *CL");
    assert!(wait_for(m, "CLEAR-8", 10), "LCOMM ne démarre pas. Écran :\n{}", screen(m));
    m.serial_take();
    type_line(m, "ATDT BBS");
    (0..120).for_each(|_| m.run_frame());
    let sent = m.serial_take();
    assert!(sent.windows(8).any(|w| w.eq_ignore_ascii_case(b"ATDT BBS")), "émis : {sent:02X?}");
    m.serial_send(b"\r\nCONNECT 2400\r\nWELCOME TO THE BBS\r\n");
    (0..300).for_each(|_| m.run_frame());
    let s = screen(m);
    assert!(s.contains("CONNECT 2400") && s.contains("WELCOME TO THE BBS"), "Écran :\n{s}");
    assert_eq!(m.serial_pending(), 0);
}

#[test]
fn model1_lcomm_over_rs232() {
    let Some(rom) = local("M1L2_1.3.bin").or_else(|| local("level2.rom")) else { return };
    let mut m = Trs80::new(&rom).unwrap();
    m.insert_disk(0, published("ldos-531-dd.dsk")).unwrap();
    // RS232R (Model I) de LDOS 5.3.1 : sans caractère BREAK (BREAK=OFF, par défaut), sa
    // lecture rend chaque octet reçu avec l'indicateur « rien reçu », et LCOMM l'ignore. Un
    // caractère BREAK qui n'arrive jamais (255) contourne ce défaut.
    lcomm_talks(&mut m, "SET *CL RS232R (BAUD=2400,WORD=8,PARITY=OFF,BREAK=255)");
}

#[test]
fn model3_lcomm_over_rs232() {
    let Some(rom) = local("M3_REVC.bin") else { return };
    let mut m = Trs80::with_model(&rom, Model::III).unwrap();
    m.insert_disk(0, published("ldos-531-m3.dsk")).unwrap();
    // RS232T : réception par interruption (bit 5 du port E0h).
    lcomm_talks(&mut m, "SET *CL RS232T (BAUD=2400,WORD=8,PARITY=OFF)");
}

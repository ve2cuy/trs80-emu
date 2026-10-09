//! Disque dur Radio Shack (WD1010, ports C0h-CFh) avec les pilotes RSHARD de MISOSYS :
//! installation du pilote, formatage (RSFORM), copie d'un fichier, puis l'image enregistrée
//! est rebranchée sur une machine neuve et relue.
//!
//! Exige `rshard.dsk` (archive rshard.zip de la page MISOSYS de Tim Mann) et les ROM dans
//! `crates/trs80/tests/roms/` (non fournis : test ignoré sinon). Model 4 : la disquette
//! TRSDOS 6.2.1 `m4-trsdos621.dsk` en plus; Model III : LDOS 5.3.1 `m3-ldos531.dsk`
//! (ld3-531.zip, même page).

use trs80::{HardDisk, Model, Trs80};

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

/// L'écran contient-il `text` (sans tenir compte de la casse : majuscules du Model I) ?
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

/// Attend l'invite du DOS sur la dernière ligne (pas une ancienne, plus haut à l'écran).
fn wait_ready(m: &mut Trs80, seconds: usize) -> bool {
    let ready = |m: &Trs80| {
        let s = screen(m).to_uppercase();
        let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
        match lines.as_slice() {
            [.., prompt, cursor] if cursor.trim().chars().count() <= 1 => prompt.ends_with("READY"),
            [.., prompt] => prompt.ends_with("READY"),
            [] => false,
        }
    };
    for _ in 0..seconds * 60 {
        if ready(m) {
            return true;
        }
        m.run_frame();
    }
    ready(m)
}

fn type_line(m: &mut Trs80, text: &str) {
    // Une touche frappée pendant que le DOS finit d'afficher son invite serait perdue.
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

/// Répond à une invite : attend `prompt`, puis tape `answer` et ENTRÉE.
fn answer(m: &mut Trs80, prompt: &str, answer: &str) {
    assert!(wait_for(m, prompt, 30), "invite « {prompt} » absente. Écran :\n{}", screen(m));
    type_line(m, answer);
}

/// Installe le pilote RSHARD`v` sur le lecteur 2 : 306 cylindres, 4 têtes (les valeurs
/// proposées), une seule partition.
fn install_driver(m: &mut Trs80, v: u8) {
    type_line(m, &format!("SYSTEM (DRIVE=2,DISABLE,DRIVER=\"RSHARD{v}\")"));
    answer(m, "DRIVE ADDRESS", "");
    answer(m, "STEP RATE", "");
    answer(m, "TRACKS", "");
    answer(m, "TOTAL NUMBER OF HEADS", "");
    answer(m, "PARTITION'S NUMBER OF HEADS", "4");
    answer(m, "CYLINDERS", "");
    assert!(wait_ready(m, 10), "pilote non installé. Écran :\n{}", screen(m));
}

/// Formate le disque dur (lecteur 2), y copie le pilote, et vérifie le répertoire.
fn format_and_copy(m: &mut Trs80, v: u8) {
    type_line(m, &format!("RSFORM{v} :2 (NAME=\"RIGID1\",MPW=\"SECRET\")"));
    answer(m, "FORMAT IT", "Y");
    answer(m, "LOCK OUT TRACK", "N");
    assert!(wait_ready(m, 300), "formatage inachevé. Écran :\n{}", screen(m));
    assert!(!shows(m, "NOT FOUND") && !shows(m, "ERROR"), "Écran :\n{}", screen(m));
    type_line(m, &format!("COPY RSHARD{v}/DCT:1 :2"));
    assert!(wait_ready(m, 30), "Écran :\n{}", screen(m));
    check_dir(m, v);
}

fn check_dir(m: &mut Trs80, v: u8) {
    type_line(m, "DIR :2");
    assert!(wait_ready(m, 10), "Écran :\n{}", screen(m));
    let s = screen(m).to_uppercase();
    assert!(s.contains("RIGID1") && s.contains(&format!("RSHARD{v}/DCT")), "DIR :2. Écran :\n{s}");
}

#[test]
fn model1_ldos_formats_and_uses_a_hard_disk() {
    let (Some(rom), Some(rshard)) = (local("M1L2_1.3.bin").or_else(|| local("level2.rom")), local("rshard.dsk")) else {
        return;
    };
    let boot = |hard: Vec<u8>| {
        let mut m = Trs80::new(&rom).unwrap();
        m.insert_disk(0, published("ldos-531-dd.dsk")).unwrap();
        m.insert_disk(1, rshard.clone()).unwrap();
        m.insert_hard_disk(0, hard).unwrap();
        answer(&mut m, "DATE", "10/08/91");
        answer(&mut m, "TIME", "12:00:00");
        assert!(wait_ready(&mut m, 10), "pas d'invite LDOS. Écran :\n{}", screen(&m));
        install_driver(&mut m, 5);
        m
    };
    let mut m = boot(HardDisk::blank(306, 4));
    assert!(shows(&m, "UNFORMATTED"), "Écran :\n{}", screen(&m));
    format_and_copy(&mut m, 5);
    let disk = m.hard_disk(0).unwrap();
    assert!(disk.modified());
    // L'image enregistrée, rebranchée sur une machine neuve : même contenu.
    let image = disk.image().to_vec();
    let mut m = boot(image);
    assert!(!shows(&m, "UNFORMATTED"), "Écran :\n{}", screen(&m));
    check_dir(&mut m, 5);
}

#[test]
fn model4_trsdos6_formats_and_uses_a_hard_disk() {
    let (Some(rom), Some(dos), Some(rshard)) = (local("M3_REVC.bin"), local("m4-trsdos621.dsk"), local("rshard.dsk"))
    else {
        return;
    };
    let mut m = Trs80::with_model(&rom, Model::IV).unwrap();
    m.insert_disk(0, dos).unwrap();
    m.insert_disk(1, rshard).unwrap();
    m.insert_hard_disk(0, HardDisk::blank(306, 4)).unwrap();
    assert!(wait_for(&mut m, "TRSDOS Ready", 15), "Écran :\n{}", screen(&m));
    (0..300).for_each(|_| m.run_frame());
    install_driver(&mut m, 6);
    format_and_copy(&mut m, 6);
    eprintln!("{}", screen(&m));
}

#[test]
fn model3_ldos_formats_and_uses_a_hard_disk() {
    let (Some(rom), Some(dos), Some(rshard)) = (local("M3_REVC.bin"), local("m3-ldos531.dsk"), local("rshard.dsk"))
    else {
        return;
    };
    let mut m = Trs80::with_model(&rom, Model::III).unwrap();
    m.insert_disk(0, dos).unwrap();
    m.insert_disk(1, rshard).unwrap();
    m.insert_hard_disk(0, HardDisk::blank(306, 4)).unwrap();
    answer(&mut m, "DATE", "10/08/91");
    answer(&mut m, "TIME", "12:00:00");
    assert!(wait_ready(&mut m, 10), "pas d'invite LDOS. Écran :
{}", screen(&m));
    install_driver(&mut m, 5);
    format_and_copy(&mut m, 5);
}

#[test]
fn model2_has_no_hard_disk_yet() {
    let Some(rom) = local("m2_boot_v5.bin") else { return };
    let mut m = Trs80::new(&rom).unwrap();
    assert!(m.insert_hard_disk(0, HardDisk::blank(306, 4)).is_err());
}

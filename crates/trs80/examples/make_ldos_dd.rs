//! Fabrique une disquette système LDOS 5.3.1 en double densité, avec LDOS lui-même,
//! selon la marche à suivre du manuel (commande SOLE) :
//!
//! 1. FDUBL (TANDY)               pilote du doubleur Radio Shack
//! 2. FORMAT :1 (SYSTEM)          disquette double densité, piste 0 en simple densité
//! 3. BACKUP :0 :1 (SYS,INV)      copie du système
//! 4. SYSTEM (SYSGEN,DRIVE=1)     configuration (pilote compris)
//! 5. SOLE :1                     pilote d'amorçage double densité
//! 6. BACKUP :2 :1 (INV)          fichiers de la deuxième disquette LDOS (facultatif)
//!
//!     cargo run --release -p trs80 --example make_ldos_dd -- <rom> <ldos-sd.dsk> <sortie.dsk> [ldos-disque2.dsk]
//!
//! Chaque étape attend une invite à l'écran; l'écran est affiché pour suivre le déroulement.

use trs80::Trs80;

fn screen(m: &Trs80) -> String {
    (0..16)
        .map(|r| (0..64).map(|c| m.char_at(r, c)).collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Dernière ligne non vide de l'écran (l'invite en cours), sans le curseur « _ ».
fn last_line(m: &Trs80) -> String {
    screen(m)
        .lines()
        .map(|l| l.trim_end_matches(['_', ' ']))
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .to_string()
}

fn type_and_wait(m: &mut Trs80, text: &str) {
    m.type_text(text);
    while m.typing() {
        m.run_frame();
    }
}

/// Attend que la dernière ligne contienne `text` (au plus `seconds` secondes).
fn wait_last(m: &mut Trs80, text: &str, seconds: usize) -> bool {
    for _ in 0..seconds * 60 {
        if last_line(m).contains(text) {
            return true;
        }
        m.run_frame();
    }
    false
}

/// Tape une commande LDOS et répond aux invites jusqu'au retour de « LDOS READY ».
fn command(m: &mut Trs80, cmd: &str, answers: &[(&str, &str)], seconds: usize) {
    println!("\n>>> {cmd}");
    type_and_wait(m, &format!("{cmd}\n"));
    let mut answered = vec![false; answers.len()];
    for _ in 0..seconds * 60 {
        let last = last_line(m);
        if last.contains("READY") && !last.contains(cmd) {
            break;
        }
        if let Some(i) = (0..answers.len()).find(|&i| !answered[i] && last.contains(answers[i].0)) {
            println!("    « {} » -> {:?}", last.trim(), answers[i].1);
            type_and_wait(m, answers[i].1);
            answered[i] = true;
            continue;
        }
        m.run_frame();
    }
    println!("{}", screen(m));
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [rom, ldos, out] = [&args[0], &args[1], &args[2]];
    let disk2 = args.get(3);
    let mut m = Trs80::new(&std::fs::read(rom).unwrap()).unwrap();
    m.insert_disk(0, std::fs::read(ldos).unwrap()).unwrap();

    assert!(wait_last(&mut m, "DATE", 20), "LDOS ne démarre pas :\n{}", screen(&m));
    type_and_wait(&mut m, "10/08/91\n");
    assert!(wait_last(&mut m, "TIME", 5));
    type_and_wait(&mut m, "12:00:00\n");
    assert!(wait_last(&mut m, "READY", 10));

    command(&mut m, "FDUBL (TANDY)", &[], 10);

    m.insert_blank_disk(1);
    command(
        &mut m,
        "FORMAT :1 (SYSTEM)",
        &[
            ("NAME", "LDOS531\n"),
            ("PASSWORD", "\n"),
            ("DENSITY", "D
"),
            ("CYLINDERS", "40\n"),
            ("SIDES", "1\n"),
            ("STEP", "\n"),
        ],
        120,
    );
    command(&mut m, "BACKUP :0 :1 (SYS,INV)", &[], 300);
    command(&mut m, "SYSTEM (SYSGEN,DRIVE=1)", &[], 30);
    command(&mut m, "SOLE :1", &[], 30);
    if let Some(d2) = disk2 {
        m.insert_disk(2, std::fs::read(d2).unwrap()).unwrap();
        command(&mut m, "BACKUP :2 :1 (INV)", &[], 300);
    }
    command(&mut m, "DIR :1", &[], 20);

    let disk = m.disk(1).unwrap();
    std::fs::write(out, disk.image()).unwrap();
    println!("\n{out} : {} secteurs, format {}", disk.sector_count(), disk.image_format().name());
}

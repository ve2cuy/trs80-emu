//! Démarre une ROM, tape éventuellement une ligne, puis affiche l'écran en texte.
//! Pratique pour vérifier une ROM ou un programme sans fureteur.
//!
//!     cargo run -p trs80 --example screen -- <rom> [secondes] [texte à taper] [programme.cmd]
//!
//! Exemple : cargo run -p trs80 --example screen -- level2.rom 3 "PRINT 2+2"

use trs80::{Key, Trs80};

fn tap(m: &mut Trs80, name: &str) {
    let key = Key::from_name(name).unwrap_or_else(|| panic!("touche inconnue : {name}"));
    m.key_down(key);
    (0..3).for_each(|_| m.run_frame());
    m.key_up(key);
    (0..3).for_each(|_| m.run_frame());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rom = std::fs::read(args.first().expect("chemin de la ROM")).expect("lecture de la ROM");
    let seconds: usize = args.get(1).map_or(2, |s| s.parse().expect("secondes"));
    let mut m = Trs80::new(&rom).unwrap_or_else(|e| panic!("{e}"));

    let mut answered = false;
    for _ in 0..seconds * 60 {
        m.run_frame();
        // Répond une seule fois à l'invite de taille mémoire (elle reste affichée ensuite).
        if !answered && m.screen_contains("SIZE?") {
            tap(&mut m, "Enter");
            answered = true;
        }
    }
    if let Some(text) = args.get(2).filter(|t| !t.is_empty()) {
        text.chars().for_each(|c| tap(&mut m, &c.to_string()));
        tap(&mut m, "Enter");
        (0..60).for_each(|_| m.run_frame());
    }
    if let Some(cmd) = args.get(3) {
        let entry = m.load_cmd(&std::fs::read(cmd).expect("lecture du programme")).unwrap();
        println!("programme lancé en {entry:04X}h");
        (0..300).for_each(|_| m.run_frame());
    }

    println!("+{}+", "-".repeat(64));
    for row in 0..16 {
        let line: String = (0..64).map(|c| m.char_at(row, c)).collect();
        println!("|{line}|");
    }
    println!("+{}+", "-".repeat(64));
    println!("PC = {:04X}h, {} T-states", m.cpu().pc, m.cpu().cycles);
}

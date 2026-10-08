//! Suites de validation ZEXDOC et ZEXALL (Frank Cringle, GPL).
//!
//! Ce sont des programmes CP/M : ils comparent une empreinte (CRC) des résultats de
//! millions de combinaisons d'instructions à celle d'un vrai Z80. ZEXDOC ne vérifie
//! que les indicateurs documentés; ZEXALL vérifie aussi X et Y.
//!
//! Longs à exécuter : à lancer en mode release.
//!     cargo test --release -p z80 --test zex -- --ignored --nocapture

use std::io::Write;
use z80::{Bus, Cpu};

struct Cpm {
    mem: Vec<u8>,
}

impl Bus for Cpm {
    fn read(&mut self, addr: u16) -> u8 {
        self.mem[addr as usize]
    }
    fn write(&mut self, addr: u16, val: u8) {
        self.mem[addr as usize] = val;
    }
}

fn run_zex(name: &str) {
    let path = format!("{}/tests/zex/{name}", env!("CARGO_MANIFEST_DIR"));
    let program = std::fs::read(&path).unwrap_or_else(|e| panic!("{path} : {e}"));

    let mut bus = Cpm { mem: vec![0; 0x10000] };
    bus.mem[0x100..0x100 + program.len()].copy_from_slice(&program);
    bus.mem[0x0000] = 0x76; // Retour au système (JP 0) : HALT
    bus.mem[0x0005] = 0xC9; // Point d'entrée BDOS : RET (les appels sont interceptés)
    bus.mem[0x0006] = 0x00; // Le programme lit (0006h) pour placer sa pile
    bus.mem[0x0007] = 0xF0;

    let mut cpu = Cpu::new();
    cpu.pc = 0x100;
    let mut output = String::new();
    let mut stdout = std::io::stdout();

    loop {
        if cpu.pc == 0x0005 {
            // Appels BDOS : 2 = afficher le caractère E, 9 = afficher la chaîne DE jusqu'à '$'.
            let mut text = String::new();
            match cpu.c {
                2 => text.push(cpu.e as char),
                9 => {
                    let mut addr = cpu.de() as usize;
                    while bus.mem[addr] != b'$' {
                        text.push(bus.mem[addr] as char);
                        addr += 1;
                    }
                }
                _ => {}
            }
            print!("{text}");
            stdout.flush().ok();
            output.push_str(&text);
        }
        cpu.step(&mut bus);
        if cpu.halted {
            break;
        }
    }

    assert!(output.contains("Tests complete"), "{name} ne s'est pas terminé :\n{output}");
    assert!(!output.contains("ERROR"), "{name} a détecté des erreurs :\n{output}");
}

#[test]
#[ignore = "long : cargo test --release -- --ignored"]
fn zexdoc() {
    run_zex("zexdoc.com");
}

#[test]
#[ignore = "long : cargo test --release -- --ignored"]
fn zexall() {
    run_zex("zexall.com");
}

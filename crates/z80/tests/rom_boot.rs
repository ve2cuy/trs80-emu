//! Démarrage de la ROM Level II du TRS-80 Model 1 jusqu'à l'invite de taille mémoire :
//! « MEM SIZE? » (ROM 1.3) ou « MEMORY SIZE? » (versions antérieures).
//!
//! La ROM est protégée par le droit d'auteur et n'est pas dans le dépôt : placez votre
//! copie dans `tests/roms/` (voir `tests/roms/README.md`). Sans elle, le test est ignoré.

use z80::{Bus, Cpu};

const ROM_NAMES: [&str; 2] = ["M1L2_1.3.bin", "level2.rom"];

/// Machine minimale : ROM, clavier sans touche enfoncée, vidéo et 16 Ko de RAM.
/// Sans interface d'expansion, les adresses 37E0-37FF lisent FFh (bus flottant).
struct MiniTrs80 {
    rom: Vec<u8>,
    ram: Vec<u8>,
}

impl Bus for MiniTrs80 {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x2FFF => self.rom[addr as usize],
            0x3800..=0x3BFF => 0x00,
            0x3C00..=0x7FFF => self.ram[addr as usize],
            _ => 0xFF,
        }
    }
    fn write(&mut self, addr: u16, val: u8) {
        if let 0x3C00..=0x7FFF = addr {
            self.ram[addr as usize] = val;
        }
    }
}

fn find_rom() -> Option<Vec<u8>> {
    ROM_NAMES.iter().find_map(|name| {
        std::fs::read(format!("{}/tests/roms/{name}", env!("CARGO_MANIFEST_DIR"))).ok()
    })
}

/// Caractère affiché pour un octet de la mémoire vidéo du Model 1 (sans modification minuscules) :
/// la mémoire n'a pas de bit 6, les codes 00h-1Fh s'affichent donc comme les majuscules 40h-5Fh;
/// 80h-FFh sont des blocs semi-graphiques.
fn glyph(b: u8) -> char {
    match b {
        0x00..=0x1F => (b + 0x40) as char,
        0x20..=0x7F => b as char,
        _ => ' ',
    }
}

/// Texte affiché : 16 lignes de 64 caractères (mémoire vidéo 3C00-3FFF).
fn screen(bus: &MiniTrs80) -> Vec<String> {
    bus.ram[0x3C00..0x4000]
        .chunks(64)
        .map(|line| line.iter().map(|&b| glyph(b)).collect::<String>().trim_end().to_string())
        .collect()
}

#[test]
fn level2_rom_reaches_memory_size_prompt() {
    let Some(rom) = find_rom() else {
        eprintln!("ROM Level II absente de tests/roms/ : test ignoré");
        return;
    };
    assert_eq!(rom.len(), 0x3000, "la ROM Level II fait 12 Ko");

    let mut bus = MiniTrs80 { rom, ram: vec![0; 0x10000] };
    let mut cpu = Cpu::new();

    // Deux secondes de temps machine (1,774 MHz) suffisent largement.
    while cpu.cycles < 2 * 1_774_000 {
        for _ in 0..10_000 {
            cpu.step(&mut bus);
        }
        if screen(&bus).iter().any(|l| l.contains("MEM SIZE?") || l.contains("MEMORY SIZE?")) {
            return;
        }
    }
    panic!(
        "Invite de taille mémoire jamais affichée. Écran :\n{}\nPremière ligne (hex) : {:02X?}",
        screen(&bus).join("\n"),
        &bus.ram[0x3C00..0x3C20]
    );
}

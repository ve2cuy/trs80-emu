//! Comparaison avec zmac, l'assembleur de référence :
//! - `data/all.asm` : toutes les formes documentées des instructions; `all.zmac.cmd` a été
//!   produit par zmac à partir du même source.
//! - `asm/invaders.asm` : VE2CUY Invaders, dont le .CMD publié vient de zmac.

fn compare(name: &str, source: &str, expected: &[u8]) {
    let asm = z80asm::assemble(source);
    let errors: Vec<_> = asm.diagnostics.iter().filter(|d| !d.warning).collect();
    assert!(errors.is_empty(), "{name} : erreurs {errors:#?}");
    let ours = asm.cmd();
    if ours != expected {
        let at = ours.iter().zip(expected).position(|(a, b)| a != b).unwrap_or(ours.len().min(expected.len()));
        // Ligne du source qui a produit l'octet fautif (position approximative dans le .CMD).
        panic!(
            "{name} : différent à l'octet {at} ({} contre {} octets). Ici : {:02X?} / zmac : {:02X?}",
            ours.len(),
            expected.len(),
            &ours[at.saturating_sub(4)..(at + 8).min(ours.len())],
            &expected[at.saturating_sub(4)..(at + 8).min(expected.len())]
        );
    }
}

#[test]
fn every_instruction_matches_zmac() {
    let dir = format!("{}/tests/data", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(format!("{dir}/all.asm")).unwrap();
    let expected = std::fs::read(format!("{dir}/all.zmac.cmd")).unwrap();
    compare("all.asm", &source, &expected);
}

#[test]
fn invaders_matches_zmac() {
    let root = format!("{}/../..", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(format!("{root}/asm/invaders.asm")).unwrap();
    let expected = std::fs::read(format!("{root}/www/programs/ve2cuy-invaders.cmd")).unwrap();
    compare("invaders.asm", &source, &expected);
}

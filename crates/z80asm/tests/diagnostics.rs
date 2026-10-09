//! Messages d'erreur et corrections proposées; syntaxes acceptées.

use z80asm::{Diagnostic, assemble};

fn diags(src: &str) -> Vec<Diagnostic> {
    assemble(src).diagnostics
}

fn only(src: &str) -> Diagnostic {
    let d = diags(src);
    assert_eq!(d.len(), 1, "{src} : {d:#?}");
    d.into_iter().next().unwrap()
}

const ORG: &str = "        ORG     5200H\n";

#[test]
fn unknown_mnemonic_suggests_closest() {
    let d = only(&format!("{ORG}        PSH     BC\n"));
    assert_eq!(d.code, "unknownMnemonic");
    assert_eq!(d.args, ["PSH", "PUSH"]);
    assert_eq!(d.fix.as_deref(), Some("        PUSH     BC"));
}

#[test]
fn hex_number_needs_leading_digit() {
    let d = only(&format!("{ORG}        LD      A,FFH\n"));
    assert_eq!(d.code, "hexDigit");
    assert_eq!(d.fix.as_deref(), Some("        LD      A,0FFH"));
}

#[test]
fn misspelled_symbol_suggests_existing_one() {
    let d = only(&format!("{ORG}START:  JP      STRAT\n"));
    assert_eq!(d.code, "undefinedSymbol");
    assert_eq!(d.args, ["STRAT", "START"]);
    assert_eq!(d.fix.as_deref(), Some("START:  JP      START"));
}

#[test]
fn bad_operands_list_valid_forms() {
    let d = only(&format!("{ORG}        LD      (HL),(HL)\n"));
    assert_eq!(d.code, "badOperands");
    assert!(d.args[1].contains("LD r,(HL)"), "{:?}", d.args);
}

#[test]
fn jr_too_far_suggests_jp() {
    let d = only(&format!("{ORG}START:  JR      FAR\n        DS      200\nFAR:    RET\n"));
    assert_eq!(d.code, "jrRange");
    assert_eq!(d.fix.as_deref(), Some("START:  JP      FAR"));
    let d = only(&format!("{ORG}        JR      PE,$\n"));
    assert_eq!(d.code, "jrCondition");
    assert_eq!(d.fix.as_deref(), Some("        JP      PE,$"));
}

#[test]
fn byte_range_and_duplicate_label() {
    assert_eq!(only(&format!("{ORG}        LD      A,300\n")).code, "byteRange");
    let d = only(&format!("{ORG}X:      NOP\nX:      NOP\n"));
    assert_eq!((d.code, d.line, d.args[1].as_str()), ("duplicateSymbol", 3, "2"));
}

#[test]
fn indented_label_and_column_one_instruction() {
    // (JP LOOP signale aussi LOOP indéfini; la correction règle les deux.)
    let d = diags(&format!("{ORG}        LOOP\n        JP      LOOP\n")).remove(0);
    assert_eq!(d.code, "labelNeedsColumn");
    assert_eq!(d.fix.as_deref(), Some("        LOOP:"));
    let d = only(&format!("{ORG}LD      A,1\n"));
    assert!(d.warning && d.code == "labelColumn");
    assert_eq!(assemble(&format!("{ORG}LD      A,1\n")).blocks[0].1, [0x3E, 1]);
}

#[test]
fn missing_org_is_a_warning_with_a_fix() {
    let d = only("        RET\n");
    assert!(d.warning && d.code == "noOrg" && d.insert);
    assert_eq!(assemble("        RET\n").entry, 0x5200);
}

#[test]
fn model3_address_of_ldos_service_is_flagged() {
    let d = only(&format!("@TIME   EQU     3036H\n{ORG}        CALL    @TIME\n"));
    assert!(d.warning && d.code == "model3Address");
    assert_eq!(d.fix.as_deref(), Some("@TIME   EQU     446DH"));
}

#[test]
fn ldos_services_are_predefined() {
    let asm = assemble(&format!("{ORG}        LD      HL,MSG\n        CALL    @DSPLY\n        JP      @EXIT\nMSG:    DB      'HI',0DH\n"));
    assert!(asm.ok(), "{:#?}", asm.diagnostics);
    assert_eq!(&asm.blocks[0].1[3..6], [0xCD, 0x67, 0x44]);
    assert!(asm.uses_ldos());
    assert_eq!(asm.builtins_used, ["@DSPLY", "@EXIT"]);
    // Une définition du programme a priorité.
    let asm = assemble(&format!("@DSPLY  EQU     1234H\n{ORG}        CALL    @DSPLY\n"));
    assert_eq!(asm.blocks[0].1, [0xCD, 0x34, 0x12]);
    assert!(asm.builtins_used.is_empty());
}

#[test]
fn number_syntaxes_and_expressions() {
    let asm = assemble(&format!(
        "{ORG}        DB      0FFH,0xFF,$FF,255,%11111111,11111111B,377O,'A',\"B\",-1\n\
         \x20       DW      HIGH(1234H),LOW(1234H),3*(2+1),100/7,100 MOD 7,1 SHL 4,0F0H AND 3CH,~0\n\
         V       =       7\n\
         \x20       DB      V,V OR 8\n"
    ));
    assert!(asm.ok(), "{:#?}", asm.diagnostics);
    assert_eq!(asm.blocks[0].1[..10], [0xFF, 0xFF, 0xFF, 255, 0xFF, 0xFF, 0xFF, b'A', b'B', 0xFF]);
    let words: Vec<u16> = asm.blocks[0].1[10..26].chunks(2).map(|w| u16::from_le_bytes([w[0], w[1]])).collect();
    assert_eq!(words, [0x12, 0x34, 9, 14, 2, 16, 0x30, 0xFFFF]);
    assert_eq!(asm.blocks[0].1[26..], [7, 15]);
}

#[test]
fn comments_strings_and_exx_quote() {
    let asm = assemble(&format!(
        "* commentaire EDTASM\n{ORG}        EX      AF,AF'  ; l'apostrophe de AF' n'ouvre pas de chaîne\n\
         \x20       DB      'A;B',\"C'D\"\n"
    ));
    assert!(asm.ok(), "{:#?}", asm.diagnostics);
    assert_eq!(asm.blocks[0].1, [0x08, b'A', b';', b'B', b'C', b'\'', b'D']);
}

#[test]
fn cmd_records_and_line_addresses() {
    let asm = assemble(&format!("{ORG}START:  NOP\n        DS      2\n        RET\n        END     START\n"));
    assert_eq!(asm.cmd(), [1, 3, 0x00, 0x52, 0x00, 1, 3, 0x03, 0x52, 0xC9, 2, 2, 0x00, 0x52]);
    assert_eq!((asm.lines[2].addr, asm.lines[2].len), (0x5201, 0)); // DS : réservé, rien de produit
    assert_eq!((asm.lines[3].addr, asm.lines[3].len), (0x5203, 1));
    assert_eq!(asm.bounds(), Some((0x5200, 0x5203)));
}

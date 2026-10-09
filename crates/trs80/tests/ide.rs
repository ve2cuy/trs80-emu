//! Atelier d'assemblage : fichiers LDOS (lecture, écriture, vérifiées par LDOS lui-même),
//! programmes assemblés qui appellent les services de LDOS, débogueur.
//!
//! Exige la ROM Level II dans `crates/trs80/tests/roms/` (non fournie); sinon les tests qui
//! démarrent la machine sont ignorés.

use trs80::{FsError, Stop, Trs80};

fn load_rom() -> Option<Vec<u8>> {
    let dir = format!("{}/tests/roms", env!("CARGO_MANIFEST_DIR"));
    ["M1L2_1.3.bin", "level2.rom"].iter().find_map(|name| std::fs::read(format!("{dir}/{name}")).ok())
}

fn disk(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/../../www/disks/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn screen(m: &Trs80) -> String {
    (0..16)
        .map(|r| (0..64).map(|c| m.char_at(r, c)).collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn wait_for(m: &mut Trs80, text: &str, seconds: usize) -> bool {
    for _ in 0..seconds * 60 {
        if m.screen_contains(text) {
            return true;
        }
        m.run_frame();
    }
    m.screen_contains(text)
}

fn type_line(m: &mut Trs80, text: &str) {
    m.type_text(text);
    m.type_text("\n");
    for _ in 0..2000 {
        if !m.typing() {
            break;
        }
        m.run_frame();
    }
}

/// LDOS démarré (date et heure entrées) sur la disquette `name`, à l'invite.
fn ldos(rom: &[u8], name: &str) -> Trs80 {
    let mut m = Trs80::new(rom).unwrap();
    m.insert_disk(0, disk(name)).unwrap();
    assert!(wait_for(&mut m, "DATE", 20), "LDOS n'a pas démarré. Écran :\n{}", screen(&m));
    type_line(&mut m, "10/08/91");
    type_line(&mut m, "12:00:00");
    assert!(wait_for(&mut m, "READY", 10), "pas d'invite LDOS. Écran :\n{}", screen(&m));
    m
}

/// Assemble, place en mémoire et lance le programme comme le ferait le système.
fn run(m: &mut Trs80, source: &str, frames: usize) -> z80asm::Assembly {
    let asm = z80asm::assemble(source);
    assert!(asm.ok(), "{:#?}", asm.diagnostics);
    for (addr, bytes) in &asm.blocks {
        m.poke(*addr, bytes);
    }
    m.launch(asm.entry).unwrap();
    (0..frames).for_each(|_| m.run_frame());
    asm
}

#[test]
fn reads_ldos_files_without_a_rom() {
    let mut m = Trs80::new(&[0u8; 12288]).unwrap();
    m.insert_disk(0, disk("ldos-531.dsk")).unwrap();
    let files = m.disk_files(0).unwrap();
    let format = files.iter().find(|f| f.name == "FORMAT/CMD").expect("FORMAT/CMD");
    assert_eq!(format.size, 4989);
    assert!(files.iter().any(|f| f.name == "SYS0/SYS" && f.system));
    let data = m.read_disk_file(0, "format/cmd").unwrap();
    assert_eq!(data.len(), 4989);
    assert_eq!(&data[data.len() - 4..data.len() - 2], [0x02, 0x02], "transfert à la fin d'un .CMD");
    assert_eq!(m.read_disk_file(0, "NOPE/CMD"), Err(FsError::NoSuchFile));
    assert_eq!(m.read_disk_file(0, "1BAD/CMD"), Err(FsError::BadName));
    // La disquette système en simple densité est pleine.
    assert_eq!(m.disk_free(0), Ok(0));
    assert_eq!(m.write_disk_file(0, "HELLO/ASM", b"X"), Err(FsError::DiskFull));
    assert_eq!(m.disk_files(1), Err(FsError::NoDisk));
}

#[test]
fn written_file_is_read_back_and_listed_by_ldos() {
    let Some(rom) = load_rom() else { return };
    let mut m = Trs80::new(&rom).unwrap();
    m.insert_disk(0, disk("ldos-531-dd.dsk")).unwrap();
    let free = m.disk_free(0).unwrap();
    assert!(free > 10_000, "espace libre : {free}");
    // Un source de 700 octets : 3 secteurs, dont le dernier partiel.
    let mut text = String::new();
    for i in 0..35 {
        text.push_str(&format!("; LIGNE {i:02} DU FICHIER DE TEST\r"));
    }
    m.write_disk_file(0, "TEST/ASM", text.as_bytes()).unwrap();
    assert_eq!(m.read_disk_file(0, "TEST/ASM").unwrap(), text.as_bytes());
    assert_eq!(m.disk_free(0).unwrap(), free - 6 * 256, "un granule de 6 secteurs");
    // Remplacement : l'ancien granule est libéré.
    m.write_disk_file(0, "TEST/ASM", b"; COURT\r").unwrap();
    assert_eq!(m.read_disk_file(0, "TEST/ASM").unwrap(), b"; COURT\r");
    assert_eq!(m.disk_free(0).unwrap(), free - 6 * 256);
    m.write_disk_file(0, "TEST/ASM", text.as_bytes()).unwrap();

    // LDOS lui-même voit le fichier et le lit.
    assert!(wait_for(&mut m, "DATE", 20), "LDOS n'a pas démarré. Écran :\n{}", screen(&m));
    type_line(&mut m, "10/08/91");
    type_line(&mut m, "12:00:00");
    assert!(wait_for(&mut m, "READY", 10));
    type_line(&mut m, "LIST TEST/ASM");
    (0..200).for_each(|_| m.run_frame());
    let s = screen(&m);
    // (LIST s'arrête à la fin de l'écran.)
    assert!(s.contains("; LIGNE 00 DU FICHIER DE TEST") && s.contains("; LIGNE 05 DU FICHIER DE TEST"), "LIST TEST/ASM. Écran :\n{s}");
}

const SERVICES: &str = "\
        ORG     5200H
START:  LD      HL,MSG
        CALL    @DSPLY
        LD      HL,BUFFER
        CALL    @TIME
        LD      HL,BUFFER
        CALL    @DSPLY
        LD      HL,BUFFER
        CALL    @DATE
        LD      HL,BUFFER
        CALL    @DSPLY
        JP      @EXIT
MSG:    DB      'HELLO FROM Z80',0DH
BUFFER: DS      8
        DB      0DH
        END     START
";

#[test]
fn program_uses_ldos_services() {
    let Some(rom) = load_rom() else { return };
    let mut m = ldos(&rom, "ldos-531.dsk");
    type_line(&mut m, "CLS");
    (0..30).for_each(|_| m.run_frame());
    run(&mut m, SERVICES, 120);
    let s = screen(&m);
    assert!(s.contains("HELLO FROM Z80"), "@DSPLY. Écran :\n{s}");
    assert!(s.contains("12:00:"), "@TIME. Écran :\n{s}");
    assert!(s.contains("10/08/91"), "@DATE. Écran :\n{s}");
    assert!(s.lines().filter(|l| l.contains("READY")).count() >= 1, "@EXIT : retour à LDOS. Écran :\n{s}");
}

#[test]
fn program_writes_a_file_with_ldos_services() {
    let Some(rom) = load_rom() else { return };
    let mut m = ldos(&rom, "ldos-531-dd.dsk");
    let source = "\
        ORG     5200H
START:  LD      HL,NAME
        LD      DE,FCB
        CALL    @FSPEC
        LD      HL,BUFFER
        LD      DE,FCB
        LD      B,0             ; enregistrements de 256 octets
        CALL    @INIT
        JR      NZ,FAIL
        LD      DE,FCB
        CALL    @WRITE
        JR      NZ,FAIL
        LD      DE,FCB
        CALL    @CLOSE
        JP      @EXIT
FAIL:   LD      HL,ERRMSG
        CALL    @DSPLY
        JP      @EXIT
NAME:   DB      'OUT/TXT:0',0DH
ERRMSG: DB      'FILE ERROR',0DH
FCB:    DS      32
        ORG     5400H
BUFFER: DB      'WRITTEN BY LDOS @WRITE',0DH
        DS      233,'.'
        END     START
";
    run(&mut m, source, 300);
    let s = screen(&m);
    assert!(!s.contains("FILE ERROR"), "Écran :\n{s}");
    let data = m.read_disk_file(0, "OUT/TXT").unwrap_or_else(|e| panic!("{e:?}. Écran :\n{s}"));
    assert_eq!(data.len(), 256);
    assert!(data.starts_with(b"WRITTEN BY LDOS @WRITE\r"));
}

#[test]
fn program_runs_in_basic_without_dos() {
    let Some(rom) = load_rom() else { return };
    let mut m = Trs80::new(&rom).unwrap();
    let source = "\
        ORG     7000H
START:  CALL    @CLS
        LD      HL,MSG
        CALL    @DSPLY
        RET
MSG:    DB      'NO DOS NEEDED',0DH
        END     START
";
    run(&mut m, source, 60);
    let s = screen(&m);
    assert!(s.contains("NO DOS NEEDED") && s.contains("READY"), "Écran :\n{s}");
}

#[test]
fn debugger_breakpoints_and_steps() {
    let Some(rom) = load_rom() else { return };
    let mut m = Trs80::new(&rom).unwrap();
    let source = "\
        ORG     7000H
START:  LD      A,1             ; 7000
        LD      B,2             ; 7002
        CALL    @CLS            ; 7004 : appel de la ROM
        ADD     A,B             ; 7007
LOOP:   JR      LOOP            ; 7008
        END     START
";
    let asm = z80asm::assemble(source);
    for (addr, bytes) in &asm.blocks {
        m.poke(*addr, bytes);
    }
    let back = m.launch(asm.entry).unwrap();
    assert_eq!(back, 0x1A19);
    assert_eq!(m.cpu().pc, 0x7000);
    // Point d'arrêt : l'exécution s'arrête sur l'instruction, avant de l'exécuter.
    m.set_breakpoints(&[0x7002]);
    m.run_frame();
    assert_eq!(m.take_stop(), Some(Stop::Breakpoint));
    assert_eq!((m.cpu().pc, m.cpu().a), (0x7002, 1));
    // Pas : une instruction.
    m.step_instruction();
    assert_eq!((m.cpu().pc, m.cpu().b), (0x7004, 2));
    // Pas qui passe par-dessus l'appel de la ROM : on attend le retour dans le programme.
    m.step_instruction();
    assert!(m.cpu().pc < 0x7000);
    m.set_breakpoints(&[]);
    m.set_stop_range(Some((0x7000, 0x7009)));
    for _ in 0..60 {
        m.run_frame();
        if m.stopped() {
            break;
        }
    }
    assert_eq!(m.take_stop(), Some(Stop::Range));
    assert_eq!(m.cpu().pc, 0x7007);
    let a = m.cpu().a; // (la routine de la ROM a changé A)
    // Reprise : sans arrêt actif, le programme tourne (boucle infinie en 7008).
    m.set_stop_range(None);
    m.resume();
    m.run_frame();
    assert_eq!(m.cpu().pc, 0x7008);
    assert_eq!(m.cpu().a, a.wrapping_add(2));
}

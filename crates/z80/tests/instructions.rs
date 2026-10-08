//! Tests unitaires ciblés : résultats, indicateurs, durées et comportements non documentés.

use z80::flags::{C, H, N, PV, S, X, Y, Z};
use z80::{Bus, Cpu};

struct TestBus {
    mem: Vec<u8>,
    outputs: Vec<(u16, u8)>,
    input_value: u8,
}

impl TestBus {
    fn with(code: &[u8]) -> Self {
        let mut mem = vec![0u8; 0x10000];
        mem[..code.len()].copy_from_slice(code);
        TestBus { mem, outputs: Vec::new(), input_value: 0xFF }
    }
}

impl Bus for TestBus {
    fn read(&mut self, addr: u16) -> u8 {
        self.mem[addr as usize]
    }
    fn write(&mut self, addr: u16, val: u8) {
        self.mem[addr as usize] = val;
    }
    fn input(&mut self, _port: u16) -> u8 {
        self.input_value
    }
    fn output(&mut self, port: u16, val: u8) {
        self.outputs.push((port, val));
    }
}

/// Exécute jusqu'au HALT et retourne la durée de chaque instruction.
fn run(cpu: &mut Cpu, bus: &mut TestBus) -> Vec<u32> {
    let mut times = Vec::new();
    while !cpu.halted {
        times.push(cpu.step(bus));
        assert!(times.len() < 10_000, "pas de HALT atteint");
    }
    times
}

fn exec(code: &[u8]) -> (Cpu, TestBus, Vec<u32>) {
    let mut cpu = Cpu::new();
    let mut bus = TestBus::with(code);
    let times = run(&mut cpu, &mut bus);
    (cpu, bus, times)
}

#[test]
fn ld_immediate_and_halt() {
    let (cpu, _, times) = exec(&[0x3E, 42, 0x76]); // LD A,42 ; HALT
    assert_eq!(cpu.a, 42);
    assert_eq!(times, [7, 4]);
    assert_eq!(cpu.pc, 3, "PC pointe après le HALT");
}

#[test]
fn add_overflow_flags() {
    let (cpu, ..) = exec(&[0x3E, 0x7F, 0xC6, 0x01, 0x76]); // LD A,7Fh ; ADD A,1
    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.f, S | H | PV);
}

#[test]
fn sub_with_borrow_flags() {
    let (cpu, ..) = exec(&[0x3E, 0x10, 0xD6, 0x20, 0x76]); // LD A,10h ; SUB 20h
    assert_eq!(cpu.a, 0xF0);
    assert_eq!(cpu.f, S | Y | N | C);
}

#[test]
fn cp_takes_xy_from_operand() {
    let (cpu, ..) = exec(&[0x3E, 0x00, 0xFE, 0x28, 0x76]); // LD A,0 ; CP 28h
    assert_eq!(cpu.a, 0, "CP ne modifie pas A");
    assert_eq!(cpu.f, S | Y | H | X | N | C);
}

#[test]
fn daa_after_bcd_addition() {
    let (cpu, ..) = exec(&[0x3E, 0x15, 0xC6, 0x27, 0x27, 0x76]); // 15 + 27 ; DAA
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.f & C, 0);
}

#[test]
fn inc_sets_overflow_and_half_carry() {
    let (cpu, ..) = exec(&[0x3E, 0x7F, 0x3C, 0x76]); // LD A,7Fh ; INC A
    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.f & (S | Z | H | PV | N), S | H | PV);
}

#[test]
fn neg_flags() {
    let (cpu, ..) = exec(&[0x3E, 0x01, 0xED, 0x44, 0x76]); // LD A,1 ; NEG
    assert_eq!(cpu.a, 0xFF);
    assert_eq!(cpu.f, S | Y | H | X | N | C);
}

#[test]
fn call_and_ret() {
    let mut code = vec![0u8; 0x20];
    code[..7].copy_from_slice(&[0x31, 0x00, 0x80, 0xCD, 0x10, 0x00, 0x76]); // LD SP ; CALL 10h ; HALT
    code[0x10..0x13].copy_from_slice(&[0x3E, 0x07, 0xC9]); // LD A,7 ; RET
    let (cpu, _, times) = exec(&code);
    assert_eq!(cpu.a, 7);
    assert_eq!(cpu.sp, 0x8000);
    assert_eq!(times, [10, 17, 7, 10, 4]);
}

#[test]
fn djnz_loop() {
    // LD B,5 ; LD A,0 ; boucle: INC A ; DJNZ boucle ; HALT
    let (cpu, _, times) = exec(&[0x06, 0x05, 0x3E, 0x00, 0x3C, 0x10, 0xFD, 0x76]);
    assert_eq!(cpu.a, 5);
    assert_eq!(cpu.b, 0);
    assert_eq!(times.iter().sum::<u32>(), 7 + 7 + 5 * 4 + 4 * 13 + 8 + 4);
}

#[test]
fn jr_condition_timing() {
    // XOR A (Z=1) ; JR NZ,+0 (non pris) ; JR Z,+0 (pris)
    let (_, _, times) = exec(&[0xAF, 0x20, 0x00, 0x28, 0x00, 0x76]);
    assert_eq!(times, [4, 7, 12, 4]);
}

#[test]
fn indexed_addressing() {
    // LD IX,4000h ; LD (IX+5),99h ; LD A,(IX+5) ; HALT
    let (cpu, bus, times) = exec(&[
        0xDD, 0x21, 0x00, 0x40, 0xDD, 0x36, 0x05, 0x99, 0xDD, 0x7E, 0x05, 0x76,
    ]);
    assert_eq!(cpu.a, 0x99);
    assert_eq!(bus.mem[0x4005], 0x99);
    assert_eq!(times, [14, 19, 19, 4]);
}

#[test]
fn negative_displacement() {
    // LD IY,4010h ; LD (IY-1),A avec A=FFh au RESET
    let (_, bus, _) = exec(&[0xFD, 0x21, 0x10, 0x40, 0xFD, 0x77, 0xFF, 0x76]);
    assert_eq!(bus.mem[0x400F], 0xFF);
}

#[test]
fn index_halves_undocumented() {
    // LD IX,1234h ; LD A,IXH ; LD B,IXL
    let (cpu, _, times) = exec(&[0xDD, 0x21, 0x34, 0x12, 0xDD, 0x7C, 0xDD, 0x45, 0x76]);
    assert_eq!(cpu.a, 0x12);
    assert_eq!(cpu.b, 0x34);
    assert_eq!(times, [14, 8, 8, 4]);
}

#[test]
fn ld_h_from_indexed_uses_real_h() {
    // LD IX,4000h ; LD (IX+0),55h ; LD H,(IX+0) : charge H, pas IXH
    let (cpu, ..) = exec(&[0xDD, 0x21, 0x00, 0x40, 0xDD, 0x36, 0x00, 0x55, 0xDD, 0x66, 0x00, 0x76]);
    assert_eq!(cpu.h, 0x55);
    assert_eq!(cpu.ix, 0x4000);
}

#[test]
fn ddcb_operations() {
    let (cpu, bus, times) = exec(&[
        0xDD, 0x21, 0x00, 0x40, // LD IX,4000h
        0xDD, 0x36, 0x01, 0x01, // LD (IX+1),1
        0xDD, 0xCB, 0x01, 0xF8, // SET 7,(IX+1),B  (non documenté : copie dans B)
        0xDD, 0xCB, 0x01, 0x06, // RLC (IX+1)
        0xDD, 0xCB, 0x01, 0x46, // BIT 0,(IX+1)
        0x76,
    ]);
    assert_eq!(cpu.b, 0x81);
    assert_eq!(bus.mem[0x4001], 0x03);
    assert_eq!(cpu.f & (Z | C), C, "bit 0 à 1, retenue de RLC conservée");
    assert_eq!(times, [14, 19, 23, 23, 20, 4]);
}

#[test]
fn cb_on_registers_and_memory() {
    // LD A,81h ; SRL A ; LD HL,5000h ; SET 3,(HL) ; HALT
    let (cpu, bus, times) = exec(&[0x3E, 0x81, 0xCB, 0x3F, 0x21, 0x00, 0x50, 0xCB, 0xDE, 0x76]);
    assert_eq!(cpu.a, 0x40);
    assert_eq!(cpu.f & C, C);
    assert_eq!(bus.mem[0x5000], 0x08);
    assert_eq!(times, [7, 8, 10, 15, 4]);
}

#[test]
fn ldir_copies_block() {
    let mut code = vec![
        0x21, 0x00, 0x50, // LD HL,5000h
        0x11, 0x00, 0x60, // LD DE,6000h
        0x01, 0x03, 0x00, // LD BC,3
        0xED, 0xB0, // LDIR
        0x76,
    ];
    code.resize(0x10000, 0);
    code[0x5000..0x5003].copy_from_slice(&[1, 2, 3]);
    let (cpu, bus, times) = exec(&code);
    assert_eq!(&bus.mem[0x6000..0x6003], &[1, 2, 3]);
    assert_eq!((cpu.bc(), cpu.hl(), cpu.de()), (0, 0x5003, 0x6003));
    assert_eq!(cpu.f & PV, 0);
    assert_eq!(&times[3..], &[21, 21, 16, 4]);
}

#[test]
fn cpir_finds_value() {
    let mut code = vec![
        0x21, 0x00, 0x50, // LD HL,5000h
        0x01, 0x0A, 0x00, // LD BC,10
        0x3E, 0x33, // LD A,33h
        0xED, 0xB1, // CPIR
        0x76,
    ];
    code.resize(0x10000, 0);
    code[0x5000..0x5004].copy_from_slice(&[0x11, 0x22, 0x33, 0x44]);
    let (cpu, ..) = exec(&code);
    assert_eq!((cpu.hl(), cpu.bc()), (0x5003, 7));
    assert_eq!(cpu.f & (Z | PV | N), Z | PV | N);
}

#[test]
fn exchange_instructions() {
    // LD BC,1234h ; EXX ; LD BC,5678h ; EXX ; LD A,9 ; EX AF,AF'
    let (cpu, ..) = exec(&[
        0x01, 0x34, 0x12, 0xD9, 0x01, 0x78, 0x56, 0xD9, 0x3E, 0x09, 0x08, 0x76,
    ]);
    assert_eq!(cpu.bc(), 0x1234);
    assert_eq!((cpu.b_, cpu.c_), (0x56, 0x78));
    assert_eq!(cpu.a_, 9);
}

#[test]
fn adc_sbc_16_bits() {
    // LD HL,7FFFh ; LD BC,1 ; OR A ; ADC HL,BC
    let (cpu, ..) = exec(&[0x21, 0xFF, 0x7F, 0x01, 0x01, 0x00, 0xB7, 0xED, 0x4A, 0x76]);
    assert_eq!(cpu.hl(), 0x8000);
    assert_eq!(cpu.f & (S | Z | H | PV | N | C), S | H | PV);

    // LD HL,8000h ; LD BC,1 ; OR A ; SBC HL,BC
    let (cpu, ..) = exec(&[0x21, 0x00, 0x80, 0x01, 0x01, 0x00, 0xB7, 0xED, 0x42, 0x76]);
    assert_eq!(cpu.hl(), 0x7FFF);
    assert_eq!(cpu.f & (S | Z | H | PV | N | C), H | PV | N);
}

#[test]
fn rld_rotates_digits() {
    let mut code = vec![0x21, 0x00, 0x50, 0x3E, 0x12, 0xED, 0x6F, 0x76]; // LD HL ; LD A,12h ; RLD
    code.resize(0x10000, 0);
    code[0x5000] = 0x34;
    let (cpu, bus, _) = exec(&code);
    assert_eq!(bus.mem[0x5000], 0x42);
    assert_eq!(cpu.a, 0x13);
}

#[test]
fn io_ports() {
    let mut cpu = Cpu::new();
    // LD A,12h ; OUT (FEh),A ; LD BC,3456h ; IN D,(C) ; HALT
    let mut bus = TestBus::with(&[0x3E, 0x12, 0xD3, 0xFE, 0x01, 0x56, 0x34, 0xED, 0x50, 0x76]);
    bus.input_value = 0x80;
    run(&mut cpu, &mut bus);
    assert_eq!(bus.outputs, [(0x12FE, 0x12)]);
    assert_eq!(cpu.d, 0x80);
    assert_eq!(cpu.f & (S | Z | PV), S, "0x80 : un seul bit, parité impaire");
}

#[test]
fn refresh_register_counts_m1_cycles() {
    // NOP ; LD IX,0 (préfixe + opcode = 2 cycles M1) ; HALT
    let (cpu, ..) = exec(&[0x00, 0xDD, 0x21, 0x00, 0x00, 0x76]);
    assert_eq!(cpu.r, 4);
}

#[test]
fn interrupt_mode_1_after_ei_delay() {
    let mut code = vec![0u8; 0x40];
    // IM 1 ; LD SP,8000h ; EI ; NOP ; HALT
    code[..9].copy_from_slice(&[0xED, 0x56, 0x31, 0x00, 0x80, 0xFB, 0x00, 0x76, 0x00]);
    let mut cpu = Cpu::new();
    let mut bus = TestBus::with(&code);

    cpu.step(&mut bus); // IM 1
    cpu.step(&mut bus); // LD SP
    cpu.step(&mut bus); // EI
    assert_eq!(cpu.interrupt(&mut bus, 0xFF), 0, "masquée juste après EI");
    cpu.step(&mut bus); // NOP
    cpu.step(&mut bus); // HALT
    assert!(cpu.halted);

    assert_eq!(cpu.interrupt(&mut bus, 0xFF), 13);
    assert!(!cpu.halted);
    assert_eq!(cpu.pc, 0x0038);
    assert_eq!(cpu.sp, 0x7FFE);
    assert_eq!(u16::from_le_bytes([bus.mem[0x7FFE], bus.mem[0x7FFF]]), 0x0008, "retour après le HALT");
    assert!(!cpu.iff1);
    assert_eq!(cpu.interrupt(&mut bus, 0xFF), 0, "IFF1 désactivé pendant le traitement");
}

#[test]
fn interrupt_mode_2_vector() {
    let mut code = vec![0u8; 0x10000];
    // LD A,12h ; LD I,A ; IM 2 ; LD SP,8000h ; EI ; NOP
    code[..12].copy_from_slice(&[0x3E, 0x12, 0xED, 0x47, 0xED, 0x5E, 0x31, 0x00, 0x80, 0xFB, 0x00, 0x00]);
    code[0x1234] = 0xCD;
    code[0x1235] = 0xAB;
    let mut cpu = Cpu::new();
    let mut bus = TestBus::with(&code);
    for _ in 0..6 {
        cpu.step(&mut bus);
    }
    assert_eq!(cpu.interrupt(&mut bus, 0x34), 19);
    assert_eq!(cpu.pc, 0xABCD);
}

#[test]
fn nmi_jumps_to_0066() {
    let mut cpu = Cpu::new();
    let mut bus = TestBus::with(&[0x31, 0x00, 0x80, 0x76]);
    run(&mut cpu, &mut bus);
    assert_eq!(cpu.nmi(&mut bus), 11);
    assert_eq!(cpu.pc, 0x0066);
    assert!(!cpu.halted);
}

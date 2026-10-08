//! Le processeur Z80 : registres, décodage et exécution des instructions.
//!
//! Le décodage suit la décomposition classique d'un opcode en champs
//! `x` (bits 7-6), `y` (bits 5-3), `z` (bits 2-0), `p` (bits 5-4) et `q` (bit 3).
//! Référence : « Decoding Z80 Opcodes », http://www.z80.info/decoding.htm

use crate::bus::Bus;
use crate::flags::{C, H, N, PV, S, SZ, SZP, X, Y, Z};

/// Registre utilisé à la place de HL selon le préfixe (aucun, `DD` ou `FD`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Idx {
    Hl,
    Ix,
    Iy,
}

/// État complet du processeur.
#[derive(Clone, Debug)]
pub struct Cpu {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    /// Jeu de registres alternatif (AF', BC', DE', HL').
    pub a_: u8,
    pub f_: u8,
    pub b_: u8,
    pub c_: u8,
    pub d_: u8,
    pub e_: u8,
    pub h_: u8,
    pub l_: u8,
    pub ix: u16,
    pub iy: u16,
    pub sp: u16,
    pub pc: u16,
    /// Vecteur d'interruption (mode 2).
    pub i: u8,
    /// Rafraîchissement : 7 bits incrémentés à chaque cycle M1, bit 7 conservé.
    pub r: u8,
    pub iff1: bool,
    pub iff2: bool,
    /// Mode d'interruption : 0, 1 ou 2.
    pub im: u8,
    /// Vrai après HALT, jusqu'à la prochaine interruption.
    pub halted: bool,
    /// Registre interne MEMPTR (WZ) : invisible, mais il fixe les bits X/Y de `BIT n,(HL)`.
    pub wz: u16,
    /// Total des T-states exécutés depuis la création.
    pub cycles: u64,
    /// Vrai pendant l'instruction qui suit EI : les interruptions y sont masquées.
    ei_delay: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    /// Processeur dans l'état qui suit un RESET.
    pub fn new() -> Self {
        Cpu {
            a: 0xFF,
            f: 0xFF,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            a_: 0,
            f_: 0,
            b_: 0,
            c_: 0,
            d_: 0,
            e_: 0,
            h_: 0,
            l_: 0,
            ix: 0xFFFF,
            iy: 0xFFFF,
            sp: 0xFFFF,
            pc: 0,
            i: 0,
            r: 0,
            iff1: false,
            iff2: false,
            im: 0,
            halted: false,
            wz: 0,
            cycles: 0,
            ei_delay: false,
        }
    }

    /// RESET : PC, I, R, interruptions et mode remis à zéro (les autres registres sont conservés).
    pub fn reset(&mut self) {
        self.pc = 0;
        self.i = 0;
        self.r = 0;
        self.iff1 = false;
        self.iff2 = false;
        self.im = 0;
        self.halted = false;
        self.ei_delay = false;
    }

    // ------------------------------------------------------------ paires de registres

    pub fn af(&self) -> u16 {
        u16::from_be_bytes([self.a, self.f])
    }
    pub fn bc(&self) -> u16 {
        u16::from_be_bytes([self.b, self.c])
    }
    pub fn de(&self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }
    pub fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }
    pub fn set_af(&mut self, v: u16) {
        [self.a, self.f] = v.to_be_bytes();
    }
    pub fn set_bc(&mut self, v: u16) {
        [self.b, self.c] = v.to_be_bytes();
    }
    pub fn set_de(&mut self, v: u16) {
        [self.d, self.e] = v.to_be_bytes();
    }
    pub fn set_hl(&mut self, v: u16) {
        [self.h, self.l] = v.to_be_bytes();
    }

    /// HL, IX ou IY selon le préfixe.
    fn get_xy(&self, idx: Idx) -> u16 {
        match idx {
            Idx::Hl => self.hl(),
            Idx::Ix => self.ix,
            Idx::Iy => self.iy,
        }
    }

    fn set_xy(&mut self, idx: Idx, v: u16) {
        match idx {
            Idx::Hl => self.set_hl(v),
            Idx::Ix => self.ix = v,
            Idx::Iy => self.iy = v,
        }
    }

    /// Table `rp` : BC, DE, HL (ou IX/IY), SP.
    fn get_rp(&self, p: u8, idx: Idx) -> u16 {
        match p {
            0 => self.bc(),
            1 => self.de(),
            2 => self.get_xy(idx),
            _ => self.sp,
        }
    }

    fn set_rp(&mut self, p: u8, idx: Idx, v: u16) {
        match p {
            0 => self.set_bc(v),
            1 => self.set_de(v),
            2 => self.set_xy(idx, v),
            _ => self.sp = v,
        }
    }

    /// Table `rp2` : BC, DE, HL (ou IX/IY), AF.
    fn get_rp2(&self, p: u8, idx: Idx) -> u16 {
        if p == 3 { self.af() } else { self.get_rp(p, idx) }
    }

    fn set_rp2(&mut self, p: u8, idx: Idx, v: u16) {
        if p == 3 { self.set_af(v) } else { self.set_rp(p, idx, v) }
    }

    /// Registre 8 bits `r[n]` (0=B 1=C 2=D 3=E 4=H 5=L 7=A). Avec un préfixe,
    /// H et L deviennent les moitiés de IX/IY (IXH, IXL, IYH, IYL).
    /// Le cas 6, (HL), est traité à part par l'appelant.
    fn get_r(&self, n: u8, idx: Idx) -> u8 {
        match n {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => (self.get_xy(idx) >> 8) as u8,
            5 => self.get_xy(idx) as u8,
            7 => self.a,
            _ => unreachable!("r[6] est un accès mémoire"),
        }
    }

    fn set_r(&mut self, n: u8, idx: Idx, v: u8) {
        match n {
            0 => self.b = v,
            1 => self.c = v,
            2 => self.d = v,
            3 => self.e = v,
            4 => {
                let xy = self.get_xy(idx);
                self.set_xy(idx, (xy & 0x00FF) | ((v as u16) << 8));
            }
            5 => {
                let xy = self.get_xy(idx);
                self.set_xy(idx, (xy & 0xFF00) | v as u16);
            }
            7 => self.a = v,
            _ => unreachable!("r[6] est un accès mémoire"),
        }
    }

    /// Condition `cc[y]` : NZ, Z, NC, C, PO, PE, P, M.
    fn cond(&self, y: u8) -> bool {
        let f = self.f;
        match y {
            0 => f & Z == 0,
            1 => f & Z != 0,
            2 => f & C == 0,
            3 => f & C != 0,
            4 => f & PV == 0,
            5 => f & PV != 0,
            6 => f & S == 0,
            _ => f & S != 0,
        }
    }

    // ------------------------------------------------------------ accès au bus

    fn inc_r(&mut self) {
        self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);
    }

    /// Lecture d'un opcode (cycle M1 : incrémente R).
    fn fetch_op<B: Bus>(&mut self, bus: &mut B) -> u8 {
        self.inc_r();
        self.fetch8(bus)
    }

    fn fetch8<B: Bus>(&mut self, bus: &mut B) -> u8 {
        let v = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        v
    }

    fn fetch16<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let lo = self.fetch8(bus);
        let hi = self.fetch8(bus);
        u16::from_le_bytes([lo, hi])
    }

    fn read16<B: Bus>(bus: &mut B, addr: u16) -> u16 {
        u16::from_le_bytes([bus.read(addr), bus.read(addr.wrapping_add(1))])
    }

    fn write16<B: Bus>(bus: &mut B, addr: u16, v: u16) {
        let [lo, hi] = v.to_le_bytes();
        bus.write(addr, lo);
        bus.write(addr.wrapping_add(1), hi);
    }

    fn push<B: Bus>(&mut self, bus: &mut B, v: u16) {
        let [lo, hi] = v.to_le_bytes();
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, hi);
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, lo);
    }

    fn pop<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let v = Self::read16(bus, self.sp);
        self.sp = self.sp.wrapping_add(2);
        v
    }

    /// Adresse de l'opérande mémoire « (HL) » : HL, ou IX+d / IY+d avec un préfixe
    /// (le déplacement d est lu ici).
    fn mem_addr<B: Bus>(&mut self, bus: &mut B, idx: Idx) -> u16 {
        match idx {
            Idx::Hl => self.hl(),
            _ => {
                let d = self.fetch8(bus) as i8;
                let addr = self.get_xy(idx).wrapping_add(d as u16);
                self.wz = addr;
                addr
            }
        }
    }

    // ------------------------------------------------------------ exécution

    /// Exécute une instruction complète (préfixes compris) et retourne sa durée en T-states.
    pub fn step<B: Bus>(&mut self, bus: &mut B) -> u32 {
        self.ei_delay = false;
        if self.halted {
            // En HALT, le Z80 exécute des NOP sans avancer PC.
            self.inc_r();
            self.cycles += 4;
            return 4;
        }

        let mut idx = Idx::Hl;
        let mut prefix_cycles = 0;
        let t = loop {
            let op = self.fetch_op(bus);
            match op {
                // Un préfixe DD/FD suivi d'un autre préfixe se comporte comme un NOP.
                0xDD => {
                    prefix_cycles += 4;
                    idx = Idx::Ix;
                }
                0xFD => {
                    prefix_cycles += 4;
                    idx = Idx::Iy;
                }
                0xCB => {
                    break if idx == Idx::Hl { self.exec_cb(bus) } else { self.exec_index_cb(bus, idx) };
                }
                // ED annule un préfixe DD/FD qui le précède.
                0xED => break self.exec_ed(bus),
                _ => break self.exec_main(bus, op, idx),
            }
        };
        let total = prefix_cycles + t;
        self.cycles += total as u64;
        total
    }

    /// Demande d'interruption masquable (broche /INT). `data` est l'octet placé sur le bus
    /// de données par le périphérique (utilisé en modes 0 et 2).
    /// Retourne la durée de la prise en charge, ou 0 si l'interruption est masquée.
    pub fn interrupt<B: Bus>(&mut self, bus: &mut B, data: u8) -> u32 {
        if !self.iff1 || self.ei_delay {
            return 0;
        }
        self.halted = false;
        self.iff1 = false;
        self.iff2 = false;
        self.inc_r();
        let t = match self.im {
            2 => {
                self.push(bus, self.pc);
                let vector = u16::from_be_bytes([self.i, data & 0xFE]);
                self.pc = Self::read16(bus, vector);
                19
            }
            1 => {
                self.push(bus, self.pc);
                self.pc = 0x0038;
                13
            }
            _ => {
                // Mode 0 : le périphérique fournit une instruction; on suppose un RST,
                // ce qui couvre l'usage réel (le TRS-80 utilise le mode 1).
                self.push(bus, self.pc);
                self.pc = (data & 0x38) as u16;
                13
            }
        };
        self.wz = self.pc;
        self.cycles += t as u64;
        t
    }

    /// Interruption non masquable (broche /NMI) : saut en 0066h.
    pub fn nmi<B: Bus>(&mut self, bus: &mut B) -> u32 {
        self.halted = false;
        self.iff1 = false;
        self.inc_r();
        self.push(bus, self.pc);
        self.pc = 0x0066;
        self.wz = self.pc;
        self.cycles += 11;
        11
    }

    /// Instructions sans préfixe (ou avec DD/FD, qui remplacent HL par IX/IY).
    /// Retourne la durée sans compter les octets de préfixe.
    fn exec_main<B: Bus>(&mut self, bus: &mut B, op: u8, idx: Idx) -> u32 {
        let x = op >> 6;
        let y = (op >> 3) & 7;
        let z = op & 7;
        let p = y >> 1;
        let q = y & 1;
        // Surcoût d'un accès (IX+d) : calcul de l'adresse.
        let disp = if idx == Idx::Hl { 0 } else { 8 };

        match x {
            0 => match z {
                0 => match y {
                    0 => 4, // NOP
                    1 => {
                        // EX AF,AF'
                        core::mem::swap(&mut self.a, &mut self.a_);
                        core::mem::swap(&mut self.f, &mut self.f_);
                        4
                    }
                    2 => {
                        // DJNZ d
                        let d = self.fetch8(bus) as i8;
                        self.b = self.b.wrapping_sub(1);
                        if self.b != 0 {
                            self.pc = self.pc.wrapping_add(d as u16);
                            self.wz = self.pc;
                            13
                        } else {
                            8
                        }
                    }
                    3 => {
                        // JR d
                        let d = self.fetch8(bus) as i8;
                        self.pc = self.pc.wrapping_add(d as u16);
                        self.wz = self.pc;
                        12
                    }
                    _ => {
                        // JR cc,d
                        let d = self.fetch8(bus) as i8;
                        if self.cond(y - 4) {
                            self.pc = self.pc.wrapping_add(d as u16);
                            self.wz = self.pc;
                            12
                        } else {
                            7
                        }
                    }
                },
                1 => {
                    if q == 0 {
                        // LD rp,nn
                        let v = self.fetch16(bus);
                        self.set_rp(p, idx, v);
                        10
                    } else {
                        // ADD HL,rp
                        let a = self.get_xy(idx);
                        let b = self.get_rp(p, idx);
                        let r = self.add16(a, b);
                        self.set_xy(idx, r);
                        11
                    }
                }
                2 => match (q, p) {
                    (0, 0) | (0, 1) => {
                        // LD (BC),A / LD (DE),A
                        let addr = if p == 0 { self.bc() } else { self.de() };
                        bus.write(addr, self.a);
                        self.wz = u16::from_be_bytes([self.a, addr.wrapping_add(1) as u8]);
                        7
                    }
                    (0, 2) => {
                        // LD (nn),HL
                        let addr = self.fetch16(bus);
                        Self::write16(bus, addr, self.get_xy(idx));
                        self.wz = addr.wrapping_add(1);
                        16
                    }
                    (0, _) => {
                        // LD (nn),A
                        let addr = self.fetch16(bus);
                        bus.write(addr, self.a);
                        self.wz = u16::from_be_bytes([self.a, addr.wrapping_add(1) as u8]);
                        13
                    }
                    (_, 0) | (_, 1) => {
                        // LD A,(BC) / LD A,(DE)
                        let addr = if p == 0 { self.bc() } else { self.de() };
                        self.a = bus.read(addr);
                        self.wz = addr.wrapping_add(1);
                        7
                    }
                    (_, 2) => {
                        // LD HL,(nn)
                        let addr = self.fetch16(bus);
                        let v = Self::read16(bus, addr);
                        self.set_xy(idx, v);
                        self.wz = addr.wrapping_add(1);
                        16
                    }
                    _ => {
                        // LD A,(nn)
                        let addr = self.fetch16(bus);
                        self.a = bus.read(addr);
                        self.wz = addr.wrapping_add(1);
                        13
                    }
                },
                3 => {
                    // INC rp / DEC rp (aucun indicateur modifié)
                    let v = self.get_rp(p, idx);
                    let v = if q == 0 { v.wrapping_add(1) } else { v.wrapping_sub(1) };
                    self.set_rp(p, idx, v);
                    6
                }
                4 | 5 => {
                    // INC r / DEC r
                    let inc = z == 4;
                    if y == 6 {
                        let addr = self.mem_addr(bus, idx);
                        let v = bus.read(addr);
                        let r = if inc { self.inc8(v) } else { self.dec8(v) };
                        bus.write(addr, r);
                        11 + disp
                    } else {
                        let v = self.get_r(y, idx);
                        let r = if inc { self.inc8(v) } else { self.dec8(v) };
                        self.set_r(y, idx, r);
                        4
                    }
                }
                6 => {
                    // LD r,n
                    if y == 6 {
                        let addr = self.mem_addr(bus, idx);
                        let n = self.fetch8(bus);
                        bus.write(addr, n);
                        10 + if idx == Idx::Hl { 0 } else { 5 }
                    } else {
                        let n = self.fetch8(bus);
                        self.set_r(y, idx, n);
                        7
                    }
                }
                _ => {
                    match y {
                        0 => self.rlca(),
                        1 => self.rrca(),
                        2 => self.rla(),
                        3 => self.rra(),
                        4 => self.daa(),
                        5 => self.cpl(),
                        6 => self.scf(),
                        _ => self.ccf(),
                    }
                    4
                }
            },

            1 => {
                if op == 0x76 {
                    // HALT : PC pointe déjà sur l'instruction suivante.
                    self.halted = true;
                    4
                } else if z == 6 {
                    // LD r,(HL) : avec (IX+d), le registre est le vrai H ou L.
                    let addr = self.mem_addr(bus, idx);
                    let v = bus.read(addr);
                    self.set_r(y, Idx::Hl, v);
                    7 + disp
                } else if y == 6 {
                    // LD (HL),r
                    let addr = self.mem_addr(bus, idx);
                    bus.write(addr, self.get_r(z, Idx::Hl));
                    7 + disp
                } else {
                    // LD r,r'
                    let v = self.get_r(z, idx);
                    self.set_r(y, idx, v);
                    4
                }
            }

            2 => {
                // Opération arithmétique ou logique sur A : ALU r
                if z == 6 {
                    let addr = self.mem_addr(bus, idx);
                    let v = bus.read(addr);
                    self.alu(y, v);
                    7 + disp
                } else {
                    let v = self.get_r(z, idx);
                    self.alu(y, v);
                    4
                }
            }

            _ => match z {
                0 => {
                    // RET cc
                    if self.cond(y) {
                        self.pc = self.pop(bus);
                        self.wz = self.pc;
                        11
                    } else {
                        5
                    }
                }
                1 => {
                    if q == 0 {
                        // POP rp2
                        let v = self.pop(bus);
                        self.set_rp2(p, idx, v);
                        10
                    } else {
                        match p {
                            0 => {
                                // RET
                                self.pc = self.pop(bus);
                                self.wz = self.pc;
                                10
                            }
                            1 => {
                                // EXX
                                core::mem::swap(&mut self.b, &mut self.b_);
                                core::mem::swap(&mut self.c, &mut self.c_);
                                core::mem::swap(&mut self.d, &mut self.d_);
                                core::mem::swap(&mut self.e, &mut self.e_);
                                core::mem::swap(&mut self.h, &mut self.h_);
                                core::mem::swap(&mut self.l, &mut self.l_);
                                4
                            }
                            2 => {
                                // JP (HL)
                                self.pc = self.get_xy(idx);
                                4
                            }
                            _ => {
                                // LD SP,HL
                                self.sp = self.get_xy(idx);
                                6
                            }
                        }
                    }
                }
                2 => {
                    // JP cc,nn
                    let addr = self.fetch16(bus);
                    self.wz = addr;
                    if self.cond(y) {
                        self.pc = addr;
                    }
                    10
                }
                3 => match y {
                    0 => {
                        // JP nn
                        self.pc = self.fetch16(bus);
                        self.wz = self.pc;
                        10
                    }
                    2 => {
                        // OUT (n),A
                        let n = self.fetch8(bus);
                        bus.output(u16::from_be_bytes([self.a, n]), self.a);
                        self.wz = u16::from_be_bytes([self.a, n.wrapping_add(1)]);
                        11
                    }
                    3 => {
                        // IN A,(n)
                        let n = self.fetch8(bus);
                        let port = u16::from_be_bytes([self.a, n]);
                        self.a = bus.input(port);
                        self.wz = port.wrapping_add(1);
                        11
                    }
                    4 => {
                        // EX (SP),HL
                        let v = Self::read16(bus, self.sp);
                        Self::write16(bus, self.sp, self.get_xy(idx));
                        self.set_xy(idx, v);
                        self.wz = v;
                        19
                    }
                    5 => {
                        // EX DE,HL (jamais affecté par DD/FD)
                        core::mem::swap(&mut self.d, &mut self.h);
                        core::mem::swap(&mut self.e, &mut self.l);
                        4
                    }
                    6 => {
                        // DI
                        self.iff1 = false;
                        self.iff2 = false;
                        4
                    }
                    7 => {
                        // EI : prend effet après l'instruction suivante.
                        self.iff1 = true;
                        self.iff2 = true;
                        self.ei_delay = true;
                        4
                    }
                    _ => unreachable!("CB est traité dans step()"),
                },
                4 => {
                    // CALL cc,nn
                    let addr = self.fetch16(bus);
                    self.wz = addr;
                    if self.cond(y) {
                        self.push(bus, self.pc);
                        self.pc = addr;
                        17
                    } else {
                        10
                    }
                }
                5 => {
                    if q == 0 {
                        // PUSH rp2
                        let v = self.get_rp2(p, idx);
                        self.push(bus, v);
                        11
                    } else {
                        // CALL nn (p == 0; DD, ED et FD sont traités dans step())
                        let addr = self.fetch16(bus);
                        self.wz = addr;
                        self.push(bus, self.pc);
                        self.pc = addr;
                        17
                    }
                }
                6 => {
                    // ALU n
                    let n = self.fetch8(bus);
                    self.alu(y, n);
                    7
                }
                _ => {
                    // RST y*8
                    self.push(bus, self.pc);
                    self.pc = (y as u16) * 8;
                    self.wz = self.pc;
                    11
                }
            },
        }
    }

    /// Instructions préfixées CB : rotations, décalages, BIT, RES, SET.
    fn exec_cb<B: Bus>(&mut self, bus: &mut B) -> u32 {
        let op = self.fetch_op(bus);
        let x = op >> 6;
        let y = (op >> 3) & 7;
        let z = op & 7;

        if z == 6 {
            let addr = self.hl();
            let v = bus.read(addr);
            match x {
                0 => {
                    let r = self.rot(y, v);
                    bus.write(addr, r);
                    15
                }
                1 => {
                    // BIT n,(HL) : X et Y viennent de l'octet haut de WZ.
                    self.bit(y, v, (self.wz >> 8) as u8);
                    12
                }
                2 => {
                    bus.write(addr, v & !(1 << y));
                    15
                }
                _ => {
                    bus.write(addr, v | (1 << y));
                    15
                }
            }
        } else {
            let v = self.get_r(z, Idx::Hl);
            match x {
                0 => {
                    let r = self.rot(y, v);
                    self.set_r(z, Idx::Hl, r);
                }
                1 => self.bit(y, v, v),
                2 => self.set_r(z, Idx::Hl, v & !(1 << y)),
                _ => self.set_r(z, Idx::Hl, v | (1 << y)),
            }
            8
        }
    }

    /// Instructions DD CB d op / FD CB d op : opérations CB sur (IX+d) ou (IY+d).
    /// Les octets d et op ne sont pas des cycles M1 (R n'est pas incrémenté).
    /// Retourne la durée sans le préfixe DD/FD.
    fn exec_index_cb<B: Bus>(&mut self, bus: &mut B, idx: Idx) -> u32 {
        let d = self.fetch8(bus) as i8;
        let op = self.fetch8(bus);
        let addr = self.get_xy(idx).wrapping_add(d as u16);
        self.wz = addr;
        let x = op >> 6;
        let y = (op >> 3) & 7;
        let z = op & 7;
        let v = bus.read(addr);

        let r = match x {
            1 => {
                self.bit(y, v, (addr >> 8) as u8);
                return 16;
            }
            0 => self.rot(y, v),
            2 => v & !(1 << y),
            _ => v | (1 << y),
        };
        bus.write(addr, r);
        // Non documenté : le résultat est aussi copié dans le registre r[z] (sauf z = 6).
        if z != 6 {
            self.set_r(z, Idx::Hl, r);
        }
        19
    }

    /// Instructions préfixées ED. Retourne la durée totale, octet ED compris.
    fn exec_ed<B: Bus>(&mut self, bus: &mut B) -> u32 {
        let op = self.fetch_op(bus);
        let x = op >> 6;
        let y = (op >> 3) & 7;
        let z = op & 7;
        let p = y >> 1;
        let q = y & 1;

        if x == 2 && z <= 3 && y >= 4 {
            return self.exec_block(bus, y, z);
        }
        if x != 1 {
            // Opcode ED invalide : se comporte comme deux NOP.
            return 8;
        }

        match z {
            0 => {
                // IN r,(C) ; y = 6 : IN (C), seuls les indicateurs sont modifiés.
                let port = self.bc();
                let v = bus.input(port);
                if y != 6 {
                    self.set_r(y, Idx::Hl, v);
                }
                self.f = (self.f & C) | SZP[v as usize];
                self.wz = port.wrapping_add(1);
                12
            }
            1 => {
                // OUT (C),r ; y = 6 : OUT (C),0
                let port = self.bc();
                let v = if y == 6 { 0 } else { self.get_r(y, Idx::Hl) };
                bus.output(port, v);
                self.wz = port.wrapping_add(1);
                12
            }
            2 => {
                let v = self.get_rp(p, Idx::Hl);
                if q == 0 { self.sbc16(v) } else { self.adc16(v) }
                15
            }
            3 => {
                let addr = self.fetch16(bus);
                if q == 0 {
                    // LD (nn),rp
                    Self::write16(bus, addr, self.get_rp(p, Idx::Hl));
                } else {
                    // LD rp,(nn)
                    let v = Self::read16(bus, addr);
                    self.set_rp(p, Idx::Hl, v);
                }
                self.wz = addr.wrapping_add(1);
                20
            }
            4 => {
                // NEG
                let v = self.a;
                self.a = 0;
                self.sub8(v, 0);
                8
            }
            5 => {
                // RETN / RETI
                self.iff1 = self.iff2;
                self.pc = self.pop(bus);
                self.wz = self.pc;
                14
            }
            6 => {
                // IM 0/1/2
                self.im = [0, 0, 1, 2, 0, 0, 1, 2][y as usize];
                8
            }
            _ => match y {
                0 => {
                    // LD I,A
                    self.i = self.a;
                    9
                }
                1 => {
                    // LD R,A
                    self.r = self.a;
                    9
                }
                2 | 3 => {
                    // LD A,I / LD A,R : P/V reçoit IFF2.
                    self.a = if y == 2 { self.i } else { self.r };
                    self.f = (self.f & C) | SZ[self.a as usize] | if self.iff2 { PV } else { 0 };
                    9
                }
                4 => {
                    self.rrd(bus);
                    18
                }
                5 => {
                    self.rld(bus);
                    18
                }
                _ => 8, // NOP
            },
        }
    }

    /// Instructions de bloc : LDI, CPI, INI, OUTI, leurs variantes décroissantes (D)
    /// et répétées (IR / DR).
    fn exec_block<B: Bus>(&mut self, bus: &mut B, y: u8, z: u8) -> u32 {
        let dec = y & 1 == 1;
        let repeat = y >= 6;
        let step = |v: u16| if dec { v.wrapping_sub(1) } else { v.wrapping_add(1) };

        let again = match z {
            0 => {
                // LDI / LDD / LDIR / LDDR
                let v = bus.read(self.hl());
                bus.write(self.de(), v);
                self.set_hl(step(self.hl()));
                self.set_de(step(self.de()));
                self.set_bc(self.bc().wrapping_sub(1));
                let n = v.wrapping_add(self.a);
                self.f = (self.f & (S | Z | C))
                    | (n & X)
                    | ((n << 4) & Y)
                    | if self.bc() != 0 { PV } else { 0 };
                self.bc() != 0
            }
            1 => {
                // CPI / CPD / CPIR / CPDR
                let v = bus.read(self.hl());
                let r = self.a.wrapping_sub(v);
                let half = (self.a ^ v ^ r) & H;
                let n = r.wrapping_sub(if half != 0 { 1 } else { 0 });
                self.set_hl(step(self.hl()));
                self.set_bc(self.bc().wrapping_sub(1));
                self.wz = step(self.wz);
                self.f = (self.f & C)
                    | N
                    | (SZ[r as usize] & (S | Z))
                    | half
                    | (n & X)
                    | ((n << 4) & Y)
                    | if self.bc() != 0 { PV } else { 0 };
                self.bc() != 0 && r != 0
            }
            2 => {
                // INI / IND / INIR / INDR
                let v = bus.input(self.bc());
                self.wz = step(self.bc());
                bus.write(self.hl(), v);
                self.b = self.b.wrapping_sub(1);
                self.set_hl(step(self.hl()));
                let k = v as u16 + step(self.c as u16) as u8 as u16;
                self.block_io_flags(v, k);
                self.b != 0
            }
            _ => {
                // OUTI / OUTD / OTIR / OTDR
                let v = bus.read(self.hl());
                self.b = self.b.wrapping_sub(1);
                self.wz = step(self.bc());
                bus.output(self.bc(), v);
                self.set_hl(step(self.hl()));
                let k = v as u16 + self.l as u16;
                self.block_io_flags(v, k);
                self.b != 0
            }
        };

        if repeat && again {
            // On revient sur l'instruction : elle sera réexécutée au prochain step().
            self.pc = self.pc.wrapping_sub(2);
            self.wz = self.pc.wrapping_add(1);
            21
        } else {
            16
        }
    }

    /// Indicateurs (en partie non documentés) des instructions de bloc d'E/S.
    fn block_io_flags(&mut self, v: u8, k: u16) {
        let hc = if k > 0xFF { H | C } else { 0 };
        let parity = SZP[((k as u8 & 7) ^ self.b) as usize] & PV;
        self.f = SZ[self.b as usize] | hc | parity | if v & 0x80 != 0 { N } else { 0 };
    }

    // ------------------------------------------------------------ ALU 8 bits

    /// Opération `alu[y]` sur A : ADD, ADC, SUB, SBC, AND, XOR, OR, CP.
    fn alu(&mut self, y: u8, v: u8) {
        let carry = self.f & C;
        match y {
            0 => self.add8(v, 0),
            1 => self.add8(v, carry),
            2 => self.sub8(v, 0),
            3 => self.sub8(v, carry),
            4 => {
                self.a &= v;
                self.f = SZP[self.a as usize] | H;
            }
            5 => {
                self.a ^= v;
                self.f = SZP[self.a as usize];
            }
            6 => {
                self.a |= v;
                self.f = SZP[self.a as usize];
            }
            _ => self.cp8(v),
        }
    }

    fn add8(&mut self, v: u8, carry: u8) {
        let a = self.a;
        let r16 = a as u16 + v as u16 + carry as u16;
        let r = r16 as u8;
        let overflow = (a ^ r) & (v ^ r) & 0x80 != 0;
        self.f = SZ[r as usize]
            | ((a ^ v ^ r) & H)
            | if overflow { PV } else { 0 }
            | if r16 > 0xFF { C } else { 0 };
        self.a = r;
    }

    /// Soustraction A - v - carry; met à jour les indicateurs et retourne le résultat.
    fn sub_flags(&mut self, v: u8, carry: u8) -> u8 {
        let a = self.a;
        let r16 = (a as u16).wrapping_sub(v as u16).wrapping_sub(carry as u16);
        let r = r16 as u8;
        let overflow = (a ^ v) & (a ^ r) & 0x80 != 0;
        self.f = SZ[r as usize]
            | N
            | ((a ^ v ^ r) & H)
            | if overflow { PV } else { 0 }
            | if r16 > 0xFF { C } else { 0 };
        r
    }

    fn sub8(&mut self, v: u8, carry: u8) {
        self.a = self.sub_flags(v, carry);
    }

    /// CP : comme SUB sans modifier A; X et Y viennent de l'opérande.
    fn cp8(&mut self, v: u8) {
        self.sub_flags(v, 0);
        self.f = (self.f & !(X | Y)) | (v & (X | Y));
    }

    fn inc8(&mut self, v: u8) -> u8 {
        let r = v.wrapping_add(1);
        self.f = (self.f & C)
            | SZ[r as usize]
            | if r == 0x80 { PV } else { 0 }
            | if r & 0x0F == 0 { H } else { 0 };
        r
    }

    fn dec8(&mut self, v: u8) -> u8 {
        let r = v.wrapping_sub(1);
        self.f = (self.f & C)
            | N
            | SZ[r as usize]
            | if r == 0x7F { PV } else { 0 }
            | if r & 0x0F == 0x0F { H } else { 0 };
        r
    }

    fn rlca(&mut self) {
        self.a = self.a.rotate_left(1);
        self.f = (self.f & (S | Z | PV)) | (self.a & (X | Y | C));
    }

    fn rrca(&mut self) {
        let c = self.a & 1;
        self.a = self.a.rotate_right(1);
        self.f = (self.f & (S | Z | PV)) | (self.a & (X | Y)) | c;
    }

    fn rla(&mut self) {
        let c = self.a >> 7;
        self.a = (self.a << 1) | (self.f & C);
        self.f = (self.f & (S | Z | PV)) | (self.a & (X | Y)) | c;
    }

    fn rra(&mut self) {
        let c = self.a & 1;
        self.a = (self.a >> 1) | ((self.f & C) << 7);
        self.f = (self.f & (S | Z | PV)) | (self.a & (X | Y)) | c;
    }

    /// Ajustement décimal de A après une addition ou une soustraction BCD.
    fn daa(&mut self) {
        let a = self.a;
        let low = a & 0x0F;
        let mut diff = 0u8;
        let mut carry = self.f & C;
        if self.f & H != 0 || low > 9 {
            diff |= 0x06;
        }
        if carry != 0 || a > 0x99 {
            diff |= 0x60;
            carry = C;
        }
        let subtract = self.f & N != 0;
        let r = if subtract { a.wrapping_sub(diff) } else { a.wrapping_add(diff) };
        let half = if subtract { self.f & H != 0 && low < 6 } else { low > 9 };
        self.f = SZP[r as usize] | (self.f & N) | carry | if half { H } else { 0 };
        self.a = r;
    }

    fn cpl(&mut self) {
        self.a = !self.a;
        self.f = (self.f & (S | Z | PV | C)) | H | N | (self.a & (X | Y));
    }

    fn scf(&mut self) {
        self.f = (self.f & (S | Z | PV)) | C | (self.a & (X | Y));
    }

    fn ccf(&mut self) {
        let old_c = self.f & C;
        self.f = (self.f & (S | Z | PV)) | (self.a & (X | Y)) | if old_c != 0 { H } else { C };
    }

    // ------------------------------------------------------------ ALU 16 bits

    /// ADD HL,rr : seuls H, C, N (et X/Y) changent.
    fn add16(&mut self, a: u16, b: u16) -> u16 {
        let r32 = a as u32 + b as u32;
        let r = r32 as u16;
        self.wz = a.wrapping_add(1);
        self.f = (self.f & (S | Z | PV))
            | (((a ^ b ^ r) >> 8) as u8 & H)
            | ((r >> 8) as u8 & (X | Y))
            | if r32 > 0xFFFF { C } else { 0 };
        r
    }

    fn adc16(&mut self, v: u16) {
        let hl = self.hl();
        let r32 = hl as u32 + v as u32 + (self.f & C) as u32;
        let r = r32 as u16;
        let overflow = (hl ^ r) & (v ^ r) & 0x8000 != 0;
        self.wz = hl.wrapping_add(1);
        self.f = ((r >> 8) as u8 & (S | X | Y))
            | if r == 0 { Z } else { 0 }
            | (((hl ^ v ^ r) >> 8) as u8 & H)
            | if overflow { PV } else { 0 }
            | if r32 > 0xFFFF { C } else { 0 };
        self.set_hl(r);
    }

    fn sbc16(&mut self, v: u16) {
        let hl = self.hl();
        let r32 = (hl as u32).wrapping_sub(v as u32).wrapping_sub((self.f & C) as u32);
        let r = r32 as u16;
        let overflow = (hl ^ v) & (hl ^ r) & 0x8000 != 0;
        self.wz = hl.wrapping_add(1);
        self.f = ((r >> 8) as u8 & (S | X | Y))
            | N
            | if r == 0 { Z } else { 0 }
            | (((hl ^ v ^ r) >> 8) as u8 & H)
            | if overflow { PV } else { 0 }
            | if r32 > 0xFFFF { C } else { 0 };
        self.set_hl(r);
    }

    // ------------------------------------------------------------ CB et divers

    /// Rotation / décalage `rot[y]` : RLC, RRC, RL, RR, SLA, SRA, SLL (non documenté), SRL.
    fn rot(&mut self, y: u8, v: u8) -> u8 {
        let c = self.f & C;
        let (r, carry) = match y {
            0 => (v.rotate_left(1), v >> 7),
            1 => (v.rotate_right(1), v & 1),
            2 => ((v << 1) | c, v >> 7),
            3 => ((v >> 1) | (c << 7), v & 1),
            4 => (v << 1, v >> 7),
            5 => ((v >> 1) | (v & 0x80), v & 1),
            6 => ((v << 1) | 1, v >> 7),
            _ => (v >> 1, v & 1),
        };
        self.f = SZP[r as usize] | carry;
        r
    }

    /// BIT n,v : `xy` fournit les bits X et Y (registre testé, ou WZ pour un accès mémoire).
    fn bit(&mut self, n: u8, v: u8, xy: u8) {
        let set = v & (1 << n) != 0;
        self.f = (self.f & C)
            | H
            | (xy & (X | Y))
            | if set { 0 } else { Z | PV }
            | if set && n == 7 { S } else { 0 };
    }

    fn rrd<B: Bus>(&mut self, bus: &mut B) {
        let addr = self.hl();
        let v = bus.read(addr);
        bus.write(addr, (self.a << 4) | (v >> 4));
        self.a = (self.a & 0xF0) | (v & 0x0F);
        self.f = (self.f & C) | SZP[self.a as usize];
        self.wz = addr.wrapping_add(1);
    }

    fn rld<B: Bus>(&mut self, bus: &mut B) {
        let addr = self.hl();
        let v = bus.read(addr);
        bus.write(addr, (v << 4) | (self.a & 0x0F));
        self.a = (self.a & 0xF0) | (v >> 4);
        self.f = (self.f & C) | SZP[self.a as usize];
        self.wz = addr.wrapping_add(1);
    }
}

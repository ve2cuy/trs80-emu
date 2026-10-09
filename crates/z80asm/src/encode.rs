//! Encodage des instructions Z80 documentées (avec IX, IY, préfixes CB et ED).

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

/// Problème dans une ligne : code (clé de traduction de la page), arguments, correction.
#[derive(Debug, Clone)]
pub struct Issue {
    pub code: &'static str,
    pub args: Vec<String>,
    pub fix: Fix,
}

/// Correction proposée, appliquée à la ligne fautive.
#[derive(Debug, Clone, PartialEq)]
pub enum Fix {
    None,
    /// Remplacer le mnémonique.
    Mnemonic(String),
    /// Remplacer un mot (symbole, nombre) par un autre.
    Word(String, String),
    /// Insérer cette ligne avant la ligne fautive.
    Insert(String),
}

pub fn issue(code: &'static str, args: &[&str]) -> Issue {
    Issue { code, args: args.iter().map(|s| s.to_string()).collect(), fix: Fix::None }
}

/// Valeurs des opérandes, fournies par l'assembleur.
pub trait Values {
    /// Valeur d'une expression : 0 si un symbole est encore inconnu (passes préliminaires),
    /// erreur à la dernière passe.
    fn value(&mut self, text: &str) -> Result<i64, Issue>;
    /// Adresse de l'instruction.
    fn pc(&self) -> u16;
    /// Dernière passe : les valeurs sont définitives, on vérifie les bornes.
    fn checking(&self) -> bool;
}

#[derive(Clone, Copy, Debug)]
enum O<'a> {
    /// B C D E H L A : codes 0 à 5 et 7.
    R(u8),
    /// (HL)
    HLi,
    /// (IX+d), (IY+d) : préfixe, texte du déplacement (vide pour (IX)).
    Idx(u8, &'a str),
    /// BC DE HL SP : codes 0 à 3.
    RP(u8),
    AF,
    AFx,
    /// IX, IY : préfixe.
    XY(u8),
    BCi,
    DEi,
    SPi,
    Ci,
    I,
    Rr,
    /// (nn)
    Mem(&'a str),
    Imm(&'a str),
}

/// Contenu d'un opérande entièrement entre parenthèses : « (5+3) », mais pas « (5)+(3) ».
fn wrapped(t: &str) -> Option<&str> {
    if !(t.starts_with('(') && t.ends_with(')')) {
        return None;
    }
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    for (i, c) in t.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => {
                depth -= 1;
                if depth == 0 && i != t.len() - 1 {
                    return None;
                }
            }
            _ => {}
        }
    }
    Some(t[1..t.len() - 1].trim())
}

fn operand(t: &str) -> O<'_> {
    let u = t.to_ascii_uppercase();
    match u.as_str() {
        "B" => return O::R(0),
        "C" => return O::R(1),
        "D" => return O::R(2),
        "E" => return O::R(3),
        "H" => return O::R(4),
        "L" => return O::R(5),
        "A" => return O::R(7),
        "BC" => return O::RP(0),
        "DE" => return O::RP(1),
        "HL" => return O::RP(2),
        "SP" => return O::RP(3),
        "AF" => return O::AF,
        "AF'" => return O::AFx,
        "IX" => return O::XY(0xDD),
        "IY" => return O::XY(0xFD),
        "I" => return O::I,
        "R" => return O::Rr,
        _ => {}
    }
    if let Some(inner) = wrapped(t) {
        match inner.to_ascii_uppercase().as_str() {
            "HL" => return O::HLi,
            "BC" => return O::BCi,
            "DE" => return O::DEi,
            "SP" => return O::SPi,
            "C" => return O::Ci,
            _ => {}
        }
        let iu = inner.to_ascii_uppercase();
        for (name, prefix) in [("IX", 0xDD), ("IY", 0xFD)] {
            if let Some(rest) = iu.strip_prefix(name) {
                let rest = rest.trim();
                if rest.is_empty() {
                    return O::Idx(prefix, "");
                }
                if rest.starts_with('+') || rest.starts_with('-') {
                    let start = inner.len() - rest.len();
                    return O::Idx(prefix, inner[start..].trim());
                }
            }
        }
        return O::Mem(inner);
    }
    O::Imm(t)
}

fn condition(t: &str) -> Option<u8> {
    Some(match t.to_ascii_uppercase().as_str() {
        "NZ" => 0,
        "Z" => 1,
        "NC" => 2,
        "C" => 3,
        "PO" => 4,
        "PE" => 5,
        "P" => 6,
        "M" => 7,
        _ => return None,
    })
}

/// Formes valides de chaque mnémonique (proposées quand les opérandes sont refusés).
pub fn forms(mn: &str) -> &'static [&'static str] {
    match mn {
        "LD" => &[
            "LD r,r'", "LD r,n", "LD r,(HL)", "LD (HL),r", "LD (HL),n", "LD r,(IX+d)", "LD (IX+d),r",
            "LD A,(BC)", "LD A,(DE)", "LD A,(nn)", "LD (BC),A", "LD (DE),A", "LD (nn),A", "LD rr,nn",
            "LD HL,(nn)", "LD (nn),HL", "LD rr,(nn)", "LD (nn),rr", "LD SP,HL", "LD A,I", "LD I,A",
        ],
        "ADD" => &["ADD A,r", "ADD A,n", "ADD A,(HL)", "ADD A,(IX+d)", "ADD HL,rr", "ADD IX,rr"],
        "ADC" | "SBC" => &["ADC A,r", "ADC A,n", "ADC A,(HL)", "ADC HL,rr"],
        "SUB" | "AND" | "XOR" | "OR" | "CP" => &["SUB r", "SUB n", "SUB (HL)", "SUB (IX+d)"],
        "INC" | "DEC" => &["INC r", "INC (HL)", "INC (IX+d)", "INC rr", "INC IX"],
        "RLC" | "RRC" | "RL" | "RR" | "SLA" | "SRA" | "SLL" | "SRL" => &["RL r", "RL (HL)", "RL (IX+d)"],
        "BIT" | "SET" | "RES" => &["BIT b,r", "BIT b,(HL)", "BIT b,(IX+d)  (b = 0..7)"],
        "JP" => &["JP nn", "JP cc,nn", "JP (HL)", "JP (IX)"],
        "JR" => &["JR e", "JR NZ,e", "JR Z,e", "JR NC,e", "JR C,e"],
        "DJNZ" => &["DJNZ e"],
        "CALL" => &["CALL nn", "CALL cc,nn"],
        "RET" => &["RET", "RET cc"],
        "RST" => &["RST 00H/08H/10H/18H/20H/28H/30H/38H"],
        "PUSH" | "POP" => &["PUSH BC/DE/HL/AF", "PUSH IX/IY"],
        "EX" => &["EX DE,HL", "EX AF,AF'", "EX (SP),HL", "EX (SP),IX"],
        "IN" => &["IN A,(n)", "IN r,(C)"],
        "OUT" => &["OUT (n),A", "OUT (C),r"],
        "IM" => &["IM 0", "IM 1", "IM 2"],
        _ => &[],
    }
}

fn bad(mn: &str) -> Issue {
    let mut args = vec![mn.to_string()];
    let list = forms(mn);
    if !list.is_empty() {
        args.push(list.join("  ·  "));
    }
    Issue { code: if list.is_empty() { "noOperands" } else { "badOperands" }, args, fix: Fix::None }
}

/// Instructions sans opérande.
fn simple(mn: &str) -> Option<&'static [u8]> {
    Some(match mn {
        "NOP" => &[0x00],
        "HALT" => &[0x76],
        "DI" => &[0xF3],
        "EI" => &[0xFB],
        "EXX" => &[0xD9],
        "RLCA" => &[0x07],
        "RRCA" => &[0x0F],
        "RLA" => &[0x17],
        "RRA" => &[0x1F],
        "DAA" => &[0x27],
        "CPL" => &[0x2F],
        "SCF" => &[0x37],
        "CCF" => &[0x3F],
        "NEG" => &[0xED, 0x44],
        "RETN" => &[0xED, 0x45],
        "RETI" => &[0xED, 0x4D],
        "RRD" => &[0xED, 0x67],
        "RLD" => &[0xED, 0x6F],
        "LDI" => &[0xED, 0xA0],
        "CPI" => &[0xED, 0xA1],
        "INI" => &[0xED, 0xA2],
        "OUTI" => &[0xED, 0xA3],
        "LDD" => &[0xED, 0xA8],
        "CPD" => &[0xED, 0xA9],
        "IND" => &[0xED, 0xAA],
        "OUTD" => &[0xED, 0xAB],
        "LDIR" => &[0xED, 0xB0],
        "CPIR" => &[0xED, 0xB1],
        "INIR" => &[0xED, 0xB2],
        "OTIR" => &[0xED, 0xB3],
        "LDDR" => &[0xED, 0xB8],
        "CPDR" => &[0xED, 0xB9],
        "INDR" => &[0xED, 0xBA],
        "OTDR" => &[0xED, 0xBB],
        _ => return None,
    })
}

/// Tous les mnémoniques (pour reconnaître les instructions et proposer un nom proche).
pub const MNEMONICS: &[&str] = &[
    "ADC", "ADD", "AND", "BIT", "CALL", "CCF", "CP", "CPD", "CPDR", "CPI", "CPIR", "CPL", "DAA", "DEC", "DI",
    "DJNZ", "EI", "EX", "EXX", "HALT", "IM", "IN", "INC", "IND", "INDR", "INI", "INIR", "JP", "JR", "LD", "LDD",
    "LDDR", "LDI", "LDIR", "NEG", "NOP", "OR", "OTDR", "OTIR", "OUT", "OUTD", "OUTI", "POP", "PUSH", "RES", "RET",
    "RETI", "RETN", "RL", "RLA", "RLC", "RLCA", "RLD", "RR", "RRA", "RRC", "RRCA", "RRD", "RST", "SBC", "SCF",
    "SET", "SLA", "SLL", "SRA", "SRL", "SUB", "XOR",
];

/// Registre 8 bits ou case mémoire (HL), (IX+d) : (préfixe, code, déplacement).
fn rq<'a>(o: &O<'a>) -> Option<(Option<u8>, u8, Option<&'a str>)> {
    match *o {
        O::R(r) => Some((None, r, None)),
        O::HLi => Some((None, 6, None)),
        O::Idx(p, d) => Some((Some(p), 6, Some(d))),
        _ => None,
    }
}

struct Enc<'v> {
    v: &'v mut dyn Values,
}

impl Enc<'_> {
    fn byte(&mut self, t: &str) -> Result<u8, Issue> {
        let x = self.v.value(t)?;
        if self.v.checking() && !(-128..=255).contains(&x) {
            return Err(issue("byteRange", &[t, &x.to_string()]));
        }
        Ok(x as u8)
    }

    fn word(&mut self, t: &str) -> Result<[u8; 2], Issue> {
        let x = self.v.value(t)?;
        if self.v.checking() && !(-32768..=65535).contains(&x) {
            return Err(issue("wordRange", &[t, &x.to_string()]));
        }
        Ok((x as u16).to_le_bytes())
    }

    fn disp(&mut self, t: &str) -> Result<u8, Issue> {
        if t.is_empty() {
            return Ok(0);
        }
        let x = self.v.value(t)?;
        if self.v.checking() && !(-128..=127).contains(&x) {
            return Err(issue("indexRange", &[&x.to_string()]));
        }
        Ok(x as u8)
    }

    /// Déplacement relatif d'un JR ou DJNZ (instruction de 2 octets).
    fn rel(&mut self, t: &str, mn: &str) -> Result<u8, Issue> {
        let target = self.v.value(t)?;
        let offset = target - (self.v.pc() as i64 + 2);
        if self.v.checking() && !(-128..=127).contains(&offset) {
            let mut i = issue("jrRange", &[mn, &offset.to_string()]);
            if mn == "JR" {
                i.fix = Fix::Mnemonic("JP".to_string());
            }
            return Err(i);
        }
        Ok(offset as u8)
    }

    /// Instruction sur un registre ou une case mémoire : [préfixe] opcode [déplacement].
    fn with_r(&mut self, q: (Option<u8>, u8, Option<&str>), opcode: u8) -> Result<Vec<u8>, Issue> {
        let mut out = Vec::new();
        if let Some(p) = q.0 {
            out.push(p);
        }
        out.push(opcode);
        if let Some(d) = q.2 {
            out.push(self.disp(d)?);
        }
        Ok(out)
    }

    /// Préfixe CB : rotations, BIT, SET, RES. Avec IX/IY : préfixe, CB, déplacement, opcode.
    fn cb(&mut self, q: (Option<u8>, u8, Option<&str>), op: u8) -> Result<Vec<u8>, Issue> {
        match q.0 {
            Some(p) => {
                let d = self.disp(q.2.unwrap_or(""))?;
                Ok(vec![p, 0xCB, d, op | 6])
            }
            None => Ok(vec![0xCB, op | q.1]),
        }
    }
}

/// Encode une instruction (mnémonique en majuscules, opérandes en texte).
pub fn encode(mn: &str, ops: &[&str], v: &mut dyn Values) -> Result<Vec<u8>, Issue> {
    if let Some(bytes) = simple(mn) {
        if !ops.is_empty() {
            return Err(issue("noOperands", &[mn]));
        }
        return Ok(bytes.to_vec());
    }
    let o: Vec<O> = ops.iter().map(|t| operand(t)).collect();
    let mut e = Enc { v };
    let alu = |m: &str| match m {
        "ADD" => 0,
        "ADC" => 1,
        "SUB" => 2,
        "SBC" => 3,
        "AND" => 4,
        "XOR" => 5,
        "OR" => 6,
        _ => 7,
    };
    match mn {
        "LD" => {
            let [dst, src] = o.as_slice() else { return Err(bad(mn)) };
            ld(&mut e, *dst, *src).ok_or_else(|| bad(mn))??
        }
        "ADD" | "ADC" | "SBC" | "SUB" | "AND" | "XOR" | "OR" | "CP" => {
            let op = alu(mn);
            // 16 bits : ADD HL,rr  ADC HL,rr  SBC HL,rr  ADD IX,rr
            if let [dst, src] = o.as_slice() {
                match (mn, *dst, *src) {
                    ("ADD", O::RP(2), O::RP(rp)) => return Ok(vec![0x09 | rp << 4]),
                    ("ADC", O::RP(2), O::RP(rp)) => return Ok(vec![0xED, 0x4A | rp << 4]),
                    ("SBC", O::RP(2), O::RP(rp)) => return Ok(vec![0xED, 0x42 | rp << 4]),
                    ("ADD", O::XY(p), O::RP(rp)) if rp != 2 => return Ok(vec![p, 0x09 | rp << 4]),
                    ("ADD", O::XY(p), O::XY(q)) if p == q => return Ok(vec![p, 0x29]),
                    _ => {}
                }
            }
            // 8 bits : « ADD A,x », et « SUB x » (la forme « SUB A,x » est aussi acceptée).
            let src = match (mn, o.as_slice()) {
                (_, [O::R(7), s]) => *s,
                ("SUB" | "AND" | "XOR" | "OR" | "CP", [s]) => *s,
                _ => return Err(bad(mn)),
            };
            if let Some(q) = rq(&src) {
                e.with_r(q, 0x80 | op << 3 | q.1)?
            } else if let O::Imm(t) = src {
                vec![0xC6 | op << 3, e.byte(t)?]
            } else {
                return Err(bad(mn));
            }
        }
        "INC" | "DEC" => {
            let [x] = o.as_slice() else { return Err(bad(mn)) };
            let dec = (mn == "DEC") as u8;
            match *x {
                O::RP(rp) => vec![0x03 | rp << 4 | dec << 3],
                O::XY(p) => vec![p, 0x23 | dec << 3],
                ref x => match rq(x) {
                    Some(q) => e.with_r(q, 0x04 | q.1 << 3 | dec)?,
                    None => return Err(bad(mn)),
                },
            }
        }
        "RLC" | "RRC" | "RL" | "RR" | "SLA" | "SRA" | "SLL" | "SRL" => {
            let [x] = o.as_slice() else { return Err(bad(mn)) };
            let op = ["RLC", "RRC", "RL", "RR", "SLA", "SRA", "SLL", "SRL"].iter().position(|m| *m == mn).unwrap() as u8;
            let q = rq(x).ok_or_else(|| bad(mn))?;
            e.cb(q, op << 3)?
        }
        "BIT" | "RES" | "SET" => {
            let [O::Imm(b), x] = o.as_slice() else { return Err(bad(mn)) };
            let q = rq(x).ok_or_else(|| bad(mn))?;
            let bit = e.v.value(b)?;
            if !(0..=7).contains(&bit) {
                return Err(issue("bitRange", &[&bit.to_string()]));
            }
            let base = match mn {
                "BIT" => 0x40,
                "RES" => 0x80,
                _ => 0xC0,
            };
            e.cb(q, base | (bit as u8) << 3)?
        }
        "JP" => match o.as_slice() {
            [O::HLi] => vec![0xE9],
            [O::Idx(p, "")] => vec![*p, 0xE9],
            [O::Imm(t)] => [&[0xC3][..], &e.word(t)?].concat(),
            [_, O::Imm(t)] => {
                let cc = condition(ops[0]).ok_or_else(|| issue("badCondition", &[ops[0]]))?;
                [&[0xC2 | cc << 3][..], &e.word(t)?].concat()
            }
            _ => return Err(bad(mn)),
        },
        "CALL" => match o.as_slice() {
            [O::Imm(t)] => [&[0xCD][..], &e.word(t)?].concat(),
            [_, O::Imm(t)] => {
                let cc = condition(ops[0]).ok_or_else(|| issue("badCondition", &[ops[0]]))?;
                [&[0xC4 | cc << 3][..], &e.word(t)?].concat()
            }
            _ => return Err(bad(mn)),
        },
        "RET" => match ops {
            [] => vec![0xC9],
            [c] => vec![0xC0 | condition(c).ok_or_else(|| issue("badCondition", &[*c]))? << 3],
            _ => return Err(bad(mn)),
        },
        "JR" => match o.as_slice() {
            [O::Imm(t)] => vec![0x18, e.rel(t, mn)?],
            [_, O::Imm(t)] => {
                let cc = condition(ops[0]).ok_or_else(|| issue("badCondition", &[ops[0]]))?;
                if cc > 3 {
                    let mut i = issue("jrCondition", &[ops[0]]);
                    i.fix = Fix::Mnemonic("JP".to_string());
                    return Err(i);
                }
                vec![0x20 | cc << 3, e.rel(t, mn)?]
            }
            _ => return Err(bad(mn)),
        },
        "DJNZ" => match o.as_slice() {
            [O::Imm(t)] => vec![0x10, e.rel(t, mn)?],
            _ => return Err(bad(mn)),
        },
        "RST" => {
            let [O::Imm(t)] = o.as_slice() else { return Err(bad(mn)) };
            let x = e.v.value(t)?;
            if x & !0x38 != 0 {
                return Err(issue("rstValue", &[t]));
            }
            vec![0xC7 | x as u8]
        }
        "PUSH" | "POP" => {
            let base = if mn == "PUSH" { 0xC5 } else { 0xC1 };
            match o.as_slice() {
                [O::RP(rp)] if *rp < 3 => vec![base | rp << 4],
                [O::AF] => vec![base | 0x30],
                [O::XY(p)] => vec![*p, base | 0x20],
                _ => return Err(bad(mn)),
            }
        }
        "EX" => match o.as_slice() {
            [O::RP(1), O::RP(2)] | [O::RP(2), O::RP(1)] => vec![0xEB],
            [O::AF, O::AFx] => vec![0x08],
            [O::SPi, O::RP(2)] => vec![0xE3],
            [O::SPi, O::XY(p)] => vec![*p, 0xE3],
            _ => return Err(bad(mn)),
        },
        "IN" => match o.as_slice() {
            [O::R(7), O::Mem(t)] => vec![0xDB, e.byte(t)?],
            [O::R(r), O::Ci] => vec![0xED, 0x40 | r << 3],
            _ => return Err(bad(mn)),
        },
        "OUT" => match o.as_slice() {
            [O::Mem(t), O::R(7)] => vec![0xD3, e.byte(t)?],
            [O::Ci, O::R(r)] => vec![0xED, 0x41 | r << 3],
            _ => return Err(bad(mn)),
        },
        "IM" => {
            let [O::Imm(t)] = o.as_slice() else { return Err(bad(mn)) };
            match e.v.value(t)? {
                0 => vec![0xED, 0x46],
                1 => vec![0xED, 0x56],
                2 => vec![0xED, 0x5E],
                _ => return Err(issue("imMode", &[t])),
            }
        }
        _ => return Err(issue("unknownMnemonic", &[mn])),
    }
    .pipe(Ok)
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}

/// LD : toutes ses formes; `None` si la combinaison n'existe pas.
fn ld(e: &mut Enc, dst: O, src: O) -> Option<Result<Vec<u8>, Issue>> {
    let r = |res: Result<Vec<u8>, Issue>| Some(res);
    let cat = |a: &[u8], b: Result<[u8; 2], Issue>| b.map(|w| [a, &w[..]].concat());
    match (dst, src) {
        (O::R(7), O::BCi) => r(Ok(vec![0x0A])),
        (O::R(7), O::DEi) => r(Ok(vec![0x1A])),
        (O::BCi, O::R(7)) => r(Ok(vec![0x02])),
        (O::DEi, O::R(7)) => r(Ok(vec![0x12])),
        (O::R(7), O::Mem(t)) => r(cat(&[0x3A], e.word(t))),
        (O::Mem(t), O::R(7)) => r(cat(&[0x32], e.word(t))),
        (O::R(7), O::I) => r(Ok(vec![0xED, 0x57])),
        (O::R(7), O::Rr) => r(Ok(vec![0xED, 0x5F])),
        (O::I, O::R(7)) => r(Ok(vec![0xED, 0x47])),
        (O::Rr, O::R(7)) => r(Ok(vec![0xED, 0x4F])),
        (O::RP(rp), O::Imm(t)) => r(cat(&[0x01 | rp << 4], e.word(t))),
        (O::XY(p), O::Imm(t)) => r(cat(&[p, 0x21], e.word(t))),
        (O::RP(2), O::Mem(t)) => r(cat(&[0x2A], e.word(t))),
        (O::RP(rp), O::Mem(t)) => r(cat(&[0xED, 0x4B | rp << 4], e.word(t))),
        (O::XY(p), O::Mem(t)) => r(cat(&[p, 0x2A], e.word(t))),
        (O::Mem(t), O::RP(2)) => r(cat(&[0x22], e.word(t))),
        (O::Mem(t), O::RP(rp)) => r(cat(&[0xED, 0x43 | rp << 4], e.word(t))),
        (O::Mem(t), O::XY(p)) => r(cat(&[p, 0x22], e.word(t))),
        (O::RP(3), O::RP(2)) => r(Ok(vec![0xF9])),
        (O::RP(3), O::XY(p)) => r(Ok(vec![p, 0xF9])),
        (d, O::Imm(t)) => {
            let q = rq(&d)?;
            r(e.with_r(q, 0x06 | q.1 << 3).and_then(|mut v| {
                v.push(e.byte(t)?);
                Ok(v)
            }))
        }
        (d, s) => {
            let qd = rq(&d)?;
            let qs = rq(&s)?;
            // Pas de mémoire vers mémoire (76h serait HALT).
            if qd.1 == 6 && qs.1 == 6 {
                return None;
            }
            let q = if qd.0.is_some() { qd } else { qs };
            r(e.with_r((q.0, 0, q.2), 0x40 | qd.1 << 3 | qs.1))
        }
    }
}

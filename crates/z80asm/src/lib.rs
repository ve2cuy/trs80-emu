//! Assembleur Z80 pour le TRS-80, sans dépendance (`no_std` + `alloc`).
//!
//! Syntaxe des assembleurs de l'époque (EDTASM, zmac) :
//!
//! ```text
//! ; commentaire                  * commentaire (en colonne 1)
//! START:  LD      HL,MSG         ; étiquette avec deux-points...
//! LOOP    DJNZ    LOOP           ; ...ou en colonne 1
//! COUNT   EQU     10             ; aussi : COUNT = 10, DEFL
//!         ORG     5200H          ; adresse (5200H par défaut : programmes LDOS)
//! MSG:    DB      'HELLO',0DH    ; DB DEFB DM DEFM TEXT; DW DEFW; DS DEFS
//!         END     START          ; adresse de départ du .CMD
//! ```
//!
//! Nombres : 4467H, 0x4467, $4467, 255, %1010, 1010B, 'A'. Les symboles de [`builtins`]
//! (services de LDOS, routines de la ROM) sont prédéfinis.
//!
//! Chaque diagnostic porte un code (traduit par la page), ses arguments et, quand c'est
//! possible, la ligne corrigée.

#![no_std]

extern crate alloc;

pub mod builtins;
mod encode;
mod expr;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use encode::{Fix, Issue, Values, issue};
pub use encode::{MNEMONICS, forms};

/// Adresse par défaut : début de la mémoire libre sous LDOS.
pub const DEFAULT_ORG: u16 = 0x5200;

const DIRECTIVES: &[&str] = &[
    "ORG", "EQU", "DEFL", "=", "DB", "DEFB", "BYTE", "DM", "DEFM", "TEXT", "ASCII", "DW", "DEFW", "WORD", "DS",
    "DEFS", "BLOCK", "RMB", "END",
];
/// Directives de mise en page des listings : acceptées et ignorées.
const IGNORED: &[&str] = &["TITLE", "SUBTTL", "PAGE", "EJECT", "LIST", "NOLIST", "NAME"];
const RESERVED: &[&str] = &[
    "A", "B", "C", "D", "E", "H", "L", "I", "R", "AF", "BC", "DE", "HL", "SP", "IX", "IY", "NZ", "Z", "NC", "PO",
    "PE", "P", "M",
];

/// Un message pour une ligne.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Ligne (à partir de 1).
    pub line: usize,
    pub warning: bool,
    /// Code du message (clé de traduction).
    pub code: &'static str,
    pub args: Vec<String>,
    /// Correction proposée : la ligne corrigée, ou une ligne à insérer avant (`insert`).
    pub fix: Option<String>,
    pub insert: bool,
}

/// Code produit par une ligne : adresse et nombre d'octets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LineInfo {
    pub addr: u16,
    pub len: u16,
}

/// Résultat de l'assemblage.
#[derive(Debug, Clone, Default)]
pub struct Assembly {
    /// Blocs d'octets contigus, dans l'ordre du source : (adresse, octets).
    pub blocks: Vec<(u16, Vec<u8>)>,
    /// Adresse de départ (END, sinon le premier octet produit).
    pub entry: u16,
    /// Pour chaque ligne du source.
    pub lines: Vec<LineInfo>,
    /// Symboles du programme, par ordre alphabétique.
    pub symbols: Vec<(String, u16)>,
    /// Symboles prédéfinis utilisés (services de LDOS, routines de la ROM).
    pub builtins_used: Vec<&'static str>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Assembly {
    pub fn errors(&self) -> usize {
        self.diagnostics.iter().filter(|d| !d.warning).count()
    }

    pub fn ok(&self) -> bool {
        self.errors() == 0
    }

    /// Nombre d'octets produits.
    pub fn size(&self) -> usize {
        self.blocks.iter().map(|b| b.1.len()).sum()
    }

    /// Adresses du programme : (première, dernière), ou `None` s'il est vide.
    pub fn bounds(&self) -> Option<(u16, u16)> {
        let lo = self.blocks.iter().map(|b| b.0).min()?;
        let hi = self.blocks.iter().map(|b| b.0 as usize + b.1.len() - 1).max()?;
        Some((lo, hi.min(0xFFFF) as u16))
    }

    /// Le programme appelle-t-il des services de LDOS ?
    pub fn uses_ldos(&self) -> bool {
        self.builtins_used.iter().any(|n| builtins::find(n).is_some_and(|b| b.ldos))
    }

    /// Fichier .CMD : blocs de chargement (01h) de 256 octets au plus, puis l'adresse de
    /// départ (02h), comme zmac.
    pub fn cmd(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for (addr, data) in &self.blocks {
            for (i, chunk) in data.chunks(256).enumerate() {
                let a = addr.wrapping_add((i * 256) as u16);
                out.push(0x01);
                out.push(((chunk.len() + 2) & 0xFF) as u8);
                out.extend_from_slice(&a.to_le_bytes());
                out.extend_from_slice(chunk);
            }
        }
        out.extend_from_slice(&[0x02, 0x02]);
        out.extend_from_slice(&self.entry.to_le_bytes());
        out
    }
}

// ---------------------------------------------------------------- analyse des lignes

fn is_ident(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(expr::is_ident_start) && c.all(expr::is_ident_char)
}

/// Mnémonique ou directive principale. Les synonymes (BYTE, WORD, TEXT...) et les directives
/// de mise en page (TITLE, PAGE...) n'en font pas partie : ce sont aussi des noms de symboles
/// courants, reconnus comme directives seulement à la place d'une instruction.
fn is_keyword(word: &str) -> bool {
    const CORE: &[&str] = &["ORG", "EQU", "DEFL", "DB", "DEFB", "DM", "DEFM", "DW", "DEFW", "DS", "DEFS", "END"];
    MNEMONICS.contains(&word) || CORE.contains(&word)
}

/// Fin de la partie « code » d'une ligne : avant le « ; » qui n'est pas dans une chaîne.
/// L'apostrophe de AF' n'ouvre pas de chaîne.
fn code_part(line: &str) -> &str {
    let b = line.as_bytes();
    let mut quote: Option<u8> = None;
    for i in 0..b.len() {
        let c = b[i];
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => {
                if c == b';' {
                    return &line[..i];
                }
                if c == b'"' || (c == b'\'' && !is_af_quote(line, i)) {
                    quote = Some(c);
                }
            }
        }
    }
    line
}

fn is_af_quote(line: &str, i: usize) -> bool {
    let b = line.as_bytes();
    i >= 2 && b[i - 2].eq_ignore_ascii_case(&b'A') && b[i - 1].eq_ignore_ascii_case(&b'F')
        && (i < 3 || !expr::is_ident_char(b[i - 3] as char))
}

/// Opérandes séparés par des virgules (hors chaînes et parenthèses).
fn split_operands(s: &str) -> Vec<&str> {
    let s = s.trim();
    if s.is_empty() {
        return Vec::new();
    }
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut quote: Option<u8> = None;
    let mut depth = 0i32;
    let mut start = 0;
    for i in 0..b.len() {
        let c = b[i];
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                b'"' => quote = Some(c),
                b'\'' if !is_af_quote(s, i) => quote = Some(c),
                b'(' => depth += 1,
                b')' => depth -= 1,
                b',' if depth == 0 => {
                    out.push(s[start..i].trim());
                    start = i + 1;
                }
                _ => {}
            },
        }
    }
    out.push(s[start..].trim());
    out
}

fn first_word(s: &str) -> (&str, &str) {
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    (&s[..end], &s[end..])
}

#[derive(Debug, Default)]
struct Stmt<'a> {
    label: Option<&'a str>,
    op: Option<String>,
    operands: Vec<&'a str>,
    /// Instruction écrite en colonne 1 (là où vont les étiquettes).
    col1: bool,
}

fn parse_line(line: &str) -> Stmt<'_> {
    if line.starts_with('*') {
        return Stmt::default();
    }
    let code = code_part(line);
    let mut rest = code.trim();
    if rest.is_empty() {
        return Stmt::default();
    }
    let indented = code.starts_with([' ', '\t']);
    let mut st = Stmt::default();
    // NOM = valeur
    if let Some(eq) = rest.find('=') {
        let name = rest[..eq].trim();
        if is_ident(name) && !is_keyword(&name.to_ascii_uppercase()) {
            st.label = Some(name.trim_end_matches(':'));
            st.op = Some("EQU".into());
            st.operands = split_operands(&rest[eq + 1..]);
            return st;
        }
    }
    let (w, after) = first_word(rest);
    if let Some(colon) = w.find(':') {
        st.label = Some(&w[..colon]);
        rest = rest[colon + 1..].trim_start();
    } else if !indented {
        let upper = w.to_ascii_uppercase();
        let next = first_word(after.trim_start()).0.to_ascii_uppercase();
        if is_keyword(&upper) && !is_keyword(&next) {
            st.col1 = true;
        } else {
            st.label = Some(w);
            rest = after.trim_start();
        }
    }
    let (op, args) = first_word(rest);
    if !op.is_empty() {
        st.op = Some(op.to_ascii_uppercase());
        st.operands = split_operands(args);
    }
    st
}

/// Chaîne entre apostrophes ou guillemets (pour DB) : ses octets.
fn string_literal(t: &str) -> Option<Vec<u8>> {
    let q = *t.as_bytes().first()?;
    if !(q == b'\'' || q == b'"') || t.len() < 2 || *t.as_bytes().last()? != q {
        return None;
    }
    let inner = &t.as_bytes()[1..t.len() - 1];
    let mut out = Vec::new();
    let mut i = 0;
    while i < inner.len() {
        if inner[i] == q {
            if inner.get(i + 1) == Some(&q) {
                i += 1;
            } else {
                return None; // 'A'+'B' : une expression, pas une chaîne
            }
        }
        out.push(inner[i]);
        i += 1;
    }
    Some(out)
}

// ---------------------------------------------------------------- suggestions

fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = Vec::with_capacity(b.len() + 1);
        cur.push(i);
        for j in 1..=b.len() {
            let cost = (a[i - 1] != b[j - 1]) as usize;
            cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Nom le plus proche (au plus 2 différences, et moins que la longueur du nom).
fn closest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let limit = 2.min(name.len().saturating_sub(1)).max(1);
    candidates
        .map(|c| (distance(name, c), c))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

/// « FFH », « C000H » : un nombre hexadécimal doit commencer par un chiffre.
fn hex_without_digit(name: &str) -> bool {
    name.len() > 1 && name.ends_with('H') && name[..name.len() - 1].chars().all(|c| c.is_ascii_hexdigit())
        && !name.starts_with(|c: char| c.is_ascii_digit())
}

/// Remplace la première occurrence du mot `old` (sans tenir compte de la casse) hors des
/// commentaires.
fn replace_word(line: &str, old: &str, new: &str) -> Option<String> {
    let code_len = code_part(line).len();
    let upper = line[..code_len].to_ascii_uppercase();
    let target = old.to_ascii_uppercase();
    let b = upper.as_bytes();
    let mut from = 0;
    while let Some(pos) = upper[from..].find(&target) {
        let start = from + pos;
        let end = start + target.len();
        let before_ok = start == 0 || !expr::is_ident_char(b[start - 1] as char);
        let after_ok = end >= b.len() || !expr::is_ident_char(b[end] as char);
        if before_ok && after_ok {
            return Some(format!("{}{}{}", &line[..start], new, &line[end..]));
        }
        from = end;
    }
    None
}

// ---------------------------------------------------------------- passes

#[derive(Debug, Clone)]
struct Sym {
    value: Option<i64>,
    line: usize,
}

struct Ctx<'s> {
    symbols: &'s BTreeMap<String, Sym>,
    pc: i64,
    final_pass: bool,
    used: &'s mut Vec<&'static str>,
}

impl expr::Scope for Ctx<'_> {
    fn symbol(&mut self, name: &str) -> Option<i64> {
        if let Some(s) = self.symbols.get(name) {
            return s.value;
        }
        let b = builtins::find(name)?;
        if !self.used.contains(&b.name) {
            self.used.push(b.name);
        }
        Some(b.value as i64)
    }

    fn pc(&self) -> i64 {
        self.pc
    }
}

impl Ctx<'_> {
    fn undefined(&self, name: &str) -> Issue {
        if hex_without_digit(name) {
            let fixed = format!("0{name}");
            let mut i = issue("hexDigit", &[name, &fixed]);
            i.fix = Fix::Word(name.into(), fixed);
            return i;
        }
        let names = self.symbols.keys().map(|k| k.as_str()).chain(builtins::BUILTINS.iter().map(|b| b.name));
        match closest(name, names) {
            Some(best) => {
                let mut i = issue("undefinedSymbol", &[name, best]);
                i.fix = Fix::Word(name.into(), best.into());
                i
            }
            None => issue("undefinedSymbol", &[name]),
        }
    }

    /// Valeur d'une expression qui doit être connue dès cette passe (ORG, DS, EQU).
    fn eval(&mut self, text: &str) -> Result<Option<i64>, Issue> {
        let mut undefined = Vec::new();
        match expr::eval(text, self, &mut undefined) {
            Ok(Some(v)) => Ok(Some(v)),
            Ok(None) if self.final_pass => Err(self.undefined(&undefined[0])),
            Ok(None) => Ok(None),
            Err(e) => Err(issue(e.code, &[&e.arg])),
        }
    }
}

impl Values for Ctx<'_> {
    fn value(&mut self, text: &str) -> Result<i64, Issue> {
        match self.eval(text) {
            Ok(v) => Ok(v.unwrap_or(0)),
            Err(e) if self.final_pass => Err(e),
            Err(_) => Ok(0),
        }
    }

    fn pc(&self) -> u16 {
        self.pc as u16
    }

    fn checking(&self) -> bool {
        self.final_pass
    }
}

struct Pass {
    blocks: Vec<(u16, Vec<u8>)>,
    lines: Vec<LineInfo>,
    entry: Option<i64>,
    diagnostics: Vec<Diagnostic>,
}

fn emit(blocks: &mut Vec<(u16, Vec<u8>)>, addr: i64, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let addr = addr as u16;
    if let Some((a, data)) = blocks.last_mut()
        && (*a as usize + data.len()) == addr as usize
    {
        data.extend_from_slice(bytes);
        return;
    }
    blocks.push((addr, bytes.to_vec()));
}

fn diagnostic(line_no: usize, text: &str, stmt: &Stmt, warning: bool, i: Issue) -> Diagnostic {
    let (fix, insert) = match &i.fix {
        Fix::None => (None, false),
        Fix::Mnemonic(new) => (stmt.op.as_deref().and_then(|op| replace_word(text, op, new)), false),
        Fix::Word(old, new) => (replace_word(text, old, new), false),
        Fix::Insert(line) => (Some(line.clone()), true),
    };
    Diagnostic { line: line_no, warning, code: i.code, args: i.args, fix, insert }
}

fn run_pass(stmts: &[Stmt], text: &[&str], symbols: &mut BTreeMap<String, Sym>, used: &mut Vec<&'static str>, final_pass: bool) -> Pass {
    let mut pass = Pass { blocks: Vec::new(), lines: Vec::new(), entry: None, diagnostics: Vec::new() };
    let mut pc: i64 = DEFAULT_ORG as i64;
    let mut org_seen = false;
    let mut ended = false;
    for (i, st) in stmts.iter().enumerate() {
        let line_no = i + 1;
        let start = pc;
        if ended {
            pass.lines.push(LineInfo::default());
            continue;
        }
        let report = |pass: &mut Pass, warning: bool, issue: Issue| {
            if final_pass {
                pass.diagnostics.push(diagnostic(line_no, text[i], st, warning, issue));
            }
        };
        if st.col1 {
            report(&mut pass, true, issue("labelColumn", &[st.op.as_deref().unwrap_or("")]));
        }
        let op = st.op.as_deref();
        let is_equ = matches!(op, Some("EQU" | "DEFL" | "="));

        // Étiquette : l'adresse courante (sauf EQU, qui donne sa propre valeur).
        if let Some(label) = st.label {
            let name = label.to_ascii_uppercase();
            if !is_ident(label) {
                report(&mut pass, false, issue("badLabel", &[label]));
            } else if RESERVED.contains(&name.as_str()) || is_keyword(&name) {
                report(&mut pass, false, issue("reservedName", &[label]));
            } else if !is_equ {
                define(symbols, &name, Some(pc), line_no, op == Some("DEFL"));
                if let Some(first) = symbols.get(&name).filter(|s| s.line != line_no) {
                    let first = first.line.to_string();
                    report(&mut pass, false, issue("duplicateSymbol", &[label, &first]));
                }
            }
        }

        let mut bytes: Vec<u8> = Vec::new();
        let mut result: Result<(), Issue> = Ok(());
        let mut ctx = Ctx { symbols, pc, final_pass, used };
        match op {
            None => {}
            Some("EQU" | "DEFL" | "=") => match (st.label, st.operands.as_slice()) {
                (None, _) => result = Err(issue("equNeedsLabel", &[])),
                (Some(_), [value]) => match ctx.eval(value) {
                    Ok(v) => {
                        let name = st.label.unwrap().to_ascii_uppercase();
                        // Adresse du Model III pour un service du Model I ?
                        if final_pass
                            && let Some((_, m3, m1)) = builtins::MODEL3.iter().find(|m| m.0 == name)
                            && v == Some(*m3 as i64)
                        {
                            let fixed = format!("{m1:04X}H");
                            let mut is = issue("model3Address", &[&name, &format!("{m3:04X}H"), &fixed]);
                            is.fix = Fix::Word((*value).into(), fixed);
                            report(&mut pass, true, is);
                        }
                        drop(ctx);
                        define(symbols, &name, v, line_no, op == Some("DEFL"));
                        if let Some(first) = symbols.get(&name).filter(|s| s.line != line_no && op != Some("DEFL")) {
                            let first = first.line.to_string();
                            report(&mut pass, false, issue("duplicateSymbol", &[st.label.unwrap(), &first]));
                        }
                        ctx = Ctx { symbols, pc, final_pass, used };
                    }
                    Err(e) => result = Err(e),
                },
                _ => result = Err(issue("oneOperand", &["EQU"])),
            },
            Some("ORG") => match st.operands.as_slice() {
                [value] => match ctx.eval(value) {
                    Ok(Some(v)) => {
                        if !(0..=0xFFFF).contains(&v) {
                            result = Err(issue("wordRange", &[value, &v.to_string()]));
                        } else {
                            pc = v;
                            org_seen = true;
                            if v < 0x3C00 {
                                result = Err(issue("romArea", &[&format!("{v:04X}H")]));
                            } else if (0x4000..DEFAULT_ORG as i64).contains(&v) {
                                report(&mut pass, true, issue("lowOrg", &[&format!("{v:04X}H")]));
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(e) => result = Err(e),
                },
                _ => result = Err(issue("oneOperand", &["ORG"])),
            },
            Some("DB" | "DEFB" | "BYTE" | "DM" | "DEFM" | "TEXT" | "ASCII") => {
                if st.operands.is_empty() {
                    result = Err(issue("expectedValue", &[]));
                }
                for t in &st.operands {
                    ctx.pc = pc + bytes.len() as i64; // $ : adresse de l'octet produit, comme zmac
                    if let Some(s) = string_literal(t) {
                        bytes.extend(s);
                        continue;
                    }
                    match ctx.value(t) {
                        Ok(v) if final_pass && !(-128..=255).contains(&v) => {
                            result = Err(issue("byteRange", &[t, &v.to_string()]));
                        }
                        Ok(v) => bytes.push(v as u8),
                        Err(e) => {
                            result = Err(e);
                            bytes.push(0);
                        }
                    }
                }
            }
            Some("DW" | "DEFW" | "WORD") => {
                if st.operands.is_empty() {
                    result = Err(issue("expectedValue", &[]));
                }
                for t in &st.operands {
                    ctx.pc = pc + bytes.len() as i64;
                    match ctx.value(t) {
                        Ok(v) if final_pass && !(-32768..=65535).contains(&v) => {
                            result = Err(issue("wordRange", &[t, &v.to_string()]));
                        }
                        Ok(v) => bytes.extend_from_slice(&(v as u16).to_le_bytes()),
                        Err(e) => {
                            result = Err(e);
                            bytes.extend_from_slice(&[0, 0]);
                        }
                    }
                }
            }
            Some("DS" | "DEFS" | "BLOCK" | "RMB") => match st.operands.as_slice() {
                [count, rest @ ..] if rest.len() <= 1 => match ctx.eval(count) {
                    Ok(n) => {
                        let n = n.unwrap_or(0);
                        if !(0..=0xFFFF).contains(&n) {
                            result = Err(issue("wordRange", &[count, &n.to_string()]));
                        } else if let [fill] = rest {
                            let f = ctx.value(fill).unwrap_or(0) as u8;
                            bytes.resize(n as usize, f);
                        } else {
                            pc += n; // réservé, sans octets dans le .CMD
                        }
                    }
                    Err(e) => result = Err(e),
                },
                _ => result = Err(issue("oneOperand", &["DS"])),
            },
            Some("END") => {
                if let [value] = st.operands.as_slice() {
                    match ctx.eval(value) {
                        Ok(v) => pass.entry = v.or(pass.entry),
                        Err(e) => result = Err(e),
                    }
                }
                ended = true;
            }
            Some(o) if IGNORED.contains(&o) => {}
            Some(o) if MNEMONICS.contains(&o) => match encode::encode(o, &st.operands, &mut ctx) {
                Ok(b) => bytes = b,
                Err(e) => {
                    result = Err(e);
                    // Taille plausible, pour que les adresses suivantes restent cohérentes.
                    bytes = Vec::new();
                }
            },
            Some(o) => {
                let raw = text[i].trim();
                let mut is = issue("unknownMnemonic", &[o]);
                if st.operands.is_empty() && is_ident(o) && !raw.contains(':') {
                    // Une étiquette décalée : « START » au lieu de « START: ».
                    is = issue("labelNeedsColumn", &[o]);
                    is.fix = Fix::Word(o.into(), format!("{}:", first_word(raw).0));
                } else if let Some(best) = closest(o, MNEMONICS.iter().chain(DIRECTIVES.iter()).copied()) {
                    is = issue("unknownMnemonic", &[o, best]);
                    is.fix = Fix::Mnemonic(best.into());
                }
                result = Err(is);
            }
        }
        drop(ctx);
        if let Err(e) = result {
            report(&mut pass, false, e);
        }
        if !bytes.is_empty() {
            if !org_seen && pass.blocks.is_empty() {
                let mut is = issue("noOrg", &[&format!("{DEFAULT_ORG:04X}H")]);
                is.fix = Fix::Insert(format!("        ORG     {DEFAULT_ORG:04X}H"));
                report(&mut pass, true, is);
            }
            if pc + bytes.len() as i64 > 0x10000 {
                report(&mut pass, false, issue("addressOverflow", &[]));
            } else {
                emit(&mut pass.blocks, pc, &bytes);
            }
            pc += bytes.len() as i64;
        }
        // Octets produits (pas l'espace réservé par DS) : les points d'arrêt n'y vont que là.
        pass.lines.push(LineInfo { addr: start as u16, len: bytes.len().min(0xFFFF) as u16 });
    }
    pass
}

/// Définit ou met à jour un symbole. La première définition garde son numéro de ligne (pour
/// signaler les doublons); DEFL peut être redéfini.
fn define(symbols: &mut BTreeMap<String, Sym>, name: &str, value: Option<i64>, line: usize, redefinable: bool) {
    match symbols.get_mut(name) {
        Some(s) if s.line == line || redefinable => s.value = value,
        Some(_) => {}
        None => {
            symbols.insert(name.into(), Sym { value, line });
        }
    }
}

/// Assemble un source complet.
pub fn assemble(source: &str) -> Assembly {
    let text: Vec<&str> = source.split('\n').map(|l| l.trim_end_matches('\r')).collect();
    let stmts: Vec<Stmt> = text.iter().map(|l| parse_line(l)).collect();
    let mut symbols: BTreeMap<String, Sym> = BTreeMap::new();
    let mut used = Vec::new();
    // Passes préliminaires jusqu'à ce que les valeurs des symboles ne changent plus.
    let mut previous: Vec<Option<i64>> = Vec::new();
    for _ in 0..8 {
        run_pass(&stmts, &text, &mut symbols, &mut used, false);
        let values: Vec<Option<i64>> = symbols.values().map(|s| s.value).collect();
        if values == previous {
            break;
        }
        previous = values;
    }
    used.clear();
    let pass = run_pass(&stmts, &text, &mut symbols, &mut used, true);
    let entry = pass.entry.map(|e| e as u16).or_else(|| pass.blocks.first().map(|b| b.0)).unwrap_or(DEFAULT_ORG);
    Assembly {
        blocks: pass.blocks,
        entry,
        lines: pass.lines,
        symbols: symbols.iter().filter_map(|(k, s)| Some((k.clone(), s.value? as u16))).collect(),
        builtins_used: used,
        diagnostics: pass.diagnostics,
    }
}

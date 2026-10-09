//! Expressions : nombres (décimal, 4467H, 0x4467, $4467, %1010, 1010B, 17O), caractères
//! ('A'), symboles, `$` (adresse courante), opérateurs + - * / % MOD, << >> SHL SHR,
//! & AND, ^ XOR, | OR, unaires - + ~ NOT, fonctions HIGH() et LOW().
//!
//! Un symbole encore inconnu (premières passes) donne `None` au lieu d'une erreur : sa
//! valeur sera connue à la passe suivante.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Erreur de syntaxe d'une expression : code (voir `lib.rs`) et argument.
#[derive(Debug, Clone)]
pub struct ExprError {
    pub code: &'static str,
    pub arg: String,
}

fn err(code: &'static str, arg: &str) -> ExprError {
    ExprError { code, arg: arg.to_string() }
}

/// Ce que l'expression sait des symboles.
pub trait Scope {
    /// Valeur d'un symbole (nom en majuscules); `None` s'il n'est pas (encore) défini.
    fn symbol(&mut self, name: &str) -> Option<i64>;
    /// Adresse de l'instruction courante (`$`).
    fn pc(&self) -> i64;
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(i64),
    Ident(String),
    Dollar,
    Op(&'static str),
    LParen,
    RParen,
}

pub fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || matches!(c, '_' | '@' | '?' | '.')
}

pub fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '@' | '?' | '.' | '$')
}

/// Nombre écrit à la manière des assembleurs Z80, ou `None`.
pub fn parse_number(text: &str) -> Option<i64> {
    let t = text.to_ascii_uppercase();
    let (digits, radix) = if let Some(h) = t.strip_prefix("0X") {
        (h, 16)
    } else if let Some(h) = t.strip_prefix('$') {
        (h, 16)
    } else if let Some(b) = t.strip_prefix('%') {
        (b, 2)
    } else if let Some(h) = t.strip_suffix('H') {
        (h, 16)
    } else if t.len() > 1 && t.ends_with('B') && t[..t.len() - 1].chars().all(|c| c == '0' || c == '1') {
        (&t[..t.len() - 1], 2)
    } else if let Some(o) = t.strip_suffix('O').or_else(|| t.strip_suffix('Q')) {
        (o, 8)
    } else if let Some(d) = t.strip_suffix('D') {
        (d, 10)
    } else {
        (t.as_str(), 10)
    };
    if digits.is_empty() || digits.len() > 16 {
        return None;
    }
    i64::from_str_radix(digits, radix).ok()
}

fn tokenize(text: &str) -> Result<Vec<Tok>, ExprError> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    // Position d'opérande (après un opérateur ou au début) : % et $ y sont des préfixes.
    let operand_position = |out: &Vec<Tok>| matches!(out.last(), None | Some(Tok::Op(_)) | Some(Tok::LParen));
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            match parse_number(&word) {
                Some(v) => out.push(Tok::Num(v)),
                None => return Err(err("badNumber", &word)),
            }
            continue;
        }
        if c == '$' {
            if i + 1 < chars.len() && chars[i + 1].is_ascii_hexdigit() {
                let start = i;
                i += 1;
                while i < chars.len() && chars[i].is_ascii_alphanumeric() {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                match parse_number(&word) {
                    Some(v) => out.push(Tok::Num(v)),
                    None => return Err(err("badNumber", &word)),
                }
            } else {
                out.push(Tok::Dollar);
                i += 1;
            }
            continue;
        }
        if c == '%' && operand_position(&out) && i + 1 < chars.len() && matches!(chars[i + 1], '0' | '1') {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i].is_ascii_alphanumeric() {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            match parse_number(&word) {
                Some(v) => out.push(Tok::Num(v)),
                None => return Err(err("badNumber", &word)),
            }
            continue;
        }
        if c == '\'' || c == '"' {
            // Caractère : 'A' (ou 'AB' : deux octets); '' dans la chaîne = une apostrophe.
            let quote = c;
            i += 1;
            let mut value: Vec<char> = Vec::new();
            loop {
                if i >= chars.len() {
                    return Err(err("unterminatedString", text));
                }
                if chars[i] == quote {
                    if i + 1 < chars.len() && chars[i + 1] == quote {
                        value.push(quote);
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                value.push(chars[i]);
                i += 1;
            }
            match value.as_slice() {
                [a] => out.push(Tok::Num(*a as i64 & 0xFF)),
                [a, b] => out.push(Tok::Num(((*a as i64 & 0xFF) << 8) | (*b as i64 & 0xFF))),
                _ => return Err(err("badChar", text)),
            }
            continue;
        }
        if is_ident_start(c) {
            let start = i;
            while i < chars.len() && is_ident_char(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect::<String>().to_ascii_uppercase();
            let op = match word.as_str() {
                "AND" => Some("&"),
                "OR" => Some("|"),
                "XOR" => Some("^"),
                "MOD" => Some("%"),
                "SHL" => Some("<<"),
                "SHR" => Some(">>"),
                "NOT" => Some("~"),
                _ => None,
            };
            out.push(match op {
                Some(o) => Tok::Op(o),
                None => Tok::Ident(word),
            });
            continue;
        }
        let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
        if two == "<<" || two == ">>" {
            out.push(Tok::Op(if two == "<<" { "<<" } else { ">>" }));
            i += 2;
            continue;
        }
        let op = match c {
            '+' => "+",
            '-' => "-",
            '*' => "*",
            '/' => "/",
            '%' => "%",
            '&' => "&",
            '|' => "|",
            '^' => "^",
            '~' => "~",
            '(' => {
                out.push(Tok::LParen);
                i += 1;
                continue;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
                continue;
            }
            _ => return Err(err("unexpectedChar", &c.to_string())),
        };
        out.push(Tok::Op(op));
        i += 1;
    }
    Ok(out)
}

struct Parser<'a, S: Scope + ?Sized> {
    toks: Vec<Tok>,
    pos: usize,
    scope: &'a mut S,
    undefined: &'a mut Vec<String>,
}

type Val = Result<Option<i64>, ExprError>;

fn bin(a: Option<i64>, b: Option<i64>, f: impl Fn(i64, i64) -> i64) -> Option<i64> {
    Some(f(a?, b?))
}

impl<S: Scope + ?Sized> Parser<'_, S> {
    fn peek_op(&self) -> Option<&'static str> {
        match self.toks.get(self.pos) {
            Some(Tok::Op(o)) => Some(o),
            _ => None,
        }
    }

    fn level(&mut self, ops: &[&'static str], next: fn(&mut Self) -> Val) -> Val {
        let mut left = next(self)?;
        while let Some(op) = self.peek_op().filter(|o| ops.contains(o)) {
            self.pos += 1;
            let right = next(self)?;
            left = match op {
                "+" => bin(left, right, |a, b| a.wrapping_add(b)),
                "-" => bin(left, right, |a, b| a.wrapping_sub(b)),
                "*" => bin(left, right, |a, b| a.wrapping_mul(b)),
                "/" | "%" => {
                    if right == Some(0) {
                        return Err(err("divByZero", ""));
                    }
                    if op == "/" { bin(left, right, |a, b| a / b) } else { bin(left, right, |a, b| a % b) }
                }
                "<<" => bin(left, right, |a, b| a.wrapping_shl((b & 63) as u32)),
                ">>" => bin(left, right, |a, b| a.wrapping_shr((b & 63) as u32)),
                "&" => bin(left, right, |a, b| a & b),
                "^" => bin(left, right, |a, b| a ^ b),
                _ => bin(left, right, |a, b| a | b),
            };
        }
        Ok(left)
    }

    fn or(&mut self) -> Val {
        self.level(&["|"], Self::xor)
    }
    fn xor(&mut self) -> Val {
        self.level(&["^"], Self::and)
    }
    fn and(&mut self) -> Val {
        self.level(&["&"], Self::shift)
    }
    fn shift(&mut self) -> Val {
        self.level(&["<<", ">>"], Self::add)
    }
    fn add(&mut self) -> Val {
        self.level(&["+", "-"], Self::mul)
    }
    fn mul(&mut self) -> Val {
        self.level(&["*", "/", "%"], Self::unary)
    }

    fn unary(&mut self) -> Val {
        match self.peek_op() {
            Some("-") => {
                self.pos += 1;
                Ok(self.unary()?.map(|v| v.wrapping_neg()))
            }
            Some("+") => {
                self.pos += 1;
                self.unary()
            }
            Some("~") => {
                self.pos += 1;
                Ok(self.unary()?.map(|v| !v))
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Val {
        let tok = self.toks.get(self.pos).cloned();
        self.pos += 1;
        match tok {
            Some(Tok::Num(v)) => Ok(Some(v)),
            Some(Tok::Dollar) => Ok(Some(self.scope.pc())),
            Some(Tok::LParen) => {
                let v = self.or()?;
                if self.toks.get(self.pos) != Some(&Tok::RParen) {
                    return Err(err("unbalancedParens", ""));
                }
                self.pos += 1;
                Ok(v)
            }
            Some(Tok::Ident(name)) => {
                if (name == "HIGH" || name == "LOW") && self.toks.get(self.pos) == Some(&Tok::LParen) {
                    let v = self.primary()?;
                    return Ok(v.map(|v| if name == "HIGH" { (v >> 8) & 0xFF } else { v & 0xFF }));
                }
                let v = self.scope.symbol(&name);
                if v.is_none() && !self.undefined.contains(&name) {
                    self.undefined.push(name);
                }
                Ok(v)
            }
            Some(Tok::RParen) => Err(err("unbalancedParens", "")),
            Some(Tok::Op(o)) => Err(err("expectedValue", o)),
            None => Err(err("expectedValue", "")),
        }
    }
}

/// Évalue `text`. Les symboles inconnus sont ajoutés à `undefined` et donnent `Ok(None)`.
pub fn eval<S: Scope + ?Sized>(text: &str, scope: &mut S, undefined: &mut Vec<String>) -> Val {
    let toks = tokenize(text)?;
    if toks.is_empty() {
        return Err(err("expectedValue", ""));
    }
    let mut p = Parser { toks, pos: 0, scope, undefined };
    let v = p.or()?;
    match p.toks.get(p.pos) {
        None => Ok(v),
        Some(Tok::RParen) => Err(err("unbalancedParens", "")),
        Some(_) => Err(err("unexpectedText", text)),
    }
}

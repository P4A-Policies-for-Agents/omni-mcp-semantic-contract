// Copyright 2026 Salesforce, Inc. All rights reserved.

//! The `jsonpath-subset` expression dialect.
//!
//! Grammar:
//!
//! ```text
//! expr       := comparison (('and' | 'or') comparison)*
//! comparison := operand (op operand)?
//! operand    := path | literal | call
//! path       := 'payload' ('.' ident | '[' int ']')*
//! call       := ('sizeOf' | 'exists') '(' path ')'
//! op         := '==' | '!=' | '>' | '<' | '>=' | '<='
//! literal    := string | number | 'true' | 'false' | 'null'
//! ```
//!
//! Expressions are parsed once, at contract load time, into an [`Expr`] and
//! evaluated per request. Evaluation is total: it returns `bool`, never an
//! error. Anything the grammar cannot express is rejected at parse time.

use serde_json::Value;
use std::fmt;

/// A path segment: an object key or an array index.
#[derive(Debug, Clone, PartialEq)]
pub enum Seg {
    Key(String),
    Index(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    /// A path rooted at `payload`.
    Path(Vec<Seg>),
    /// `sizeOf(path)`.
    SizeOf(Vec<Seg>),
    /// `exists(path)`.
    Exists(Vec<Seg>),
    /// A literal value.
    Lit(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Or(Vec<Expr>),
    And(Vec<Expr>),
    Compare {
        left: Operand,
        op: CmpOp,
        right: Operand,
    },
    /// A bare operand used as a condition. True only when it evaluates to
    /// boolean `true`; every other value, including truthy-looking ones, is
    /// false.
    Truthy(Operand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn err<T>(msg: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError(msg.into()))
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Str(String),
    Num(f64),
    Dot,
    LBracket,
    RBracket,
    LParen,
    RParen,
    Op(CmpOp),
}

fn tokenize(src: &str) -> Result<Vec<Tok>, ParseError> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;

    while i < chars.len() {
        let c = chars[i];

        if c.is_whitespace() {
            i += 1;
            continue;
        }

        match c {
            '.' => {
                out.push(Tok::Dot);
                i += 1;
            }
            '[' => {
                out.push(Tok::LBracket);
                i += 1;
            }
            ']' => {
                out.push(Tok::RBracket);
                i += 1;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            '=' => {
                if chars.get(i + 1) == Some(&'=') {
                    out.push(Tok::Op(CmpOp::Eq));
                    i += 2;
                } else {
                    return err("`=` is not an operator; use `==`");
                }
            }
            '!' => {
                if chars.get(i + 1) == Some(&'=') {
                    out.push(Tok::Op(CmpOp::Ne));
                    i += 2;
                } else {
                    return err("`!` is not an operator; use `!=`");
                }
            }
            '>' => {
                if chars.get(i + 1) == Some(&'=') {
                    out.push(Tok::Op(CmpOp::Ge));
                    i += 2;
                } else {
                    out.push(Tok::Op(CmpOp::Gt));
                    i += 1;
                }
            }
            '<' => {
                if chars.get(i + 1) == Some(&'=') {
                    out.push(Tok::Op(CmpOp::Le));
                    i += 2;
                } else {
                    out.push(Tok::Op(CmpOp::Lt));
                    i += 1;
                }
            }
            '"' => {
                let mut s = String::new();
                i += 1;
                loop {
                    match chars.get(i) {
                        None => return err("unterminated string literal"),
                        Some('\\') => match chars.get(i + 1) {
                            Some('"') => {
                                s.push('"');
                                i += 2;
                            }
                            Some('\\') => {
                                s.push('\\');
                                i += 2;
                            }
                            Some('n') => {
                                s.push('\n');
                                i += 2;
                            }
                            Some('t') => {
                                s.push('\t');
                                i += 2;
                            }
                            Some(other) => {
                                return err(format!("unsupported escape `\\{}`", other));
                            }
                            None => return err("unterminated escape in string literal"),
                        },
                        Some('"') => {
                            i += 1;
                            break;
                        }
                        Some(ch) => {
                            s.push(*ch);
                            i += 1;
                        }
                    }
                }
                out.push(Tok::Str(s));
            }
            c if c.is_ascii_digit()
                || (c == '-' && matches!(chars.get(i + 1), Some(d) if d.is_ascii_digit())) =>
            {
                let start = i;
                if chars[i] == '-' {
                    i += 1;
                }
                while matches!(chars.get(i), Some(d) if d.is_ascii_digit()) {
                    i += 1;
                }
                if chars.get(i) == Some(&'.')
                    && matches!(chars.get(i + 1), Some(d) if d.is_ascii_digit())
                {
                    i += 1;
                    while matches!(chars.get(i), Some(d) if d.is_ascii_digit()) {
                        i += 1;
                    }
                }
                let text: String = chars[start..i].iter().collect();
                match text.parse::<f64>() {
                    Ok(n) => out.push(Tok::Num(n)),
                    Err(_) => return err(format!("invalid number literal `{}`", text)),
                }
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                while matches!(chars.get(i), Some(ch) if ch.is_ascii_alphanumeric() || *ch == '_') {
                    i += 1;
                }
                out.push(Tok::Ident(chars[start..i].iter().collect()));
            }
            other => return err(format!("unexpected character `{}`", other)),
        }
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn eat_keyword(&mut self, kw: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Ident(id)) if id == kw) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        let mut terms = vec![self.parse_and()?];
        while self.eat_keyword("or") {
            terms.push(self.parse_and()?);
        }
        Ok(if terms.len() == 1 {
            terms.remove(0)
        } else {
            Expr::Or(terms)
        })
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut terms = vec![self.parse_comparison()?];
        while self.eat_keyword("and") {
            terms.push(self.parse_comparison()?);
        }
        Ok(if terms.len() == 1 {
            terms.remove(0)
        } else {
            Expr::And(terms)
        })
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_operand()?;
        if let Some(Tok::Op(op)) = self.peek().cloned() {
            self.pos += 1;
            let right = self.parse_operand()?;
            Ok(Expr::Compare { left, op, right })
        } else {
            Ok(Expr::Truthy(left))
        }
    }

    fn parse_operand(&mut self) -> Result<Operand, ParseError> {
        match self.next() {
            None => err("unexpected end of expression"),
            Some(Tok::Str(s)) => Ok(Operand::Lit(Value::String(s))),
            Some(Tok::Num(n)) => match serde_json::Number::from_f64(n) {
                Some(num) => Ok(Operand::Lit(Value::Number(num))),
                None => err("number literal is not finite"),
            },
            Some(Tok::LParen) => err(
                "parentheses are not supported in v1; `and` binds tighter than `or`, \
                 restructure the expression or split it into separate rules",
            ),
            Some(Tok::Ident(id)) => match id.as_str() {
                "true" => Ok(Operand::Lit(Value::Bool(true))),
                "false" => Ok(Operand::Lit(Value::Bool(false))),
                "null" => Ok(Operand::Lit(Value::Null)),
                "payload" => Ok(Operand::Path(self.parse_path_tail()?)),
                "sizeOf" | "exists" => {
                    if self.next() != Some(Tok::LParen) {
                        return err(format!("`{}` must be followed by `(`", id));
                    }
                    if !matches!(self.next(), Some(Tok::Ident(ref p)) if p == "payload") {
                        return err(format!("`{}(...)` takes a path rooted at `payload`", id));
                    }
                    let path = self.parse_path_tail()?;
                    if self.next() != Some(Tok::RParen) {
                        return err(format!("unterminated `{}(` call", id));
                    }
                    Ok(if id == "sizeOf" {
                        Operand::SizeOf(path)
                    } else {
                        Operand::Exists(path)
                    })
                }
                "and" | "or" => err(format!("`{}` where an operand was expected", id)),
                other => err(format!(
                    "unknown identifier `{}`; paths must be rooted at `payload`",
                    other
                )),
            },
            Some(other) => err(format!(
                "unexpected token {:?} where an operand was expected",
                other
            )),
        }
    }

    /// Parses the `('.' ident | '[' int ']')*` tail of a path, `payload`
    /// having already been consumed.
    fn parse_path_tail(&mut self) -> Result<Vec<Seg>, ParseError> {
        let mut segs = Vec::new();
        loop {
            match self.peek() {
                Some(Tok::Dot) => {
                    self.pos += 1;
                    match self.next() {
                        Some(Tok::Ident(id)) => segs.push(Seg::Key(id)),
                        _ => return err("expected a field name after `.`"),
                    }
                }
                Some(Tok::LBracket) => {
                    self.pos += 1;
                    let idx = match self.next() {
                        Some(Tok::Num(n)) if n >= 0.0 && n.fract() == 0.0 => n as usize,
                        _ => return err("array index must be a non-negative integer"),
                    };
                    if self.next() != Some(Tok::RBracket) {
                        return err("expected `]` after array index");
                    }
                    segs.push(Seg::Index(idx));
                }
                _ => break,
            }
        }
        Ok(segs)
    }
}

/// Parses a `when` expression into an evaluable AST.
pub fn parse(src: &str) -> Result<Expr, ParseError> {
    if src.trim().is_empty() {
        return err("expression is empty");
    }
    let toks = tokenize(src)?;
    let mut p = Parser { toks, pos: 0 };
    let expr = p.parse_expr()?;
    if p.pos != p.toks.len() {
        return err(format!(
            "trailing input after a complete expression at token {}",
            p.pos
        ));
    }
    Ok(expr)
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

fn resolve<'a>(payload: &'a Value, path: &[Seg]) -> &'a Value {
    let mut cur = payload;
    for seg in path {
        cur = match seg {
            Seg::Key(k) => match cur.get(k.as_str()) {
                Some(v) => v,
                None => return &Value::Null,
            },
            Seg::Index(i) => match cur.get(*i) {
                Some(v) => v,
                None => return &Value::Null,
            },
        };
    }
    cur
}

fn eval_operand(op: &Operand, payload: &Value) -> Value {
    match op {
        Operand::Lit(v) => v.clone(),
        Operand::Path(p) => resolve(payload, p).clone(),
        Operand::SizeOf(p) => {
            let n = resolve(payload, p).as_array().map(|a| a.len()).unwrap_or(0);
            Value::Number(n.into())
        }
        Operand::Exists(p) => Value::Bool(!resolve(payload, p).is_null()),
    }
}

/// Ordering comparison. Defined only for number/number and string/string.
/// Every other pairing, `null` included, is false.
fn compare_ordered(l: &Value, r: &Value, op: CmpOp) -> bool {
    let ord = match (l, r) {
        (Value::Number(a), Value::Number(b)) => match (a.as_f64(), b.as_f64()) {
            (Some(a), Some(b)) => a.partial_cmp(&b),
            _ => None,
        },
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
        _ => None,
    };
    match ord {
        None => false,
        Some(ord) => match op {
            CmpOp::Gt => ord == std::cmp::Ordering::Greater,
            CmpOp::Lt => ord == std::cmp::Ordering::Less,
            CmpOp::Ge => ord != std::cmp::Ordering::Less,
            CmpOp::Le => ord != std::cmp::Ordering::Greater,
            _ => false,
        },
    }
}

/// Structural equality. Numbers compare by value so `1` equals `1.0`.
fn json_eq(l: &Value, r: &Value) -> bool {
    match (l, r) {
        (Value::Number(a), Value::Number(b)) => match (a.as_f64(), b.as_f64()) {
            (Some(a), Some(b)) => a == b,
            _ => a == b,
        },
        _ => l == r,
    }
}

/// Evaluates the expression against a bound payload. Always total.
pub fn eval(expr: &Expr, payload: &Value) -> bool {
    match expr {
        Expr::Or(terms) => terms.iter().any(|t| eval(t, payload)),
        Expr::And(terms) => terms.iter().all(|t| eval(t, payload)),
        Expr::Truthy(op) => matches!(eval_operand(op, payload), Value::Bool(true)),
        Expr::Compare { left, op, right } => {
            let l = eval_operand(left, payload);
            let r = eval_operand(right, payload);
            match op {
                CmpOp::Eq => json_eq(&l, &r),
                CmpOp::Ne => !json_eq(&l, &r),
                other => compare_ordered(&l, &r, *other),
            }
        }
    }
}

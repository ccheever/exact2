//! CSS's comparison functions, `min()`, `max()` and `clamp()` (CSS Values 4
//! §10.2), over the lengths the kernel resolves before layout: px (and the
//! other absolute units), `env(safe-area-inset-*)`, the viewport units, and
//! `calc()` sums of them — `clamp(15px, env(safe-area-inset-bottom), 60px)`,
//! `calc(max(15px, env(safe-area-inset-bottom)) + 44px)`. A row holds one by
//! handle ([`Comparison`]) so [`Dimension`] stays `Copy`: the expression is
//! interned here, equal ones share a handle (an unchanged style still
//! compares equal by value), and [`Dimension::resolve`] turns it into points
//! against the kernel's [`Env`] where every `env()` length is resolved. A
//! percentage is refused: no basis is known where the insets resolve.
//! @ref LLP 1001 §2 (the environment; comparisons, 2026-10-07)

use super::{absolute_length, env, parse_pixel_length, viewport, Dimension, Edge, Env};
use super::{EnvRefusal, ViewportUnit};
use crate::error::DecodeError;
use crate::wire::codec::{Reader, Writer};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Mutex;

/// The deepest nesting of functions and parentheses one length takes.
pub const MAX_DEPTH: u8 = 8;
/// The most terms and functions one length holds.
pub const MAX_TERMS: u16 = 64;

/// Which comparison a [`Expr::Pick`] makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Op {
    /// `min(a, …)`: the smallest.
    Min = 0,
    /// `max(a, …)`: the largest.
    Max = 1,
    /// `clamp(lo, v, hi)`: `max(lo, min(v, hi))`, so `lo` wins over `hi`.
    Clamp = 2,
}

impl Op {
    fn name(self) -> &'static str {
        ["min", "max", "clamp"][self as usize]
    }
}

/// What a term's points are added to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Base {
    /// Nothing: the term is its points.
    Points,
    /// A safe-area inset.
    Inset(Edge),
    /// A percentage of a viewport dimension.
    Viewport(ViewportUnit, f32),
}

/// A comparison's tree: a term, or a comparison of terms, each plus points.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// `base + points`.
    Term(Base, f32),
    /// `op(args) + points`.
    Pick(Op, Vec<Expr>, f32),
}

impl Expr {
    /// The points this resolves to under `env`.
    pub fn value(&self, env: &Env) -> f32 {
        match self {
            Expr::Term(Base::Points, plus) => *plus,
            Expr::Term(Base::Inset(edge), plus) => env.inset(*edge) + plus,
            Expr::Term(Base::Viewport(unit, n), plus) => unit.basis(env) * n / 100.0 + plus,
            Expr::Pick(op, args, plus) => {
                let mut v = args.iter().map(|a| a.value(env));
                let picked = match op {
                    Op::Min => v.fold(f32::INFINITY, f32::min),
                    Op::Max => v.fold(f32::NEG_INFINITY, f32::max),
                    Op::Clamp => {
                        let (lo, val, hi) = (v.next(), v.next(), v.next());
                        let (lo, val, hi) =
                            (lo.unwrap_or(0.0), val.unwrap_or(0.0), hi.unwrap_or(0.0));
                        lo.max(val.min(hi))
                    }
                };
                picked + plus
            }
        }
    }

    /// The CSS text: `clamp(15px, env(safe-area-inset-bottom), 60px)`, and
    /// a term or comparison with points added inside `calc()`.
    pub fn css(&self, out: &mut String) {
        let plus = match self {
            Expr::Term(Base::Points, plus) => {
                let _ = write!(out, "{}px", exact_num::Shortest32(*plus));
                return;
            }
            Expr::Term(_, plus) | Expr::Pick(_, _, plus) => *plus,
        };
        if plus != 0.0 {
            out.push_str("calc(");
        }
        match self {
            Expr::Term(Base::Inset(edge), _) => {
                out.push_str("env(safe-area-inset-");
                out.push_str(edge.name());
                out.push(')');
            }
            Expr::Term(Base::Viewport(unit, n), _) => {
                let _ = write!(out, "{}{}", exact_num::Shortest32(*n), unit.name());
            }
            Expr::Pick(op, args, _) => {
                out.push_str(op.name());
                out.push('(');
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    a.css(out);
                }
                out.push(')');
            }
            Expr::Term(Base::Points, _) => unreachable!("written above"),
        }
        if plus != 0.0 {
            out.push_str(if plus < 0.0 { " - " } else { " + " });
            let _ = write!(out, "{}px)", exact_num::Shortest32(plus.abs()));
        }
    }

    /// Whether every number is finite, every `clamp()` has three arguments
    /// and every `min()`/`max()` one or more, within [`MAX_DEPTH`] and
    /// [`MAX_TERMS`] — what the parser and the wire decoder both hold to.
    fn well_formed(&self) -> bool {
        fn walk(e: &Expr, depth: u8, terms: &mut u16) -> bool {
            *terms += 1;
            if depth > MAX_DEPTH || *terms > MAX_TERMS {
                return false;
            }
            match e {
                Expr::Term(Base::Viewport(_, n), plus) => n.is_finite() && plus.is_finite(),
                Expr::Term(_, plus) => plus.is_finite(),
                Expr::Pick(op, args, plus) => {
                    plus.is_finite()
                        && match op {
                            Op::Clamp => args.len() == 3,
                            Op::Min | Op::Max => !args.is_empty(),
                        }
                        && args.iter().all(|a| walk(a, depth + 1, terms))
                }
            }
        }
        walk(self, 0, &mut 0)
    }

    /// This plus `points`.
    fn plus(self, points: f32) -> Expr {
        match self {
            Expr::Term(base, p) => Expr::Term(base, p + points),
            Expr::Pick(op, args, p) => Expr::Pick(op, args, p + points),
        }
    }

    /// Append the wire form: a tag (0 points, 1 an inset with its edge, 2 a
    /// viewport length with its unit and number, 3–5 `min`/`max`/`clamp`
    /// with a count and the arguments), then the points added.
    fn encode(&self, w: &mut Writer) {
        match self {
            Expr::Term(Base::Points, _) => w.u8(0),
            Expr::Term(Base::Inset(edge), _) => {
                w.u8(1);
                w.u8(*edge as u8);
            }
            Expr::Term(Base::Viewport(unit, n), _) => {
                w.u8(2);
                w.u8(*unit as u8);
                w.f32(*n);
            }
            Expr::Pick(op, args, _) => {
                w.u8(3 + *op as u8);
                w.u8(args.len() as u8);
                for a in args {
                    a.encode(w);
                }
            }
        }
        let (Expr::Term(_, plus) | Expr::Pick(_, _, plus)) = self;
        w.f32(*plus);
    }

    fn decode(r: &mut Reader<'_>, depth: u8) -> Result<Expr, DecodeError> {
        if depth > MAX_DEPTH {
            return Err(DecodeError::InvalidComparison);
        }
        let expr = match r.u8()? {
            0 => Expr::Term(Base::Points, 0.0),
            1 => {
                let edge = Edge::from_index(r.u8()?).ok_or(DecodeError::InvalidComparison)?;
                Expr::Term(Base::Inset(edge), 0.0)
            }
            2 => {
                let unit = ViewportUnit::ALL
                    .get(usize::from(r.u8()?))
                    .copied()
                    .ok_or(DecodeError::InvalidComparison)?;
                Expr::Term(Base::Viewport(unit, r.f32()?), 0.0)
            }
            tag @ 3..=5 => {
                let op = [Op::Min, Op::Max, Op::Clamp][usize::from(tag - 3)];
                let count = r.u8()?;
                if u16::from(count) > MAX_TERMS {
                    return Err(DecodeError::InvalidComparison);
                }
                let args = (0..count)
                    .map(|_| Expr::decode(r, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                Expr::Pick(op, args, 0.0)
            }
            _ => return Err(DecodeError::InvalidComparison),
        };
        let plus = r.f32()?;
        Ok(match expr {
            Expr::Term(base, _) => Expr::Term(base, plus),
            Expr::Pick(op, args, _) => Expr::Pick(op, args, plus),
        })
    }
}

/// A comparison a row holds: a handle to its interned [`Expr`].
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Comparison(u32);

/// The interned expressions, by handle, and each one's wire bytes to its
/// handle, so equal expressions (bit for bit) share one.
struct Table {
    exprs: Vec<Expr>,
    handles: HashMap<Vec<u8>, u32>,
}

static TABLE: Mutex<Option<Table>> = Mutex::new(None);

impl Comparison {
    /// The handle of a well-formed expression, interned.
    fn intern(expr: Expr) -> Comparison {
        let mut w = Writer::new();
        expr.encode(&mut w);
        let mut table = TABLE.lock().unwrap_or_else(|e| e.into_inner());
        let table = table.get_or_insert_with(|| Table {
            exprs: Vec::new(),
            handles: HashMap::new(),
        });
        let next = table.exprs.len() as u32;
        let handle = *table.handles.entry(w.into_vec()).or_insert(next);
        if handle == next {
            table.exprs.push(expr);
        }
        Comparison(handle)
    }

    fn with<T>(self, f: impl FnOnce(&Expr) -> T) -> T {
        let table = TABLE.lock().unwrap_or_else(|e| e.into_inner());
        f(&table.as_ref().expect("a handle was interned").exprs[self.0 as usize])
    }

    /// The expression.
    pub fn expr(self) -> Expr {
        self.with(Expr::clone)
    }

    /// The points this resolves to under `env`.
    pub fn value(self, env: &Env) -> f32 {
        self.with(|e| e.value(env))
    }

    /// The CSS text, for a host whose browser resolves it.
    pub fn css(self, out: &mut String) {
        self.with(|e| e.css(out));
    }

    /// Whether the tree reads the safe-area inset at `edge`.
    pub fn reads(self, edge: Edge) -> bool {
        fn walk(e: &Expr, edge: Edge) -> bool {
            match e {
                Expr::Term(Base::Inset(at), _) => *at == edge,
                Expr::Term(..) => false,
                Expr::Pick(_, args, _) => args.iter().any(|a| walk(a, edge)),
            }
        }
        self.with(|e| walk(e, edge))
    }

    /// This, never below zero: `max(0px, …)`, as CSS clamps a math function
    /// to a property's range (CSS Values 4 §10.12) on a row that refuses a
    /// negative length. Itself when it is already that.
    pub(crate) fn at_least_zero(self) -> Comparison {
        let zero = Expr::Term(Base::Points, 0.0);
        let expr = self.expr();
        if let Expr::Pick(Op::Max, args, plus) = &expr {
            if *plus == 0.0 && args.first() == Some(&zero) {
                return self;
            }
        }
        let wrapped = Expr::Pick(Op::Max, vec![zero, expr], 0.0);
        if wrapped.well_formed() {
            Comparison::intern(wrapped)
        } else {
            self
        }
    }

    /// Append the wire form ([`Expr::encode`]).
    pub(crate) fn encode(self, w: &mut Writer) {
        self.with(|e| e.encode(w));
    }

    /// Read the wire form, refusing what the parser would.
    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Comparison, DecodeError> {
        let expr = Expr::decode(r, 0)?;
        if !expr.well_formed() {
            return Err(DecodeError::InvalidComparison);
        }
        Ok(Comparison::intern(expr))
    }
}

impl std::fmt::Debug for Comparison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut css = String::new();
        self.css(&mut css);
        f.debug_tuple("Comparison").field(&css).finish()
    }
}

/// Why a length text with a comparison in it is not one the kernel holds.
pub(crate) mod refusal {
    pub(crate) const PERCENT: &str = "a percentage inside min(), max() or clamp() is not supported: the kernel resolves them before layout, against the safe-area insets and the viewport, where a percentage has no basis";
    pub(crate) const RELATIVE: &str =
        "rem and em are not supported inside min(), max() or clamp(); write px";
    pub(crate) const UNITLESS: &str =
        "a nonzero number inside min(), max() or clamp() takes a unit, as CSS requires: 15px";
    pub(crate) const NOT_A_LENGTH: &str = "min(), max() and clamp() take px (or in, cm, mm, pt, pc) lengths, env(safe-area-inset-*), viewport lengths (vw, vh, vmin, vmax, svh, lvh, dvh, …) and calc() sums of them";
    pub(crate) const SEGMENT: &str =
        "env(viewport-segment-*) is not supported inside min(), max() or clamp()";
    pub(crate) const FUNCTION: &str =
        "inside min(), max() and clamp() the functions are calc(), min(), max(), clamp() and env()";
    pub(crate) const CLAMP_ARITY: &str =
        "clamp() takes exactly three arguments: clamp(<min>, <value>, <max>)";
    pub(crate) const EMPTY: &str =
        "min() and max() take one or more arguments, separated by commas";
    pub(crate) const TWO_VARIABLES: &str = "a sum inside min(), max() or clamp() adds px to at most one env(), viewport length or comparison; compare two of them instead";
    pub(crate) const NEGATED: &str = "subtracting env(), a viewport length or a comparison negates it, which no length holds; subtract px only";
    pub(crate) const SPACING: &str = "`+` and `-` take white space on both sides, as CSS requires: calc(max(15px, env(safe-area-inset-bottom)) + 44px)";
    pub(crate) const PRODUCT: &str = "`*` and `/` are not supported inside min(), max() or clamp()";
    pub(crate) const SYNTAX: &str =
        "min(), max() and clamp() take arguments separated by commas, inside balanced parentheses";
    pub(crate) const TOP_SUM: &str = "a sum is a length only inside calc() or a comparison: calc(max(15px, env(safe-area-inset-bottom)) + 44px)";
    pub(crate) const DEPTH: &str =
        "min(), max(), clamp() and calc() nest at most 8 deep in one length";
    pub(crate) const SIZE: &str = "one length holds at most 64 terms and functions";
    pub(crate) const NONFINITE: &str =
        "a length inside min(), max() or clamp() is not a finite number";
}

/// A term's value while parsing: points alone, or an expression that reads
/// the environment.
enum Val {
    Points(f32),
    Var(Expr),
}

impl Val {
    fn expr(self) -> Expr {
        match self {
            Val::Points(p) => Expr::Term(Base::Points, p),
            Val::Var(e) => e,
        }
    }
}

struct Parser<'a> {
    s: &'a str,
    at: usize,
    depth: u8,
    terms: u16,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self
            .s
            .as_bytes()
            .get(self.at)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.at += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.as_bytes().get(self.at).copied()
    }

    fn expect(&mut self, b: u8) -> Result<(), &'static str> {
        self.ws();
        if self.peek() != Some(b) {
            return Err(refusal::SYNTAX);
        }
        self.at += 1;
        Ok(())
    }

    fn count(&mut self) -> Result<(), &'static str> {
        self.terms += 1;
        if self.terms > MAX_TERMS {
            return Err(refusal::SIZE);
        }
        Ok(())
    }

    /// `term (( + | - ) term)*`, the operators set off by white space: at
    /// most one term reads the environment, and it is not subtracted.
    fn sum(&mut self) -> Result<Val, &'static str> {
        let (mut points, mut var, mut sign) = (0.0f32, None, 1.0f32);
        loop {
            self.ws();
            match self.term()? {
                Val::Points(p) => points += sign * p,
                Val::Var(_) if sign < 0.0 => return Err(refusal::NEGATED),
                Val::Var(_) if var.is_some() => return Err(refusal::TWO_VARIABLES),
                Val::Var(e) => var = Some(e),
            }
            let before = self.at;
            self.ws();
            match self.peek() {
                Some(op @ (b'+' | b'-')) => {
                    let after = self.s.as_bytes().get(self.at + 1);
                    if self.at == before || !after.is_some_and(u8::is_ascii_whitespace) {
                        return Err(refusal::SPACING);
                    }
                    sign = if op == b'-' { -1.0 } else { 1.0 };
                    self.at += 1;
                }
                Some(b'*' | b'/') => return Err(refusal::PRODUCT),
                _ => break,
            }
        }
        if !points.is_finite() {
            return Err(refusal::NONFINITE);
        }
        Ok(match var {
            Some(e) => Val::Var(e.plus(points)),
            None => Val::Points(points),
        })
    }

    /// A parenthesized sum, a function, or a length token.
    fn term(&mut self) -> Result<Val, &'static str> {
        self.count()?;
        let rest = &self.s[self.at..];
        let open = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        if let Some(name_len) = open.filter(|&n| rest.as_bytes()[n] == b'(') {
            let name = &rest[..name_len];
            self.at += name_len + 1;
            self.depth += 1;
            if self.depth > MAX_DEPTH {
                return Err(refusal::DEPTH);
            }
            let val = match name {
                "" | "calc" => {
                    let v = self.sum()?;
                    self.expect(b')')?;
                    v
                }
                "min" => self.pick(Op::Min)?,
                "max" => self.pick(Op::Max)?,
                "clamp" => self.pick(Op::Clamp)?,
                "env" => self.env(rest, name_len)?,
                _ => return Err(refusal::FUNCTION),
            };
            self.depth -= 1;
            return Ok(val);
        }
        let len = rest
            .find(|c: char| c.is_ascii_whitespace() || matches!(c, ',' | ')' | '(' | '*' | '/'))
            .unwrap_or(rest.len());
        let token = &rest[..len];
        self.at += len;
        token_length(token)
    }

    /// The arguments of `min(`, `max(` or `clamp(`, through the `)`; folded
    /// to points when none reads the environment.
    fn pick(&mut self, op: Op) -> Result<Val, &'static str> {
        let mut args = Vec::new();
        loop {
            self.ws();
            if args.is_empty() && self.peek() == Some(b')') {
                return Err(if op == Op::Clamp {
                    refusal::CLAMP_ARITY
                } else {
                    refusal::EMPTY
                });
            }
            args.push(self.sum()?);
            self.ws();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b')') => {
                    self.at += 1;
                    break;
                }
                _ => return Err(refusal::SYNTAX),
            }
        }
        if op == Op::Clamp && args.len() != 3 {
            return Err(refusal::CLAMP_ARITY);
        }
        let expr = Expr::Pick(op, args.into_iter().map(Val::expr).collect(), 0.0);
        Ok(if reads_env(&expr) {
            Val::Var(expr)
        } else {
            Val::Points(expr.value(&Env::default()))
        })
    }

    /// `env(…)`, from `rest` (the text at the term), through its `)`.
    fn env(&mut self, rest: &str, name_len: usize) -> Result<Val, &'static str> {
        let close = rest[name_len..].find(')').ok_or(refusal::SYNTAX)? + name_len;
        self.at += close - name_len;
        match env::term(&rest[..=close]) {
            Ok(Dimension::Env(edge, plus)) => Ok(Val::Var(Expr::Term(Base::Inset(edge), plus))),
            Ok(_) => Err(refusal::SEGMENT),
            Err(EnvRefusal::Unlinked) => Err(refusal::SEGMENT),
            Err(why) => Err(why.reason()),
        }
    }
}

/// Whether an expression reads an inset or the viewport.
fn reads_env(e: &Expr) -> bool {
    match e {
        Expr::Term(Base::Points, _) => false,
        Expr::Term(..) => true,
        Expr::Pick(_, args, _) => args.iter().any(reads_env),
    }
}

/// One length token: px or unitless zero, another absolute unit, or a
/// viewport length.
fn token_length(token: &str) -> Result<Val, &'static str> {
    if token.ends_with('%') {
        return Err(refusal::PERCENT);
    }
    if let Some(p) = parse_pixel_length(token) {
        return Ok(Val::Points(p));
    }
    match absolute_length(token) {
        Some(Dimension::Points(p)) => return Ok(Val::Points(p)),
        Some(_) => return Err(refusal::PERCENT),
        None => {}
    }
    if let Some(Dimension::Viewport(unit, n)) = viewport::parse(token) {
        return Ok(Val::Var(Expr::Term(Base::Viewport(unit, n), 0.0)));
    }
    let lower = token.to_ascii_lowercase();
    if lower.ends_with("rem") || lower.ends_with("em") {
        return Err(refusal::RELATIVE);
    }
    if exact_num::parse_f64(token).is_ok_and(f64::is_finite) {
        return Err(refusal::UNITLESS);
    }
    Err(refusal::NOT_A_LENGTH)
}

/// Whether a length text names a comparison function, so [`parse`] reads it.
fn names_comparison(text: &str) -> bool {
    ["min(", "max(", "clamp("].iter().any(|f| text.contains(f))
}

/// A length with `min()`, `max()` or `clamp()` in it, by CSS's grammar —
/// the whole text one function, or a `calc()` around one. `Ok(None)` when
/// the text names no comparison (the other length grammars read it); `Err`
/// with the reason when it names one wrongly. Points when nothing in it
/// reads the environment; an inset's or a viewport length's own dimension
/// when the comparisons in it fold away; otherwise [`Dimension::Compare`].
pub fn parse(text: &str) -> Result<Option<Dimension>, &'static str> {
    let text = text.trim_matches(['\t', '\n', '\u{c}', '\r', ' ']);
    if !names_comparison(text) {
        return Ok(None);
    }
    // CSS parenthesizes a sum only inside a math function.
    if text.starts_with('(') {
        return Err(refusal::TOP_SUM);
    }
    let mut p = Parser {
        s: text,
        at: 0,
        depth: 0,
        terms: 0,
    };
    let val = p.term()?;
    p.ws();
    match p.peek() {
        None => {}
        Some(b'+' | b'-' | b'*' | b'/') => return Err(refusal::TOP_SUM),
        Some(_) => return Err(refusal::SYNTAX),
    }
    if matches!(&val, Val::Var(e) if !e.well_formed()) {
        return Err(refusal::NONFINITE);
    }
    Ok(Some(match val {
        Val::Points(p) => Dimension::Points(p),
        Val::Var(Expr::Term(Base::Inset(edge), plus)) => Dimension::Env(edge, plus),
        Val::Var(Expr::Term(Base::Viewport(unit, n), 0.0)) => Dimension::Viewport(unit, n),
        Val::Var(e) => Dimension::Compare(Comparison::intern(e)),
    }))
}

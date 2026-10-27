//! CSS length arithmetic: parse, type-check, simplify, serialize and evaluate
//! `calc()`, `min()`, `max()` and `clamp()` (CSS Values 4 §10).
//!
//! Arithmetic uses `f64` and narrows once at the result. Precision is
//! implementation-defined; exceptional values follow §10.9. Compact expressions
//! stay in `Dimension`; other trees are interned so it remains `Copy` and equal
//! styles share a handle. Percentages resolve during layout against their basis.
//! @ref LLP 1001 §2

mod parse;
mod simplify;
mod substitute;
mod text;
mod wire;

pub use parse::nonnegative_row;
pub(crate) use parse::{grid, is_env, names_math, parse, refusal, signed, substitutes, Refused};

use super::env::{self, Edge, Env, Region, SegmentVar, TitleBarVar};
use super::{Dimension, StyleId, ViewportUnit};
use std::collections::HashMap;
use std::sync::Mutex;

/// The values CSS Values 4 §10.8 has a UA support in one calculation, as
/// written, and the levels of parentheses and functions: the kernel's
/// limits too.
pub const MAX_VALUES: usize = 32;
/// See [`MAX_VALUES`].
pub const MAX_DEPTH: usize = 32;

/// A numeric value's unit: a number, a percentage, a length in px (the
/// canonical unit every absolute length converts to when parsed) or in a
/// viewport unit, which resolves only with the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// A number: a coefficient, or a divisor.
    Number,
    /// A percentage of the basis.
    Percent,
    /// px, every absolute length's canonical unit.
    Px,
    /// A viewport unit.
    Viewport(ViewportUnit),
}

impl Unit {
    /// The unit as CSS writes it after the number.
    fn name(self) -> &'static str {
        match self {
            Unit::Number => "",
            Unit::Percent => "%",
            Unit::Px => "px",
            Unit::Viewport(unit) => unit.name(),
        }
    }
}

/// An `env()` variable a length reads: a safe-area inset, a viewport
/// segment's variable at its indices (CSS Environment Variables 1), a
/// layout region's inset, or a side of the title-bar area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvTerm {
    /// `env(safe-area-inset-<edge>)`.
    Inset(Edge),
    /// `env(viewport-segment-<var> <x> <y>)`.
    Segment(SegmentVar, u8, u8),
    /// `env(-exact-<region>-inset-<edge>)` (LLP 1123 D10).
    Region(Region, Edge),
    /// `env(titlebar-area-<var>)` (Window Controls Overlay). No compact
    /// [`Dimension`] holds it: a length that reads it is always a tree.
    TitleBar(TitleBarVar),
    /// `var(--exact-…)`: a content owner's value (LLP 1123 D9).
    Owner(crate::style::OwnerVar),
}

impl EnvTerm {
    /// Its points under `env`; `None` for a segment or a title-bar area
    /// `env` does not define.
    fn value(self, env: &Env) -> Option<f64> {
        match self {
            EnvTerm::Inset(edge) => Some(f64::from(env.inset(edge))),
            EnvTerm::Segment(var, x, y) => env::resolve_wide(var, x, y, env),
            EnvTerm::Region(region, edge) => Some(f64::from(env.region_inset(region, edge))),
            EnvTerm::TitleBar(var) => env.titlebar(var).map(f64::from),
            EnvTerm::Owner(var) => var.value(env),
        }
    }
}

/// A calculation tree as §10.10 builds one and §10.10.1 leaves it: numeric
/// values and `env()` at the leaves, the calc-operator nodes and the
/// comparisons above them. A simplified tree's sums and products hold their
/// children in §10.13's order, so equal values are equal trees.
#[derive(Debug, Clone)]
pub enum Node {
    /// A number, a percentage or a length in one unit.
    Value(f64, Unit),
    /// `env()` without a fallback: a substitution of a length, which a tree
    /// may hold as a leaf (one with a fallback is held as text, [`Entry`]).
    Env(EnvTerm),
    /// Its children added.
    Sum(Vec<Node>),
    /// Its children multiplied.
    Product(Vec<Node>),
    /// Its child negated: what follows a `-`.
    Negate(Box<Node>),
    /// Its child's reciprocal: what follows a `/`.
    Invert(Box<Node>),
    /// `min()`.
    Min(Vec<Node>),
    /// `max()`.
    Max(Vec<Node>),
    /// `clamp(MIN, VAL, MAX)`; `None` is `none`.
    Clamp(Box<[Option<Node>; 3]>),
}

impl PartialEq for Node {
    /// Equal bit for bit: a NaN is the NaN it is, and `-0` is not `0`.
    fn eq(&self, other: &Node) -> bool {
        use Node::*;
        match (self, other) {
            (Value(a, u), Value(b, v)) => a.to_bits() == b.to_bits() && u == v,
            (Env(a), Env(b)) => a == b,
            (Sum(a), Sum(b)) | (Product(a), Product(b)) | (Min(a), Min(b)) | (Max(a), Max(b)) => {
                a == b
            }
            (Negate(a), Negate(b)) | (Invert(a), Invert(b)) => a == b,
            (Clamp(a), Clamp(b)) => a == b,
            _ => false,
        }
    }
}

impl Node {
    /// The children a calculation reads, in order.
    fn children(&self) -> impl Iterator<Item = &Node> {
        let (nodes, optional): (&[Node], &[Option<Node>]) = match self {
            Node::Value(..) | Node::Env(..) => (&[], &[]),
            Node::Sum(c) | Node::Product(c) | Node::Min(c) | Node::Max(c) => (c, &[]),
            Node::Negate(c) | Node::Invert(c) => (std::slice::from_ref(c), &[]),
            Node::Clamp(c) => (&[], c.as_slice()),
        };
        nodes.iter().chain(optional.iter().flatten())
    }

    /// Whether a node anywhere in it satisfies `f`.
    fn any(&self, f: &dyn Fn(&Node) -> bool) -> bool {
        f(self) || self.children().any(|c| c.any(f))
    }

    /// Whether it is a leaf a content owner gives: `var(--exact-…)` or a
    /// container unit (LLP 1123 D9, D12).
    fn owned(&self) -> bool {
        match self {
            Node::Env(EnvTerm::Owner(_)) => true,
            Node::Value(_, Unit::Viewport(unit)) => unit.is_container(),
            _ => false,
        }
    }

    /// It with each leaf an owner gives replaced by its points under `env`.
    fn bind(&self, env: &Env) -> Node {
        let all = |c: &[Node]| c.iter().map(|n| n.bind(env)).collect();
        let one = |c: &Node| Box::new(c.bind(env));
        match self {
            n if n.owned() => n
                .value(env, None)
                .map_or_else(|| n.clone(), |v| Node::Value(v, Unit::Px)),
            Node::Sum(c) => Node::Sum(all(c)),
            Node::Product(c) => Node::Product(all(c)),
            Node::Min(c) => Node::Min(all(c)),
            Node::Max(c) => Node::Max(all(c)),
            Node::Negate(c) => Node::Negate(one(c)),
            Node::Invert(c) => Node::Invert(one(c)),
            Node::Clamp(c) => Node::Clamp(Box::new(c.clone().map(|n| n.map(|n| n.bind(env))))),
            n => n.clone(),
        }
    }

    /// The value under `env`, a percentage of `basis`, in IEEE-754 `f64`
    /// as §10.9.2 has it inside a tree (an infinity or a NaN included);
    /// `None` where a segment is undefined or a percentage has no basis.
    pub(crate) fn value(&self, env: &Env, basis: Option<f64>) -> Option<f64> {
        Some(match self {
            Node::Value(v, unit) => match unit {
                Unit::Number | Unit::Px => *v,
                Unit::Percent => basis? * v / 100.0,
                Unit::Viewport(u) => f64::from(u.basis(env)) * v / 100.0,
            },
            Node::Env(term) => term.value(env)?,
            Node::Sum(c) => {
                let mut total = 0.0;
                for n in c {
                    total += n.value(env, basis)?;
                }
                total
            }
            Node::Product(c) => {
                let mut total = 1.0;
                for n in c {
                    total *= n.value(env, basis)?;
                }
                total
            }
            Node::Negate(c) => -c.value(env, basis)?,
            Node::Invert(c) => 1.0 / c.value(env, basis)?,
            Node::Min(c) | Node::Max(c) => {
                let max = matches!(self, Node::Max(_));
                let mut out: Option<f64> = None;
                for n in c {
                    let v = n.value(env, basis)?;
                    out = Some(out.map_or(v, |o| pick(o, v, max)));
                }
                out?
            }
            Node::Clamp(c) => {
                let v = c[1].as_ref()?.value(env, basis)?;
                let v = match &c[2] {
                    Some(hi) => pick(v, hi.value(env, basis)?, false),
                    None => v,
                };
                match &c[0] {
                    Some(lo) => pick(lo.value(env, basis)?, v, true),
                    None => v,
                }
            }
        })
    }
}

/// The smaller or (when `max`) the larger of two numbers, as §10.9.2 has
/// it: a NaN on either side is NaN, and `-0` is less than `0`.
fn pick(a: f64, b: f64, max: bool) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    let a_first = if a == b {
        a.is_sign_negative() != max
    } else {
        (a < b) != max
    };
    if a_first {
        a
    } else {
        b
    }
}

/// A top-level calculation's result as the `f32` a length holds, censored
/// as §10.9.2 has it: NaN is 0, an infinity or a value past what an `f32`
/// holds is the largest it does, and `-0` is `0`. The arithmetic before it
/// is `f64`; this is the final conversion to layout precision.
pub(crate) fn narrow(v: f64) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32 + 0.0
    }
}

/// Whether `v` is an `f32` exactly, so a compact form loses nothing.
fn exact(v: f64) -> bool {
    f64::from(v as f32).to_bits() == v.to_bits()
}

/// A simplified tree as the dimension that holds it: a compact form where
/// one holds exactly this tree (a lone numeric value, `env()`, a percentage
/// beside px, px beside `env()`), else the tree by handle.
pub(crate) fn dimension(tree: Node) -> Dimension {
    let compact = match &tree {
        Node::Value(v, Unit::Px) => Some(Dimension::Points(narrow(*v))),
        // A lone length is its result, its rounding the result's; a
        // percentage or a viewport length is multiplied after, so it is held
        // only where an `f32` holds it exactly
        Node::Value(v, Unit::Percent) if exact(*v) => Some(Dimension::Percent(*v as f32)),
        Node::Value(v, Unit::Viewport(u)) if exact(*v) => Some(Dimension::Viewport(*u, *v as f32)),
        Node::Env(term) => env_dimension(*term, 0.0),
        Node::Sum(c) => match &c[..] {
            [Node::Value(p, Unit::Percent), Node::Value(x, Unit::Px)] if exact(*p) && exact(*x) => {
                Some(Dimension::Calc(*p as f32, *x as f32))
            }
            // A zero is kept in a sum, and `Env`'s zero is no term at all
            [Node::Value(x, Unit::Px), Node::Env(term)] if exact(*x) && *x != 0.0 => {
                env_dimension(*term, *x as f32)
            }
            _ => None,
        },
        _ => None,
    };
    compact.unwrap_or_else(|| Dimension::Compare(Comparison::intern(tree)))
}

/// The compact form of `env()` plus points, where one holds it.
fn env_dimension(term: EnvTerm, plus: f32) -> Option<Dimension> {
    Some(match term {
        EnvTerm::Inset(edge) => Dimension::Env(edge, plus),
        EnvTerm::Segment(var, x, y) => Dimension::Segment(var, x, y, plus),
        EnvTerm::Region(region, edge) => Dimension::Region(region, edge, plus),
        EnvTerm::Owner(var) => Dimension::Owner(var, plus),
        EnvTerm::TitleBar(_) => return None,
    })
}

/// A bare `env()` without a fallback as the dimension that holds it.
pub(crate) fn lone_env(term: EnvTerm) -> Dimension {
    dimension(Node::Env(term))
}

/// A calculation a row holds: a handle to its interned entry.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Comparison(u32);

/// What a handle names: a simplified tree, or a text whose `env()` has a
/// fallback, which CSS Environment Variables 1 §3 substitutes before the
/// row's grammar reads it, so it is held as written, with its row.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Entry {
    Tree(Node),
    Text { text: String, row: StyleId },
}

/// The interned entries, by handle, and each one's wire bytes to its
/// handle, so equal entries (bit for bit) share one.
struct Table {
    entries: Vec<Entry>,
    handles: HashMap<Vec<u8>, u32>,
}

static TABLE: Mutex<Option<Table>> = Mutex::new(None);

impl Comparison {
    /// The handle of a simplified tree, interned.
    pub(crate) fn intern(tree: Node) -> Comparison {
        Self::intern_entry(Entry::Tree(tree))
    }

    /// The handle of a text whose `env()` has a fallback, for `row`; the
    /// text as written, trimmed.
    pub(crate) fn substitution(text: &str, row: StyleId) -> Comparison {
        Self::intern_entry(Entry::Text {
            text: text.to_string(),
            row,
        })
    }

    fn intern_entry(entry: Entry) -> Comparison {
        let mut w = crate::wire::codec::Writer::new();
        wire::encode(&entry, &mut w);
        if let Entry::Text { row, .. } = &entry {
            // The row is no part of the wire form, which the decoder reads
            // for its own row: it keys the entry here
            w.u16(*row as u16);
        }
        let mut table = TABLE.lock().unwrap_or_else(|e| e.into_inner());
        let table = table.get_or_insert_with(|| Table {
            entries: Vec::new(),
            handles: HashMap::new(),
        });
        let next = table.entries.len() as u32;
        let handle = *table.handles.entry(w.into_vec()).or_insert(next);
        if handle == next {
            table.entries.push(entry);
        }
        Comparison(handle)
    }

    fn with<T>(self, f: impl FnOnce(&Entry) -> T) -> T {
        let table = TABLE.lock().unwrap_or_else(|e| e.into_inner());
        f(&table.as_ref().expect("a handle was interned").entries[self.0 as usize])
    }

    /// The points this resolves to under `env`, its percentages of `basis`,
    /// censored as a top-level calculation is ([`narrow`]); `None` where it
    /// is invalid or a percentage has no basis.
    pub fn value(self, env: &Env, basis: Option<f32>) -> Option<f32> {
        self.wide(env, basis.map(f64::from)).map(narrow)
    }

    /// [`Comparison::value`] before it is censored: what a caller that adds
    /// to it censors after.
    pub(crate) fn wide(self, env: &Env, basis: Option<f64>) -> Option<f64> {
        match self.tree(|t| t.value(env, basis)) {
            Some(v) => v,
            None => substitute::value(self.substituted(env)?, env, basis),
        }
    }

    /// It with the values a content owner gives bound to `env`'s (LLP 1123
    /// D9, D12). The engine resolves a tree with a percentage later, under
    /// the document's environment ([`crate::style::LayoutEnv`]), which is not
    /// a node's inside an owner. A top-layer box in an owner has no owner
    /// values but keeps the owner's container (`NodeArena::env_for`). Itself
    /// with neither, or when it reads none.
    pub(crate) fn bound(self, env: &Env) -> Comparison {
        if env.owner.is_none() && env.container.is_none() {
            return self;
        }
        let bound = self.tree(|t| t.any(&Node::owned).then(|| t.bind(env)));
        bound.flatten().map_or(self, Comparison::intern)
    }

    /// `f` of the tree, or `None` for a text, without copying either.
    fn tree<T>(self, f: impl FnOnce(&Node) -> T) -> Option<T> {
        self.with(|e| match e {
            Entry::Tree(t) => Some(f(t)),
            Entry::Text { .. } => None,
        })
    }

    /// A text's value under `env` as its row reads it with every `env()`
    /// substituted (CSS Environment Variables 1 §3): a variable `env`
    /// defines is its own, an undefined one's fallback stands in its place;
    /// `None` when the result is no value of the row, invalid at
    /// computed-value time. A tree is itself.
    pub(crate) fn substituted(self, env: &Env) -> Option<Dimension> {
        let text = self.with(|e| match e {
            Entry::Tree(_) => None,
            Entry::Text { text, row } => Some((text.clone(), *row)),
        });
        match text {
            None => Some(Dimension::Compare(self)),
            Some((text, row)) => {
                let text = substitute::substitute(&text, env)?;
                // Read as an author's value of the row is, by the one
                // conversion `set_dynamic` makes (`ch`, `in`, `auto` and the
                // rest); a fallback refuses `rem` and `em`
                // (`refusal::RELATIVE_FALLBACK`)
                let value = match parse::is_keyword(&text, "auto") {
                    true => crate::StyleValue::Auto,
                    false => crate::StyleValue::Text(text.into()),
                };
                let mut props = crate::StyleProps::default();
                props.set_dynamic(row, &value).ok()?;
                match props.get(row) {
                    crate::RowValue::Dimension(d) if props.relative.is_empty() => Some(d),
                    _ => None,
                }
            }
        }
    }

    /// Whether it is invalid at computed-value time under `env`, the row
    /// taking its initial value: a segment a tree reads is undefined,
    /// whichever argument it is in, or a text substituted is no value of its
    /// row (CSS Environment Variables 1 §3).
    pub(crate) fn undefined(self, env: &Env) -> bool {
        self.tree(|t| t.any(&|n| matches!(n, Node::Env(term) if term.value(env).is_none())))
            .unwrap_or_else(|| {
                self.substituted(env)
                    .is_none_or(|d| d.undefined_segment(env))
            })
    }

    /// The CSS text: a tree's as §10.13 serializes it, a text as written,
    /// for a host whose browser resolves it.
    pub fn css(self, out: &mut String) {
        self.with(|e| match e {
            Entry::Tree(t) => text::math_function(t, out),
            Entry::Text { text, .. } => out.push_str(text),
        });
    }

    /// Whether it reads the safe-area inset at `edge` under `env`, or the
    /// corner-adapted safe area's there, which is never less (LLP 1123 D10):
    /// a text once substituted.
    pub fn reads(self, edge: Edge, env: &Env) -> bool {
        self.tree(|t| {
            t.any(&|n| {
                matches!(n, Node::Env(EnvTerm::Inset(e) | EnvTerm::Region(Region::SafeAreaCorners, e)) if *e == edge)
            })
        })
        .unwrap_or_else(|| match self.substituted(env) {
            Some(Dimension::Env(e, _) | Dimension::Region(Region::SafeAreaCorners, e, _)) => e == edge,
            Some(Dimension::Compare(c)) => c.reads(edge, env),
            _ => false,
        })
    }

    /// Append the wire form ([`wire::encode`]).
    pub(crate) fn encode(self, w: &mut crate::wire::codec::Writer) {
        self.with(|e| wire::encode(e, w));
    }

    /// Read the wire form for `row`, admitting only what the parser makes.
    pub(crate) fn decode(
        r: &mut crate::wire::codec::Reader<'_>,
        row: StyleId,
    ) -> Result<Comparison, crate::error::DecodeError> {
        wire::decode(r, row)
    }

    pub(crate) fn is_length_percentage(self) -> bool {
        self.tree(|t| !t.any(&|n| matches!(n, Node::Env(_) | Node::Value(_, Unit::Viewport(_)))))
            .unwrap_or(false)
    }

    /// Whether a percentage stands in it: in a tree, at a leaf, which
    /// simplification keeps (a percentage combines only with another); a
    /// text is checked once substituted, as the row reads it.
    pub(crate) fn has_percent(self) -> bool {
        self.tree(|t| t.any(&|n| matches!(n, Node::Value(_, Unit::Percent))))
            .unwrap_or(false)
    }
}

impl std::fmt::Debug for Comparison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut css = String::new();
        self.css(&mut css);
        f.debug_tuple("Comparison").field(&css).finish()
    }
}

/// Whether a row takes no negative length in CSS: sizes, padding, radii, an
/// SVG radius and a column's width (margins and insets take one). A math
/// function's result is clamped to 0 there (CSS Values 4 §9.1).
pub(super) fn no_negative_lengths(style: StyleId) -> bool {
    matches!(
        style,
        StyleId::Width
            | StyleId::Height
            | StyleId::MinWidth
            | StyleId::MinHeight
            | StyleId::MaxWidth
            | StyleId::MaxHeight
            | StyleId::PaddingTop
            | StyleId::PaddingRight
            | StyleId::PaddingBottom
            | StyleId::PaddingLeft
            | StyleId::FlexBasis
            | StyleId::BorderRadiusTopLeft
            | StyleId::BorderRadiusTopRight
            | StyleId::BorderRadiusBottomRight
            | StyleId::BorderRadiusBottomLeft
            | StyleId::R
            | StyleId::Rx
            | StyleId::Ry
            | StyleId::ColumnWidth
    )
}

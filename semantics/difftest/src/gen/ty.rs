//! Types, shapes and literals: the values a generated program starts from,
//! biased toward the ones whose printing or arithmetic is easy to get wrong.

use super::Gen;

/// A Contract type the generator writes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Ty {
    Num,
    Str,
    Bool,
    Opt(Box<Ty>),
    List(Box<Ty>),
    /// A declared shape, by index.
    Rec(usize),
}

impl Ty {
    pub(crate) fn opt(t: Ty) -> Ty {
        Ty::Opt(Box::new(t))
    }

    pub(crate) fn list(t: Ty) -> Ty {
        Ty::List(Box::new(t))
    }

    /// Whether a value of this type, written without context, could need a
    /// `[]` whose element type nothing fixes.
    pub(crate) fn needs_list(&self) -> bool {
        match self {
            Ty::List(_) => true,
            Ty::Opt(t) => t.needs_list(),
            _ => false,
        }
    }
}

/// `shape Name` and its fields, in declaration order.
#[derive(Debug, Clone)]
pub(crate) struct Shape {
    pub(crate) name: String,
    pub(crate) fields: Vec<(String, Ty)>,
}

/// Numbers whose arithmetic or printing is a known trap: zero's sign, binary
/// fractions, the exponent thresholds of JS `Number.prototype.toString`, the
/// edge of exact integers, the smallest subnormal.
const NUMBERS: &[f64] = &[
    0.0,
    -0.0,
    1.0,
    -1.0,
    2.0,
    3.0,
    7.0,
    10.0,
    100.0,
    0.1,
    0.2,
    0.3,
    0.5,
    1.5,
    -2.5,
    2.5,
    0.000001,
    0.0000001,
    1e-7,
    123e-20,
    1e20,
    1e21,
    1e300,
    1.7976931348623157e308,
    5e-324,
    9007199254740992.0,
    9007199254740991.0,
    9007199254740993.0,
    123456789012345680000.0,
    4294967296.0,
    2147483647.0,
    -2147483648.0,
    0.3333333333333333,
    1234.5678,
    -7.0,
    -0.1,
];

/// Strings that probe trimming (JS and Rust disagree on U+0085 and U+FEFF),
/// UTF-16 lengths (astral characters count two), escaping and URI encoding.
pub(crate) const STRINGS: &[&str] = &[
    "",
    "",
    "a",
    "b",
    "hello",
    "Hello World",
    " padded  ",
    "\t tab\n",
    "  ",
    "\u{a0}nbsp\u{a0}",
    "\u{feff}bom",
    "\u{85}nel\u{85}",
    "\u{2028}ls\u{3000}",
    "émoji 😀",
    "😀",
    "𝄞x",
    "日本語",
    "a,b",
    "%20&?=/#+",
    "0",
    "-0",
    "1e21",
    "NaN",
    "true",
    "none",
    "q\"uote",
    "back\\slash",
    "`tick`",
    "$",
    "${x}",
    "{}",
    "e\u{301}",
];

impl Gen<'_> {
    /// The written spelling of a type.
    pub(crate) fn ty_name(&self, t: &Ty) -> String {
        match t {
            Ty::Num => "number".into(),
            Ty::Str => "string".into(),
            Ty::Bool => "bool".into(),
            Ty::Opt(t) => format!("option<{}>", self.ty_name(t)),
            Ty::List(t) => format!("list<{}>", self.ty_name(t)),
            Ty::Rec(i) => self.shapes[*i].name.clone(),
        }
    }

    /// A scalar type, numbers and strings most often.
    pub(crate) fn scalar_ty(&mut self) -> Ty {
        match self.rng.weighted(&[5, 4, 2]) {
            0 => Ty::Num,
            1 => Ty::Str,
            _ => Ty::Bool,
        }
    }

    /// A scalar or, when there are shapes, a record.
    pub(crate) fn base_ty(&mut self) -> Ty {
        if !self.shapes.is_empty() && self.rng.chance(1, 4) {
            Ty::Rec(self.rng.below(self.shapes.len() as u64) as usize)
        } else {
            self.scalar_ty()
        }
    }

    /// Any type the generator writes: a base, an option of one, a list of
    /// one. Options of options are left out.
    pub(crate) fn any_ty(&mut self) -> Ty {
        match self.rng.weighted(&[10, 2, 2]) {
            0 => self.base_ty(),
            1 => Ty::opt(self.base_ty()),
            _ => Ty::list(self.base_ty()),
        }
    }

    /// A number literal, negative ones parenthesized.
    pub(crate) fn num_lit(&mut self) -> String {
        let n = match self.rng.weighted(&[6, 3, 2]) {
            0 => *self.rng.pick(NUMBERS),
            1 => self.rng.below(20) as f64,
            _ => (self.rng.below(2000) as f64 - 1000.0) / 8.0,
        };
        number(n)
    }

    /// A string literal.
    pub(crate) fn str_lit(&mut self) -> String {
        quote(self.rng.pick(STRINGS))
    }

    /// A literal of the type: what a `state` starts as. `none` and `[]`
    /// only when `pinned` (something else says the element type).
    pub(crate) fn lit(&mut self, t: &Ty, pinned: bool) -> String {
        match t {
            Ty::Num => self.num_lit(),
            Ty::Str => self.str_lit(),
            Ty::Bool => (if self.rng.chance(1, 2) {
                "true"
            } else {
                "false"
            })
            .into(),
            Ty::Opt(inner) => {
                if pinned && self.rng.chance(1, 2) {
                    "none".into()
                } else {
                    format!("some({})", self.lit(inner, pinned))
                }
            }
            Ty::List(_) => "[]".into(),
            Ty::Rec(i) => self.record_lit(*i),
        }
    }

    /// `Shape(field=literal, …)`, fields sometimes out of order.
    pub(crate) fn record_lit(&mut self, i: usize) -> String {
        let shape = self.shapes[i].clone();
        let mut fields: Vec<String> = shape
            .fields
            .iter()
            .map(|(f, t)| format!("{f}={}", self.lit(t, true)))
            .collect();
        self.maybe_shuffle(&mut fields);
        format!("{}({})", shape.name, fields.join(", "))
    }

    pub(crate) fn maybe_shuffle<T>(&mut self, xs: &mut [T]) {
        if self.rng.chance(1, 3) {
            for i in (1..xs.len()).rev() {
                let j = self.rng.below(i as u64 + 1) as usize;
                xs.swap(i, j);
            }
        }
    }
}

/// A number as Contract source: plain decimal digits (the lexer has no
/// exponent), negatives as a parenthesized negation.
pub(crate) fn number(n: f64) -> String {
    // Rust's `Display` for `f64` never uses an exponent and round-trips.
    let digits = format!("{}", n.abs());
    if n.is_sign_negative() {
        format!("(-{digits})")
    } else {
        digits
    }
}

/// A `"…"` string literal.
pub(crate) fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Literal text inside a template. The parser keeps a template's text raw
/// (an escape is never undone, and `\${` still interpolates), so text with
/// a backslash, backtick, `$` or line break is left out.
pub(crate) fn template_text(s: &str) -> Option<&str> {
    (!s.contains(['`', '$', '\\', '\n'])).then_some(s)
}

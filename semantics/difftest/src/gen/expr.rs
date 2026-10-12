//! Well-typed expressions of a requested type, fully parenthesized so no
//! precedence question arises, and `show`, which prints any value as a
//! string expression for a `text` to observe.

use super::ty::{number, quote, template_text, Ty, STRINGS};
use super::{Env, Gen};

/// Instants (epoch ms) at the edges `format*` names: the epoch, a day's
/// last millisecond, a fraction, before 1970, the first and last
/// formattable wall times and one past each.
const EPOCHS: &[f64] = &[
    0.0,
    -0.5,
    86_399_999.0,
    1_790_000_000_000.0,
    1_790_043_210_987.5,
    -1.0,
    951_782_400_000.0,
    -62_135_596_800_000.0,
    -62_135_596_800_001.0,
    253_402_300_799_999.0,
    253_402_300_800_000.0,
];

/// Text for `parseNumber`: numerals in each of the grammar's forms, at
/// rounding's edges, padded as `trim` strips, and near misses.
const NUMERALS: &[&str] = &[
    "12.5",
    " -3 ",
    "\u{a0}1e3\n",
    "+.5",
    "5.",
    "-0",
    "0",
    "1.7976931348623159e308",
    "2.4703282292062328e-324",
    "1e-400",
    "9007199254740993",
    "0.1",
    "",
    ".",
    "12px",
    "0x1F",
    "1_000",
    "Infinity",
    "1e",
    "\u{85}1",
];

/// Dates for `calendarDiff`: Feb 29 and the month ends, either order, and
/// near misses.
const DATES: &[&str] = &[
    "2024-02-29",
    "2025-02-28",
    "2025-03-01",
    "2028-02-29",
    "2024-01-31",
    "2024-03-01",
    "2024-04-30",
    "1990-06-15",
    "2026-06-14",
    "2026-06-15",
    "0000-01-01",
    "9999-12-31",
    "2025-02-29",
    "2024-13-01",
    "2024-1-01",
    "",
];

/// Numbers for `toFixed` and `formatDecimal`: binary-rounding cases
/// (`1.005`), halves, signed zero, a tiny negative, counts, and `1e21`.
const DECIMALS: &[f64] = &[
    1.005,
    2.5,
    -2.5,
    0.5,
    -0.0,
    -0.001,
    1234.0,
    -5.0,
    7.0,
    0.1,
    1e21,
    999999999999999900000.0,
    123.456,
    9007199254740993.0,
    5e-324,
];

/// UTC offsets in minutes east: whole, half and quarter hours, the ±18 h
/// bounds and just past them.
const OFFSETS: &[f64] = &[
    0.0, 60.0, -300.0, 330.0, 345.0, -0.5, 1080.0, -1080.0, 1081.0,
];

/// Counts at compact notation's boundaries and truncations.
const COUNTS: &[f64] = &[
    0.0,
    -0.0,
    0.29,
    0.005,
    9.99,
    999.0,
    1000.0,
    1250.0,
    -1250.0,
    999_999.0,
    1_000_000.0,
    12_345_678.0,
    1e12,
    1e16,
    123_456_789_012_345_680_000.0,
];

impl Gen<'_> {
    /// Every name in scope and the record fields under it, two deep.
    pub(crate) fn paths(&self, env: &Env) -> Vec<(String, Ty)> {
        let mut out = Vec::new();
        for (name, t) in &env.vars {
            self.push_paths(name, t, 2, &mut out);
        }
        out
    }

    fn push_paths(&self, name: &str, t: &Ty, depth: usize, out: &mut Vec<(String, Ty)>) {
        out.push((name.to_string(), t.clone()));
        if let (Ty::Rec(i), true) = (t, depth > 0) {
            for (f, ft) in &self.shapes[*i].fields {
                self.push_paths(&format!("{name}.{f}"), ft, depth - 1, out);
            }
        }
    }

    fn has_list(&self, env: &Env) -> bool {
        self.paths(env)
            .iter()
            .any(|(_, t)| matches!(t, Ty::List(_)))
    }

    /// The element type of an option (`opt`) or list in scope, most often;
    /// else any base type. Matching and mapping what the program holds
    /// beats matching `some(literal)`.
    pub(crate) fn held(&mut self, env: &Env, opt: bool) -> Ty {
        let held: Vec<Ty> = self
            .paths(env)
            .into_iter()
            .filter_map(|(_, t)| match (t, opt) {
                (Ty::Opt(e), true) | (Ty::List(e), false) => Some(*e),
                _ => None,
            })
            .collect();
        if !held.is_empty() && self.rng.chance(2, 3) {
            self.rng.pick(&held).clone()
        } else {
            self.base_ty()
        }
    }

    /// A random type an unpinned expression here can have: any, since a
    /// list literal makes a list of any type (LLP 1088 §9.1).
    pub(crate) fn pick_ty(&mut self, _env: &Env) -> Ty {
        self.any_ty()
    }

    /// An expression of type `t`. `pinned`: the context fixes the type, so
    /// a bare `none` or `[]` is allowed at the top.
    pub(crate) fn expr(&mut self, env: &Env, t: &Ty, depth: usize, pinned: bool) -> String {
        if depth == 0 || self.rng.chance(1, 5) {
            return self.leaf(env, t, pinned);
        }
        let d = depth - 1;
        let calls: Vec<usize> = (0..env.fns.min(self.fns.len()))
            .filter(|&k| self.fns[k].ret == *t)
            .collect();
        let fields = self.fields_of(t);
        let w = [
            12,
            2,
            2,
            if calls.is_empty() { 0 } else { 3 },
            if fields.is_empty() { 0 } else { 2 },
        ];
        match self.rng.weighted(&w) {
            0 => self.specific(env, t, d, pinned),
            1 => {
                let c = self.expr(env, &Ty::Bool, d, false);
                let (pa, pb) = self.arm_pins(pinned);
                let a = self.expr(env, t, d, pa);
                let b = self.expr(env, t, d, pb);
                format!("({c} ? {a} : {b})")
            }
            2 => {
                let inner = self.held(env, true);
                let subject = self.expr(env, &Ty::opt(inner.clone()), d, false);
                let v = self.fresh("v");
                let (pa, pb) = self.arm_pins(pinned);
                let a = self.expr(&env.with(&v, inner), t, d, pa);
                let b = self.expr(env, t, d, pb);
                format!("(match {subject} {{ case some({v}) => {a}, case none => {b} }})")
            }
            3 => {
                let k = *self.rng.pick(&calls);
                self.call(env, k, d)
            }
            _ => {
                let (shape, field) = self.rng.pick(&fields).clone();
                let rec = self.expr(env, &Ty::Rec(shape), d, false);
                format!("({rec}).{field}")
            }
        }
    }

    /// `name(args)` of the `k`th `fn`.
    pub(crate) fn call(&mut self, env: &Env, k: usize, d: usize) -> String {
        let f = self.fns[k].clone();
        let args: Vec<String> = f
            .params
            .iter()
            .map(|p| self.expr(env, p, d, true))
            .collect();
        format!("{}({})", f.name, args.join(", "))
    }

    /// Of a two-armed choice whose type the context does not pin, one arm
    /// must say it alone.
    fn arm_pins(&mut self, pinned: bool) -> (bool, bool) {
        if pinned {
            (true, true)
        } else if self.rng.chance(1, 2) {
            (false, true)
        } else {
            (true, false)
        }
    }

    /// Every (shape, field) whose field has type `t`.
    fn fields_of(&self, t: &Ty) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        for (i, s) in self.shapes.iter().enumerate() {
            for (f, ft) in &s.fields {
                if ft == t {
                    out.push((i, f.clone()));
                }
            }
        }
        out
    }

    /// A name, a field, or a literal of `t`.
    pub(crate) fn leaf(&mut self, env: &Env, t: &Ty, pinned: bool) -> String {
        let vars: Vec<String> = self
            .paths(env)
            .into_iter()
            .filter(|(_, vt)| vt == t)
            .map(|(n, _)| n)
            .collect();
        if !vars.is_empty() && self.rng.chance(3, 5) {
            return self.rng.pick(&vars).clone();
        }
        match t {
            Ty::Num => {
                if env.now && self.rng.chance(1, 30) {
                    "performanceNow()".into()
                } else if self.rng.chance(1, 16) {
                    (*self
                        .rng
                        .pick(&["(0 / 0)", "(1 / 0)", "((-1) / 0)", "(0 * (-1))"]))
                    .into()
                } else {
                    self.num_lit()
                }
            }
            Ty::List(elem) => {
                if pinned && self.rng.chance(1, 3) {
                    "[]".into()
                } else if !self.has_list(env) || self.rng.chance(1, 3) {
                    self.literal(env, elem, pinned, |g, env, t, pinned| {
                        g.leaf(env, t, pinned)
                    })
                } else {
                    let (src, src_elem) = self.list_path(env);
                    let (head, inner) = self.arrow_head(env, &src_elem);
                    let body = self.leaf(&inner, elem, false);
                    format!("map({src}, {head} {body})")
                }
            }
            Ty::Opt(inner) => {
                if pinned && self.rng.chance(1, 2) {
                    "none".into()
                } else {
                    format!("some({})", self.leaf(env, inner, pinned))
                }
            }
            Ty::Rec(i) => {
                let shape = self.shapes[*i].clone();
                let mut fields: Vec<String> = shape
                    .fields
                    .iter()
                    .map(|(f, ft)| format!("{f}={}", self.leaf(env, ft, true)))
                    .collect();
                self.maybe_shuffle(&mut fields);
                format!("{}({})", shape.name, fields.join(", "))
            }
            _ => self.lit(t, pinned),
        }
    }

    /// `[a, b]` of `elem`, each item made by `item` (LLP 1088 §9.1): one
    /// to three of them, or none where the context pins the type; the first
    /// item pins the rest, as a list's items unify.
    fn literal(
        &mut self,
        env: &Env,
        elem: &Ty,
        pinned: bool,
        mut item: impl FnMut(&mut Self, &Env, &Ty, bool) -> String,
    ) -> String {
        let n = self.rng.range(if pinned { 0 } else { 1 }, 3);
        let items: Vec<String> = (0..n)
            .map(|k| item(self, env, elem, pinned || k > 0))
            .collect();
        let comma = if n > 0 && self.rng.chance(1, 8) {
            ","
        } else {
            ""
        };
        format!("[{}{comma}]", items.join(", "))
    }

    /// A list-typed path in scope and its element type.
    fn list_path(&mut self, env: &Env) -> (String, Ty) {
        let lists: Vec<(String, Ty)> = self
            .paths(env)
            .into_iter()
            .filter_map(|(n, t)| match t {
                Ty::List(e) => Some((n, *e)),
                _ => None,
            })
            .collect();
        self.rng.pick(&lists).clone()
    }

    /// An arrow's parameter list and the scope its body sees.
    fn arrow_head(&mut self, env: &Env, elem: &Ty) -> (String, Env) {
        let x = self.fresh("x");
        let i = self.fresh("i");
        match self.rng.weighted(&[4, 4, 1]) {
            0 => (
                format!("({x}, {i}) =>"),
                env.with(&x, elem.clone()).with(&i, Ty::Num),
            ),
            1 => (format!("{x} =>"), env.with(&x, elem.clone())),
            _ => ("() =>".into(), env.clone()),
        }
    }

    fn specific(&mut self, env: &Env, t: &Ty, d: usize, pinned: bool) -> String {
        match t {
            Ty::Num => self.num_expr(env, d),
            Ty::Str => self.str_expr(env, d),
            Ty::Bool => self.bool_expr(env, d),
            Ty::Opt(inner) => {
                let list = Ty::list((**inner).clone());
                let read = if **inner == Ty::Num { 3 } else { 0 };
                match self
                    .rng
                    .weighted(&[5, if pinned { 2 } else { 0 }, 2, 2, read])
                {
                    0 => format!("some({})", self.expr(env, inner, d, pinned)),
                    1 => "none".into(),
                    2 => format!("first({})", self.expr(env, &list, d, false)),
                    3 => {
                        let l = self.expr(env, &list, d, false);
                        let n = self.expr(env, &Ty::Num, d, false);
                        format!("at({l}, {n})")
                    }
                    _ => self.read_number(env, d),
                }
            }
            Ty::List(elem) => match self.rng.weighted(&[
                5,
                3,
                if pinned { 1 } else { 0 },
                3,
                2,
                2,
                if **elem == Ty::Str { 2 } else { 0 },
            ]) {
                0 => {
                    let src_ty = self.held(env, false);
                    let src = self.expr(env, &Ty::list(src_ty.clone()), d, false);
                    let (head, inner) = self.arrow_head(env, &src_ty);
                    let body = self.expr(&inner, elem, d, false);
                    format!("map({src}, {head} {body})")
                }
                1 => {
                    let src = self.expr(env, t, d, false);
                    let (head, inner) = self.arrow_head(env, elem);
                    let body = self.expr(&inner, &Ty::Bool, d, false);
                    format!("filter({src}, {head} {body})")
                }
                2 => "[]".into(),
                3 => self.literal(env, elem, pinned, |g, env, t, pinned| {
                    g.expr(env, t, d, pinned)
                }),
                // LLP 1088 §9.1: the first list says the type; the second
                // may be `[]`.
                4 => {
                    let a = self.expr(env, t, d, pinned);
                    let b = self.expr(env, t, d, true);
                    format!("concat({a}, {b})")
                }
                5 => {
                    let l = self.expr(env, t, d, pinned);
                    let a = self.expr(env, &Ty::Num, d, false);
                    if self.rng.chance(1, 3) {
                        format!("slice({l}, {a})")
                    } else {
                        format!("slice({l}, {a}, {})", self.expr(env, &Ty::Num, d, false))
                    }
                }
                // `split` over code units: an empty separator cuts an astral
                // character into two U+FFFD (LLP 1088 §9.1).
                _ => {
                    let s = self.expr(env, &Ty::Str, d, false);
                    let sep = self.expr(env, &Ty::Str, d, false);
                    format!("split({s}, {sep})")
                }
            },
            Ty::Rec(i) => {
                let shape = self.shapes[*i].clone();
                if self.rng.chance(1, 2) {
                    let mut fields: Vec<String> = shape
                        .fields
                        .iter()
                        .map(|(f, ft)| format!("{f}={}", self.expr(env, ft, d, true)))
                        .collect();
                    self.maybe_shuffle(&mut fields);
                    format!("{}({})", shape.name, fields.join(", "))
                } else {
                    let base = self.expr(env, t, d, false);
                    let mut fields = Vec::new();
                    for (f, ft) in &shape.fields {
                        if fields.is_empty() || self.rng.chance(1, 3) {
                            fields.push(format!("{f}={}", self.expr(env, ft, d, true)));
                        }
                    }
                    format!("{}({base}, {})", shape.name, fields.join(", "))
                }
            }
        }
    }

    fn num_expr(&mut self, env: &Env, d: usize) -> String {
        match self.rng.weighted(&[8, 1, 3, 1, 2, 1]) {
            0 => {
                let op = *self.rng.pick(&["+", "-", "*", "/", "%", "+", "-", "*"]);
                let a = self.expr(env, &Ty::Num, d, false);
                let b = self.expr(env, &Ty::Num, d, false);
                format!("({a} {op} {b})")
            }
            1 => format!("(-{})", self.expr(env, &Ty::Num, d, false)),
            2 => format!("length({})", self.sized(env, d)),
            // `round` is JavaScript's `Math.round` (LLP 1102 §3.2).
            3 => {
                let f = *self.rng.pick(&["floor", "ceil", "round"]);
                format!("{f}({})", self.expr(env, &Ty::Num, d, false))
            }
            // `indexOf` over a list of strings, numbers or bools by strict
            // equality, or over text in code units (LLP 1088 §9.1).
            5 if self.rng.chance(1, 2) => {
                let t = self.scalar_ty();
                let l = self.expr(env, &Ty::list(t.clone()), d, false);
                let x = self.expr(env, &t, d, false);
                format!("indexOf({l}, {x})")
            }
            5 => {
                let a = self.expr(env, &Ty::Str, d, false);
                let b = self.expr(env, &Ty::Str, d, false);
                format!("indexOf({a}, {b})")
            }
            _ => {
                let f = *self.rng.pick(&["max", "min"]);
                let a = self.expr(env, &Ty::Num, d, false);
                let b = self.expr(env, &Ty::Num, d, false);
                format!("{f}({a}, {b})")
            }
        }
    }

    /// A string or a list: what `length` and `isEmpty` take.
    fn sized(&mut self, env: &Env, d: usize) -> String {
        if self.has_list(env) && self.rng.chance(1, 2) {
            let t = Ty::list(self.held(env, false));
            self.expr(env, &t, d, false)
        } else {
            self.expr(env, &Ty::Str, d, false)
        }
    }

    fn str_expr(&mut self, env: &Env, d: usize) -> String {
        let formats = if self.formats { 3 } else { 0 };
        match self.rng.weighted(&[6, 3, 2, 2, 2, formats, 2]) {
            0 => {
                let mut out = String::from("`");
                for _ in 0..self.rng.range(1, 3) {
                    if let (true, Some(text)) =
                        (self.rng.chance(1, 3), template_text(self.rng.pick(STRINGS)))
                    {
                        out.push_str(text);
                    } else {
                        let t = self.scalar_ty();
                        out.push_str(&format!("${{{}}}", self.expr(env, &t, d, false)));
                    }
                }
                out.push('`');
                out
            }
            1 => {
                let a = self.expr(env, &Ty::Str, d, false);
                let b = self.expr(env, &Ty::Str, d, false);
                format!("({a} + {b})")
            }
            2 => {
                let t = self.scalar_ty();
                format!("toString({})", self.expr(env, &t, d, false))
            }
            3 => {
                let f = *self.rng.pick(&["trim", "encodeURIComponent", "trim"]);
                format!("{f}({})", self.expr(env, &Ty::Str, d, false))
            }
            5 => self.format_expr(env, d),
            // LLP 1088 D2: over UTF-16 code units, an astral string cut or
            // split by an empty pattern; `toLowerCase` is left out, as the
            // semantics leaves it out.
            6 => {
                let s = self.expr(env, &Ty::Str, d, false);
                if self.rng.chance(1, 2) {
                    let a = self.expr(env, &Ty::Num, d, false);
                    if self.rng.chance(1, 3) {
                        format!("slice({s}, {a})")
                    } else {
                        format!("slice({s}, {a}, {})", self.expr(env, &Ty::Num, d, false))
                    }
                } else {
                    let find = self.expr(env, &Ty::Str, d, false);
                    let with = self.str_lit();
                    format!("replaceAll({s}, {find}, {with})")
                }
            }
            _ => {
                if !self.has_list(env) {
                    return self.expr(env, &Ty::Str, d, false);
                }
                let t = Ty::list(self.scalar_ty());
                let l = self.expr(env, &t, d, false);
                let sep = self.str_lit();
                format!("join({l}, {sep})")
            }
        }
    }

    /// `parseNumber` of a numeral, a near one or any text, or `calendarDiff`
    /// of two dates (LLP 1102 §3.1, §3.4).
    fn read_number(&mut self, env: &Env, d: usize) -> String {
        if self.rng.chance(1, 2) {
            let s = if self.rng.chance(2, 3) {
                quote(self.rng.pick(NUMERALS))
            } else {
                self.expr(env, &Ty::Str, d, false)
            };
            format!("parseNumber({s})")
        } else {
            let date = |g: &mut Self| {
                if g.rng.chance(4, 5) {
                    quote(g.rng.pick(DATES))
                } else {
                    g.expr(env, &Ty::Str, d, false)
                }
            };
            let (a, b) = (date(self), date(self));
            let unit = *self.rng.pick(&["\"years\"", "\"months\""]);
            format!("calendarDiff({a}, {b}, {unit})")
        }
    }

    /// `formatTime`, `formatDate` or `formatNumber`: an instant (epoch ms)
    /// at a UTC offset (minutes east; past ±18 h prints `""`), or a count.
    fn format_expr(&mut self, env: &Env, d: usize) -> String {
        let num = |g: &mut Self| {
            if g.rng.chance(1, 2) {
                number(*g.rng.pick(EPOCHS))
            } else {
                g.expr(env, &Ty::Num, d, false)
            }
        };
        match self.rng.weighted(&[2, 2, 3, 2, 2]) {
            0 => {
                let (at, off) = (num(self), number(*self.rng.pick(OFFSETS)));
                format!("formatTime({at}, {off}, \"short\")")
            }
            1 => {
                let (at, off) = (num(self), number(*self.rng.pick(OFFSETS)));
                let style = *self.rng.pick(&["\"medium\"", "\"month-year\"", "\"iso\""]);
                format!("formatDate({at}, {off}, {style})")
            }
            // LLP 1116 D8: `decimal`, `percent` and `currency` beside
            // `compact`, over counts and the halves and signed zero of
            // `DECIMALS`, each style rounding at its own cut.
            2 => {
                let style = *self.rng.pick(&[
                    "\"compact\"",
                    "\"compact\"",
                    "\"decimal\"",
                    "\"percent\"",
                    "\"currency\", \"USD\"",
                    "\"currency\", \"JPY\"",
                    "\"currency\", \"CHF\"",
                ]);
                let n = match self.rng.below(3) {
                    0 => number(*self.rng.pick(COUNTS)),
                    1 => number(*self.rng.pick(DECIMALS)),
                    _ => self.expr(env, &Ty::Num, d, false),
                };
                format!("formatNumber({n}, {style})")
            }
            // LLP 1102 §3.2: `toFixed` of any number at a literal 0–100,
            // `formatDecimal` of a count (often rounded, as money is) at 0–20.
            3 => {
                let n = if self.rng.chance(1, 2) {
                    number(*self.rng.pick(DECIMALS))
                } else {
                    self.expr(env, &Ty::Num, d, false)
                };
                let digits = *self.rng.pick(&[0, 1, 2, 2, 3, 20, 100]);
                format!("toFixed({n}, {digits})")
            }
            _ => {
                let n = match self.rng.below(3) {
                    0 => number(*self.rng.pick(DECIMALS)),
                    1 => format!("round({} * 100)", self.expr(env, &Ty::Num, d, false)),
                    _ => self.expr(env, &Ty::Num, d, false),
                };
                let digits = *self.rng.pick(&[0, 1, 2, 2, 3, 20]);
                format!("formatDecimal({n}, {digits})")
            }
        }
    }

    fn bool_expr(&mut self, env: &Env, d: usize) -> String {
        match self.rng.weighted(&[4, 5, 3, 1, 1, 2]) {
            // Numbers, or two strings in code-unit order (LLP 1088 D1).
            0 => {
                let op = *self.rng.pick(&["<", "<=", ">", ">="]);
                let t = if self.rng.chance(1, 3) {
                    Ty::Str
                } else {
                    Ty::Num
                };
                let a = self.expr(env, &t, d, false);
                let b = self.expr(env, &t, d, false);
                format!("({a} {op} {b})")
            }
            1 => {
                let t = self.pick_ty(env);
                let op = *self.rng.pick(&["==", "!="]);
                let a = self.expr(env, &t, d, false);
                let b = self.expr(env, &t, d, true);
                format!("({a} {op} {b})")
            }
            2 => {
                let op = *self.rng.pick(&["and", "or", "&&", "||"]);
                let a = self.expr(env, &Ty::Bool, d, false);
                let b = self.expr(env, &Ty::Bool, d, false);
                format!("({a} {op} {b})")
            }
            3 => {
                let op = *self.rng.pick(&["not ", "!"]);
                format!("({op}{})", self.expr(env, &Ty::Bool, d, false))
            }
            4 => format!("isEmpty({})", self.sized(env, d)),
            // `includes` over a list of strings, numbers or bools, by
            // SameValueZero (LLP 1088 §9.1), or over text.
            _ if self.rng.chance(1, 3) => {
                let t = self.scalar_ty();
                let l = self.expr(env, &Ty::list(t.clone()), d, false);
                let x = self.expr(env, &t, d, false);
                format!("includes({l}, {x})")
            }
            _ => {
                let f = *self.rng.pick(&["includes", "startsWith", "endsWith"]);
                let a = self.expr(env, &Ty::Str, d, false);
                let b = self.expr(env, &Ty::Str, d, false);
                format!("{f}({a}, {b})")
            }
        }
    }

    /// A string expression printing the value at `path` (a name or a field
    /// path, so repeating it costs nothing).
    pub(crate) fn show(&mut self, path: &str, t: &Ty) -> String {
        match t {
            Ty::Num | Ty::Bool => {
                if self.rng.chance(1, 2) {
                    format!("toString({path})")
                } else {
                    format!("`${{{path}}}`")
                }
            }
            Ty::Str => path.to_string(),
            Ty::Opt(inner) => {
                let v = self.fresh("v");
                let shown = self.show(&v, inner);
                format!("(match {path} {{ case some({v}) => (\"some(\" + {shown} + \")\"), case none => \"none\" }})")
            }
            Ty::List(inner) => {
                let x = self.fresh("x");
                let shown = self.show(&x, inner);
                format!("(\"[\" + join(map({path}, {x} => {shown}), \",\") + \"]\")")
            }
            Ty::Rec(i) => {
                let shape = self.shapes[*i].clone();
                let parts: Vec<String> = shape
                    .fields
                    .iter()
                    .map(|(f, ft)| self.show(&format!("{path}.{f}"), ft))
                    .collect();
                format!(
                    "(\"{}{{\" + {} + \"}}\")",
                    shape.name,
                    parts.join(" + \",\" + ")
                )
            }
        }
    }
}

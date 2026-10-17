//! Tamber, Exact's design system (LLP 1121): the `.tamber` grammar (§3),
//! its checker (D3–D5) and the build's one evaluator (D7), which folds a
//! design to the IR every generator and the editor read (D2).
//!
//! Stage 2 builds colour tokens and app axes, stage 6 length and number
//! tokens; every other kind is parsed where the grammar has it and refused
//! with the stage that brings it.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod check;
mod color_fn;
mod component;
mod css;
mod edit;
pub mod export;
mod export_kinds;
mod fmt;
mod fold;
pub mod history;
mod icon;
pub mod import;
mod ir;
mod lex;
mod measure;
mod motion;
mod session;
mod shadow;
mod syntax;
mod typeface;

pub use check::{Checked, MAX_TUPLES};
pub use component::{Component, Param};
pub use edit::{apply, apply_all, undo, Edited, Op, WhenLine};
pub use fmt::format;
pub use fold::{Folded, Half, Host};
pub use icon::Icon;
pub use ir::FORMAT;
pub use measure::{Kind, Length, Value};
pub use session::{plan, Class, Planned, Refused, Retention, Synthetic};
pub use shadow::{Shadow, Shadows};
pub use syntax::{parse, At, Axis, Design, Dial, DialKind, Token, When};
pub use typeface::Type;

/// A refusal, at a line and column of the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable id (`tamber-dial-inert`, `tamber-under-light` …).
    pub id: &'static str,
    /// What and why.
    pub message: String,
    /// The line, 1-based.
    pub line: u32,
    /// The column, 1-based.
    pub col: u32,
}

impl Diagnostic {
    pub(crate) fn new(
        id: &'static str,
        message: impl Into<String>,
        line: u32,
        col: u32,
    ) -> Diagnostic {
        Diagnostic {
            id,
            message: message.into(),
            line,
            col,
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}: {}: {}",
            self.line, self.col, self.id, self.message
        )
    }
}

/// A colour role, as the kernel's schema lists it (LLP 1095 D2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Role {
    /// The schema's name: CSS's (`Canvas`) or Exact's (`secondary-label`).
    pub name: String,
    /// A CSS system colour, written by its CSS name; else `-exact-<name>`.
    pub system: bool,
    /// The light and dark pair every host shows until it reports its own.
    pub fallback: [String; 2],
    /// The web draws it in the nearest accent (`--exact-accent`): a token
    /// whose value is it is written so on the JS target.
    pub accent: bool,
}

/// What a design is checked against: the schema's vocabulary, which the
/// caller reads from `kernel/tables/schema.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vocab {
    /// The colour roles, in the schema's order.
    pub roles: Vec<Role>,
    /// The named lengths the kernel reads, as a row writes them (a text
    /// style, `-exact-body` or its WebKit alias; `-exact-hairline`); matched
    /// without case, as CSS matches keywords.
    pub named_lengths: Vec<String>,
    /// Each named length's px at the default root size (16), as text: what
    /// the DTCG export writes a text style or the hairline as.
    pub sizes: Vec<(String, String)>,
}

impl Vocab {
    /// [`Vocab::named_lengths`] from the schema's text styles (each name and
    /// its WebKit alias, `title-1` for `title1` too, as the kernel reads
    /// them) and its named lengths (`hairline`), each `-exact-<name>`.
    pub fn named_lengths<'a>(
        text_styles: impl IntoIterator<Item = (&'a str, &'a str)>,
        lengths: impl IntoIterator<Item = &'a str>,
    ) -> Vec<String> {
        let mut out = Vec::new();
        for (name, alias) in text_styles {
            out.push(format!("-exact-{name}"));
            if let Some(d) = name.chars().last().filter(char::is_ascii_digit) {
                out.push(format!("-exact-{}-{d}", &name[..name.len() - 1]));
            }
            if !alias.is_empty() {
                out.push(alias.to_string());
            }
        }
        out.extend(lengths.into_iter().map(|n| format!("-exact-{n}")));
        out
    }
}

/// A checked, folded design.
#[derive(Debug, Clone, PartialEq)]
pub struct Folding {
    /// The design as parsed.
    pub design: Design,
    /// Its tokens, in declaration order.
    pub tokens: Vec<Checked>,
    /// Its icon tokens, in declaration order (D10).
    pub icons: Vec<Icon>,
    /// How many full app tuples its axes make.
    pub tuples: usize,
    /// What the author should know and the build does not refuse: a pair
    /// below WCAG's text contrast (`tamber-contrast`, D3); a type token in
    /// px (`tamber-type-fixed`, D6).
    pub warnings: Vec<Diagnostic>,
    fallbacks: Vec<Option<[String; 2]>>,
    dials: Vec<String>,
}

/// Parse, check and fold a `.tamber` source.
pub fn check(src: &str, vocab: &Vocab) -> Result<Folding, Vec<Diagnostic>> {
    check_design(parse(src)?, vocab)
}

/// Check and fold a design already parsed, or built by a caller: the
/// compiler's inline `theme` (LLP 1095 D4) and its synthetic tokens (D3).
pub fn check_design(design: Design, vocab: &Vocab) -> Result<Folding, Vec<Diagnostic>> {
    let (icons, mut refused) = icon::check(&design.icons, &design.tokens);
    // An icon under `theme` is refused by the icons check alone.
    let stripped;
    let colours = if design.tokens.iter().any(icon::misplaced) {
        let mut d = design.clone();
        // Kept as a colour no one sees, so a value naming it is not
        // refused a second time.
        for t in d.tokens.iter_mut().filter(|t| icon::misplaced(t)) {
            t.kind = None;
            t.value.value = "transparent".into();
            t.whens.clear();
        }
        stripped = d;
        &stripped
    } else {
        &design
    };
    let (tokens, tuples, dials) = match check::check(colours, vocab) {
        Ok(checked) if refused.is_empty() => checked,
        Ok(_) => {
            refused.sort_by_key(|d| (d.line, d.col));
            return Err(refused);
        }
        Err(mut errors) => {
            errors.append(&mut refused);
            errors.sort_by_key(|d| (d.line, d.col));
            return Err(errors);
        }
    };
    let fallbacks = tokens
        .iter()
        .map(|t| match t.values.first()? {
            Value::Color(f) => fallback(f, vocab),
            other => Some([other.css(), other.css()]),
        })
        .collect();
    let mut warnings = contrast(&design, &tokens, tuples, vocab);
    warnings.extend(type_fixed(&design, &tokens));
    Ok(Folding {
        design,
        tokens,
        icons,
        tuples,
        warnings,
        fallbacks,
        dials: dials.iter().map(ir::dial_text).collect(),
    })
}

/// Each type token whose size is px in some tuple (`tamber-type-fixed`,
/// D6): the author's choice not to scale with the person's text size,
/// which the build does not refuse.
fn type_fixed(design: &Design, tokens: &[Checked]) -> Vec<Diagnostic> {
    tokens
        .iter()
        .zip(&design.tokens)
        .filter(|(t, _)| {
            t.values
                .iter()
                .any(|v| matches!(v, Value::Type(t) if matches!(t.size, Length::Px(_))))
        })
        .map(|(t, declared)| {
            Diagnostic::new(
                "tamber-type-fixed",
                format!("`{}`'s size is px, so it does not follow the person's text size (Dynamic Type, the browser's setting): a text style or `rem` does", t.name),
                declared.name.line,
                declared.name.col,
            )
        })
        .collect()
}

/// WCAG 2's text ratio, below which a declared pair is warned of.
pub const TEXT_CONTRAST: f64 = 4.5;

/// Each declared pair (`pair=`, D3) whose colours, in some tuple and scheme,
/// fall below 4.5:1: one warning a pair, at its lowest, naming the tuple,
/// the scheme and the ratio. A residual side (a role, a platform colour) is
/// read at its fallback, and the warning says so; a background that is not
/// opaque has no one ratio, and is passed over.
fn contrast(design: &Design, tokens: &[Checked], tuples: usize, vocab: &Vocab) -> Vec<Diagnostic> {
    let sides = |v: &Folded| -> Option<([String; 2], bool)> {
        match v {
            Folded::One(h) => Some(([h.clip(), h.clip()], false)),
            Folded::Pair(l, d) => Some(([l.clip(), d.clip()], false)),
            Folded::Host(_) => fallback(v, vocab).map(|f| (f, true)),
        }
    };
    let mut out = Vec::new();
    for (t, declared) in tokens.iter().zip(&design.tokens) {
        for pair in &t.pairs {
            let under = tokens.iter().find(|u| &u.name == pair);
            let role = Folded::Host(Host::Role(pair.clone()));
            let mut lowest: Option<(f64, usize, usize, bool)> = None;
            for tuple in 0..tuples {
                let Some((fg, a)) = t.values.get(tuple).and_then(Value::color).and_then(sides)
                else {
                    continue;
                };
                let bg = match under {
                    Some(u) => u.values.get(tuple).and_then(Value::color).and_then(sides),
                    None => sides(&role),
                };
                let Some((bg, b)) = bg else { continue };
                for side in 0..2 {
                    // As the badges show it, two places cut, never rounded up past
                    // the line: 4.499 is 4.49, below 4.5.
                    let Some(r) = ratio(&fg[side], &bg[side]).map(|r| (r * 100.0).floor() / 100.0)
                    else {
                        continue;
                    };
                    if r < TEXT_CONTRAST && lowest.is_none_or(|(l, ..)| r < l) {
                        lowest = Some((r, tuple, side, a || b));
                    }
                }
            }
            if let Some((r, tuple, side, residual)) = lowest {
                let modes = modes_of(design, tuple);
                let at = if modes.is_empty() {
                    String::new()
                } else {
                    format!(" {modes},")
                };
                let fell = if residual {
                    "; a role or platform colour is read at its fallback"
                } else {
                    ""
                };
                out.push(Diagnostic::new(
                    "tamber-contrast",
                    format!(
                        "`{}` on `{pair}` is {r:.2}:1 in{at} {}: below {TEXT_CONTRAST}:1, WCAG's ratio for text{fell}",
                        t.name,
                        if side == 0 { "light" } else { "dark" }
                    ),
                    declared.name.line,
                    declared.name.col,
                ));
            }
        }
    }
    out
}

/// Tuple `tuple`'s modes off their defaults, `axis=value`, the first axis
/// most significant.
fn modes_of(design: &Design, tuple: usize) -> String {
    let mut rest = tuple;
    let mut out = vec![String::new(); design.axes.len()];
    for (i, a) in design.axes.iter().enumerate().rev() {
        let n = a.values.len().max(1);
        let v = rest % n;
        rest /= n;
        if v != 0 {
            out[i] = format!("{}={}", a.name.value, a.values[v]);
        }
    }
    out.retain(|m| !m.is_empty());
    out.join(" ")
}

/// WCAG 2's contrast ratio of a text colour over a background, each
/// `#rrggbb[aa]`, the text composited over the background; none when the
/// background is not opaque or either is no such colour.
pub fn ratio(fg: &str, bg: &str) -> Option<f64> {
    let rgba = |hex: &str| -> Option<[f64; 4]> {
        let h = hex.strip_prefix('#')?;
        if !(h.len() == 6 || h.len() == 8) || !h.is_ascii() {
            return None;
        }
        let at = |i: usize| {
            u8::from_str_radix(&h[i..i + 2], 16)
                .ok()
                .map(|v| f64::from(v) / 255.0)
        };
        Some([
            at(0)?,
            at(2)?,
            at(4)?,
            if h.len() == 8 { at(6)? } else { 1.0 },
        ])
    };
    let (f, b) = (rgba(fg)?, rgba(bg)?);
    if b[3] < 1.0 {
        return None;
    }
    let over = [0, 1, 2].map(|i| f[i] * f[3] + b[i] * (1.0 - f[3]));
    let lum = |c: [f64; 3]| {
        let l = c.map(|v| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        });
        0.2126 * l[0] + 0.7152 * l[1] + 0.0722 * l[2]
    };
    let (x, y) = (lum(over), lum([b[0], b[1], b[2]]));
    Some((x.max(y) + 0.05) / (x.min(y) + 0.05))
}

/// Whether an `exports` line (D7) names token `name`, which overrides `role`
/// if it does: its name, or the role in any spelling a row may write
/// (`canvastext`, `secondary-label` for `-exact-secondary-label`).
pub fn exports_names(line: &str, name: &str, role: Option<&str>) -> bool {
    let spelled = |a: &str, b: &str| {
        a.eq_ignore_ascii_case(b) || a.eq_ignore_ascii_case(b.trim_start_matches("-exact-"))
    };
    line == name || role.is_some_and(|r| spelled(line, r) || spelled(line, name))
}

/// A value's fallback pair (D7): light and dark, roles at theirs.
pub fn fallback(value: &Folded, vocab: &Vocab) -> Option<[String; 2]> {
    match value {
        Folded::One(h) => Some([h.clip(), h.clip()]),
        Folded::Pair(l, d) => Some([l.clip(), d.clip()]),
        Folded::Host(Host::Role(row)) => vocab
            .roles
            .iter()
            .find(|r| {
                let spelled = if r.system {
                    r.name.clone()
                } else {
                    format!("-exact-{}", r.name)
                };
                spelled.eq_ignore_ascii_case(row)
            })
            .map(|r| {
                r.fallback.clone().map(|c| match exact_color::parse(&c) {
                    Some(exact_color::Parsed::Legacy(c)) => Half::legacy(c).clip(),
                    _ => c,
                })
            }),
        Folded::Host(Host::Platform(_, pair)) => Some(pair.clone()),
    }
}

impl Folding {
    /// The IR, `tamber.json` (D2): generated, never committed.
    pub fn ir(&self) -> String {
        ir::write(self)
    }

    /// A token's value for one tuple, by name.
    pub fn value(&self, name: &str, tuple: usize) -> Option<&Value> {
        self.tokens
            .iter()
            .find(|t| t.name == name)
            .and_then(|t| t.values.get(tuple))
    }

    /// The tuple for `axis=value` pairs, the rest at their defaults.
    pub fn tuple(&self, modes: &[(&str, &str)]) -> Option<usize> {
        let mut tuple = 0;
        for axis in &self.design.axes {
            let chosen = modes
                .iter()
                .find(|(a, _)| *a == axis.name.value)
                .map_or(Some(0), |(_, v)| axis.values.iter().position(|x| x == v))?;
            tuple = tuple * axis.values.len() + chosen;
        }
        modes
            .iter()
            .all(|(a, _)| self.design.axes.iter().any(|x| x.name.value == *a))
            .then_some(tuple)
    }
}

//! Tamber, Exact's design system (LLP 1121): the `.tamber` grammar (§3),
//! its checker (D3–D5) and the build's one evaluator (D7), which folds a
//! design to the IR every generator and the editor read (D2).
//!
//! Stage 2 builds colour tokens and app axes, stage 6 length and number
//! tokens; every other kind is parsed where the grammar has it and refused
//! with the stage that brings it.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod bound;
mod budget;
mod check;
mod checked;
mod color_fn;
mod component;
mod css;
mod edit;
pub mod export;
mod export_batch;
pub use export_batch::own_digest;
mod export_dials;
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
pub mod params;
mod residual;
mod selectors;
mod session;
mod shadow;
mod syntax;
mod trace;
mod typeface;

pub use bound::{MAX_AXES, MAX_REFERENCE_DEPTH, MAX_WHENS};
pub use check::{Checked, MAX_TUPLES};
pub use component::{Component, Param};
pub use css::{MAX_VALUE_BYTES, MAX_VALUE_DEPTH};
pub use edit::{apply, apply_all, apply_all_with, undo, Edited, Op, WhenLine};
pub use fmt::format;
pub use fold::{Folded, Half, Host};
pub use icon::Icon;
pub use ir::FORMAT;
pub use lex::is_name;
pub use measure::{Kind, Length, Value};
pub use residual::{Atom, Residual, MAX_RESIDUAL_ATOMS};
pub use session::{plan, Class, Planned, Refused, Retention, Synthetic};
pub use shadow::{Shadow, ShadowLength, Shadows};
pub use syntax::{parse, At, Axis, Design, Dial, DialKind, Token, When};
pub use trace::{traced, ByTuple};
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
            message: bounded_words(message.into()),
            line,
            col,
        }
    }
}

/// The most a diagnostic's words may be, in bytes: a refusal quotes what it
/// refuses, and a design may hold text of any length a parser or a check
/// names before the checker's bounds see it (a malformed range, an unknown
/// kind), so past this the words keep their start and say their length.
const MAX_WORDS: usize = 1024;

/// `words` within [`MAX_WORDS`]: a diagnostic's, or a refusal's a caller
/// words itself (the wire's).
pub fn bounded_words(words: String) -> String {
    if words.len() <= MAX_WORDS {
        return words;
    }
    let mut cut = MAX_WORDS - 64;
    while !words.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}… ({} bytes in all)", &words[..cut], words.len())
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
    /// the DTCG export writes a text style or the hairline as (and a base
    /// name, the caller's chosen column's: Exact gives the neutral one's).
    pub sizes: Vec<(String, String)>,
    /// The base names the host resolves per platform (Exact's base scale,
    /// `space.md`; LLP 1121 stage 11 B), matched exactly. One the design
    /// leaves unset: a value naming it bare is a named length, kept as
    /// written and never folded, so each host reads its own; a length's
    /// `calc()`, `min()`, `max()` or `clamp()` over it is kept too, folded
    /// as far as it goes ([`Residual`], stage 12 B2), and so is a shadow's
    /// length over it ([`ShadowLength`], stage 12 B3); any other value over
    /// it is refused (`tamber-base-expression`). One the design sets is its
    /// token, as any.
    pub bases: Vec<String>,
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
    /// The design as parsed, but an icon naming another holds that one's
    /// `icon(…)` (a written alias is the source's to say).
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

/// Whether `text` is within the design checker's bounds on a value (its
/// bytes and nesting): a caller that rewrites values before the check (the
/// compiler's profile colours) leaves one past them as written, so the
/// check refuses it in the same words the design server's does.
pub fn within_bounds(text: &str) -> bool {
    css::over_limit(text).is_none()
}

/// A name past the checker's bound (`tamber-value-size`), in the words a
/// design's or an edit's name is refused in: for a caller that reads a name
/// before the checker does (the wire's param names).
pub fn over_name(name: &str) -> Option<(&'static str, String)> {
    css::over_bytes("a name", name)
}

/// Parse, check and fold a `.tamber` source.
pub fn check(src: &str, vocab: &Vocab) -> Result<Folding, Vec<Diagnostic>> {
    check_design(parse(src)?, vocab)
}

/// Check and fold a design already parsed, or built by a caller: the
/// compiler's inline `theme` (LLP 1095 D4) and its synthetic tokens (D3).
pub fn check_design(design: Design, vocab: &Vocab) -> Result<Folding, Vec<Diagnostic>> {
    check_uncached(design, vocab)
}

/// [`check_design`], answered from the last few designs checked when this one
/// is among them unchanged: for a server's reads (`describe`, `resolve`, a
/// scrub's frame), so a client asking can't make it fold the same design,
/// dial influence and all, again and again (`checked`).
pub fn check_design_cached(design: Design, vocab: &Vocab) -> Result<Folding, Vec<Diagnostic>> {
    checked::cached(design, vocab, check_uncached)
}

fn check_uncached(mut design: Design, vocab: &Vocab) -> Result<Folding, Vec<Diagnostic>> {
    // The checker's own bounds first: a value past them is refused where it
    // is written, before anything reads it (`bound::preflight`).
    let over = bound::preflight(&design);
    if !over.is_empty() {
        return Err(over);
    }
    // An icon naming another is read as that one, whoever folds the design
    // (`params::laid` did this alone, so `check` and an export refused it).
    icon::aliased(&mut design.icons);
    let (icons, mut refused) = icon::check(&design.icons, &design.tokens);
    // A base name is the scale's length: an icon is never one (a dial's
    // name is an identifier, so no dial is one).
    for name in design.icons.iter().map(|t| &t.name) {
        if vocab.bases.contains(&name.value) {
            refused.push(Diagnostic::new("tamber-base-kind", format!("`{}` is a base length (the scale's): an app sets it as a length token, not an icon", name.value), name.line, name.col));
        }
    }
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
        Ok(checked)
            if refused.is_empty() && misnamed(&design, kinds(&checked.0), vocab).is_empty() =>
        {
            checked
        }
        Ok(checked) if refused.is_empty() => {
            return Err(misnamed(&design, kinds(&checked.0), vocab))
        }
        // Each refusal at once: a base name misdeclared beside another
        // refusal is said too (round 3, Grok).
        Ok(checked) => {
            refused.extend(misnamed(&design, kinds(&checked.0), vocab));
            refused.sort_by_key(|d| (d.line, d.col));
            return Err(refused);
        }
        Err((mut errors, inferred)) => {
            // The checker refused: each token's kind, written or inferred.
            errors.extend(misnamed(&design, inferred, vocab));
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

/// `tamber-base-kind` (stage 11 B, theme2's (c)): a base name the design
/// declares as anything but a length, which every reader of the design
/// refuses (the build, a write's plan), not the lowering alone.
fn misnamed(design: &Design, kinds: Vec<Kind>, vocab: &Vocab) -> Vec<Diagnostic> {
    kinds
        .into_iter()
        .zip(&design.tokens)
        .filter(|(k, d)| vocab.bases.contains(&d.name.value) && *k != Kind::Length)
        .map(|(k, d)| {
            let name = &d.name;
            Diagnostic::new("tamber-base-kind", format!("`{0}` is a base length (the scale's `{0}`): an app sets it to a length (`{0} = 12px`), not a {1}", name.value, k.name()), name.line, name.col)
        })
        .collect()
}

/// Each design token's kind, in the design's order, from the checked
/// tokens: one a token, as the checker refused none.
fn kinds(tokens: &[Checked]) -> Vec<Kind> {
    tokens.iter().map(|t| t.kind).collect()
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
    // The checked tokens by name, the first of each (linear: round 1 of the
    // checker sweep).
    let mut by_name = std::collections::HashMap::new();
    for u in tokens.iter().rev() {
        by_name.insert(u.name.as_str(), u);
    }
    for (t, declared) in tokens.iter().zip(&design.tokens) {
        for pair in &t.pairs {
            let under = by_name.get(pair.as_str()).copied();
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

/// The lowest contrast white text has over the colour of the token that
/// overrides `role` (by the schema's name), over the tuples and both
/// schemes, and whether it falls in dark: what a filled button's white
/// label reads at on `AccentColor` (LLP 1116.019 D3). None when no token
/// overrides the role or its colour is the host's.
pub fn white_over(tokens: &[Checked], role: &str) -> Option<(f64, bool)> {
    let t = tokens.iter().find(|t| t.role.as_deref() == Some(role))?;
    let mut lowest: Option<(f64, bool)> = None;
    for v in t.values.iter().filter_map(Value::color) {
        let sides = match v {
            Folded::One(h) => [*h, *h],
            Folded::Pair(l, d) => [*l, *d],
            Folded::Host(_) => continue,
        };
        for (dark, h) in [false, true].into_iter().zip(sides) {
            if let Some(r) = ratio("#ffffff", &h.clip()) {
                if lowest.is_none_or(|(l, _)| r < l) {
                    lowest = Some((r, dark));
                }
            }
        }
    }
    lowest
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

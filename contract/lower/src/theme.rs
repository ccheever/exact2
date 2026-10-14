//! `theme` (LLP 1095 D4) and the app's design, `app.tamber` (LLP 1121 D2):
//! the app's own colours, named once at its root. Either is checked and
//! folded by `exact-tamber`, the build's one evaluator, and the default app
//! tuple's values are bound while the app's styles are parsed
//! (`exact_kernel::style::theme`), so a row writes `color="brand"` and the
//! kernel reads the token's colour. A row's expression naming a token
//! (`color-mix(in srgb, brand 60%, white)`) is a synthetic token (D3),
//! folded here too and named by its whole text. The plan keeps D7's
//! retention: the tokens a literal names, every role override, and every
//! token when a computed string may name one.

mod scan;

use crate::{LowerError, Lowerer};
use contract_syntax::{Attr, Component, Expr, File, Span};
use exact_kernel::style::roles;
use exact_kernel::{Color, ColorValue};
use exact_tamber as tamber;
use scan::Said;

/// One resolved entry: the names it answers to and the colour text it
/// stands for, which names no entry.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ThemeEntry {
    /// The name as written; a synthetic token's is its expression.
    pub name: String,
    /// The role it overrides, as a row writes it (`-exact-secondary-label`,
    /// `Canvas`).
    pub role: Option<String>,
    /// The colour text.
    pub value: String,
}

/// A role's id, as a row writes it (`-exact-label`, `CanvasText`) or as a
/// theme key may (LLP 1095 D4's bare `secondary-label`).
fn role_named(name: &str) -> Option<(u8, String)> {
    let id = roles::role(name).or_else(|| {
        (!name.starts_with('-'))
            .then(|| roles::role(&format!("-exact-{name}")))
            .flatten()
    })?;
    let role = roles::role_of(id)?;
    let row = if roles::is_css_system(role) {
        role.name.to_string()
    } else {
        format!("-exact-{}", role.name)
    };
    Some((id, row))
}

/// A colour the kernel reads with no theme bound.
fn parses(text: &str) -> bool {
    ColorValue::parse_light_dark(text).is_some() || Color::parse(text).is_some()
}

/// The schema's roles, as the crate checks a design against them.
fn vocab() -> tamber::Vocab {
    let hex = |c: u32| format!("#{c:08x}");
    tamber::Vocab {
        roles: exact_kernel::COLOR_ROLES
            .iter()
            .map(|r| tamber::Role {
                name: r.name.into(),
                system: roles::is_css_system(r),
                fallback: [hex(r.light), hex(r.dark)],
            })
            .collect(),
    }
}

/// The words a colour-bearing value writes beside its colours (a gradient's,
/// a shadow's, a colour space's), which a token's name would shadow where
/// the kernel reads a whole component.
const RESERVED: &[&str] = &[
    "none",
    "auto",
    "to",
    "top",
    "bottom",
    "left",
    "right",
    "center",
    "at",
    "inset",
    "circle",
    "ellipse",
    "closest-side",
    "closest-corner",
    "farthest-side",
    "farthest-corner",
    "repeat",
    "no-repeat",
    "solid",
    "dashed",
    "dotted",
    "double",
    "hidden",
    "thin",
    "medium",
    "thick",
    "srgb",
    "srgb-linear",
    "display-p3",
    "a98-rgb",
    "prophoto-rgb",
    "rec2020",
    "xyz",
    "xyz-d50",
    "xyz-d65",
    "lab",
    "lch",
    "oklab",
    "oklch",
    "hsl",
    "hwb",
    "hue",
    "shorter",
    "longer",
    "increasing",
    "decreasing",
];

/// Where a design's token was written, for its refusals.
enum Origin {
    /// The design file: its source id.
    File(u32),
    /// An inline `theme`: each entry's span.
    Inline(Vec<Span>),
}

/// A synthetic token's line in the design the crate checks: past any file's.
const SYNTHETIC: u32 = 1 << 30;

/// A `color(--profile …)` entry (LLP 1100 D3), the plan's to resolve: kept
/// out of the crate's fold, as are the entries naming it whole.
struct Profiled {
    name: String,
    value: String,
    line: u32,
}

impl Lowerer<'_> {
    /// Check and fold the app's design, inline or `app.tamber`, and bind it
    /// for the styles that follow; the plan's table is what D7 keeps.
    pub(crate) fn declare_theme(
        &mut self,
        file: &File,
        root: &Component,
    ) -> exact_kernel::style::theme::ThemeScope {
        let Some((mut design, origin)) = self.design(file) else {
            return exact_kernel::style::theme::declarations([]);
        };
        let mut said = Said::of(file, root, &design);
        let profiled = self.profiles(&mut design, &origin);
        // An expression over a profile's colour is not the build's to fold.
        said.synthetic.retain(|s| {
            let named = scan::words(&s.expr).any(|w| profiled.iter().any(|p| p.name == w));
            if named && s.strict {
                self.errors.push(LowerError {
                    id: "lower-theme-value",
                    message: format!("`{}`: a profile's colour (`color(--…)`) is named whole, not inside an expression", s.expr),
                    span: s.span,
                });
            }
            !named
        });
        // A synthetic token from a literal no colour row is known to read
        // (a `fn`'s, a state's) is dropped when it does not fold.
        let folding = loop {
            let mut checked = design.clone();
            for (i, s) in said.synthetic.iter().enumerate() {
                let at = |value: String| tamber::At {
                    value,
                    line: SYNTHETIC + i as u32,
                    col: 1,
                };
                checked.tokens.push(tamber::Token {
                    name: at(format!("~{i}")),
                    kind: None,
                    value: at(s.expr.clone()),
                    whens: Vec::new(),
                });
            }
            match tamber::check_design(checked, &vocab()) {
                Ok(f) => break f,
                Err(all) => {
                    let loose = |d: &tamber::Diagnostic| {
                        d.line >= SYNTHETIC && !said.synthetic[(d.line - SYNTHETIC) as usize].strict
                    };
                    if all.iter().all(loose) {
                        let dropped: Vec<usize> =
                            all.iter().map(|d| (d.line - SYNTHETIC) as usize).collect();
                        let mut i = 0;
                        said.synthetic.retain(|_| {
                            i += 1;
                            !dropped.contains(&(i - 1))
                        });
                        continue;
                    }
                    for d in all.iter().filter(|d| !loose(d)) {
                        self.design_refusal(d, &origin, &said);
                    }
                    return exact_kernel::style::theme::declarations([]);
                }
            }
        };
        let mut entries = Vec::new();
        for (i, t) in folding.tokens.iter().enumerate() {
            let Some(value) = t.values.first() else {
                continue;
            };
            let synthetic = t
                .name
                .strip_prefix('~')
                .and_then(|n| n.parse::<usize>().ok());
            // A token's name as written (LLP 1095 D4's bare `secondary-label`).
            let name = match synthetic {
                Some(n) => said.synthetic[n].expr.clone(),
                None => folding.design.tokens[i].name.value.clone(),
            };
            let role = synthetic.is_none().then(|| role_named(&name)).flatten();
            let line = folding.design.tokens[i].name.line;
            if synthetic.is_none()
                && !self.admitted(&name, role.as_ref(), Some(value), line, &origin)
            {
                continue;
            }
            entries.push(ThemeEntry {
                name,
                role: role.map(|(_, row)| row),
                value: value.css(),
            });
        }
        for p in profiled {
            let role = role_named(&p.name);
            if self.admitted(&p.name, role.as_ref(), None, p.line, &origin) {
                entries.push(ThemeEntry {
                    name: p.name,
                    role: role.map(|(_, row)| row),
                    value: p.value,
                });
            }
        }
        self.theme = entries;
        // D7's retention: what the view may name, and every role override.
        for e in &self.theme {
            if e.role.is_some() || said.computed || said.names(&e.name) {
                self.b
                    .theme_entry(&e.name, e.role.as_deref().unwrap_or(""), &e.value);
            }
        }
        exact_kernel::style::theme::declarations(self.theme.iter().flat_map(|e| {
            let tint = e
                .role
                .as_deref()
                .and_then(roles::role)
                .is_some_and(roles::tint);
            (!tint)
                .then_some((e.name.as_str(), e.value.as_str(), false))
                .into_iter()
                .chain(
                    e.role
                        .as_deref()
                        .filter(|_| !tint)
                        .map(|r| (r, e.value.as_str(), true)),
                )
        }))
    }

    /// The app's design as the crate reads it: `app.tamber` parsed, or the
    /// inline `theme` built, one token an entry. Both is refused.
    fn design(&mut self, file: &File) -> Option<(tamber::Design, Origin)> {
        if let (Some(_), Some(theme)) = (&file.design, &file.theme) {
            self.errors.push(LowerError {
                id: "tamber-two-sources",
                message: "this app has a design (`app.tamber`) and an inline `theme`: an app has one or the other; move the entries into `app.tamber`'s `theme` block (LLP 1121 D2)".into(),
                span: theme.span,
            });
        }
        if let Some(d) = &file.design {
            let origin = Origin::File(d.source_id);
            return match tamber::parse(&d.text) {
                Ok(design) => Some((design, origin)),
                Err(all) => {
                    for e in all {
                        self.design_refusal(&e, &origin, &Said::default());
                    }
                    None
                }
            };
        }
        let theme = file.theme.as_ref()?;
        let names: Vec<&str> = theme.entries.iter().map(|e| e.name.as_str()).collect();
        let mut tokens = Vec::new();
        for (i, entry) in theme.entries.iter().enumerate() {
            let Expr::Str(text, _) = &entry.value else {
                continue;
            };
            let text = text.trim();
            // LLP 1095 D4's bare role (`"secondary-grouped-background"`), as
            // the crate spells it, unless an entry is called that.
            let value = match role_named(text) {
                Some((_, row)) if !names.contains(&text) => row,
                _ => text.to_string(),
            };
            let at = |value: String| tamber::At {
                value,
                line: i as u32 + 1,
                col: 1,
            };
            tokens.push(tamber::Token {
                name: at(entry.name.clone()),
                kind: None,
                value: at(value),
                whens: Vec::new(),
            });
        }
        let design = tamber::Design {
            name: "app".into(),
            tokens,
            ..Default::default()
        };
        let spans = theme.entries.iter().map(|e| e.span).collect();
        Some((design, Origin::Inline(spans)))
    }

    /// Takes the profile colours (LLP 1100 D3) out of `design`: an entry
    /// that is one, and an entry naming such an entry whole. Another use of
    /// one's name is refused, as the crate cannot fold it.
    fn profiles(&mut self, design: &mut tamber::Design, origin: &Origin) -> Vec<Profiled> {
        let mut out: Vec<Profiled> = Vec::new();
        loop {
            let found = design.tokens.iter().position(|t| {
                let text = t.value.value.trim();
                t.whens.is_empty()
                    && (text.to_ascii_lowercase().starts_with("color(--") && parses(text)
                        || out.iter().any(|p| p.name == text))
            });
            let Some(i) = found else { break };
            let t = design.tokens.remove(i);
            let text = t.value.value.trim().to_string();
            let value = match out.iter().find(|p| p.name == text) {
                Some(p) => p.value.clone(),
                None => text,
            };
            out.push(Profiled {
                name: t.name.value,
                value,
                line: t.name.line,
            });
        }
        for t in &design.tokens {
            let values = std::iter::once(&t.value).chain(t.whens.iter().map(|w| &w.value));
            if values
                .into_iter()
                .any(|v| scan::words(&v.value).any(|w| out.iter().any(|p| p.name == w)))
            {
                let span = self.token_span(origin, t.name.line, t.name.col);
                self.push(LowerError {
                    id: "lower-theme-value",
                    message: format!("`{}`: a profile's colour (`color(--…)`) is named whole by another token, not inside an expression or under `when`", t.name.value),
                    span,
                });
            }
        }
        out
    }

    /// The refusals a design's token earns beyond the crate's: a name that
    /// shadows a value's words, and the tint rule (LLP 1095 D1).
    fn admitted(
        &mut self,
        name: &str,
        role: Option<&(u8, String)>,
        value: Option<&tamber::Folded>,
        line: u32,
        origin: &Origin,
    ) -> bool {
        let span = self.token_span(origin, line, 1);
        let refuse = |id: &'static str, message: String| LowerError { id, message, span };
        let error = match role {
            None if RESERVED.contains(&name.to_ascii_lowercase().as_str()) => Some(refuse(
                "lower-theme-name",
                format!("`{name}` is a CSS keyword; name the token otherwise"),
            )),
            // The tint is the app's: `AccentColor` sets it; a role drawn in
            // it follows (LLP 1095 D1, an authored accent over the design).
            Some((id, _)) if roles::tint(*id) && *id != roles::accent() => Some(refuse(
                "lower-theme-tint",
                format!("`{name}` is drawn in the app's tint: a theme sets the tint with `AccentColor`"),
            )),
            Some((id, _)) if *id == roles::accent() => match value {
                Some(tamber::Folded::Host(tamber::Host::Role(row)))
                    if roles::role(row).is_some_and(roles::tint) =>
                {
                    Some(refuse("lower-theme-tint", format!("`{name}=\"{row}\"` names a role drawn in the tint itself: the tint is a colour, a platform colour or another role")))
                }
                _ => None,
            },
            _ => None,
        };
        match error {
            Some(e) => {
                self.push(e);
                false
            }
            None => true,
        }
    }

    /// Where a design's token line is: its entry's span, or its place in
    /// the design file.
    fn token_span(&self, origin: &Origin, line: u32, col: u32) -> Span {
        match origin {
            Origin::Inline(spans) => spans
                .get(line.saturating_sub(1) as usize)
                .copied()
                .unwrap_or_default(),
            Origin::File(source_id) => Span {
                line,
                col,
                end_col: col,
                source_id: *source_id,
            },
        }
    }

    /// The crate's refusal, where it was written: an inline entry, a
    /// synthetic token's row, or the design file's line.
    fn design_refusal(&mut self, d: &tamber::Diagnostic, origin: &Origin, said: &Said) {
        let (span, message) = if d.line >= SYNTHETIC {
            let s = &said.synthetic[(d.line - SYNTHETIC) as usize];
            (s.span, format!("`{}`: {}", s.expr, d.message))
        } else {
            (self.token_span(origin, d.line, d.col), d.message.clone())
        };
        self.push(LowerError {
            id: d.id,
            message,
            span,
        });
    }

    fn push(&mut self, e: LowerError) {
        if !self
            .errors
            .iter()
            .any(|o| o.id == e.id && o.span == e.span && o.message == e.message)
        {
            self.errors.push(e);
        }
    }
}

/// The app's colours as `theme` entries, for the passes that read them
/// before lowering (the lint, the one-scheme warning): the inline block, or
/// the design's tokens at their default values.
pub(crate) fn theme_decl(file: &File) -> Option<contract_syntax::ThemeDecl> {
    if let Some(theme) = &file.theme {
        return Some(theme.clone());
    }
    let source = file.design.as_ref()?;
    let design = tamber::parse(&source.text).ok()?;
    // Each at its place in the design file.
    let at = |t: &tamber::At<String>| Span {
        line: t.line,
        col: t.col,
        end_col: t.col + t.value.len() as u32,
        source_id: source.source_id,
    };
    let entries = design
        .tokens
        .into_iter()
        .map(|t| Attr {
            span: at(&t.name),
            value: Expr::Str(t.value.value.clone(), at(&t.value)),
            name: t.name.value,
            written: true,
        })
        .collect();
    Some(contract_syntax::ThemeDecl {
        entries,
        span: Span::default(),
    })
}

/// `tamber-unknown-token` and `tamber-kind` (LLP 1121 D3) for a row's
/// literal the kernel refused, while a theme is bound: a name no token has,
/// in a colour row; a token's name in a row that takes no colour.
pub(crate) fn token_error(a: &Attr, rows: &[exact_kernel::StyleId], e: LowerError) -> LowerError {
    // The literal refused: the value, or the branch of `?:` or `match` at
    // the refusal's span.
    fn at(v: &Expr, span: Span) -> Option<(&str, Span)> {
        match v {
            Expr::Str(text, s) if *s == span => Some((text, *s)),
            Expr::Ternary(_, yes, no, _) => at(yes, span).or_else(|| at(no, span)),
            Expr::Match { some, none, .. } => at(some, span).or_else(|| at(none, span)),
            Expr::Let { body, .. } => at(body, span),
            _ => None,
        }
    }
    let found = match &a.value {
        Expr::Str(text, s) => Some((text.as_str(), *s)),
        other => at(other, e.span),
    };
    let Some((text, span)) = found else {
        return e;
    };
    let text = text.trim();
    let name_like = text.starts_with(|c: char| c.is_ascii_alphabetic() || c == '-')
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    if e.id != "lower-attr-value" || !name_like || !exact_kernel::style::theme::bound() {
        return e;
    }
    let colour = rows.iter().any(|r| scan::colour_codec(*r));
    let named = exact_kernel::style::theme::names(text);
    let (id, message) = match (colour, named) {
        (true, false) => (
            "tamber-unknown-token",
            format!("`{}=\"{text}\"`: `{text}` is no token of this app's design, no role and no CSS colour", a.name),
        ),
        (false, true) => (
            "tamber-kind",
            format!("`{}=\"{text}\"`: `{text}` is a colour token, and `{}` takes no colour (only colour tokens are built yet: LLP 1121 stage 2)", a.name, a.name),
        ),
        _ => return e,
    };
    LowerError { id, message, span }
}

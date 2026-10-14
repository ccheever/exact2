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
    /// The design file: its path.
    File(String),
    /// An inline `theme`: each entry's span.
    Inline(Vec<Span>),
}

/// A synthetic token's line in the design the crate checks: past any file's.
const SYNTHETIC: u32 = 1 << 30;

impl Lowerer<'_> {
    /// Check and fold the app's design, inline or `app.tamber`, and bind it
    /// for the styles that follow; the plan's table is what D7 keeps.
    pub(crate) fn declare_theme(
        &mut self,
        file: &File,
        root: &Component,
    ) -> exact_kernel::style::theme::ThemeScope {
        let (design, origin) = match self.design(file) {
            Some(d) => d,
            None => return exact_kernel::style::theme::declarations([]),
        };
        let said = Said::of(file, root, &design);
        let mut design = design;
        let profiled = self.stand_in_profiles(&mut design, &origin);
        for (i, (expr, _)) in said.synthetic.iter().enumerate() {
            let at = |value: String| tamber::At {
                value,
                line: SYNTHETIC + i as u32,
                col: 1,
            };
            design.tokens.push(tamber::Token {
                name: at(format!("~{i}")),
                kind: None,
                value: at(expr.clone()),
                whens: Vec::new(),
            });
        }
        let folding = match tamber::check_design(design, &vocab()) {
            Ok(f) => f,
            Err(all) => {
                for d in all {
                    self.design_refusal(&d, &origin, &said);
                }
                return exact_kernel::style::theme::declarations([]);
            }
        };
        let mut entries = Vec::new();
        for (i, t) in folding.tokens.iter().enumerate() {
            let Some(value) = t.values.first() else {
                continue;
            };
            let mut text = value.css();
            if let Some((_, original)) = profiled.iter().find(|(css, _)| *css == text) {
                text = original.clone();
            }
            let synthetic = t
                .name
                .strip_prefix('~')
                .and_then(|n| n.parse::<usize>().ok());
            // A token's name as written (LLP 1095 D4's bare `secondary-label`).
            let name = match synthetic {
                Some(n) => said.synthetic[n].0.clone(),
                None => folding.design.tokens[i].name.value.clone(),
            };
            let role = synthetic.is_none().then(|| role_named(&name)).flatten();
            let line = folding.design.tokens[i].name.line;
            if synthetic.is_none() && !self.admitted(&name, role.as_ref(), value, line, &origin) {
                continue;
            }
            entries.push(ThemeEntry {
                name,
                role: role.map(|(_, row)| row),
                value: text,
            });
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
            return match tamber::parse(&d.text) {
                Ok(design) => Some((design, Origin::File(d.path.clone()))),
                Err(all) => {
                    for e in all {
                        self.design_refusal(&e, &Origin::File(d.path.clone()), &Said::default());
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

    /// A `color(--profile …)` (LLP 1100 D3) is the plan's to resolve, so the
    /// crate folds a stand-in for it, named whole; returns each stand-in's
    /// folded text with the colour it stands for.
    fn stand_in_profiles(
        &mut self,
        design: &mut tamber::Design,
        origin: &Origin,
    ) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut names = Vec::new();
        for (n, t) in design.tokens.iter_mut().enumerate() {
            let text = t.value.value.trim().to_string();
            if !text.to_ascii_lowercase().starts_with("color(--") || !parses(&text) {
                continue;
            }
            // An 8-bit colour no author writes: alpha 1/255, the token's index.
            let stand_in = format!("#{:06x}01", n + 1);
            out.push((stand_in.clone(), text));
            names.push(t.name.value.clone());
            t.value.value = stand_in;
        }
        // Named whole only: the stand-in is no colour to mix.
        for t in &design.tokens {
            let value = t.value.value.trim();
            let mixes = value
                .split(|c: char| !(c.is_ascii_alphanumeric() || "-_.".contains(c)))
                .any(|w| names.iter().any(|n| n == w));
            if mixes && !names.iter().any(|n| n == value) {
                let span = self.token_span(origin, t.value.line);
                self.push(LowerError {
                    id: "lower-theme-value",
                    message: format!("`{}`: a profile's colour (`color(--…)`) is named whole by another token, not inside an expression", t.name.value),
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
        value: &tamber::Folded,
        line: u32,
        origin: &Origin,
    ) -> bool {
        let span = self.token_span(origin, line);
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
                tamber::Folded::Host(tamber::Host::Role(row))
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

    /// Where a design's token line is: its entry's span, else the root's.
    fn token_span(&self, origin: &Origin, line: u32) -> Span {
        match origin {
            Origin::Inline(spans) => spans
                .get(line.saturating_sub(1) as usize)
                .copied()
                .unwrap_or_default(),
            Origin::File(_) => Span::default(),
        }
    }

    /// The crate's refusal, where it was written: an inline entry, a
    /// synthetic token's row, or the design file's line.
    fn design_refusal(&mut self, d: &tamber::Diagnostic, origin: &Origin, said: &Said) {
        let (span, message) = if d.line >= SYNTHETIC {
            let (expr, span) = &said.synthetic[(d.line - SYNTHETIC) as usize];
            (*span, format!("`{expr}`: {}", d.message))
        } else {
            match origin {
                Origin::File(path) => (
                    Span::default(),
                    format!("{path}:{}:{}: {}", d.line, d.col, d.message),
                ),
                Origin::Inline(_) => (self.token_span(origin, d.line), d.message.clone()),
            }
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
    let design = tamber::parse(&file.design.as_ref()?.text).ok()?;
    let entries = design
        .tokens
        .into_iter()
        .map(|t| Attr {
            name: t.name.value,
            value: Expr::Str(t.value.value, Span::default()),
            span: Span::default(),
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
    let Expr::Str(text, span) = &a.value else {
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
    LowerError {
        id,
        message,
        span: *span,
    }
}

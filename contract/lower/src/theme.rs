//! `theme` (LLP 1095 D4) and the app's design, `app.tamber` (LLP 1121 D2):
//! the app's own colours, named once at its root. Either is checked and
//! folded by `exact-tamber`, the build's one evaluator, and the default app
//! tuple's values are bound while the app's styles are parsed
//! (`exact_kernel::style::theme`), so a row writes `color="brand"` and the
//! kernel reads the token's colour. A row's expression naming a token
//! (`color-mix(in srgb, brand 60%, white)`) is a synthetic token (D3),
//! folded here too and named by its whole text. The plan keeps D7's
//! retention: the tokens a literal names, every role override, every token
//! when a computed string may name one, and in an app that can name tokens
//! outside its views (a data or a native module) every token, or its
//! design's `exports`.

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
    /// The colour text, the default app tuple's.
    pub value: String,
    /// Its text for every full app tuple (LLP 1121 D5) when it varies.
    pub slices: Vec<String>,
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
pub fn vocab() -> tamber::Vocab {
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

/// One token's name, whatever role spelling it is written in.
fn same_name(a: &str, b: &str) -> bool {
    a == b || matches!((role_named(a), role_named(b)), (Some((x, _)), Some((y, _))) if x == y)
}

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
}

impl Lowerer<'_> {
    /// Check and fold the app's design, inline or `app.tamber`, and bind it
    /// for the styles that follow; the plan's table is what D7 keeps.
    /// `bindable`: the app can name a token outside its views
    /// ([`crate::names_tokens_outside_views`]).
    pub(crate) fn declare_theme(
        &mut self,
        file: &File,
        root: &Component,
        bindable: bool,
    ) -> exact_kernel::style::theme::ThemeScope {
        AXES.with(|axes| axes.borrow_mut().clear());
        CANDIDATES.with(|c| c.borrow_mut().clear());
        let Some((mut design, origin)) = self.design(file) else {
            return exact_kernel::style::theme::declarations([]);
        };
        let mut said = Said::of(file, root, &design);
        CANDIDATES.with(|c| {
            *c.borrow_mut() = said
                .synthetic
                .iter()
                .map(|s| (s.expr.clone(), s.strict))
                .collect();
        });
        let profiled = self.profiles(&mut design, &origin);
        // An expression over a profile's colour is not the build's to fold.
        said.synthetic.retain(|s| {
            let named = scan::words(&s.expr).any(|w| profiled.iter().any(|p| same_name(&p.name, w)));
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
                    meta: Vec::new(),
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
            let at = &folding.design.tokens[i].name;
            if synthetic.is_none()
                && !self.admitted(&name, role.as_ref(), value, (at.line, at.col), &origin)
            {
                continue;
            }
            // A profile's colour, carried as written, is the default's.
            let profile = profiled
                .iter()
                .find(|p| synthetic.is_none() && p.name == name);
            let mut slices: Vec<String> = t.values.iter().map(|v| v.css()).collect();
            // Every tuple the default stands in, which no `when` replaced.
            if let Some(p) = profile {
                let stand_in = slices[0].clone();
                for s in slices.iter_mut().filter(|s| **s == stand_in) {
                    *s = p.value.clone();
                }
            }
            let value = slices[0].clone();
            if slices.iter().all(|s| *s == value) {
                slices.clear();
            }
            // A role crosses to the hosts once, for the surfaces they draw
            // (LLP 1095 D4's reach): it has one value, whatever the modes.
            if role.is_some() && !slices.is_empty() {
                self.push(LowerError {
                    id: "tamber-role-axis",
                    message: format!("`{name}` overrides a role and varies by an app axis: a role has one value, which the hosts' own surfaces read; vary a token the rows name instead (LLP 1121 D5)"),
                    span: self.token_span(&origin, at.line, at.col),
                });
                continue;
            }
            entries.push(ThemeEntry {
                name,
                role: role.map(|(_, row)| row),
                value,
                slices,
            });
        }
        self.theme = entries;
        // The app's axes, which a pin and `setMode` name (D5).
        for axis in &folding.design.axes {
            let values: Vec<&str> = axis.values.iter().map(String::as_str).collect();
            self.b.axis(&axis.name.value, &values);
        }
        AXES.with(|axes| {
            *axes.borrow_mut() = folding
                .design
                .axes
                .iter()
                .map(|a| (a.name.value.clone(), a.values.clone()))
                .collect()
        });
        // D7's retention: what the view may name, every role override, and
        // in an app that can name a token outside its views (generated
        // bindings, D12) every token, or those its `exports` block lists.
        let exported = |e: &ThemeEntry| {
            folding.design.exports.as_ref().is_none_or(|exports| {
                exports
                    .iter()
                    .any(|x| tamber::exports_names(&x.value, &e.name, e.role.as_deref()))
            })
        };
        for e in &self.theme {
            if e.role.is_some() || said.computed || said.names(&e.name) || bindable && exported(e) {
                let slices: Vec<&str> = e.slices.iter().map(String::as_str).collect();
                self.b
                    .theme_token(&e.name, e.role.as_deref().unwrap_or(""), &e.value, &slices);
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
        let design = inline_design(theme);
        let spans = theme.entries.iter().map(|e| e.span).collect();
        Some((design, Origin::Inline(spans)))
    }

    /// Takes the profile colours (LLP 1100 D3) out of the crate's fold: an
    /// entry that is one, and an entry naming such an entry whole, are
    /// declared to it as `transparent` (so their names are still checked)
    /// and carried as written. Another use of one's name is refused, as the
    /// crate cannot fold it.
    fn profiles(&mut self, design: &mut tamber::Design, origin: &Origin) -> Vec<Profiled> {
        let same = same_name;
        let mut out: Vec<Profiled> = Vec::new();
        loop {
            // A profile's colour is the default app tuple's, which is all the
            // plan carries until it carries the axes (stage 2, commit 3).
            let found = design.tokens.iter().position(|t| {
                let text = t.value.value.trim();
                !out.iter().any(|p| same(&p.name, &t.name.value))
                    && (text.to_ascii_lowercase().starts_with("color(--") && parses(text)
                        || t.whens.is_empty() && out.iter().any(|p| same(&p.name, text)))
            });
            let Some(i) = found else { break };
            // A `when` that holds at the default modes would be the default's.
            let axes = &design.axes;
            let defaults = |w: &tamber::When| {
                w.conds.iter().all(|(a, v)| {
                    axes.iter()
                        .any(|x| x.name.value == *a && x.values.first() == Some(v))
                })
            };
            if let Some(w) = design.tokens[i].whens.iter().find(|w| defaults(w)) {
                let span = self.token_span(origin, w.line, w.col);
                self.push(LowerError {
                    id: "lower-theme-value",
                    message: format!("`{}`: this `when` holds at the default modes, where the token is a profile's colour (`color(--…)`): the plan carries the default modes' value, which a profile's colour must be", design.tokens[i].name.value),
                    span,
                });
            }
            let t = &mut design.tokens[i];
            let text = t.value.value.trim().to_string();
            let value = match out.iter().find(|p| same(&p.name, &text)) {
                Some(p) => p.value.clone(),
                None => text,
            };
            out.push(Profiled {
                name: t.name.value.clone(),
                value,
            });
            t.value.value = "transparent".into();
        }
        for t in &design.tokens {
            // A profile token's default is taken; its `when` values are read.
            let profile = out.iter().any(|p| p.name == t.name.value);
            let values = std::iter::once(&t.value)
                .filter(|_| !profile)
                .chain(t.whens.iter().map(|w| &w.value));
            let names = |v: &tamber::At<String>| {
                scan::words(&v.value).any(|w| out.iter().any(|p| same(&p.name, w)))
            };
            if values.into_iter().any(names) {
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
        value: &tamber::Folded,
        (line, col): (u32, u32),
        origin: &Origin,
    ) -> bool {
        let span = self.token_span(origin, line, col);
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

thread_local! {
    /// The app's axes while its styles are lowered, each with its values:
    /// what a `-exact-mode` pin may name.
    static AXES: std::cell::RefCell<Vec<(String, Vec<String>)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The synthetic tokens the last design lowered on this thread could
    /// have (D3), each its expression, before those that do not fold are
    /// dropped: what the design server re-folds (LLP 1121 stage 4).
    static CANDIDATES: std::cell::RefCell<Vec<(String, bool)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The synthetic tokens the last build on this thread could have, folding
/// or not, each its expression and whether a colour row writes it (then
/// the build refuses it unfolded, where another is dropped), taken: a build
/// that stops before its design leaves none, never an earlier build's. A
/// value edit that changes which fold changes the plan's table, so it is a
/// rebuild.
pub fn synthetic_candidates() -> Vec<(String, bool)> {
    CANDIDATES.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

/// Bind `file`'s design's axes for the pins a pass checks before lowering
/// (the lint): none for an inline `theme` or no design.
pub(crate) fn bind_axes(file: &File) {
    let axes = file
        .design
        .as_ref()
        .and_then(|d| tamber::parse(&d.text).ok())
        .map(|d| {
            d.axes
                .into_iter()
                .map(|a| (a.name.value, a.values))
                .collect()
        })
        .unwrap_or_default();
    AXES.with(|a| *a.borrow_mut() = axes);
}

/// `-exact-mode`'s pins (LLP 1121 D5): `axis=value`,
/// space- or comma-separated, each axis once, every one the design's. A computed pin
/// is the runner's to read; one it cannot is unset, as an invalid row is.
pub(crate) fn mode_pins(a: &Attr) -> Result<(), LowerError> {
    let mut literals = Vec::new();
    scan::literal_leaves(&a.value, &mut literals);
    // An empty pin declares nothing: the tuple around (`StyleValue::text_unsets`).
    for (text, span) in literals.into_iter().filter(|(t, _)| !t.trim().is_empty()) {
        AXES.with(|axes| check_pins(&text, &axes.borrow()))
            .map_err(|message| LowerError {
                id: "tamber-mode",
                message: format!("`-exact-mode=\"{text}\"`: {message}"),
                span,
            })?;
    }
    Ok(())
}

/// `setMode(axis, value)`'s literal arguments name an axis of the design
/// and one of its values (LLP 1121 D5): an axis literal by itself, a value
/// with its axis; a computed one is the runner's.
pub(crate) fn set_mode_args(args: &[Expr], span: Span) -> Result<(), LowerError> {
    let [axis, value] = args else {
        return Ok(());
    };
    let Expr::Str(axis, _) = axis else {
        return Ok(());
    };
    let value = match value {
        Expr::Str(v, _) => Some(v.as_str()),
        _ => None,
    };
    let refused = AXES.with(|axes| {
        let axes = axes.borrow();
        let Some((_, values)) = axes.iter().find(|(name, _)| name == axis) else {
            let known: Vec<&str> = axes.iter().map(|(n, _)| n.as_str()).collect();
            return Some(match known.is_empty() {
                true => format!("this app's design declares no axes, so `{axis}` is none"),
                false => format!(
                    "`{axis}` is no axis of this app's design ({})",
                    known.join(", ")
                ),
            });
        };
        value
            .filter(|v| !values.iter().any(|x| x == v))
            .map(|v| format!("`{v}` is no value of `{axis}` ({})", values.join(" | ")))
    });
    match refused {
        Some(message) => Err(LowerError {
            id: "tamber-mode",
            message: format!("`setMode`: {message}"),
            span,
        }),
        None => Ok(()),
    }
}

/// Each pin of `text` in `axes`; the reason one is not.
pub(crate) fn check_pins(text: &str, axes: &[(String, Vec<String>)]) -> Result<(), String> {
    let mut seen = Vec::new();
    for pin in text.split([' ', ',']).filter(|p| !p.is_empty()) {
        // `=` as everywhere a mode is written (`when`, `--mode`, `prefer mode`).
        if let Some((axis, value)) = pin.split_once(':') {
            return Err(format!(
                "`{pin}` is written `{axis}={value}`: a mode is `axis=value`"
            ));
        }
        let Some((axis, value)) = pin.split_once('=') else {
            return Err(format!("`{pin}` is no pin: write `axis=value`"));
        };
        let Some((_, values)) = axes.iter().find(|(name, _)| name == axis) else {
            let known: Vec<&str> = axes.iter().map(|(n, _)| n.as_str()).collect();
            return Err(match known.is_empty() {
                true => format!("this app's design declares no axes, so `{axis}` is none"),
                false => format!(
                    "`{axis}` is no axis of this app's design ({})",
                    known.join(", ")
                ),
            });
        };
        if !values.iter().any(|v| v == value) {
            return Err(format!(
                "`{value}` is no value of `{axis}` ({})",
                values.join(" | ")
            ));
        }
        if seen.contains(&axis) {
            return Err(format!("`{axis}` is pinned twice"));
        }
        seen.push(axis);
    }
    if seen.is_empty() {
        return Err("pins no axis".into());
    }
    Ok(())
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

/// The inline `theme` as the crate reads it: one token an entry, a bare
/// role name (LLP 1095 D4's `"secondary-grouped-background"`) in the crate's
/// spelling unless an entry is called that.
fn inline_design(theme: &contract_syntax::ThemeDecl) -> tamber::Design {
    let names: Vec<&str> = theme.entries.iter().map(|e| e.name.as_str()).collect();
    let mut tokens = Vec::new();
    for (i, entry) in theme.entries.iter().enumerate() {
        let Expr::Str(text, _) = &entry.value else {
            continue;
        };
        let text = text.trim();
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
            meta: Vec::new(),
        });
    }
    tamber::Design {
        name: "app".into(),
        tokens,
        ..Default::default()
    }
}

/// The app's tokens that are one opaque colour in light and dark in some app
/// tuple and not marked `scheme=fixed` (LLP 1121 D3), each by its name as
/// declared with that colour's text and whether it is one colour in every
/// tuple (only then may it be marked): what `lower-colour-one-scheme` reports
/// at a token's declaration. Folded by the crate, so a `light-dark()` over a
/// one-colour token is two colours and an expression is its result. A
/// profile's colour (`color(--…)`) is the plan's, not judged here.
pub(crate) fn one_scheme_tokens(file: &File) -> Vec<(String, String, bool)> {
    let mut design = match (&file.design, &file.theme) {
        (None, Some(theme)) => inline_design(theme),
        (Some(source), _) => match tamber::parse(&source.text) {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        },
        (None, None) => return Vec::new(),
    };
    for t in &mut design.tokens {
        let profiled = |v: &str| v.to_ascii_lowercase().contains("color(--");
        if profiled(&t.value.value) || t.whens.iter().any(|w| profiled(&w.value.value)) {
            // As the build stands one in (`profiles`): nothing to judge.
            t.value.value = "transparent".into();
            t.whens.clear();
            t.meta.clear();
        }
    }
    let Ok(folding) = tamber::check_design(design, &vocab()) else {
        return Vec::new();
    };
    // A token fixed, or an alias of one (`ink = -exact-label` of a fixed
    // `label`), by every name it answers to.
    let pairs: Vec<_> = folding.tokens.iter().zip(&folding.design.tokens).collect();
    let mut fixed: Vec<String> = Vec::new();
    loop {
        let before = fixed.len();
        for (t, declared) in &pairs {
            let value = declared.value.value.trim();
            // Only a whole alias with no `when` of its own is the mark's.
            let alias = declared.whens.is_empty() && fixed.iter().any(|f| same_name(f, value));
            if (t.fixed || alias) && !fixed.contains(&declared.name.value) {
                fixed.push(declared.name.value.clone());
                fixed.push(t.name.clone());
            }
        }
        if fixed.len() == before {
            break;
        }
    }
    pairs
        .into_iter()
        .filter(|(_, declared)| !fixed.contains(&declared.name.value))
        .filter_map(|(t, declared)| {
            // Opaque only: a translucent one is an overlay, which reads over
            // either scheme's ground, and a transparent one paints nothing.
            let one = t
                .values
                .iter()
                .find(|v| matches!(v, tamber::Folded::One(h) if h.wide().alpha >= 1.0 - 1e-9))?;
            let every = t.values.iter().all(|v| matches!(v, tamber::Folded::One(_)));
            Some((declared.name.value.clone(), one.css(), every))
        })
        .collect()
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

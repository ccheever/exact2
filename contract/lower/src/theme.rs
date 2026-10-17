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
use exact_kernel::style::{roles, theme};
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
    /// Its kind (D3).
    pub kind: tamber::Kind,
}

/// The plan's `theme.kind` for a kind (LLP 1121 D3); `None` for a motion
/// token, which the build writes into its rows and the plan does not keep.
fn kind_code(kind: tamber::Kind) -> Option<u8> {
    Some(match kind {
        tamber::Kind::Color => 0,
        tamber::Kind::Length => 1,
        tamber::Kind::Number => 2,
        tamber::Kind::BoxShadow => 3,
        tamber::Kind::TextShadow => 4,
        tamber::Kind::Type => 5,
        tamber::Kind::Family => 6,
        tamber::Kind::Duration | tamber::Kind::Easing => return None,
    })
}

/// The kernel's kind for a kind, as [`kind_code`].
fn kernel_kind(kind: tamber::Kind) -> Option<exact_kernel::style::theme::Kind> {
    use exact_kernel::style::theme::Kind;
    Some(match kind {
        tamber::Kind::Color => Kind::Color,
        tamber::Kind::Length => Kind::Length,
        tamber::Kind::Number => Kind::Number,
        tamber::Kind::BoxShadow => Kind::BoxShadow,
        tamber::Kind::TextShadow => Kind::TextShadow,
        tamber::Kind::Type => Kind::Type,
        tamber::Kind::Family => Kind::Family,
        tamber::Kind::Duration | tamber::Kind::Easing => return None,
    })
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
                accent: roles::web_accent(r),
            })
            .collect(),
        named_lengths: tamber::Vocab::named_lengths(
            exact_kernel::TEXT_STYLES.iter().map(|s| (s.name, s.alias)),
            exact_kernel::NAMED_LENGTHS.iter().copied(),
        ),
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
        MEASURES.with(|m| m.borrow_mut().clear());
        RETENTION.with(|r| *r.borrow_mut() = None);
        MOTION.with(|m| m.borrow_mut().clear());
        let Some((mut design, origin)) = self.design(file) else {
            return exact_kernel::style::theme::declarations([]);
        };
        let mut said = Said::of(file, root, &design);
        // D7's retention, left for the design server before the design is
        // folded, as it does not depend on the fold.
        let retention = tamber::Retention {
            bindable,
            computed: said.computed,
            computed_measure: said.computed_measure,
            words: std::mem::take(&mut said.words),
        };
        RETENTION.with(|r| *r.borrow_mut() = Some(retention.clone()));
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
                && value.color().is_some_and(|v| {
                    !self.admitted(&name, role.as_ref(), v, (at.line, at.col), &origin)
                })
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
                kind: t.kind,
            });
        }
        self.theme = entries;
        // A motion token is written into its rows here (D3): by name.
        MOTION.with(|m| {
            *m.borrow_mut() = self
                .theme
                .iter()
                .filter(|e| matches!(e.kind, tamber::Kind::Duration | tamber::Kind::Easing))
                .map(|e| (e.name.clone(), e.value.clone()))
                .collect()
        });
        MEASURES.with(|m| {
            *m.borrow_mut() = self
                .theme
                .iter()
                .filter(|e| e.kind != tamber::Kind::Color && kind_code(e.kind).is_some())
                .map(|e| {
                    let mut values = vec![e.value.clone()];
                    for v in &e.slices {
                        if !values.contains(v) {
                            values.push(v.clone());
                        }
                    }
                    (e.name.clone(), e.kind, values)
                })
                .collect()
        });
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
        // bindings, D12) every token, or those its `exports` block lists;
        // and each synthetic token its rows write. The design server's
        // `propose` asks the same `Retention` (LLP 1121 stage 5).
        for e in &self.theme {
            let Some(code) = kind_code(e.kind) else {
                continue;
            };
            let synthetic = said.synthetic.iter().any(|s| s.expr == e.name);
            if synthetic || retention.keeps(&folding.design, &e.name, e.role.as_deref(), e.kind) {
                let slices: Vec<&str> = e.slices.iter().map(String::as_str).collect();
                self.b.theme_token(
                    &e.name,
                    e.role.as_deref().unwrap_or(""),
                    &e.value,
                    code,
                    &slices,
                );
            }
        }
        // A type or family token's family is a stack the plan declares, as
        // a literal `font-family`'s is, and a bound table holds its id
        // (stage 6 C).
        let kernel_values = self.theme_stacks();
        exact_kernel::style::theme::bind(&exact_kernel::style::theme::kinded(
            self.theme
                .iter()
                .zip(&kernel_values)
                .flat_map(|(e, kernel_value)| {
                    let tint = e
                        .role
                        .as_deref()
                        .and_then(roles::role)
                        .is_some_and(roles::tint);
                    let kind = kernel_kind(e.kind);
                    kind.filter(|_| !tint)
                        .map(|kind| {
                            (
                                e.name.as_str(),
                                kernel_value.as_str(),
                                false,
                                kind,
                                !e.slices.is_empty(),
                            )
                        })
                        .into_iter()
                        .chain(e.role.as_deref().filter(|_| !tint).and_then(|r| {
                            Some((r, kernel_value.as_str(), true, kind?, !e.slices.is_empty()))
                        }))
                }),
        ))
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

    pub(crate) fn push(&mut self, e: LowerError) {
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
    /// The design's length and number tokens while its styles are lowered
    /// (LLP 1121 D3): each name, kind and distinct values over the tuples.
    static MEASURES: std::cell::RefCell<Vec<(String, tamber::Kind, Vec<String>)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The retention the last design lowered on this thread kept its
    /// tokens by (D7): what the design server classes an edit with.
    static RETENTION: std::cell::RefCell<Option<tamber::Retention>> = const { std::cell::RefCell::new(None) };
    /// The design's duration and easing tokens while its styles are lowered
    /// (D3): each name and its value, which the build writes in its place.
    static MOTION: std::cell::RefCell<Vec<(String, String)>> = const { std::cell::RefCell::new(Vec::new()) };
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

/// The retention the last build on this thread kept its design's tokens by
/// (D7), taken: none when it stopped before its design.
pub fn retention() -> Option<tamber::Retention> {
    RETENTION.with(|r| r.borrow_mut().take())
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
/// The design's tokens bound for the lint, each of its kind with its first
/// tuple's value (a type or family token's family as stack 0, which no row
/// the lint checks reads), and the kinds the style checks ask (`MEASURES`):
/// a length token stands in a length row as the build reads it. A design
/// that does not fold binds every name as a colour, as before.
pub(crate) fn lint_scope(file: &File) -> exact_kernel::style::theme::ThemeScope {
    let design = match (&file.theme, &file.design) {
        (Some(theme), _) => Some(inline_design(theme)),
        (None, Some(source)) => tamber::parse(&source.text).ok(),
        _ => None,
    };
    let folding = design.and_then(|mut d| {
        stand_in_profiles(&mut d);
        tamber::check_design(d, &vocab()).ok()
    });
    let Some(folding) = folding else {
        let theme = theme_decl(file);
        let entries: Vec<String> = theme
            .iter()
            .flat_map(|t| &t.entries)
            .map(|e| e.name.clone())
            .collect();
        return exact_kernel::style::theme::declarations(
            entries.iter().map(|n| (n.as_str(), "transparent", false)),
        );
    };
    MEASURES.with(|m| {
        *m.borrow_mut() = folding
            .tokens
            .iter()
            .filter(|t| t.kind != tamber::Kind::Color && kind_code(t.kind).is_some())
            .map(|t| {
                let mut values: Vec<String> = Vec::new();
                for v in t.values.iter().map(|v| v.css()) {
                    if !values.contains(&v) {
                        values.push(v);
                    }
                }
                (t.name.clone(), t.kind, values)
            })
            .collect()
    });
    let entries: Vec<(String, String, exact_kernel::style::theme::Kind, bool)> = folding
        .tokens
        .iter()
        .filter_map(|t| {
            let kind = kernel_kind(t.kind)?;
            let first = t.values.first().map(|v| v.css()).unwrap_or_default();
            let value = match kind {
                exact_kernel::style::theme::Kind::Color => "transparent".to_string(),
                _ => exact_kernel::style::theme::typeface::entry(kind, &first, |_| Some(0))
                    .unwrap_or(first),
            };
            let varies = t.values.windows(2).any(|w| w[0] != w[1]);
            Some((t.name.clone(), value, kind, varies))
        })
        .collect();
    exact_kernel::style::theme::bind(&exact_kernel::style::theme::kinded(
        entries
            .iter()
            .map(|(n, v, k, va)| (n.as_str(), v.as_str(), false, *k, *va)),
    ))
}

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

/// Stand `transparent` in for each token whose value is a profile's colour
/// (`color(--…)`), which the crate does not fold alone: what a caller that
/// folds the design outside the build reads.
pub(crate) fn stand_in_profiles(design: &mut tamber::Design) {
    for t in &mut design.tokens {
        let profiled = |v: &str| v.to_ascii_lowercase().contains("color(--");
        if profiled(&t.value.value) || t.whens.iter().any(|w| profiled(&w.value.value)) {
            t.value.value = "transparent".into();
            t.whens.clear();
            t.meta.clear();
        }
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
    // As the build stands one in (`profiles`): nothing to judge.
    stand_in_profiles(&mut design);
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
            let one = t.values.iter().find(|v| {
                matches!(v, tamber::Value::Color(tamber::Folded::One(h)) if h.wide().alpha >= 1.0 - 1e-9)
            })?;
            let every = t
                .values
                .iter()
                .all(|v| matches!(v, tamber::Value::Color(tamber::Folded::One(_))));
            Some((declared.name.value.clone(), one.css(), every))
        })
        .collect()
}

/// The design's length or number token `text` names, while its styles are
/// lowered: its kind and its distinct values over the app's tuples.
pub(crate) fn measure_values(text: &str) -> Option<(tamber::Kind, Vec<String>)> {
    MEASURES.with(|m| {
        m.borrow()
            .iter()
            .find(|(name, ..)| name == text.trim())
            .map(|(_, kind, values)| (*kind, values.clone()))
    })
}

/// The design's length or number token `text` names, while its styles are
/// lowered: its kind.
pub(crate) fn measure_named(text: &str) -> Option<tamber::Kind> {
    measure_values(text).map(|(kind, _)| kind)
}

/// Whether the design has a number token (a computed string may name one
/// in a number row, LLP 1121 D7).
pub(crate) fn number_tokens() -> bool {
    MEASURES.with(|m| {
        m.borrow()
            .iter()
            .any(|(_, kind, _)| *kind == tamber::Kind::Number)
    })
}

/// The design's motion tokens while its styles are lowered, by name and
/// value ([`crate::motion`] writes them in).
pub(crate) fn motion_tokens() -> Vec<(String, String)> {
    MOTION.with(|m| m.borrow().clone())
}

/// What a value may be, through `?:`, `match` and `let` (a `let`'s name
/// read where its body names it): its leaves.
pub(crate) fn leaves<'e>(
    v: &'e Expr,
    bound: &mut Vec<(&'e str, &'e Expr)>,
    out: &mut Vec<&'e Expr>,
) {
    match v {
        Expr::Ternary(_, yes, no, _) => {
            leaves(yes, bound, out);
            leaves(no, bound, out);
        }
        Expr::Match { some, none, .. } => {
            leaves(some, bound, out);
            leaves(none, bound, out);
        }
        Expr::Let {
            name, value, body, ..
        } => {
            bound.push((name, value));
            leaves(body, bound, out);
            bound.pop();
        }
        Expr::Ident(name, _) => match bound.iter().rposition(|(n, _)| n == name) {
            // Its value, in the scope it was bound in.
            Some(i) => {
                let mut outer = bound[..i].to_vec();
                leaves(bound[i].1, &mut outer, out);
            }
            None => out.push(v),
        },
        other => out.push(other),
    }
}

/// Whether every string a value may be, through `?:`, `match` and `let`,
/// names a length or number token, and one does (LLP 1121 D3).
pub(crate) fn names_measures(v: &Expr) -> bool {
    let mut out = Vec::new();
    leaves(v, &mut Vec::new(), &mut out);
    let named = |e: &&Expr| matches!(e, Expr::Str(text, _) if measure_named(text).is_some());
    out.iter().any(named)
        && out
            .iter()
            .all(|e| named(e) || matches!(e, Expr::Number(..)))
}

/// `tamber-kind` and `tamber-inherited-axis` (LLP 1121 D3, D7) for a row's
/// literals that name a length or number token: a number only in the rows
/// D3 lists; a length in no colour row and no row that takes only a
/// number; neither, when it varies by an app axis, in an inherited row,
/// since only colours re-select at a mode boundary. The kernel then reads
/// the token's value as the row's (a text style only as a font size, …).
pub(crate) fn measure_row(
    a: &Attr,
    rows: &[exact_kernel::StyleId],
    string: bool,
    check: &dyn Fn(&Attr) -> Result<(), LowerError>,
) -> Result<(), LowerError> {
    let mut all = Vec::new();
    leaves(&a.value, &mut Vec::new(), &mut all);
    // A computed string in a number row may name any number token (values.rs
    // admits one): each must suit the row in every app mode, as a literal
    // naming it must (a press scale positive). A length row's computed text
    // is the kernel's to read at run time, as any computed string is.
    let computed = string
        && all
            .iter()
            .any(|e| !matches!(e, Expr::Str(..) | Expr::Number(..)));
    if computed && rows.iter().all(|r| theme::admits(theme::Kind::Number, *r)) {
        let numbers: Vec<(String, Vec<String>)> = MEASURES.with(|m| {
            m.borrow()
                .iter()
                .filter(|(_, kind, _)| *kind == tamber::Kind::Number)
                .map(|(name, _, values)| (name.clone(), values.clone()))
                .collect()
        });
        for (name, values) in numbers {
            for value in values {
                let Ok(n) = exact_num::parse_f64(&value) else {
                    continue;
                };
                let alt = Attr {
                    value: Expr::Number(n, a.value.span()),
                    ..a.clone()
                };
                if let Err(e) = check(&alt) {
                    let why = e
                        .message
                        .rsplit_once(": ")
                        .map_or(e.message.as_str(), |(_, why)| why);
                    return Err(LowerError {
                        id: "tamber-kind",
                        message: format!("`{}`'s computed value may name `{name}`, which is `{value}` in some app mode, which `{}` does not take: {why}", a.name, a.name),
                        span: a.value.span(),
                    });
                }
            }
        }
    }
    let found = all.into_iter().filter_map(|e| match e {
        Expr::Str(text, span) => Some((text.as_str(), *span)),
        _ => None,
    });
    for (text, span) in found {
        // A motion token is a component of a transition or animation only,
        // written in there before this check (D3).
        if MOTION.with(|m| m.borrow().iter().any(|(name, _)| name == text.trim())) {
            return Err(LowerError {
                id: "tamber-kind",
                message: format!("`{}=\"{}\"`: `{}` is a motion token, a component of a `transition` or `animation` value (`transition=\"opacity {} ease\"`)", a.name, text.trim(), text.trim(), text.trim()),
                span,
            });
        }
        // A type token is `font`'s, a family `font-family`'s (stage 6 C),
        // each lowered there before this check.
        if let Some((kind @ (tamber::Kind::Type | tamber::Kind::Family), _)) = measure_values(text)
        {
            let row = if kind == tamber::Kind::Type {
                "font"
            } else {
                "font-family"
            };
            return Err(LowerError {
                id: "tamber-kind",
                message: format!(
                    "`{}=\"{}\"`: `{}` is a {} token, which `{row}` names (`{row}=\"{}\"`)",
                    a.name,
                    text.trim(),
                    text.trim(),
                    kind.name(),
                    text.trim()
                ),
                span,
            });
        }
        let Some((kind, values)) = measure_values(text) else {
            continue;
        };
        let text = text.trim();
        let refuse = |id: &'static str, message: String| Err(LowerError { id, message, span });
        let kind_name = kind.name();
        let Some(row) = rows
            .iter()
            .find(|r| kernel_kind(kind).is_none_or(|k| !theme::admits(k, **r)))
        else {
            // Each tuple's value checked as the row checks its literal (the
            // kernel's codec, and what the compiler adds: a text style only a
            // font size, a press scale positive), not the default's alone.
            for value in &values {
                let literal = match kind {
                    tamber::Kind::Number => exact_num::parse_f64(value)
                        .map(|n| Expr::Number(n, span))
                        .unwrap_or_else(|_| Expr::Str(value.clone(), span)),
                    _ => Expr::Str(value.clone(), span),
                };
                let alt = Attr {
                    value: literal,
                    ..a.clone()
                };
                if let Err(e) = check(&alt) {
                    let why = e
                        .message
                        .rsplit_once(": ")
                        .map_or(e.message.as_str(), |(_, why)| why);
                    return refuse("tamber-kind", format!("`{}=\"{text}\"`: `{text}` is the {kind_name} `{value}` in some app mode, which `{}` does not take: {why}", a.name, a.name));
                }
            }
            if values.len() > 1 && rows.iter().any(|r| r.inherited()) {
                return refuse("tamber-inherited-axis", format!("`{}=\"{text}\"`: `{text}` varies by an app axis, and `{}` is inherited: only a colour is chosen again at a mode boundary, so an inherited row names a token with one value (LLP 1121 D7)", a.name, a.name));
            }
            continue;
        };
        let takes = if theme::admits(theme::Kind::BoxShadow, *row) {
            "a box shadow"
        } else if theme::admits(theme::Kind::TextShadow, *row) {
            "a text shadow"
        } else if theme::admits(theme::Kind::Color, *row) {
            "a colour"
        } else if theme::admits(theme::Kind::Number, *row)
            && !theme::admits(theme::Kind::Length, *row)
        {
            "a number"
        } else {
            "a length"
        };
        let hint = match kind {
            tamber::Kind::Number => ", and only `opacity`, `line-height`, `flex-grow`, `font-weight` and `-exact-press-scale` take one",
            _ => "",
        };
        return refuse(
            "tamber-kind",
            format!(
                "`{}=\"{text}\"`: `{text}` is a {kind_name} token, and `{}` takes {takes}{hint}",
                a.name, a.name
            ),
        );
    }
    Ok(())
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
    // A length token whose form the row does not take (a text style is a
    // font size only, a hairline no font's): the kernel's reason.
    if let Some(kind) = measure_named(text) {
        let value = exact_kernel::style::theme::declared(text).unwrap_or_default();
        let why = e
            .message
            .rsplit_once(": ")
            .map_or(e.message.as_str(), |(_, why)| why);
        return LowerError {
            id: "tamber-kind",
            message: format!(
                "`{}=\"{text}\"`: `{text}` is the {} `{value}`, which `{}` does not take: {why}",
                a.name,
                kind.name(),
                a.name
            ),
            span,
        };
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
            format!("`{}=\"{text}\"`: `{text}` is a colour token, and `{}` takes no colour", a.name, a.name),
        ),
        _ => return e,
    };
    LowerError { id, message, span }
}

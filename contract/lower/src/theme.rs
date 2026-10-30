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

mod bind;
pub(crate) mod packages;
use bind::Lowering;
pub(crate) use bind::{bind, fold, profiles, Refusals, Sink};
pub(crate) mod retained;
pub mod rows;
pub(crate) mod scan;
mod scheme;
pub(crate) use scheme::one_scheme_tokens;

use crate::{LowerError, Lowerer};
use contract_syntax::{Attr, Component, Expr, File, Span};
use exact_kernel::style::{roles, theme};
use exact_kernel::{Color, ColorValue};
use exact_tamber as tamber;
use scan::Said;
pub(crate) use scan::{literal_leaves, words};

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
pub(crate) fn role_named(name: &str) -> Option<(u8, String)> {
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
            exact_kernel::NAMED_LENGTHS
                .iter()
                .copied()
                .chain(exact_kernel::LENGTH_RAMPS.iter().map(|(n, _)| *n)),
        ),
        bases: exact_kernel::style::scale::names()
            .map(String::from)
            .collect(),
        sizes: tamber::Vocab::named_lengths(
            exact_kernel::TEXT_STYLES.iter().map(|s| (s.name, s.alias)),
            exact_kernel::NAMED_LENGTHS
                .iter()
                .copied()
                .chain(exact_kernel::LENGTH_RAMPS.iter().map(|(n, _)| *n)),
        )
        .into_iter()
        .filter_map(|name| {
            use exact_kernel::style::relative::{parse, MEDIUM};
            let (unit, n) = parse(&name)?;
            let px = f64::from(unit.basis(MEDIUM, MEDIUM, 1.0) * n);
            Some((name, exact_num::Shortest(px).to_string()))
        })
        .chain(crate::scale::sizes())
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
pub(crate) enum Origin {
    /// The design file: its source id.
    File(u32),
    /// An inline `theme`: each entry's span.
    Inline(Vec<Span>),
}

/// A synthetic token's line in the design the crate checks: past any file's.
const SYNTHETIC: u32 = 1 << 30;

/// A `color(--profile …)` entry (LLP 1100 D3), the plan's to resolve: kept
/// out of the crate's fold, as are the entries naming it whole.
pub(crate) struct Profiled {
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
        asset_root: Option<&std::path::Path>,
    ) -> exact_kernel::style::theme::ThemeScope {
        crate::var_tokens::note(file);
        crate::icons::clear(asset_root);
        ROLES.with(|r| r.borrow_mut().clear());
        crate::control_roles::note_accent(None);
        AXES.with(|axes| axes.borrow_mut().clear());
        CANDIDATES.with(|c| c.borrow_mut().clear());
        rows::note(Vec::new());
        MEASURES.with(|m| m.borrow_mut().clear());
        crate::scale::declare(Vec::new());
        RETENTION.with(|r| *r.borrow_mut() = None);
        MOTION.with(|m| m.borrow_mut().clear());
        // The components' params, tokens of the design (D15): an app with
        // params and no design has one of them alone.
        let params = crate::params::tokens(file);
        PARAMS.with(|p| *p.borrow_mut() = params.clone());
        SHOWN.with(|p| *p.borrow_mut() = params.clone());
        let found = self.design(file).or_else(|| {
            (!params.is_empty()).then(|| (tamber::Design::default(), Origin::Inline(Vec::new())))
        });
        let Some((mut design, origin)) = found else {
            return exact_kernel::style::theme::declarations([]);
        };
        // The axes it declares, before its fold: a refused token leaves them
        // for `setMode` and the pins, so the build says the design's refusal
        // and not that the design declares no axes.
        AXES.with(|axes| {
            *axes.borrow_mut() = design
                .axes
                .iter()
                .map(|a| (a.name.value.clone(), a.values.clone()))
                .collect()
        });
        for t in design.tokens.iter().chain(&design.icons) {
            if t.name.value.starts_with(crate::params::PREFIX) {
                self.push(LowerError {
                    id: "tamber-param-name",
                    message: format!("`{}`: a token named `param.…` is a component's param, which its component's `params` section declares (D15); name this token otherwise", t.name.value),
                    span: self.token_span(&origin, t.name.line, t.name.col),
                });
            }
        }
        // A param passed through a prop to a place no row reads (D15).
        for e in crate::params::reached(root, &params) {
            self.push(e);
        }
        // Each param as declared, then those of a kind, into the design.
        let (refused, unkinded) = crate::params::declared(&params);
        // (Whether each is read is the driver's, before a motion param is
        // written into its rows: `params::unused`.)
        for e in refused {
            self.push(e);
        }
        let kinded: Vec<crate::params::ParamToken> = params
            .iter()
            .enumerate()
            .filter(|(i, _)| !unkinded.contains(i))
            .map(|(_, p)| p.clone())
            .collect();
        PARAMS.with(|p| *p.borrow_mut() = kinded.clone());
        // The app's values of its params, the design's `params` block (D15,
        // stage 7 B): `add_to` lays each in place of its param's default and
        // `when` lines. One naming no param is a warning (`params::unknown`):
        // a moved or renamed declaring file leaves it, and it no longer
        // applies. An ambiguous short name, and a second line for one param,
        // are refused.
        let identities = crate::params::identities(file);
        let mut overridden: Vec<(String, Span)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let resolver = crate::params::Resolver::new(&kinded, &identities);
        for entry in &design.params {
            if let Ok(p) = resolver.resolve(&entry.name.value) {
                if seen.insert(p.token.clone()) {
                    let span = self.token_span(&origin, entry.name.line, entry.name.col);
                    overridden.push((p.token.clone(), span));
                }
            }
        }
        // A library's names are its API (LLP 1139 D8), then the entries' and
        // the written ranges', in that order.
        let (d, k, i) = (&design, &kinded, &identities);
        let mut lines = crate::anatomy::param_lines(d, k, i, file);
        lines.extend(crate::params::entry_errors(d, k, i));
        lines.extend(crate::params::written_ranges(d, k, i, &vocab()));
        for (line, col, e) in lines {
            let span = self.token_span(&origin, line, col);
            self.push(LowerError { span, ..e });
        }
        crate::params::add_to(&mut design, &kinded, &identities);
        let scope = self.types.component_scope(root, &self.types.components[0]);
        let typing = rows::Typing::new(scope.clone(), self.types.shapes.clone());
        let declared = std::sync::Arc::new(rows::Declared::new(
            self.declared_fonts.clone(),
            self.font_files.clone(),
        ));
        let mut said = Said::of(file, root, &design, &declared, &typing);
        // The rows a design server's write check reads again (`rows`).
        rows::note(std::mem::take(&mut said.rows));
        // D7's retention, left for the design server before the design is
        // folded, as it does not depend on the fold.
        let mut words = std::mem::take(&mut said.words);
        // A component that isn't Contract reads its params outside the views:
        // its declaration keeps each (D15), with every token its default
        // reads, through any chain.
        // A package design's components too (stage 12 B1).
        let designed = file
            .design
            .iter()
            .chain(&file.package_designs)
            .flat_map(|d| crate::params::design_tokens(d).0);
        let mut kept: Vec<String> = designed.map(|p| p.token).collect();
        let mut roots = std::collections::BTreeSet::new();
        // A role's spelling in any case, as the fold reads one: the tokens
        // and icons by their names lowercased, once (linear: the checker
        // bound's sweep).
        let mut by_lower: std::collections::HashMap<String, Vec<&tamber::Token>> =
            std::collections::HashMap::new();
        for t in design.tokens.iter().chain(&design.icons) {
            by_lower
                .entry(t.name.value.to_ascii_lowercase())
                .or_default()
                .push(t);
        }
        // Each spelling kept; each lowercased name's tokens read once, however
        // many spellings reach it (round 1 of the checker sweep).
        let mut expanded = std::collections::HashSet::new();
        while let Some(name) = kept.pop() {
            if !roots.insert(name.clone()) {
                continue;
            }
            words.insert(name.clone());
            let lower = name.to_ascii_lowercase();
            if !expanded.insert(lower.clone()) {
                continue;
            }
            for t in by_lower.get(&lower).into_iter().flatten() {
                let read =
                    std::iter::once(&t.value.value).chain(t.whens.iter().map(|w| &w.value.value));
                kept.extend(read.flat_map(|v| scan::words(v)).map(str::to_string));
            }
        }
        let retention = tamber::Retention {
            bindable,
            computed: said.computed,
            computed_measure: said.computed_measure,
            words,
            folded: said.folded.clone(),
        };
        RETENTION.with(|r| *r.borrow_mut() = Some(retention.clone()));
        CANDIDATES.with(|c| {
            *c.borrow_mut() = said
                .synthetic
                .iter()
                .map(|s| (s.expr.clone(), s.strict))
                .collect();
        });
        if retained::stops_at_retention() {
            return exact_kernel::style::theme::declarations([]);
        }
        let profiled = profiles(self, &mut design, &origin);
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
        let mut refused: Vec<(tamber::Diagnostic, Vec<scan::Synthetic>)> = Vec::new();
        let folding = fold(&design, &mut said.synthetic, &mut |d, synthetic| {
            refused.push((d.clone(), synthetic.to_vec()))
        });
        for (d, synthetic) in &refused {
            self.design_refusal(d, &origin, synthetic);
        }
        let Some(folding) = folding else {
            // Its icons' uses say no more than the design's refusal.
            crate::icons::mark_refused(design.icons.iter().map(|t| t.name.value.clone()));
            return exact_kernel::style::theme::declarations([]);
        };
        if self.package_references(&folding, file) {
            return exact_kernel::style::theme::declarations([]);
        }
        crate::control_roles::note_accent(tamber::white_over(&folding.tokens, "AccentColor"));
        for e in crate::params::folded(&kinded, &folding, &overridden, None) {
            self.push(e);
        }
        // @ref LLP 1121 D10 — the icons' SVGs, and a source per shipped target.
        let exports = &folding.design.exports;
        // An icon a component that isn't Contract reads is kept as its
        // colours are (D15).
        let exported = |name: &str| {
            roots.contains(name)
                || bindable
                    && exports
                        .as_ref()
                        .is_none_or(|e| e.iter().any(|x| x.value == name))
        };
        // Each SVG under the assets of the root whose file writes it (stage 13).
        let home = |icon: &tamber::Icon| {
            crate::icons::Home::of(icon, asset_root, &file.package_designs, &kinded)
        };
        let refused = crate::icons::declare(
            &folding.icons,
            &home,
            &|line, col| self.token_span(&origin, line, col),
            &exported,
        );
        self.errors.extend(refused);
        let synthetic: Vec<String> = said.synthetic.iter().map(|x| x.expr.clone()).collect();
        let mut sink = Lowering {
            l: self,
            synthetic: &said.synthetic,
            retention: &retention,
        };
        let (entries, scope) = bind(&mut sink, &folding, &synthetic, &profiled, &origin);
        self.theme = entries;
        scope
    }

    /// The app's own design: `app.tamber` parsed, or the inline `theme`
    /// (its packages' join it in `packages`).
    fn app_design(&mut self, file: &File) -> Option<(tamber::Design, Origin)> {
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
                        self.design_refusal(&e, &origin, &[]);
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

    /// Where a design's token line is: its entry's span, or its place in
    /// the design file.
    fn token_span(&self, origin: &Origin, line: u32, col: u32) -> Span {
        // A package's token: its line in its package's design (D11).
        if let Some(span) = packages::span(line, col) {
            return span;
        }
        // A param's token: its default, where its component declares it.
        if (crate::params::LINE..SYNTHETIC).contains(&line) {
            let at = (line - crate::params::LINE) as usize;
            if let Some(span) = PARAMS.with(|p| p.borrow().get(at).map(|p| p.param.default_span)) {
                return span;
            }
        }
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
    fn design_refusal(
        &mut self,
        d: &tamber::Diagnostic,
        origin: &Origin,
        synthetic: &[scan::Synthetic],
    ) {
        let param = (crate::params::LINE..SYNTHETIC).contains(&d.line).then(|| {
            PARAMS.with(|p| {
                p.borrow()
                    .get((d.line - crate::params::LINE) as usize)
                    .cloned()
            })
        });
        let (span, message) = if let Some(p) = param.flatten() {
            (
                p.param.default_span,
                format!("param `{}`: {}", p.full, d.message),
            )
        } else if d.line >= SYNTHETIC {
            let s = &synthetic[(d.line - SYNTHETIC) as usize];
            (s.span, format!("`{}`: {}", s.expr, d.message))
        } else {
            (
                self.token_span(origin, d.line, d.col),
                packages::unband(&d.message),
            )
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
    /// The roles the last design lowered on this thread overrides, each
    /// by its schema name (`AccentColor`, `placeholder`) and its token's
    /// span: what `tamber-unreachable` judges (LLP 1121 D9).
    static ROLES: std::cell::RefCell<Vec<(String, Span)>> = const { std::cell::RefCell::new(Vec::new()) };
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
    /// The app's components' params as tokens while its styles are lowered
    /// (D15): what a refusal names by full name, and what checks them.
    static PARAMS: std::cell::RefCell<Vec<crate::params::ParamToken>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Every param, a bad kind's too: what a refusal names by full name.
    static SHOWN: std::cell::RefCell<Vec<crate::params::ParamToken>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Forget the last design's roles.
pub(crate) fn forget_roles() {
    ROLES.with(|r| r.borrow_mut().clear());
}

/// The roles the last design overrides, each with its token's span.
pub(crate) fn overridden_roles() -> Vec<(String, Span)> {
    ROLES.with(|r| r.borrow().clone())
}

/// `errors` with each param's token name written as its full name (D15):
/// an author never reads the hash.
pub(crate) fn shown_errors(errors: Vec<LowerError>) -> Vec<LowerError> {
    SHOWN.with(|p| {
        let params = p.borrow();
        if params.is_empty() {
            return errors;
        }
        errors
            .into_iter()
            .map(|mut e| {
                e.message = crate::params::shown(&e.message, &params);
                e
            })
            .collect()
    })
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
    let mut params = crate::params::tokens(file);
    let unkinded = crate::params::declared(&params).1;
    let mut i = 0;
    params.retain(|_| {
        i += 1;
        !unkinded.contains(&(i - 1))
    });
    let design = match (&file.theme, &file.design) {
        (Some(theme), _) => Some(inline_design(theme)),
        (None, Some(source)) => tamber::parse(&source.text).ok(),
        _ => (!params.is_empty()).then(tamber::Design::default),
    };
    // Its packages' tokens too, which their views name (D11); what the
    // build refuses of them is the build's to say.
    let design = match (design, file.design.is_some() && file.theme.is_none()) {
        (None, true) => None,
        (design, _) => packages::merge(design, &file.package_designs).0,
    };
    let folding = design.and_then(|mut d| {
        crate::params::add_to(&mut d, &params, &crate::params::identities(file));
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
    crate::scale::declare(folding.tokens.iter().map(|t| t.name.clone()).collect());
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
pub(crate) fn inline_design(theme: &contract_syntax::ThemeDecl) -> tamber::Design {
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

/// The design's length or number token `text` names, while its styles are
/// lowered: its kind and its distinct values over the app's tuples.
/// A base name the design leaves unset (LLP 1121 stage 11 B) is the
/// scale's length, its values every column's that draws in px.
/// An expression over base names it leaves unset (stage 12 B2) is each
/// column's px.
pub(crate) fn measure_values(text: &str) -> Option<(tamber::Kind, Vec<String>)> {
    measure_written(text)
        .map(|(kind, values)| (kind, crate::scale::expanded(kind, &values)))
        .or_else(|| crate::scale::values(text).map(|values| (tamber::Kind::Length, values)))
}

/// The design's length or number token `text` names: its kind and its
/// distinct values over the app's tuples, as the plan carries them.
pub(crate) fn measure_written(text: &str) -> Option<(tamber::Kind, Vec<String>)> {
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
                    // An expression over base names: the column it fails in.
                    let place = measure_written(text)
                        .and_then(|(_, written)| crate::scale::place(kind, &written, value))
                        .unwrap_or_else(|| "in some app mode".into());
                    return refuse("tamber-kind", format!("`{}=\"{text}\"`: `{text}` is the {kind_name} `{value}` {place}, which `{}` does not take: {why}", a.name, a.name));
                }
            }
            // By its tuples: a base, or an expression over one, varies by
            // column, never by an app axis.
            if measure_written(text).is_some_and(|(_, written)| written.len() > 1)
                && rows.iter().any(|r| r.inherited())
            {
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
            tamber::Kind::Number => ", and only `opacity`, `line-height`, `flex-grow`, `font-weight`, `-exact-press-scale` and `-exact-press-opacity` take one",
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
    // A word among others: a shadow's, a border's, a gradient's (stage 12 B3).
    if e.id == "lower-attr-value" && !name_like {
        if let Some(e) = scan::literal_word(&a.name, text, rows, span) {
            return e;
        }
    }
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

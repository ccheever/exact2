//! The design server's write check of the rows a write reaches (LLP 1121
//! D3 and D7; QUEUE's "the server's write check doesn't see a Contract
//! row's kind", theme2's 14:19 ruling). The retention build's walk
//! (`scan::Said::of`) keeps each style row whose value is literal
//! ([`kept`]), and the design server keeps that map beside the
//! retention. On a write, [`check`] runs the compiler's own check of a row
//! (`values::check_style_value`) on just the rows that name a token the
//! write touches, through any chain of references, with the design bound
//! as the compiler binds it ([`super::fold`], [`super::bind`]). A row that
//! passes under the design before the write and fails after is refused,
//! in the compiler's words. A row already failing is the file's own, as
//! the build says, so a write that only turns one failure into another is
//! not refused either.
//!
//! A font row (`font`, `font-family`, `font-weight`, `font-style`) is
//! judged with its node's fonts ([`Fonts`]) through lowering's own
//! `type_tokens::tokens_named` and `fonts::font_use` (stage 13 D1).
//!
//! A row whose value is computed (a name, a template with holes, a choice
//! with such an arm) is judged whole, as the build judges it: its literal
//! arms, and, where its type is a string for certain ([`typed`]), every
//! number token a number row's value may name (stage 13 D2).
//!
//! An SVG element's rows are judged as lowering lowers them, renamed and
//! coerced first (`svg::coerce_lengths`, stage 13 D3): a light's `x` is the
//! prop `lightX`, no row; a rect's `x` a row, a unitless one a number.
//!
//! Not checked here, and still caught by the build after a miss: a
//! computed row whose type the root's scope doesn't give (a call, a `for`'s
//! item), whose number tokens are the build's; a row through a `let`; a
//! `font` shorthand naming no type token; a state part's row; and refusals
//! outside a style row (an SVG attribute, a canvas).

use super::{bind, fold, profiles, scan, Origin, Refusals, Sink, ThemeEntry};
use crate::{DeclaredFont, LowerError};
use contract_syntax::{Attr, Expr, Span};
use exact_tamber as tamber;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

thread_local! {
    /// The literal rows the last design lowered on this thread names.
    static ROWS: RefCell<Vec<Row>> = const { RefCell::new(Vec::new()) };
}

/// A literal row, and every place it is written alike (one name, one
/// literal: an expanded app writes most rows many times over).
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The row as first written.
    pub attr: Attr,
    /// Each place it is written.
    pub places: Vec<Place>,
    /// A font row's node's fonts, as lowering judges the row with them.
    pub fonts: Option<Fonts>,
    /// A computed row's type as lowering types it, and the scope and
    /// shapes it is judged in; `None` for a literal row.
    pub computed: Option<(contract_types::Ty, Typing)>,
}

/// The root's scope and the file's shapes, in which lowering judges a
/// computed row of a node no `for` or `match` binds a name around
/// (`values::check_style_value` types its computed arms in them): one for
/// every row of a build.
#[derive(Debug, Clone)]
pub struct Typing(Arc<(contract_types::Scope, contract_types::Shapes)>);

impl Typing {
    pub(crate) fn new(scope: contract_types::Scope, shapes: contract_types::Shapes) -> Typing {
        Typing(Arc::new((scope, shapes)))
    }
}

impl PartialEq for Typing {
    fn eq(&self, other: &Typing) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// What lowering judges a node's font row with: the node's `font-family`
/// and `font-style` (its own or its class's, as `fonts::font_use` reads
/// them), whether it has element children (`type_tokens::tokens_named`
/// refuses a type token varying by an axis there), and the app's declared
/// fonts.
#[derive(Debug, Clone, PartialEq)]
pub struct Fonts {
    attrs: Vec<Attr>,
    children: bool,
    declared: Arc<Declared>,
}

/// The app's declared fonts as a build read them, and each file whose
/// faces' weights it read from the file itself (an `auto` face): its path,
/// length and time, so a check after the file changes leaves font rows to
/// the build (Astra's D1 round 2).
#[derive(Debug, Default, PartialEq)]
pub struct Declared {
    fonts: BTreeMap<String, DeclaredFont>,
    files: Vec<FontFile>,
}

/// A font file a build read: its path as the app names it, the file that
/// path resolved to (a link followed), and that file's length and time.
pub(crate) type FontFile = (
    std::path::PathBuf,
    std::path::PathBuf,
    u64,
    Option<std::time::SystemTime>,
);

impl Declared {
    pub(crate) fn new(fonts: BTreeMap<String, DeclaredFont>, files: Vec<FontFile>) -> Declared {
        Declared { fonts, files }
    }

    /// Whether a file the build read has changed since, or its path now
    /// resolves to another (a link retargeted: Astra's D1 round 3).
    fn stale(&self) -> bool {
        self.files.iter().any(|(path, resolved, len, time)| {
            path.canonicalize().ok().as_ref() != Some(resolved)
                || std::fs::metadata(resolved)
                    .map_or(true, |m| m.len() != *len || m.modified().ok() != *time)
        })
    }
}

impl Fonts {
    /// The fonts of an element whose rows are `attrs` (`file`'s classes
    /// applied as lowering applies them, `crate::class::rows`) and whose
    /// children are `children`; `None` when a class is unknown, which the
    /// build refuses.
    pub(crate) fn of(
        file: &contract_syntax::File,
        attrs: &[Attr],
        children: bool,
        declared: &Arc<Declared>,
    ) -> Option<Fonts> {
        let style = |name: &str| {
            file.styles
                .iter()
                .rfind(|s| s.name == name)
                .map(|s| s.attrs.as_slice())
        };
        let class = crate::class::rows(attrs, &style).ok()?;
        let all = class.map(|(_, rows)| rows).unwrap_or_default();
        let attrs = all
            .iter()
            .chain(attrs)
            .filter(|a| matches!(a.name.as_str(), "font-family" | "font-style"))
            .cloned()
            .collect();
        Some(Fonts {
            attrs,
            children,
            declared: declared.clone(),
        })
    }
}

/// Whether `name` is a row lowering judges with its node's fonts.
pub(crate) fn font_row(name: &str) -> bool {
    matches!(name, "font" | "font-family" | "font-weight" | "font-style")
}

/// Where a literal is written: the authored row it is a literal of (one
/// row may write several, a choice's arms), where a refusal of the row is
/// said (the attribute for a plain literal, the arm's own literal for an
/// arm), and the literal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    /// The authored row's attribute.
    pub row: Span,
    /// Where a refusal at the row is said.
    pub at: Span,
    /// The literal.
    pub literal: Span,
}

impl Place {
    /// A place of one literal written plainly at `span` (a test's).
    pub fn plain(span: Span) -> Place {
        Place {
            row: span,
            at: span,
            literal: span,
        }
    }
}

/// `attrs` as [`Row`]s: each literal a row may write (every arm of a `?:`
/// or `match`, which the build checks each of, whatever the condition), as
/// a row of its own; those alike, with alike fonts, as one.
fn grouped(attrs: Vec<Said>) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    let mut at: std::collections::HashMap<(String, bool, String, usize), usize> =
        Default::default();
    let mut whole: std::collections::HashMap<(String, Span, usize), Vec<usize>> =
        Default::default();
    // Each distinct node's fonts, by what lowering reads of them (an
    // expanded app repeats most).
    let mut seen: std::collections::HashMap<String, usize> = Default::default();
    for Said {
        attr: a,
        fonts,
        computed,
    } in attrs
    {
        let node = match &fonts {
            None => 0,
            Some(f) => {
                let next = seen.len() + 1;
                *seen
                    .entry(format!("{:?} {}", f.attrs, f.children))
                    .or_insert(next)
            }
        };
        // A computed row is judged whole, as the build judges it, and so is
        // a `font` or `font-family` row, whose tokens `tokens_named` judges
        // together (one longhand set, one axis rule): refused where the
        // build's refusal is said. Alike, one value and one type.
        if computed.is_some() || matches!(a.name.as_str(), "font" | "font-family") {
            let place = Place {
                row: a.span,
                at: a.span,
                literal: a.value.span(),
            };
            // Found by where its value is written, then compared whole (an
            // expansion writes one row many times over, alike).
            let k = (a.name.clone(), a.value.span(), node);
            let alike = whole.entry(k).or_default();
            let same = |r: &Row| {
                r.attr.value == a.value
                    && r.computed.as_ref().map(|(t, _)| t) == computed.as_ref().map(|(t, _)| t)
            };
            match alike.iter().copied().find(|&i| same(&out[i])) {
                Some(i) => out[i].places.push(place),
                None => {
                    alike.push(out.len());
                    out.push(Row {
                        attr: a,
                        places: vec![place],
                        fonts,
                        computed,
                    });
                }
            }
            continue;
        }
        // A plain literal is refused at its attribute; an arm of a choice at
        // its own literal, as the build refuses it. Rows alike share a
        // representative only with their own kind of place.
        let plain = matches!(a.value, Expr::Str(..) | Expr::Number(..));
        let mut all = Vec::new();
        written(&a.value, &mut all);
        for value in all.into_iter().cloned() {
            let (key, literal) = match &value {
                Expr::Str(t, s) => (format!("s{t}"), *s),
                Expr::Number(n, s) => (format!("n{n}"), *s),
                _ => continue,
            };
            let place = Place {
                row: a.span,
                at: if plain { a.span } else { literal },
                literal,
            };
            let k = (a.name.clone(), plain, key, node);
            match at.get(&k) {
                Some(&i) => out[i].places.push(place),
                None => {
                    at.insert(k, out.len());
                    out.push(Row {
                        attr: Attr {
                            span: place.at,
                            value,
                            ..a.clone()
                        },
                        places: vec![place],
                        fonts: fonts.clone(),
                        computed: None,
                    });
                }
            }
        }
    }
    out
}

/// Whether the retention walk keeps `a`, an element's style row [`check`]
/// judges as the build does, and how. `Some(false)`: a literal value
/// (strings or numbers, or a `?:` or `match` whose arms all are).
/// `Some(true)`: a computed one (a name, a template with holes, or a choice
/// with such an arm beside literal ones), judged whole where its type and
/// scope are known ([`typed`]): its literal arms, and, in a number row,
/// every number token a string may name, or its string arms where the
/// design has none (stage 13 D2). Never through a `let`, which the build
/// judges through the binding; not a shorthand (the build projects it into
/// longhands first) but `font`, whose type tokens are judged whole
/// ([`font_row`]); not a transition's or an animation's, whose motion
/// tokens the loader writes in before this map is read.
pub(crate) fn kept(a: &Attr) -> Option<bool> {
    let mut leaves = Vec::new();
    match written(&a.value, &mut leaves) {
        true => style_row(a).then_some(false),
        false => (!bound(&a.value) && style_row(a)).then_some(true),
    }
}

/// `v`'s type as lowering's `expr::compile` gives it, for the values it
/// types by their form alone in `scope` (a string, a template, a name, a
/// `?:` of those, a number); `None` for any other (a call, a `match`).
pub(crate) fn typed(v: &Expr, scope: &contract_types::Scope) -> Option<contract_types::Ty> {
    use contract_types::Ref;
    // Every name it reads, a condition's and a hole's too, one `compile`
    // loads (a state, a derive, a resource or a mutation): a root prop or
    // an action as a value is lowering's refusal of its own (Grok's D2
    // round 2).
    let mut loads = true;
    scan::exprs(v, &mut |e| {
        if let Expr::Ident(name, _) = e {
            loads &= matches!(
                scope.lookup(name),
                Some((
                    Ref::Slot(_) | Ref::Derive(_) | Ref::Resource(_) | Ref::Mutation(_),
                    _
                ))
            );
        }
    });
    if !loads {
        return None;
    }
    shape(v, scope)
}

/// [`typed`]'s type of `v` by its form, its names loaded.
fn shape(v: &Expr, scope: &contract_types::Scope) -> Option<contract_types::Ty> {
    use contract_types::Ty;
    match v {
        Expr::Str(..) | Expr::Template(..) => Some(Ty::String),
        Expr::Number(..) => Some(Ty::Number),
        Expr::Ident(name, _) => scope.lookup(name).map(|(_, t)| t.clone()),
        Expr::Ternary(_, yes, no, _) => {
            let (yes, no) = (shape(yes, scope)?, shape(no, scope)?);
            Some(yes.unify(&no).unwrap_or(Ty::Unknown))
        }
        _ => None,
    }
}

/// Whether `v` binds a name with a `let` anywhere a leaf may come from.
fn bound(v: &Expr) -> bool {
    match v {
        Expr::Let { .. } => true,
        Expr::Ternary(_, yes, no, _) => bound(yes) || bound(no),
        Expr::Match { some, none, .. } => bound(some) || bound(none),
        _ => false,
    }
}

/// [`kept`]'s rows, whatever the value.
fn style_row(a: &Attr) -> bool {
    let n = a.name.as_str();
    if n.contains("transition")
        || n.contains("animation")
        || crate::tags::rows(a).is_empty()
        || (n != "font"
            && matches!(
                crate::tags::attr_valued(n, &a.value),
                Some(crate::tags::AttrTarget::Shorthand | crate::tags::AttrTarget::Flex)
            ))
    {
        return false;
    }
    true
}

/// Each literal `v` writes as written, into `out`: a string or number, or
/// each arm of a `?:` or `match`, when every arm is one; never a `let`
/// (the build judges its bound literal through the binding, as the row:
/// QUEUE's) nor a name a derive binds (checked where it is bound). Whether
/// `v` is so written: a choice with a computed arm is not, and is judged
/// whole as a computed row ([`kept`]).
fn written<'e>(v: &'e Expr, out: &mut Vec<&'e Expr>) -> bool {
    match v {
        Expr::Str(..) | Expr::Number(..) => {
            out.push(v);
            true
        }
        Expr::Ternary(_, yes, no, _) => written(yes, out) && written(no, out),
        Expr::Match { some, none, .. } => written(some, out) && written(none, out),
        _ => false,
    }
}

/// A row the retention build's walk keeps ([`scan::Said::of`]): the row,
/// a font row's node's fonts, and a computed row's type ([`Row`]'s).
pub(crate) struct Said {
    pub attr: Attr,
    pub fonts: Option<Fonts>,
    pub computed: Option<(contract_types::Ty, Typing)>,
}

/// Leave `rows` for [`rows`] (the design server's build reads it).
pub(crate) fn note(rows: Vec<Said>) {
    ROWS.with(|r| *r.borrow_mut() = grouped(rows));
}

/// The literal rows the last build on this thread names, taken: the map
/// the design server checks a write's rows from.
pub fn rows() -> Vec<Row> {
    ROWS.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

/// A row a write breaks: the row, and the compiler's refusal of it.
#[derive(Debug, Clone)]
pub struct Broken {
    /// The row, as written.
    pub row: Attr,
    /// The compiler's refusal, its words unchanged.
    pub error: LowerError,
}

/// Each of `rows` that names a token `touched` names or reaches, through
/// any chain of references in `after` (a write to `space.md` reaches
/// `padding="cards.pad"` with `cards.pad = space.md`), passing under
/// `before` and failing under `after`, each the laid design (packages
/// joined, params laid, as the build lays them). `exprs` are the app's
/// synthetic tokens, `params` its params as tokens (a refusal names one in
/// full). `before` is folded only when a row fails after.
pub fn check(
    rows: &[Row],
    touched: &[&str],
    after: &tamber::Design,
    folded: &tamber::Folding,
    before: impl FnOnce() -> Option<tamber::Design>,
    exprs: &[(String, bool)],
    params: &[crate::params::ParamToken],
) -> Vec<Broken> {
    // Every name that is touched or reaches one: back from the touched
    // names over who names whom, once.
    let mut named_by: std::collections::HashMap<&str, Vec<&str>> = Default::default();
    for t in &folded.tokens {
        for r in t.names.iter().chain(&t.bases) {
            named_by.entry(r.as_str()).or_default().push(&t.name);
        }
    }
    let mut hit: std::collections::HashSet<&str> = touched.iter().copied().collect();
    let mut todo: Vec<&str> = touched.to_vec();
    while let Some(n) = todo.pop() {
        for &by in named_by.get(n).map_or(&[][..], Vec::as_slice) {
            if hit.insert(by) {
                todo.push(by);
            }
        }
    }
    let reaches = |w: &str| hit.contains(w);
    // A font file the build read has changed: font rows are the build's.
    let stale = rows
        .iter()
        .find_map(|r| r.fonts.as_ref())
        .is_some_and(|f| f.declared.stale());
    // A row's own literals, and its node's `font-family`'s (a write to a
    // family token reaches `font-weight=700` beside it).
    let reached: Vec<&Row> = rows
        .iter()
        .filter(|r| {
            if stale && r.fonts.is_some() {
                return false;
            }
            if reads_numbers(r) {
                return true;
            }
            let mut leaves = Vec::new();
            super::leaves(&r.attr.value, &mut Vec::new(), &mut leaves);
            for a in r.fonts.iter().flat_map(|f| &f.attrs) {
                super::leaves(&a.value, &mut Vec::new(), &mut leaves);
            }
            leaves.iter().any(|l| match l {
                Expr::Str(text, _) => scan::words(text).any(reaches),
                _ => false,
            })
        })
        .collect();
    if reached.is_empty() {
        return Vec::new();
    }
    let Some(failing) = judged(after, Some(folded), &reached, exprs, params) else {
        return Vec::new();
    };
    let failing: Vec<(&Row, LowerError)> = reached
        .into_iter()
        .zip(failing)
        .filter_map(|(a, r)| r.err().map(|e| (a, e)))
        .collect();
    if failing.is_empty() {
        return Vec::new();
    }
    // Only the authored rows that passed before are the write's: every
    // literal of the row judged under `before`, not the failing one alone
    // (theme2's Q1, a row's own).
    let Some(before) = before() else {
        return Vec::new();
    };
    let authored: std::collections::HashSet<Span> = failing
        .iter()
        .flat_map(|(r, _)| r.places.iter().map(|p| p.row))
        .collect();
    let theirs: Vec<&Row> = rows
        .iter()
        .filter(|r| r.places.iter().any(|p| authored.contains(&p.row)))
        .collect();
    let Some(was) = judged(&before, None, &theirs, exprs, params) else {
        return Vec::new();
    };
    let failed_before: std::collections::HashSet<Span> = theirs
        .iter()
        .zip(&was)
        // Failing, or not judged here (a `font` naming no type token, the
        // build's to judge another way): not known to have passed.
        .filter(|(_, w)| !matches!(w, Ok(true)))
        .flat_map(|(r, _)| r.places.iter().map(|p| p.row))
        .collect();
    // Each place the row is written whose authored row passed, refused
    // there: where the representative's refusal was, moved alike.
    let mut out = Vec::new();
    for (row, error) in failing {
        let first = row.places[0];
        for p in row
            .places
            .iter()
            .filter(|p| !failed_before.contains(&p.row))
        {
            let at = if error.span == first.literal {
                p.literal
            } else if error.span == first.at {
                p.at
            } else {
                error.span
            };
            out.push(Broken {
                row: Attr {
                    span: p.row,
                    ..row.attr.clone()
                },
                error: LowerError {
                    span: at,
                    ..error.clone()
                },
            });
        }
    }
    out
}

/// Whether `r` is a computed row in a row every number token suits, which
/// lowering judges against the design's number tokens, whichever a write
/// touches: a string against each (`theme::measure_row`), its string arms
/// where there is none (`values::style_value`'s `number_tokens`). Every
/// write reaches it, one removing or retyping the last number token too.
fn reads_numbers(r: &Row) -> bool {
    r.computed.is_some()
        && crate::tags::rows(&r.attr).iter().all(|id| {
            exact_kernel::style::theme::admits(exact_kernel::style::theme::Kind::Number, *id)
        })
}

/// The design server's [`Sink`]: its refusals are the fold's, not the
/// write's, and it keeps no plan.
struct Quiet;

impl Refusals for Quiet {
    fn push(&mut self, _: LowerError) {}
    fn span(&self, _: &Origin, _: u32, _: u32) -> Span {
        Span::default()
    }
}

impl Sink for Quiet {
    fn said(&self, _: Span, _: &str) -> bool {
        false
    }
    fn plan(&mut self, _: &[ThemeEntry], _: &tamber::Folding) {}
    fn stacks(&mut self, entries: &[ThemeEntry]) -> Vec<String> {
        entries.iter().map(|e| e.value.clone()).collect()
    }
}

/// Each of `rows` judged as the compiler judges it, `design` bound as the
/// compiler binds it (`folded`, its fold, when the caller has it); `None`
/// when the design doesn't fold (no row is the write's then: the build
/// says the design). `Ok(true)` a row that passes, `Ok(false)` one not
/// judged here ([`judge`]).
fn judged(
    design: &tamber::Design,
    folded: Option<&tamber::Folding>,
    rows: &[&Row],
    exprs: &[(String, bool)],
    params: &[crate::params::ParamToken],
) -> Option<Vec<Result<bool, LowerError>>> {
    let mut design = design.clone();
    let origin = Origin::Inline(Vec::new());
    let profiled = profiles(&mut Quiet, &mut design, &origin);
    let mut synthetic: Vec<scan::Synthetic> = exprs
        .iter()
        .map(|(expr, strict)| scan::Synthetic {
            expr: expr.clone(),
            span: Span::default(),
            strict: *strict,
        })
        .collect();
    // The design's own fold where nothing joins it (no synthetic token, no
    // profile colour), as the compiler's would be.
    let refolded;
    let folding = match folded {
        Some(f) if synthetic.is_empty() && profiled.is_empty() => f,
        _ => {
            refolded = fold(&design, &mut synthetic, &mut |_, _| {})?;
            &refolded
        }
    };
    let names: Vec<String> = synthetic.iter().map(|s| s.expr.clone()).collect();
    // The server thread's tables as they were, put back however this ends.
    let _kept = Kept::take();
    super::SHOWN.with(|p| *p.borrow_mut() = params.to_vec());
    super::PARAMS.with(|p| *p.borrow_mut() = params.to_vec());
    let (_, _scope) = bind(&mut Quiet, folding, &names, &profiled, &origin);
    let shapes = contract_types::Shapes::default();
    let scope = contract_types::Scope::default();
    let out = rows
        .iter()
        .map(|r| judge(r, &scope, &shapes).map_err(|e| super::shown_errors(vec![e]).remove(0)))
        .collect();
    Some(out)
}

/// One row judged as lowering judges it, under the design bound: a type
/// or family token's row by `type_tokens::tokens_named`, as `type_font`
/// and `family_token` judge it (a value naming none is the shorthand's or
/// a literal family's, left to the build); any other by
/// `values::check_style_value`, with the node's fonts as
/// `fonts::font_use` reads them, and a computed row's type as the walk
/// knew it. `Ok(false)` when it isn't judged here.
fn judge(
    r: &Row,
    scope: &contract_types::Scope,
    shapes: &contract_types::Shapes,
) -> Result<bool, LowerError> {
    let (a, fonts) = (&r.attr, r.fonts.as_ref());
    // A family list lowering refuses stops the element before any font
    // check (`nodes.rs`'s `font_use(expanded)?`): not judged here.
    let uses = match fonts.map(|f| crate::fonts::font_use(&f.declared.fonts, &f.attrs)) {
        Some(Err(_)) => return Ok(false),
        Some(Ok(uses)) => uses,
        None => Vec::new(),
    };
    let kind = match a.name.as_str() {
        "font" => Some((tamber::Kind::Type, &uses[..])),
        "font-family" => Some((tamber::Kind::Family, &[][..])),
        _ => None,
    };
    if let (Some((kind, uses)), Some(f)) = (kind, fonts) {
        return crate::type_tokens::tokens_named(a, kind, uses, &f.declared.fonts, f.children)
            .map(|named| named.is_some());
    }
    let rows = crate::tags::rows(a);
    let mut leaves = Vec::new();
    super::leaves(&a.value, &mut Vec::new(), &mut leaves);
    let (ty, scope, shapes) = match (
        &r.computed,
        leaves.iter().all(|e| matches!(e, Expr::Number(..))),
    ) {
        (Some((ty, typing)), _) => (ty.clone(), &typing.0 .0, &typing.0 .1),
        (None, true) => (contract_types::Ty::Number, scope, shapes),
        (None, false) => (contract_types::Ty::String, scope, shapes),
    };
    crate::values::check_style_value(a, &rows, &ty, &uses, scope, shapes).map(|()| true)
}

/// The tables [`bind`] and the params set, as they were before a check,
/// put back when it ends: the design server's thread keeps no check's.
struct Kept {
    shown: Vec<crate::params::ParamToken>,
    params: Vec<crate::params::ParamToken>,
    measures: Vec<(String, tamber::Kind, Vec<String>)>,
    motion: Vec<(String, String)>,
    axes: Vec<(String, Vec<String>)>,
    roles: Vec<(String, Span)>,
    declared: Vec<String>,
}

impl Kept {
    fn take() -> Kept {
        Kept {
            shown: super::SHOWN.with(|c| std::mem::take(&mut *c.borrow_mut())),
            params: super::PARAMS.with(|c| std::mem::take(&mut *c.borrow_mut())),
            measures: super::MEASURES.with(|c| std::mem::take(&mut *c.borrow_mut())),
            motion: super::MOTION.with(|c| std::mem::take(&mut *c.borrow_mut())),
            axes: super::AXES.with(|c| std::mem::take(&mut *c.borrow_mut())),
            roles: super::ROLES.with(|c| std::mem::take(&mut *c.borrow_mut())),
            declared: crate::scale::declared(),
        }
    }
}

impl Drop for Kept {
    fn drop(&mut self) {
        super::SHOWN.with(|c| *c.borrow_mut() = std::mem::take(&mut self.shown));
        super::PARAMS.with(|c| *c.borrow_mut() = std::mem::take(&mut self.params));
        super::MEASURES.with(|c| *c.borrow_mut() = std::mem::take(&mut self.measures));
        super::MOTION.with(|c| *c.borrow_mut() = std::mem::take(&mut self.motion));
        super::AXES.with(|c| *c.borrow_mut() = std::mem::take(&mut self.axes));
        super::ROLES.with(|c| *c.borrow_mut() = std::mem::take(&mut self.roles));
        crate::scale::declare(std::mem::take(&mut self.declared));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A check leaves the thread's tables as they were (Astra's round 1):
    /// the design server's later builds read none of its binding.
    #[test]
    fn a_check_puts_the_threads_tables_back() {
        let sentinel = vec![(
            "kept".to_string(),
            tamber::Kind::Length,
            vec!["1px".to_string()],
        )];
        super::super::MEASURES.with(|m| *m.borrow_mut() = sentinel.clone());
        super::super::ROLES.with(|r| *r.borrow_mut() = vec![("label".into(), Span::default())]);
        crate::scale::declare(vec!["kept".into()]);
        let design = tamber::parse("tamber t\n\ntheme\n  gap = 10px\n").unwrap();
        let folded = tamber::check_design(design.clone(), &super::super::vocab()).unwrap();
        let row = Row {
            attr: Attr {
                name: "padding".into(),
                value: Expr::Str("gap".into(), Span::default()),
                span: Span::default(),
                written: true,
            },
            places: vec![Place::plain(Span::default())],
            fonts: None,
            computed: None,
        };
        let broken = check(&[row], &["gap"], &design, &folded, || None, &[], &[]);
        assert!(broken.is_empty());
        assert_eq!(
            super::super::MEASURES.with(|m| m.borrow().clone()),
            sentinel
        );
        assert_eq!(super::super::ROLES.with(|r| r.borrow().len()), 1);
        assert_eq!(crate::scale::declared(), vec!["kept".to_string()]);
    }

    /// A font file the build read that has changed since leaves the font
    /// rows to the build (Astra's D1 round 2): `body` made `700` over a
    /// family whose faces the build read as 400 alone is refused while the
    /// file is as it was, and not once it changes (it may be variable now).
    #[test]
    fn a_changed_font_file_leaves_font_rows_to_the_build() {
        let file = std::env::temp_dir().join(format!("exact-rows-font-{}", std::process::id()));
        std::fs::write(&file, b"static").unwrap();
        let m = std::fs::metadata(&file).unwrap();
        let fonts = [(
            "F".to_string(),
            DeclaredFont {
                stack: exact_plan::StacksId(0),
                faces: vec![(400, false)],
            },
        )];
        let declared = Arc::new(Declared::new(
            fonts.into_iter().collect(),
            vec![(
                file.clone(),
                file.canonicalize().unwrap(),
                m.len(),
                m.modified().ok(),
            )],
        ));
        let parse = |t: &str| tamber::parse(t).unwrap();
        let before = "tamber t\n\ntheme\n  body = 400 1rem \"F\"\n";
        let after = parse("tamber t\n\ntheme\n  body = 700 1rem \"F\"\n");
        let folded = tamber::check_design(after.clone(), &super::super::vocab()).unwrap();
        let row = Row {
            attr: Attr {
                name: "font".into(),
                value: Expr::Str("body".into(), Span::default()),
                span: Span::default(),
                written: true,
            },
            places: vec![Place::plain(Span::default())],
            fonts: Some(Fonts {
                attrs: Vec::new(),
                children: false,
                declared,
            }),
            computed: None,
        };
        let rows = [row];
        let check = || {
            check(
                &rows,
                &["body"],
                &after,
                &folded,
                || Some(parse(before)),
                &[],
                &[],
            )
        };
        let broken = check();
        assert_eq!(broken.len(), 1, "{broken:?}");
        assert_eq!(broken[0].error.id, "lower-font-face");
        std::fs::write(&file, b"variable now").unwrap();
        assert!(check().is_empty());
        let _ = std::fs::remove_file(&file);
    }

    /// A font path the app names that a link resolves: retargeted to another
    /// file, the old one untouched, the font rows are the build's too
    /// (Astra's D1 round 3).
    #[cfg(unix)]
    #[test]
    fn a_retargeted_font_link_leaves_font_rows_to_the_build() {
        let dir = std::env::temp_dir().join(format!("exact-rows-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (old, new, link) = (
            dir.join("static.ttf"),
            dir.join("variable.ttf"),
            dir.join("face.ttf"),
        );
        std::fs::write(&old, b"static").unwrap();
        std::fs::write(&new, b"variable").unwrap();
        std::os::unix::fs::symlink(&old, &link).unwrap();
        let m = std::fs::metadata(&old).unwrap();
        let declared = Declared::new(
            Default::default(),
            vec![(
                link.clone(),
                link.canonicalize().unwrap(),
                m.len(),
                m.modified().ok(),
            )],
        );
        assert!(!declared.stale());
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(&new, &link).unwrap();
        assert!(declared.stale());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A `font` row naming no type token isn't judged here, so it never
    /// counts as having passed (Astra's D1 round 1): `font="choice"` with
    /// `choice = 1` (a shorthand the build refuses another way), whose
    /// token a write makes a family, is not the write's to refuse.
    #[test]
    fn a_font_row_not_judged_here_never_passed() {
        let parse = |t: &str| tamber::parse(t).unwrap();
        let before = "tamber t\n\ntheme\n  choice = 1\n";
        let after = parse("tamber t\n\ntheme\n  choice: family = Georgia\n");
        let folded = tamber::check_design(after.clone(), &super::super::vocab()).unwrap();
        let row = Row {
            attr: Attr {
                name: "font".into(),
                value: Expr::Str("choice".into(), Span::default()),
                span: Span::default(),
                written: true,
            },
            places: vec![Place::plain(Span::default())],
            fonts: Some(Fonts {
                attrs: Vec::new(),
                children: false,
                declared: Default::default(),
            }),
            computed: None,
        };
        let broken = check(
            &[row],
            &["choice"],
            &after,
            &folded,
            || Some(parse(before)),
            &[],
            &[],
        );
        assert!(broken.is_empty(), "{broken:?}");
    }
}

//! The design server's write check of the rows a write reaches (LLP 1121
//! D3 and D7; QUEUE's "the server's write check doesn't see a Contract
//! row's kind", theme2's 14:19 ruling). The retention build's walk
//! (`scan::Said::of`) keeps each style row whose value is literal
//! ([`literal_row`]), and the design server keeps that map beside the
//! retention. On a write, [`check`] runs the compiler's own check of a row
//! (`values::check_style_value`) on just the rows that name a token the
//! write touches, through any chain of references, with the design bound
//! as the compiler binds it ([`super::fold`], [`super::bind`]). A row that
//! passes under the design before the write and fails after is refused,
//! in the compiler's words. A row already failing is the file's own, as
//! the build says, so a write that only turns one failure into another is
//! not refused either.
//!
//! Not checked here, and still caught by the build after a miss: a row
//! whose value is computed (a name, a template with holes), which the
//! kernel reads at run time; `font` and `font-family` rows, whose type
//! tokens are `type_tokens.rs`'s; a state part's row; and refusals outside a
//! style row (an SVG attribute, a canvas).

use super::{bind, fold, profiles, scan, Origin, Refusals, Sink, ThemeEntry};
use crate::LowerError;
use contract_syntax::{Attr, Expr, Span};
use exact_tamber as tamber;
use std::cell::RefCell;

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
/// a row of its own; those alike as one.
fn grouped(attrs: Vec<Attr>) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    let mut at: std::collections::HashMap<(String, bool, String), usize> = Default::default();
    for a in attrs {
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
            let k = (a.name.clone(), plain, key);
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
                    });
                }
            }
        }
    }
    out
}

/// Whether `a` is an element's style row [`check`] judges as the build
/// does: rows, a literal value (strings or numbers, or a `?:` or `match`
/// whose arms all are; never a `let`); not a shorthand (the build projects it into longhands
/// first); not a row the build judges with the node's fonts (`font`,
/// `font-family`, `font-weight`, `font-style`); not a transition's or an
/// animation's, whose motion tokens the loader writes in before this map
/// is read.
pub(crate) fn literal_row(a: &Attr) -> bool {
    let n = a.name.as_str();
    if matches!(n, "font" | "font-family" | "font-weight" | "font-style")
        || n.contains("transition")
        || n.contains("animation")
        || crate::tags::rows(a).is_empty()
        || matches!(
            crate::tags::attr_valued(n, &a.value),
            Some(crate::tags::AttrTarget::Shorthand | crate::tags::AttrTarget::Flex)
        )
    {
        return false;
    }
    let mut leaves = Vec::new();
    written(&a.value, &mut leaves)
}

/// Each literal `v` writes as written, into `out`: a string or number, or
/// each arm of a `?:` or `match`, when every arm is one; never a `let`
/// (the build judges its bound literal through the binding, as the row:
/// QUEUE's) nor a name a derive binds (checked where it is bound). Whether `v` is so written: a choice
/// with a computed arm is the build's (a number row's computed arm reads
/// every number token).
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

/// Leave `rows` for [`rows`] (the design server's build reads it).
pub(crate) fn note(rows: Vec<Attr>) {
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
    let reached: Vec<&Row> = rows
        .iter()
        .filter(|r| {
            let a = &r.attr;
            let mut leaves = Vec::new();
            written(&a.value, &mut leaves);
            leaves.iter().any(|l| match l {
                Expr::Str(text, _) => scan::words(text).any(reaches),
                _ => false,
            })
        })
        .collect();
    if reached.is_empty() {
        return Vec::new();
    }
    let attrs: Vec<&Attr> = reached.iter().map(|r| &r.attr).collect();
    let Some(failing) = judged(after, Some(folded), &attrs, exprs, params) else {
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
    let attrs: Vec<&Attr> = theirs.iter().map(|r| &r.attr).collect();
    let Some(was) = judged(&before, None, &attrs, exprs, params) else {
        return Vec::new();
    };
    let failed_before: std::collections::HashSet<Span> = theirs
        .iter()
        .zip(&was)
        .filter(|(_, w)| w.is_err())
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
/// says the design).
fn judged(
    design: &tamber::Design,
    folded: Option<&tamber::Folding>,
    rows: &[&Attr],
    exprs: &[(String, bool)],
    params: &[crate::params::ParamToken],
) -> Option<Vec<Result<(), LowerError>>> {
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
        .map(|a| {
            let rows = crate::tags::rows(a);
            let mut leaves = Vec::new();
            super::leaves(&a.value, &mut Vec::new(), &mut leaves);
            let ty = match leaves.iter().all(|e| matches!(e, Expr::Number(..))) {
                true => contract_types::Ty::Number,
                false => contract_types::Ty::String,
            };
            crate::values::check_style_value(a, &rows, &ty, &[], &scope, &shapes)
                .map_err(|e| super::shown_errors(vec![e]).remove(0))
        })
        .collect();
    Some(out)
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
}

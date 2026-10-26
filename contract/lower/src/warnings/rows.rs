//! `lower-row-lookup` (LLP 1116.016 D4): a `for` over a list of records from
//! the app's data whose rows each search another data list of records
//! (`users.filter(u => u.id == r.userId)`, a nested `for`). The list
//! bound is per evaluation of one binding (LLP 1017.003 D3), so each row stays
//! under it while an update does rows × items steps: measured on a Mac's
//! runner, 1,000 rows over 1,000 users took about 100 ms an update, 5,000 over
//! 5,000 about 2 s; the same rows joined in the data module, 16 and 44 ms.
//!
//! A row's own lists (`r.tags.filter(…)`) are not another list; a `for`
//! over strings (one section per category, filtering items by it) is
//! grouping by a short key, and a list of strings is most often a short set
//! of choices or ids: neither is named. A `derive` of the join is no repair:
//! it is one evaluation, and passes the bound at 1,000 × 1,000.

use crate::LowerError;
use contract_syntax::{Expr, Node, Span, TemplatePart};
use contract_types::{infer, Checked, Ref, Scope, Shapes, Ty};
use std::collections::BTreeSet;

/// The roster's list work that walks its receiver.
const WALKS: &[&str] = &[
    "filter", "map", "includes", "indexOf", "join", "concat", "slice", "sortBy",
];

/// `data.users`, `favorites`: the receiver as written, when it is a name or
/// a field of one, with the name it starts from.
fn path(e: &Expr) -> Option<(String, &str)> {
    match e {
        Expr::Ident(n, _) => Some((n.clone(), n.as_str())),
        Expr::Member(inner, field, _) => {
            path(inner).map(|(p, root)| (format!("{p}.{field}"), root))
        }
        _ => None,
    }
}

/// Whether `root` names the app's data: a state, derive, resource or prop,
/// not a row, a binding or a parameter.
fn data(root: &str, scope: &Scope) -> bool {
    matches!(
        scope.lookup(root),
        Some((
            Ref::Slot(_) | Ref::Derive(_) | Ref::Resource(_) | Ref::Prop(_),
            _
        ))
    )
}

/// Whether `e` is a list of records: a table, where a list of strings is
/// most often a short set of choices or ids.
fn table(e: &Expr, scope: &Scope, shapes: &Shapes) -> bool {
    matches!(infer(e, scope, shapes), Ok(Ty::List(t)) if matches!(*t, Ty::Record(_)))
}

struct Rows<'a> {
    shapes: &'a Shapes,
    /// The file's own shapes: a router's tabs and stack are records too, but
    /// each of their rows is a screen.
    declared: BTreeSet<&'a str>,
    seen: BTreeSet<(u32, u32, u32, u32)>,
    out: Vec<LowerError>,
}

impl Rows<'_> {
    fn warn(&mut self, outer: &str, inner: &str, how: &str, span: Span) {
        if !self
            .seen
            .insert((span.source_id, span.line, span.col, span.end_col))
        {
            return;
        }
        self.out.push(LowerError {
            id: "lower-row-lookup",
            message: format!(
                "each row of `{outer}` searches `{inner}` ({how}), so an update walks every row times every item (1,000 × 1,000 is about 100 ms on a Mac): join it in the data module, giving each row the field it reads"
            ),
            span,
        });
    }

    /// The other data lists `e` walks in a row of `outer`.
    fn expr(&mut self, e: &Expr, scope: &Scope, outer: &str) {
        if let Expr::Call(name, args, span) = e {
            if let (true, Some((inner, root))) =
                (WALKS.contains(&name.as_str()), args.first().and_then(path))
            {
                if data(root, scope) && inner != outer && table(&args[0], scope, self.shapes) {
                    let how = if contract_syntax::idioms::is_method(name) {
                        format!("`.{name}`")
                    } else {
                        format!("`{name}(…)`")
                    };
                    self.warn(outer, &inner, &how, *span);
                }
            }
        }
        match e {
            Expr::Template(parts, _) => {
                for p in parts {
                    if let TemplatePart::Expr(x) = p {
                        self.expr(x, scope, outer);
                    }
                }
            }
            Expr::Ternary(a, b, c, _) => {
                for x in [a, b, c] {
                    self.expr(x, scope, outer);
                }
            }
            Expr::Binary(_, a, b, _) => {
                self.expr(a, scope, outer);
                self.expr(b, scope, outer);
            }
            Expr::Unary(_, inner, _)
            | Expr::Some(inner, _)
            | Expr::NamedArg(_, inner, _)
            | Expr::Typed(inner, _, _)
            | Expr::Member(inner, _, _) => self.expr(inner, scope, outer),
            Expr::Call(_, items, _) | Expr::List(items, _) => {
                for x in items {
                    self.expr(x, scope, outer);
                }
            }
            Expr::Match {
                subject,
                some,
                none,
                ..
            } => {
                self.expr(subject, scope, outer);
                self.expr(some, scope, outer);
                self.expr(none, scope, outer);
            }
            Expr::Let { value, body, .. } => {
                self.expr(value, scope, outer);
                self.expr(body, scope, outer);
            }
            Expr::Arrow { body, .. } => self.expr(body, scope, outer),
            Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) | Expr::Ident(..) => {
            }
        }
    }

    /// `outer` is the data list the innermost enclosing row iterates, if any.
    fn nodes(&mut self, nodes: &[Node], scope: &Scope, outer: Option<&str>) {
        for node in nodes {
            match node {
                Node::Element {
                    positional,
                    attrs,
                    children,
                    ..
                } => {
                    if let Some(outer) = outer {
                        for e in positional.iter().chain(attrs.iter().map(|a| &a.value)) {
                            self.expr(e, scope, outer);
                        }
                    }
                    self.nodes(children, scope, outer);
                }
                Node::When {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    if let Some(outer) = outer {
                        self.expr(cond, scope, outer);
                    }
                    self.nodes(then, scope, outer);
                    self.nodes(otherwise, scope, outer);
                }
                Node::Each {
                    var,
                    index,
                    list,
                    body,
                    span,
                    ..
                } => {
                    let item = match infer(list, scope, self.shapes) {
                        Ok(Ty::List(t)) => *t,
                        _ => Ty::Unknown,
                    };
                    let records =
                        matches!(&item, Ty::Record(n) if self.declared.contains(n.as_str()));
                    let listed = path(list).filter(|(_, root)| data(root, scope));
                    if let (Some(outer), Some((inner, _)), true) = (outer, &listed, records) {
                        if inner != outer {
                            self.warn(outer, inner, "a nested `for`", *span);
                        }
                    }
                    let mut inner = scope.clone();
                    inner.push_each(var, index.as_deref(), item);
                    let rows = listed.filter(|_| records).map(|(p, _)| p);
                    self.nodes(body, &inner, rows.as_deref().or(outer));
                }
                Node::Match {
                    subject,
                    some,
                    none,
                    ..
                } => {
                    if let Some(outer) = outer {
                        self.expr(subject, scope, outer);
                    }
                    let bound = match infer(subject, scope, self.shapes) {
                        Ok(Ty::Option(t)) => *t,
                        _ => Ty::Unknown,
                    };
                    let mut inner = scope.clone();
                    inner.push_region(Some((some.0.clone(), Ref::Bound(0), bound)));
                    self.nodes(&some.1, &inner, outer);
                    let mut none_scope = scope.clone();
                    none_scope.push_region(None);
                    self.nodes(none, &none_scope, outer);
                }
                Node::Use { args, children, .. } => {
                    if let Some(outer) = outer {
                        for a in args {
                            self.expr(&a.value, scope, outer);
                        }
                    }
                    self.nodes(children, scope, outer);
                }
                Node::Children { .. } => {}
            }
        }
    }
}

/// `lower-row-lookup`'s sites in the expanded root's view, then
/// `lower-key-position`'s ([`key_position`]).
pub(super) fn row_lookup(checked: &Checked<'_>) -> Vec<LowerError> {
    let root = &checked.expanded.root;
    let keys = key_position(checked);
    let Some(types) = checked.types.components.first() else {
        return keys;
    };
    let mut rows = Rows {
        shapes: &checked.types.shapes,
        declared: checked
            .file
            .shapes
            .iter()
            .map(|s| s.name.as_str())
            .collect(),
        seen: BTreeSet::new(),
        out: Vec::new(),
    };
    let scope = checked.types.component_scope(root, types);
    rows.nodes(&root.view, &scope, None);
    rows.out.extend(keys);
    rows.out
}

/// Whether `e` reads `name` where it is not rebound (an arrow's parameter, a
/// `let`, a `match` arm shadow it).
fn reads(e: &Expr, name: &str) -> bool {
    match e {
        Expr::Ident(n, _) => n == name,
        Expr::Template(parts, _) => parts
            .iter()
            .any(|p| matches!(p, TemplatePart::Expr(x) if reads(x, name))),
        Expr::Ternary(a, b, c, _) => [a, b, c].iter().any(|x| reads(x, name)),
        Expr::Binary(_, a, b, _) => reads(a, name) || reads(b, name),
        Expr::Unary(_, inner, _)
        | Expr::Some(inner, _)
        | Expr::NamedArg(_, inner, _)
        | Expr::Typed(inner, _, _)
        | Expr::Member(inner, _, _) => reads(inner, name),
        Expr::Call(_, items, _) | Expr::List(items, _) => items.iter().any(|x| reads(x, name)),
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => reads(subject, name) || (var != name && reads(some, name)) || reads(none, name),
        Expr::Let {
            name: bound,
            value,
            body,
            ..
        } => reads(value, name) || (bound != name && reads(body, name)),
        Expr::Arrow { params, body, .. } => !params.iter().any(|p| p == name) && reads(body, name),
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => false,
    }
}

/// `lower-key-position` (LLP 1142 D6): a `for` whose `key=` reads its
/// position variable. After an insertion or a removal before them, rows keep
/// a key that now names another record: what an instance holds (its state, a
/// field's text, its motion) goes with the position, not the record, and rows
/// may be built again. An append-only list keyed so is correct, so it warns.
fn key_position(checked: &Checked<'_>) -> Vec<LowerError> {
    fn walk(nodes: &[Node], out: &mut Vec<LowerError>) {
        for node in nodes {
            match node {
                Node::Element { children, .. } | Node::Use { children, .. } => walk(children, out),
                Node::When {
                    then, otherwise, ..
                } => {
                    walk(then, out);
                    walk(otherwise, out);
                }
                Node::Each {
                    index, key, body, ..
                } => {
                    if let Some(i) = index.as_deref().filter(|i| reads(key, i)) {
                        out.push(LowerError {
                            id: "lower-key-position",
                            message: format!("this key reads `{i}`, the row's position: after a row is inserted or removed before others, their keys name other records, so what a row holds (its state, a field's text, its motion) follows the position and rows may be built again; key by something the record keeps, such as its id"),
                            span: key.span(),
                        });
                    }
                    walk(body, out);
                }
                Node::Match { some, none, .. } => {
                    walk(&some.1, out);
                    walk(none, out);
                }
                Node::Children { .. } => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&checked.expanded.root.view, &mut out);
    out
}

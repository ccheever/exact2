//! What a placeholder can't learn from a prop's type, but the component's
//! own view says: the parameters a handler calls an action prop with
//! (`change=set` on a switch passes its `checked`), and a string the view
//! names a symbol by (`image \`symbol:${symbol}\``), where the prop's name
//! would name no role.

use contract_syntax::{Component, Expr, File, Node, TemplatePart, TypeExpr};
use std::collections::{BTreeMap, HashSet};

/// A handler's parameters, as `(name, type)`.
pub(super) type Signature = Vec<(&'static str, &'static str)>;

/// The inert actions the gallery declares beside `noop`, one per handler
/// signature its components' action props are given to.
#[derive(Default)]
pub(super) struct Inert {
    names: BTreeMap<Signature, String>,
}

impl Inert {
    /// The actions `components` need, each named free of `taken`.
    pub(super) fn new<'a>(
        components: impl IntoIterator<Item = &'a Component>,
        file: &File,
        taken: &mut HashSet<String>,
    ) -> Self {
        let mut names = BTreeMap::new();
        for c in components {
            for p in &c.props {
                if let Some(signature) = handler(c, file, &p.name) {
                    names.entry(signature.clone()).or_insert_with(|| {
                        let types: Vec<String> =
                            signature.iter().map(|(_, ty)| spelled(ty)).collect();
                        super::unique(&format!("noop_{}", types.join("_")), taken)
                    });
                }
            }
        }
        Inert { names }
    }

    /// The action the action prop `prop` of `c` is given: an inert one of
    /// its handler's signature, else `noop`.
    pub(super) fn action(&self, c: &Component, file: &File, prop: &str) -> String {
        handler(c, file, prop)
            .and_then(|signature| self.names.get(&signature).cloned())
            .unwrap_or_else(|| "noop".into())
    }

    /// The Gallery's declarations of them, a line each.
    pub(super) fn declarations(&self) -> String {
        self.names
            .iter()
            .map(|(signature, name)| {
                let params: Vec<String> = signature
                    .iter()
                    .map(|(n, ty)| format!("{n}: {ty}"))
                    .collect();
                format!("  action {name}({})\n", params.join(", "))
            })
            .collect()
    }
}

/// The parameters the handlers of `c`'s own view call the action prop
/// `prop` with, when every such use passes the same types: given bare
/// (`change=set`), the handler's payload; given its own arguments
/// (`press=open(a.id)`), theirs, then the payload, when each is a literal or
/// a `for` item's field whose type the shape says. `None` for one given
/// nowhere, passed on to another component, given arguments the source
/// doesn't type, or given to handlers that pass different ones (`press`
/// none, `navigate` a string): `noop`, and the compiler says whether that
/// fits.
fn handler(c: &Component, file: &File, prop: &str) -> Option<Signature> {
    let mut uses: Vec<Option<Signature>> = Vec::new();
    calls(&c.view, &BTreeMap::new(), c, file, &mut |node, items| {
        if let Node::Element { tag, attrs, .. } = node {
            for a in attrs {
                let args = match &a.value {
                    Expr::Ident(name, _) if name == prop && !items.contains_key(name.as_str()) => {
                        Some(Vec::new())
                    }
                    Expr::Call(name, args, _)
                        if name == prop && !items.contains_key(name.as_str()) =>
                    {
                        Some(args.iter().collect())
                    }
                    _ => None,
                };
                let Some(args) = args else { continue };
                let control = contract_syntax::payload_control(tag, attrs);
                let payload = contract_types::handler_parameters(&a.name, control);
                let typed: Option<Signature> = args
                    .iter()
                    .enumerate()
                    .map(|(i, e)| Some((ARGS[i.min(ARGS.len() - 1)], typed(e, items)?)))
                    .collect();
                uses.push(typed.map(|mut s| {
                    s.extend(payload);
                    s
                }));
            }
        }
    });
    let mut uses = uses.into_iter();
    let first = uses.next()??;
    let types = |s: &Signature| s.iter().map(|(_, ty)| *ty).collect::<Vec<_>>();
    let same = uses.all(|s| s.is_some_and(|s| types(&s) == types(&first)));
    (!first.is_empty() && same).then_some(first)
}

/// Each `for` item in scope: its name and its shape's plain fields' types.
type Items<'a> = BTreeMap<&'a str, Vec<(String, &'static str)>>;

/// A call's argument names, in order.
const ARGS: [&str; 4] = ["a", "b", "c", "d"];

/// The type of an argument the source settles: a literal, or a field of a
/// `for` item (`items`: each item's name and its shape's fields).
fn typed(e: &Expr, items: &Items<'_>) -> Option<&'static str> {
    match e {
        Expr::Str(..) | Expr::Template(..) => Some("string"),
        Expr::Number(..) => Some("number"),
        Expr::Bool(..) => Some("bool"),
        Expr::Member(inner, field, _) => match inner.as_ref() {
            Expr::Ident(name, _) => items
                .get(name.as_str())?
                .iter()
                .find(|(f, _)| f == field)
                .map(|(_, ty)| *ty),
            _ => None,
        },
        _ => None,
    }
}

/// A plain type's name, as a declaration spells it.
fn plain(ty: &TypeExpr) -> Option<&'static str> {
    match ty {
        TypeExpr::Named(t, _) => match t.as_str() {
            "string" => Some("string"),
            "number" => Some("number"),
            "bool" => Some("bool"),
            _ => None,
        },
        TypeExpr::Enum(..) => Some("string"),
        _ => None,
    }
}

/// Every node of `nodes`, with each `for` item around it that iterates a
/// list prop of a shape (its plain fields' types), a name `match` binds
/// hiding one.
fn calls<'a>(
    nodes: &'a [Node],
    items: &Items<'a>,
    c: &Component,
    file: &File,
    f: &mut dyn FnMut(&Node, &Items<'a>),
) {
    for n in nodes {
        f(n, items);
        match n {
            Node::Element { children, .. } | Node::Use { children, .. } => {
                calls(children, items, c, file, f)
            }
            Node::When {
                then, otherwise, ..
            } => {
                calls(then, items, c, file, f);
                calls(otherwise, items, c, file, f);
            }
            Node::Each {
                var,
                index,
                list,
                body,
                ..
            } => {
                let mut inner = items.clone();
                let fields = match list {
                    Expr::Ident(name, _) => {
                        c.props
                            .iter()
                            .find(|p| &p.name == name)
                            .and_then(|p| match &p.ty {
                                Some(TypeExpr::List(element, _)) => match element.as_ref() {
                                    TypeExpr::Named(shape, _) => {
                                        file.shapes.iter().find(|s| &s.name == shape)
                                    }
                                    _ => None,
                                },
                                _ => None,
                            })
                    }
                    _ => None,
                };
                inner.insert(
                    var.as_str(),
                    fields
                        .map(|s| {
                            s.fields
                                .iter()
                                .filter_map(|f| Some((f.name.clone(), plain(&f.ty)?)))
                                .collect()
                        })
                        .unwrap_or_default(),
                );
                if let Some(index) = index {
                    inner.insert(index.as_str(), Vec::new());
                }
                calls(body, &inner, c, file, f);
            }
            Node::Match { some, none, .. } => {
                let mut inner = items.clone();
                inner.insert(some.0.as_str(), Vec::new());
                calls(&some.1, &inner, c, file, f);
                calls(none, items, c, file, f);
            }
            Node::Children { .. } => {}
        }
    }
}

/// A type as part of an action's name: `option<string>` is `option_string`.
fn spelled(ty: &str) -> String {
    ty.split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

/// Whether `c`'s own view names a symbol by the string prop `prop`: an
/// image source that is `symbol:` and the prop, nothing after it, which
/// the compiler reads as a role (`symbol:sf/…` takes any name, and a
/// stem, `symbol:${name}-fill`, or a prefix, `symbol:${set}${name}`, keeps
/// the prop's own name).
pub(super) fn names_symbol(c: &Component, prop: &str) -> bool {
    let mut names = false;
    each(&c.view, &[], &mut |node, bound| {
        if let Node::Element {
            tag, positional, ..
        } = node
        {
            if let (true, Some(Expr::Template(parts, _))) = (tag == "image", positional.first()) {
                names |= matches!(
                    parts.as_slice(),
                    [TemplatePart::Text(head), TemplatePart::Expr(Expr::Ident(name, _))]
                        if head == "symbol:" && name == prop && !bound.contains(name)
                );
            }
        }
    });
    names
}

/// A role to show for such a prop: its own name if that names one, else
/// the schema's first.
pub(super) fn role(prop: &str) -> &'static str {
    exact_kernel::generated::symbol_role(prop).unwrap_or(exact_kernel::generated::SYMBOL_ROLES[0])
}

/// Every node of `nodes`, at any depth, with the names a `for` or a
/// `match` binds around it, which shadow a prop of the same name.
fn each(nodes: &[Node], bound: &[String], f: &mut dyn FnMut(&Node, &[String])) {
    for n in nodes {
        f(n, bound);
        match n {
            Node::Element { children, .. } | Node::Use { children, .. } => each(children, bound, f),
            Node::When {
                then, otherwise, ..
            } => {
                each(then, bound, f);
                each(otherwise, bound, f);
            }
            Node::Each {
                var, index, body, ..
            } => {
                let inner: Vec<String> = bound.iter().chain([var]).chain(index).cloned().collect();
                each(body, &inner, f);
            }
            Node::Match { some, none, .. } => {
                let inner: Vec<String> = bound.iter().chain([&some.0]).cloned().collect();
                each(&some.1, &inner, f);
                each(none, bound, f);
            }
            Node::Children { .. } => {}
        }
    }
}

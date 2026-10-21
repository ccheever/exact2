//! What a placeholder can't learn from a prop's type, but the component's
//! own view says: the parameters a handler calls an action prop with
//! (`change=set` on a switch passes its `checked`), and a string the view
//! names a symbol by (`image \`symbol:${symbol}\``), where the prop's name
//! would name no role.

use contract_syntax::{Component, Expr, Node, TemplatePart};
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
        taken: &mut HashSet<String>,
    ) -> Self {
        let mut names = BTreeMap::new();
        for c in components {
            for p in &c.props {
                if let Some(signature) = handler(c, &p.name) {
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
    pub(super) fn action(&self, c: &Component, prop: &str) -> String {
        handler(c, prop)
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
/// `prop` with, when it is given bare (`change=set`) and every such use
/// passes the same types. `None` for one given nowhere, given with
/// arguments of its own, passed on to another component, or given to
/// handlers that pass different ones (`press` none, `navigate` a string):
/// `noop`, and the compiler says whether that fits.
fn handler(c: &Component, prop: &str) -> Option<Signature> {
    let mut uses: Vec<Signature> = Vec::new();
    each(&c.view, &[], &mut |node, bound| {
        if let Node::Element { tag, attrs, .. } = node {
            for a in attrs {
                if matches!(&a.value, Expr::Ident(name, _) if name == prop && !bound.contains(name))
                {
                    let control = contract_syntax::payload_control(tag, attrs);
                    uses.push(contract_types::handler_parameters(&a.name, control));
                }
            }
        }
    });
    let (first, rest) = uses.split_first()?;
    let types = |s: &Signature| s.iter().map(|(_, ty)| *ty).collect::<Vec<_>>();
    (!first.is_empty() && rest.iter().all(|s| types(s) == types(first))).then(|| first.clone())
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
/// stem, `symbol:${name}-fill`, keeps the prop's own name).
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

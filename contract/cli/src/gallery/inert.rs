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
                        let types: Vec<&str> = signature.iter().map(|(_, ty)| *ty).collect();
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

/// The parameters a handler of `c`'s own view calls the action prop `prop`
/// with, when it is given bare (`change=set`) and each is a type a
/// declaration can spell. `None` for one given nowhere, given with
/// arguments of its own, or passed on to another component: `noop`, and
/// the compiler says whether that fits.
fn handler(c: &Component, prop: &str) -> Option<Signature> {
    let mut found = None;
    each(&c.view, &mut |node| {
        if let Node::Element { tag, attrs, .. } = node {
            for a in attrs {
                if matches!(&a.value, Expr::Ident(name, _) if name == prop) {
                    let control = contract_syntax::payload_control(tag, attrs);
                    let params = contract_types::handler_parameters(&a.name, control);
                    if !params.is_empty() && found.is_none() {
                        found = Some(params);
                    }
                }
            }
        }
    });
    found.filter(|params| {
        params
            .iter()
            .all(|(_, ty)| matches!(*ty, "string" | "number" | "bool"))
    })
}

/// Whether `c`'s own view names a symbol by the string prop `prop`: an
/// image source `symbol:` and then the prop, which the compiler reads as a
/// role (`symbol:sf/…` takes any name).
pub(super) fn names_symbol(c: &Component, prop: &str) -> bool {
    let mut names = false;
    each(&c.view, &mut |node| {
        if let Node::Element {
            tag, positional, ..
        } = node
        {
            if let (true, Some(Expr::Template(parts, _))) = (tag == "image", positional.first()) {
                names |= matches!(
                    parts.as_slice(),
                    [TemplatePart::Text(head), TemplatePart::Expr(Expr::Ident(name, _)), ..]
                        if head == "symbol:" && name == prop
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

/// Every node of `nodes`, at any depth.
fn each(nodes: &[Node], f: &mut dyn FnMut(&Node)) {
    for n in nodes {
        f(n);
        match n {
            Node::Element { children, .. } | Node::Use { children, .. } => each(children, f),
            Node::When {
                then, otherwise, ..
            } => {
                each(then, f);
                each(otherwise, f);
            }
            Node::Each { body, .. } => each(body, f),
            Node::Match { some, none, .. } => {
                each(&some.1, f);
                each(none, f);
            }
            Node::Children { .. } => {}
        }
    }
}

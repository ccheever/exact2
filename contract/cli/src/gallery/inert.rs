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
                    .map(|(i, e)| Some((*ARGS.get(i)?, typed(e, items)?)))
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

/// Each name a `for` or `match` binds in scope: an item's shape's plain
/// fields' types, and its type when the source says it.
type Items<'a> = BTreeMap<&'a str, Bound<'a>>;

#[derive(Clone, Default)]
struct Bound<'a> {
    fields: Vec<(String, &'static str)>,
    ty: Option<&'a TypeExpr>,
}

/// A call's argument names, by position; a call with more is left to `noop`.
const ARGS: [&str; 8] = ["a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7"];

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
                .fields
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
    c: &'a Component,
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
                // The list's element type: a list prop's, or a list an outer
                // `for` item is (a name it binds hiding the prop's).
                let listed = match list {
                    Expr::Ident(name, _) => match items.get(name.as_str()) {
                        Some(bound) => bound.ty,
                        None => c
                            .props
                            .iter()
                            .find(|p| &p.name == name)
                            .and_then(|p| p.ty.as_ref()),
                    },
                    _ => None,
                };
                let element = match listed {
                    Some(TypeExpr::List(element, _)) => Some(element.as_ref()),
                    _ => None,
                };
                let fields = match element {
                    Some(TypeExpr::Named(shape, _)) => file
                        .shapes
                        .iter()
                        .find(|s| &s.name == shape)
                        .map(|s| {
                            s.fields
                                .iter()
                                .filter_map(|f| Some((f.name.clone(), plain(&f.ty)?)))
                                .collect()
                        })
                        .unwrap_or_default(),
                    _ => Vec::new(),
                };
                inner.insert(
                    var.as_str(),
                    Bound {
                        fields,
                        ty: element,
                    },
                );
                if let Some(index) = index {
                    inner.insert(index.as_str(), Bound::default());
                }
                calls(body, &inner, c, file, f);
            }
            Node::Match { some, none, .. } => {
                let mut inner = items.clone();
                inner.insert(some.0.as_str(), Bound::default());
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

/// Whether `c`'s view shows string prop `prop` as an image's source:
/// `image prop`, or `prop` passed to a component of `file` that does, at
/// any depth (PostCard's `avatar` reaches Avatar's `image src`). A `for`
/// or `match` binding of the same name shadows it.
pub(super) fn shows_image(c: &Component, file: &File, prop: &str) -> bool {
    shows(c, file, prop, &mut Vec::new())
}

fn shows(c: &Component, file: &File, prop: &str, seen: &mut Vec<(String, String)>) -> bool {
    let key = (c.name.clone(), prop.to_string());
    if seen.contains(&key) {
        return false;
    }
    seen.push(key);
    let mut found = false;
    let mut used: Vec<(String, String)> = Vec::new();
    each(&c.view, &[], &mut |node, bound| {
        if bound.iter().any(|b| b == prop) {
            return;
        }
        let is_prop = |e: &Expr| yields(e, prop);
        match node {
            Node::Element {
                tag, positional, ..
            } if tag == "image" => found |= positional.first().is_some_and(is_prop),
            Node::Use { name, args, .. } => used.extend(
                args.iter()
                    .filter(|a| is_prop(&a.value))
                    .map(|a| (name.clone(), a.name.clone())),
            ),
            _ => {}
        }
    });
    found
        || used.iter().any(|(name, param)| {
            file.components
                .iter()
                .find(|d| d.name == *name)
                .is_some_and(|d| shows(d, file, param, seen))
        })
}

/// What a string prop the view writes as a style row's value is shown
/// with, by the row's kind: a colour row's `currentcolor` (the text's
/// colour, a CSS colour every host reads), a length row's `24px`, where the
/// prop's name would be no colour, token or length (Avatar's `width=size`,
/// VerifiedBadge's `color=ink`, LLP 1121 stage 15 B); a length row is one
/// whose every row is a dimension (a width, a padding), so a font's family
/// or a weight keeps its name. Through a component
/// of `file` the prop is passed to, at any depth, as [`shows_image`]; a
/// colour row first where it reaches both. None for a prop no row takes.
pub(super) fn style_sample(c: &Component, file: &File, prop: &str) -> Option<&'static str> {
    let (colour, length) = styled(c, file, prop, &mut Vec::new());
    if colour {
        Some("currentcolor")
    } else if length {
        Some("24px")
    } else {
        None
    }
}

/// Whether `c`'s view (or a component it passes `prop` to) writes `prop`
/// as a colour row's value, and as a length row's.
fn styled(
    c: &Component,
    file: &File,
    prop: &str,
    seen: &mut Vec<(String, String)>,
) -> (bool, bool) {
    use contract_lower::tags::AttrTarget;
    use exact_kernel::generated::StyleCodec;
    use exact_kernel::style::theme::{admits, Kind};
    let key = (c.name.clone(), prop.to_string());
    if seen.contains(&key) {
        return (false, false);
    }
    seen.push(key);
    let (mut colour, mut length) = (false, false);
    let mut used: Vec<(String, String)> = Vec::new();
    each(&c.view, &[], &mut |node, bound| {
        if bound.iter().any(|b| b == prop) {
            return;
        }
        match node {
            Node::Element { attrs, .. } => {
                for a in attrs.iter().filter(|a| yields(&a.value, prop)) {
                    if let Some(AttrTarget::Styles(rows)) =
                        contract_lower::style_names::lookup(&a.name)
                    {
                        colour |= !rows.is_empty() && rows.iter().all(|r| admits(Kind::Color, *r));
                        length |= !rows.is_empty()
                            && rows
                                .iter()
                                .all(|r| matches!(r.codec(), StyleCodec::Dimension));
                    }
                }
            }
            Node::Use { name, args, .. } => used.extend(
                args.iter()
                    .filter(|a| yields(&a.value, prop))
                    .map(|a| (name.clone(), a.name.clone())),
            ),
            _ => {}
        }
    });
    for (name, param) in used {
        if let Some(d) = file.components.iter().find(|d| d.name == name) {
            let (c2, l2) = styled(d, file, &param, seen);
            colour |= c2;
            length |= l2;
        }
    }
    (colour, length)
}

/// Whether `e`'s value is `prop`'s, unchanged: the name itself, a template
/// of it alone (`` `${src}` ``), or either branch of a `?:` (a fallback
/// beside it). A template that adds text (`symbol:${name}`) makes another
/// string, never the prop's.
fn yields(e: &Expr, prop: &str) -> bool {
    match e {
        Expr::Ident(name, _) => name == prop,
        Expr::Template(parts, _) => {
            matches!(parts.as_slice(), [TemplatePart::Expr(inner)] if yields(inner, prop))
        }
        Expr::Ternary(_, then, otherwise, _) => yields(then, prop) || yields(otherwise, prop),
        _ => false,
    }
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

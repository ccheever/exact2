//! Miswirings the compiler can see (LLP 1116.010 D5): a name that points at
//! nothing, or a key that never meets its partner, which the hosts refuse or
//! ignore at run time without failing the build. Round 12's builders spent
//! 7, 8 and 1 minutes on them with the repair one line away: grocery-list's
//! delete button, indented as a sibling of the swipe row's `scroll` instead of
//! inside it (iOS: `swipeContent on #243 is refused: "delete-1" names no one
//! descendant of it`); chat-rooms' navigation root keyed by a literal while its
//! routes are keyed by id, so no push was ever shown; habit-tracker's
//! `EmptyState(symbol="sparkles")`, which draws nothing; recipe-finder's
//! `navigationSplit` on the root, which no host reads.
//!
//! Only what is certain is named. A name is compared as written: a literal, or
//! a template whose fixed text a descendant's `id` could not complete
//! (`delete-${i}` against `row-${i}`); a computed name or id, an unexpanded
//! use or a slot is the benefit of the doubt.

use crate::LowerError;
use contract_syntax::{Attr, Expr, Node, Span, TemplatePart};

/// The children of a node, through choices.
pub(super) fn arms(node: &Node) -> Vec<&[Node]> {
    match node {
        Node::Element { children, .. } | Node::Use { children, .. } => vec![children],
        Node::When {
            then, otherwise, ..
        } => vec![then, otherwise],
        Node::Match { some, none, .. } => vec![&some.1, none],
        Node::Each { body, .. } => vec![body],
        Node::Children { .. } => Vec::new(),
    }
}

/// One piece of a name: written text, or a `${…}` hole.
#[derive(Clone, PartialEq)]
enum Piece {
    Text(String),
    Hole(String),
}

/// A name as the compiler reads it: a literal or a template of holes it can
/// spell (`row-${i}`, `delete-${m.id}`).
#[derive(Clone, PartialEq)]
struct Name(Vec<Piece>);

impl Name {
    /// The name, when it is a literal or a template of holes it can spell.
    fn of(e: &Expr) -> Option<Name> {
        match e {
            Expr::Str(s, _) => Some(Name(vec![Piece::Text(s.clone())])),
            Expr::Template(parts, _) => {
                let mut pieces = Vec::new();
                for part in parts {
                    pieces.push(match part {
                        TemplatePart::Text(t) => Piece::Text(t.clone()),
                        TemplatePart::Expr(x) => Piece::Hole(spell(x)?),
                    });
                }
                Some(Name(pieces))
            }
            _ => None,
        }
    }

    /// The names a whitespace-separated list attribute holds, split at the
    /// whitespace in its written text, and whether every one was read: a
    /// choice (`c ? "a b" : ""`) holds the names of each arm it can read.
    fn list(e: &Expr) -> (Vec<Name>, bool) {
        if let Expr::Ternary(_, a, b, _) = e {
            let ((mut names, read_a), (more, read_b)) = (Name::list(a), Name::list(b));
            names.extend(more);
            return (names, read_a && read_b);
        }
        let Some(whole) = Name::of(e) else {
            return (Vec::new(), false);
        };
        let mut names = Vec::new();
        let mut name = Vec::new();
        for piece in whole.0 {
            let Piece::Text(text) = piece else {
                name.push(piece);
                continue;
            };
            for (i, word) in text.split(char::is_whitespace).enumerate() {
                if i > 0 && !name.is_empty() {
                    names.push(Name(std::mem::take(&mut name)));
                }
                if !word.is_empty() {
                    name.push(Piece::Text(word.to_string()));
                }
            }
        }
        if !name.is_empty() {
            names.push(Name(name));
        }
        (names, true)
    }

    fn literal(&self) -> Option<String> {
        self.0
            .iter()
            .all(|p| matches!(p, Piece::Text(_)))
            .then(|| self.shown())
    }

    /// The written text before the first hole (all of a literal).
    fn head(&self) -> String {
        self.0
            .iter()
            .map_while(|p| match p {
                Piece::Text(t) => Some(t.as_str()),
                Piece::Hole(_) => None,
            })
            .collect()
    }

    /// The written text after the last hole (all of a literal).
    fn tail(&self) -> String {
        let mut tail: Vec<&str> = self
            .0
            .iter()
            .rev()
            .map_while(|p| match p {
                Piece::Text(t) => Some(t.as_str()),
                Piece::Hole(_) => None,
            })
            .collect();
        tail.reverse();
        tail.concat()
    }

    /// Whether some value of the holes could make the two names one.
    fn may_match(&self, other: &Name) -> bool {
        if let (Some(a), Some(b)) = (self.literal(), other.literal()) {
            return a == b;
        }
        let (ha, hb, ta, tb) = (self.head(), other.head(), self.tail(), other.tail());
        (ha.starts_with(&hb) || hb.starts_with(&ha)) && (ta.ends_with(&tb) || tb.ends_with(&ta))
    }

    /// As written: the literal's text, a template's with its holes.
    fn shown(&self) -> String {
        self.0
            .iter()
            .map(|p| match p {
                Piece::Text(t) => t.clone(),
                Piece::Hole(h) => format!("${{{h}}}"),
            })
            .collect()
    }
}

/// An expression's spelling, when it is a name, a field of one, a call or a number.
fn spell(e: &Expr) -> Option<String> {
    match e {
        Expr::Ident(n, _) => Some(n.clone()),
        Expr::Member(base, field, _) => Some(format!("{}.{field}", spell(base)?)),
        Expr::Number(n, _) => Some(n.to_string()),
        Expr::Str(s, _) => Some(format!("{s:?}")),
        Expr::Call(name, args, _) => {
            let args: Option<Vec<String>> = args.iter().map(spell).collect();
            Some(format!("{name}({})", args?.join(", ")))
        }
        _ => None,
    }
}

fn attr<'a>(attrs: &'a [Attr], name: &str) -> Option<&'a Attr> {
    attrs.iter().rev().find(|a| a.name == name)
}

/// Every `id` among the elements under `nodes`, with the element it names; `None`
/// when one cannot be read (a computed `id`, an unexpanded use): the benefit of the
/// doubt.
fn ids<'a>(nodes: &'a [Node], out: &mut Vec<(Name, &'a Node)>) -> Option<()> {
    for node in nodes {
        if let Node::Use { .. } | Node::Children { .. } = node {
            return None;
        }
        if let Node::Element { attrs, .. } = node {
            if let Some(id) = attr(attrs, "id") {
                out.push((Name::of(&id.value)?, node));
            }
        }
        for list in arms(node) {
            ids(list, out)?;
        }
    }
    Some(())
}

/// Whether the elements under `node` (its own `id` included) hold one called `name`.
fn holds(node: &Node, name: &Name) -> Option<bool> {
    let mut found = Vec::new();
    ids(std::slice::from_ref(node), &mut found)?;
    Some(found.iter().any(|(id, _)| id.may_match(name)))
}

/// A name as an author writes it in a message.
fn written(name: &Name) -> String {
    if name.literal().is_some() {
        format!("\"{}\"", name.shown())
    } else {
        format!("`{}`", name.shown())
    }
}

const SWIPE_FIX: &str = "(docs/recipes/list-rows.md)";

/// `lower-swipe-wiring` (LLP 1116.010 D5): a row's `scroll swipeContent=… swipeLeading=…
/// swipeTrailing=…` whose names do not meet the elements inside it. iOS builds UIKit's
/// swipe action from a content element and action buttons that are all descendants of the
/// scroll, the actions beside the content and not inside it, and refuses the row at run
/// time otherwise (`logs`: `swipeContent on #243 is refused: "delete-1" names no one
/// descendant of it`); the web then leaves the button where it sits, in the page, which is
/// grocery-list's seven minutes. A swipe action named without a `swipeContent`, and a
/// `swipeContent` with no action, are refused or ignored the same way.
pub(super) fn swipe(attrs: &[Attr], children: &[Node], fresh: bool, out: &mut Vec<LowerError>) {
    let content = attr(attrs, "swipeContent");
    let actions: Vec<&Attr> = ["swipeLeading", "swipeTrailing"]
        .iter()
        .filter_map(|n| attr(attrs, n))
        .collect();
    if !fresh || (content.is_none() && actions.is_empty()) {
        return;
    }
    let warn = |span: Span, message: String, out: &mut Vec<LowerError>| {
        out.push(LowerError {
            id: "lower-swipe-wiring",
            message,
            span,
        });
    };
    let Some(content) = content else {
        let a = actions[0];
        warn(a.span, format!("`{}` is written without a `swipeContent`, so iOS ignores the row's actions: a swipe row names its content element too, `swipeContent=\"row\"` beside an `id=\"row\"` on the box that holds what the row shows, and the action buttons as its siblings inside this `scroll` {SWIPE_FIX}", a.name), out);
        return;
    };
    let mut inside = Vec::new();
    if ids(children, &mut inside).is_none() {
        return;
    }
    let find = |name: &Name| inside.iter().find(|(id, _)| id.may_match(name));
    let Some(contents) = Name::of(&content.value).map(|n| vec![n]) else {
        return;
    };
    let read: Vec<(&Attr, (Vec<Name>, bool))> =
        actions.iter().map(|a| (*a, Name::list(&a.value))).collect();
    let all_read = read.iter().all(|(_, (_, whole))| *whole);
    let named: Vec<(&Attr, Vec<Name>)> = read.into_iter().map(|(a, (n, _))| (a, n)).collect();
    for c in &contents {
        if find(c).is_none() {
            warn(content.span, format!("`swipeContent` names {}, but no element inside this `scroll` has that `id`: iOS refuses the row (`logs`: `swipeContent … names no one descendant of it`) and the web shows it as a plain row. Give the box that holds what the row shows `id=…` to match, inside this `scroll` {SWIPE_FIX}", written(c)), out);
        }
    }
    // No action only when every action is read and names none: one computed some
    // other way may name one.
    if all_read && named.iter().all(|(_, names)| names.is_empty()) {
        warn(content.span, format!("`swipeContent` is written without `swipeLeading` or `swipeTrailing`, so the row has no action to reveal and iOS refuses it (`it names no swipeLeading or swipeTrailing control`): name the action button's `id` in `swipeTrailing`, a button inside this `scroll` after the content {SWIPE_FIX}"), out);
    }
    for (a, names) in &named {
        for name in names {
            let Some((_, control)) = find(name) else {
                warn(a.span, format!("`{}` names {}, but no element inside this `scroll` has that `id`: iOS refuses the row (`logs`: `swipeContent on #N is refused: … names no one descendant of it`) and the web leaves the button where it sits, outside the row (a full-width second line under each item). Put the button inside this `scroll`, after the content, with `id=…` to match {SWIPE_FIX}", a.name, written(name)), out);
                continue;
            };
            // The action sits beside the content: one inside it is no action.
            let inner = contents.iter().filter_map(&find).any(|(_, content)| {
                !std::ptr::eq(*content, *control) && holds(content, name) == Some(true)
            });
            if inner {
                warn(a.span, format!("`{}` names {}, which sits inside the `swipeContent` element: iOS refuses the row (`… is inside swipeContent`). Move the button out of the content box, to be its sibling inside this `scroll` {SWIPE_FIX}", a.name, written(name)), out);
            }
        }
    }
}

/// How a `navigationKey` reads.
enum Key {
    /// A written key (`"rooms"`).
    Literal(String),
    /// The id of the visit on top, `${top(nav).id}`.
    TopId,
    /// An entry's id, `${e.id}`.
    EntryId(String),
    /// Anything else, a choice among keys included: the benefit of the doubt.
    Other,
}

fn key_of(e: &Expr) -> Key {
    let id = |x: &Expr| match x {
        Expr::Member(base, field, _) if field == "id" => match &**base {
            Expr::Call(name, _, _) if name == "top" => Some(Key::TopId),
            Expr::Ident(..) => spell(x).map(Key::EntryId),
            _ => None,
        },
        _ => None,
    };
    match e {
        Expr::Str(s, _) => Key::Literal(s.clone()),
        Expr::Template(parts, _) => match parts.as_slice() {
            [TemplatePart::Expr(x)] => id(x).unwrap_or(Key::Other),
            _ => Key::Other,
        },
        other => id(other).unwrap_or(Key::Other),
    }
}

/// The `navigationKey` attributes of the routes under a navigation root: elements that carry
/// one, found through choices and boxes (a tab panel), never inside another route.
fn route_keys<'a>(nodes: &'a [Node], out: &mut Vec<&'a Attr>) {
    for node in nodes {
        if let Node::Element { attrs, .. } = node {
            if let Some(k) = attr(attrs, "navigationKey") {
                out.push(k);
                continue;
            }
        }
        for list in arms(node) {
            route_keys(list, out);
        }
    }
}

/// `lower-navigation-key` (LLP 1116.010 D5): the navigation root's `navigationKey` names
/// the route to show, so it has to be a key some route carries while that route is on top.
/// A literal root key over routes keyed by their entry's id never names a pushed route:
/// chat-rooms' `main navigationKey="rooms"` over `navigationKey=`${e.id}`` showed nothing
/// when a row was tapped (eight minutes). A root keyed `${top(nav).id}`, a visit's id
/// (`"0"` at the stack's root), never names a route with a written key: that route is not
/// the selected one, the stack's root screen has no native bar on iOS and `logs` says
/// `navigationKey "0" matches no route` (agent-pitfalls). A root that is a choice (the
/// `rest` pattern's, which names "items" while the list is on top) is the benefit.
pub(super) fn navigation_keys(
    root: &[Attr],
    children: &[Node],
    declared: Option<Span>,
    fresh: bool,
    out: &mut Vec<LowerError>,
) {
    let Some(root_key) = attr(root, "navigationKey").filter(|_| fresh) else {
        return;
    };
    let mut routes = Vec::new();
    route_keys(children, &mut routes);
    // The not-found screen the compiler writes for an app that declares none sits at the
    // `routes` line, keyed by its entry's id: no author wrote it.
    routes.retain(|r| Some(r.span) != declared);
    match key_of(&root_key.value) {
        Key::Literal(root_literal) if !root_literal.is_empty() => {
            if let Some(entry) = routes.iter().find_map(|r| match key_of(&r.value) {
                Key::EntryId(spelled) => Some(spelled),
                _ => None,
            }) {
                out.push(LowerError {
                    id: "lower-navigation-key",
                    message: format!("the navigation root's `navigationKey` is the literal \"{root_literal}\", but its routes are keyed by their entry's id (`${{{entry}}}`): the root names the route to show, so a key that never changes never names a pushed one, and a push shows nothing. Key the root by the visit on top, `navigationKey=`${{top(nav).id}}``, and every route `navigationKey=`${{{entry}}}`` (docs/recipes/navigation-stack.md)"),
                    span: root_key.span,
                });
            }
        }
        Key::TopId => {
            for route in &routes {
                if let Key::Literal(l) = key_of(&route.value) {
                    if l.chars().all(|c| c.is_ascii_digit()) {
                        continue;
                    }
                    out.push(LowerError {
                        id: "lower-navigation-key",
                        message: format!("this route's `navigationKey` is the literal \"{l}\", but the navigation root's is `${{top(nav).id}}`, the id of the visit on top (\"0\" at the stack's root): the root never names this route, so it is never the selected one (on iOS the screen has no native bar, and `logs` says `navigationKey \"0\" matches no route`). Key every route `navigationKey=`${{e.id}}``, or give the root an expression that names this key while the route is on top (the `rest` pattern's) (docs/agent-pitfalls.md)"),
                        span: route.span,
                    });
                }
            }
        }
        _ => {}
    }
}

/// `lower-navigation-split` (LLP 1116.010 D5): `navigationSplit` (a width, or `"none"`) is
/// read from the stack's root route, the first route under the navigation root; on the
/// root itself or on a box no host reads it. recipe-finder wrote it on `main` and got the
/// default 320-point list over a blank detail pane (round 12). The `"placeholder"` box
/// inside the root route is the word's other form and is left alone.
pub(super) fn navigation_split(
    tag: &str,
    attrs: &[Attr],
    keyed: bool,
    is_root: bool,
    fresh: bool,
    out: &mut Vec<LowerError>,
) {
    let Some(a) = attr(attrs, "navigationSplit").filter(|a| a.written && fresh) else {
        return;
    };
    if matches!(&a.value, Expr::Str(s, _) if s == "placeholder") || (keyed && !is_root) {
        return;
    }
    let on = if is_root {
        "the navigation root"
    } else {
        "a box that is not a route"
    };
    out.push(LowerError {
        id: "lower-navigation-split",
        message: format!("`navigationSplit` is on {on} (this `{tag}`), where no host reads it, so the split keeps its default list width: it belongs to the stack's root route, the first element with a `navigationKey` under the navigation root, which says how wide a wide window's list column is, or \"none\" (docs/recipes/navigation-stack.md)"),
        span: a.span,
    });
}

/// A source with nothing in it that a path or a symbol has: no `/`, `.`, `:` or `?`.
fn bare_word(source: &str) -> bool {
    let mut chars = source.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && !source.ends_with(['-', '_'])
        && source
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// `lower-image-source` (LLP 1116.010 D5): an `image` whose literal source is one bare
/// word is neither a symbol (`symbol:<role>`) nor a file (it has no extension and no path),
/// so nothing is drawn: habit-tracker's `EmptyState(symbol="sparkles")` rendered a blank
/// where the icon goes, and only a screenshot showed it.
pub(super) fn image_source(positional: Option<&Expr>, fresh: bool, out: &mut Vec<LowerError>) {
    let Some(Expr::Str(source, span)) = positional.filter(|_| fresh) else {
        return;
    };
    if !bare_word(source) {
        return;
    }
    let repair = match exact_kernel::generated::symbol_role(source) {
        Some(role) => format!("this word is a symbol: write `symbol:{role}`"),
        None => "a symbol is written `symbol:<role>` (docs/recipes/symbols.md lists the roles), a file with its extension and path".to_string(),
    };
    out.push(LowerError {
        id: "lower-image-source",
        message: format!("the image source \"{source}\" is neither a symbol nor a file, so nothing is drawn where it goes; {repair} (docs/recipes/symbols.md)"),
        span: *span,
    });
}

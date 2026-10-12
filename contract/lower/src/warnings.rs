//! Lowering's warnings (@ref LLP 1116 D6): what compiles and runs but is
//! not what the author meant, where a host would drop it without a word.
//! Beside the refusals (`lint.rs`, [`crate::lower_all`]), never instead of
//! them: a warning fails no build and no test, and names its repair as a
//! refusal does.
//!
//! Each is read from the checked file's expanded root, as lowering reads
//! it, and gives what it cannot see through the benefit of the doubt, as
//! `routes.rs`'s `check_scroll` does: a `when`, `each` or `match`, a use
//! or a slot, a computed role or type.

use crate::tags::AttrTarget;
use crate::LowerError;
use contract_syntax::{
    Attr, BinOp, Binding, Component, Expr, FnDecl, Node, Span, Stmt, StyleDecl, TemplatePart, UnOp,
};
use contract_types::Checked;
use exact_kernel::StyleId;
use exact_plan::Stdlib;
use std::collections::BTreeSet;

mod counts;
mod miswiring;
mod roles;
mod screens;
mod title;
mod width;

/// At most this many warnings from one compile, before the colours that
/// stay one scheme: those name every site, each in a short line, since a
/// repair of the first twenty would only show the next (rules/RULES.md,
/// "Every check reports all failures in one run").
pub const MAX_WARNINGS: usize = 20;

/// Every warning the checked file earns, in source order of its checks.
pub fn warnings(checked: &Checked<'_>) -> Vec<LowerError> {
    let root = &checked.expanded.root;
    let mut out = Vec::new();
    // Each node, whether a navigation root (the outermost element with a
    // `navigationKey`) encloses it, and the id that root's `navigationBack`
    // names, when it is a literal: an element with a `navigationKey` inside
    // a root is a route.
    let mut stack: Vec<(&Node, bool, Option<&str>)> =
        root.view.iter().rev().map(|n| (n, false, None)).collect();
    let mut seen = BTreeSet::new();
    while let Some((node, in_root, back)) = stack.pop() {
        let (children, in_root, back): (Vec<&Node>, bool, Option<&str>) = match node {
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                ..
            } => {
                // A component used twice is inlined twice: one line for its source.
                let span = node.span();
                let fresh = seen.insert((span.source_id, span.line, span.col, span.end_col));
                // @ref LLP 1116.007 D3.1 — a symbol by another spelling than its role.
                if let (true, "image", Some(source)) = (fresh, tag.as_str(), positional.first()) {
                    out.extend(crate::symbols::aliases(source));
                }
                let button = (tag == "button")
                    .then(|| roles::unlabelled_button(attrs, children, span))
                    .flatten();
                out.extend(button.filter(|_| fresh));
                out.extend(roles::dialog_role(tag, attrs, span).filter(|_| fresh));
                out.extend(roles::level_without_heading(tag, attrs, span).filter(|_| fresh));
                let keyed = has(attrs, "navigationKey");
                // @ref LLP 1116.010 D5 — a name or a key that meets nothing.
                miswiring::swipe(attrs, children, fresh, &mut out);
                if tag == "image" {
                    miswiring::image_source(positional.first(), fresh, &mut out);
                }
                if keyed && !in_root {
                    let declared = checked.file.routes.as_ref().map(|r| r.span);
                    miswiring::navigation_keys(attrs, children, declared, fresh, &mut out);
                }
                miswiring::navigation_split(tag, attrs, keyed, keyed && !in_root, fresh, &mut out);
                let route = in_root && keyed && !has(attrs, "navigationBack");
                // A route that says it has no bar (LLP 1115.000 D5.2) draws
                // its header as written: nothing of it is the bar's to drop.
                let no_bar = literal(attrs, "navigationBar") == Some("none");
                if route && !no_bar {
                    route_header(children, &[], &mut out);
                }
                if let (true, Some(name)) = (route, back) {
                    drawn_back(attrs, children, name, &mut out);
                }
                if literal(attrs, "role") == Some("tablist") {
                    single_tab(node, &mut out);
                }
                let back = if keyed && !in_root {
                    literal(attrs, "navigationBack").filter(|name| !name.is_empty())
                } else {
                    back
                };
                (children.iter().collect(), in_root || keyed, back)
            }
            Node::When {
                then, otherwise, ..
            } => (then.iter().chain(otherwise).collect(), in_root, back),
            Node::Each { body, .. } => (body.iter().collect(), in_root, back),
            Node::Match { some, none, .. } => (some.1.iter().chain(none).collect(), in_root, back),
            Node::Use { children, .. } => (children.iter().collect(), in_root, back),
            Node::Children { .. } => (Vec::new(), in_root, back),
        };
        stack.extend(children.into_iter().rev().map(|n| (n, in_root, back)));
    }
    out.extend(screens::screen_by_state(checked));
    out.extend(counts::count_plural(checked));
    performance_now(root, &checked.file.fns, &mut out);
    width::width_padding(&root.view, &checked.file.styles, &mut out);
    out.extend(
        contract_analyze::read_after_assign(checked)
            .into_iter()
            .map(|e| LowerError {
                id: e.id,
                message: e.message,
                span: e.span,
            }),
    );
    out.truncate(MAX_WARNINGS);
    one_scheme(root, &checked.file.styles, &checked.file.fns, &mut out);
    out
}

fn has(attrs: &[Attr], name: &str) -> bool {
    attrs.iter().any(|a| a.name == name)
}

/// An attribute's value when it is a string literal (the last one written).
fn literal<'a>(attrs: &'a [Attr], name: &str) -> Option<&'a str> {
    match attrs
        .iter()
        .rev()
        .find(|a| a.name == name)
        .map(|a| &a.value)
    {
        Some(Expr::Str(v, _)) => Some(v),
        _ => None,
    }
}

/// Whether an attribute is written with a value the compiler cannot read.
fn computed(attrs: &[Attr], name: &str) -> bool {
    attrs
        .iter()
        .rev()
        .find(|a| a.name == name)
        .is_some_and(|a| !matches!(a.value, Expr::Str(..)))
}

/// The plain boxes a bar's walk looks inside (LLP 1075.003 §9.10).
const BOXES: &[&str] = &[
    "view", "box", "column", "row", "header", "nav", "section", "footer", "article", "aside",
    "main",
];

/// What the iOS bar makes of one node in a route's header.
enum Part {
    /// A `when`, `each`, `match`, use or slot, or a computed role or type:
    /// the benefit of the doubt.
    Doubt,
    /// Not shown in the header on any host: `display="none"`, or a popover
    /// or dialog, which shows over the page when it opens.
    Hidden,
    /// `text role="heading"`: the bar's title. An `aria-level` with no role
    /// is not one (LLP 1115.000 D12.5), there or to the bar.
    Heading,
    /// Another `text`: the subtitle when it is the first after the heading.
    Text,
    /// `input type="search"`: the bar's search field.
    Search,
    /// `role="tablist"`: the bar's segmented control.
    Tablist,
    /// A button that presses, or opens a popover: a bar item.
    Pressable,
    /// A plain box, walked into.
    Box,
    /// Anything else.
    Other,
}

fn part(node: &Node) -> Part {
    let Node::Element { tag, attrs, .. } = node else {
        return Part::Doubt;
    };
    // A popover or dialog shows over the page when opened, not in the bar.
    if literal(attrs, "display") == Some("none") || has(attrs, "popover") || tag == "dialog" {
        return Part::Hidden;
    }
    if computed(attrs, "role") || computed(attrs, "display") {
        return Part::Doubt;
    }
    let role = literal(attrs, "role");
    if role == Some("tablist") {
        return Part::Tablist;
    }
    let opens =
        has(attrs, "popovertarget") && literal(attrs, "popovertargetaction") != Some("hide");
    // A link navigates; it is given the benefit, as a pressed thing.
    if has(attrs, "press") || (tag == "button" && opens) || tag == "link" {
        return Part::Pressable;
    }
    match tag.as_str() {
        "text" if role == Some("heading") => Part::Heading,
        "text" => Part::Text,
        "input" if computed(attrs, "type") => Part::Doubt,
        "input" if literal(attrs, "type") == Some("search") => Part::Search,
        t if BOXES.contains(&t) => Part::Box,
        _ => Part::Other,
    }
}

fn kids(node: &Node) -> &[Node] {
    match node {
        Node::Element { children, .. } => children,
        _ => &[],
    }
}

/// Whether the subtree holds something the bar places or the compiler
/// cannot see through: then a box around it is walked, not dropped whole.
fn holds_placed(node: &Node) -> bool {
    kids(node).iter().any(|k| match part(k) {
        Part::Doubt | Part::Heading | Part::Search | Part::Tablist | Part::Pressable => true,
        Part::Box => holds_placed(k),
        Part::Hidden | Part::Text | Part::Other => false,
    })
}

fn holds_heading(node: &Node) -> bool {
    kids(node).iter().any(|k| match part(k) {
        Part::Heading => true,
        Part::Box | Part::Pressable => holds_heading(k),
        _ => false,
    })
}

/// A box with a literal fill: before the heading, the title's avatar.
fn filled(node: &Node) -> bool {
    let Node::Element { attrs, .. } = node else {
        return false;
    };
    has(attrs, "background-color") || has(attrs, "background")
}

/// A passive box of texts and symbol images, one of each at least: a
/// subtitle's line of glyphs (Signal's "🔕 Muted ⏱ 1w").
fn glyph_line(node: &Node) -> bool {
    let (mut symbol, mut text) = (false, false);
    for k in kids(node) {
        match (part(k), k) {
            (Part::Text, _) => text = true,
            (
                Part::Other,
                Node::Element {
                    tag, positional, ..
                },
            ) if tag == "image" => match positional.first() {
                // A computed source may be a symbol: the benefit.
                Some(Expr::Str(s, _)) if !s.starts_with("symbol:") => return false,
                Some(_) => symbol = true,
                None => return false,
            },
            (Part::Hidden, _) => {}
            _ => return false,
        }
    }
    symbol && text && !filled(node)
}

/// What the walk of one header has seen.
#[derive(Default)]
struct Bar<'a> {
    headings: usize,
    /// The last heading counted: the bar's title when it is the one.
    title: Option<&'a Node>,
    /// A `when`, `each`, `match`, use or slot, or a computed role: the
    /// headings counted may not be all.
    doubt: bool,
    /// The heading has been passed, in document order.
    passed: bool,
    subtitle: bool,
    avatar: bool,
    search: bool,
    tablist: bool,
    tap: bool,
    unplaced: Vec<(Span, String)>,
}

/// What iOS makes of a route whose `header` is not the bar (LLP 1115.000
/// D5.2), in `lower-header-no-bar`'s words.
const NOT_THE_BAR: &str = "the header is drawn in the page as written, and the bar has no title: a pushed route's holds the system Back button alone, above the header, and a stack's first route has no bar.";

/// The other repair `lower-header-no-bar` names: the route's own chrome.
const OR_NO_BAR: &str =
    "; or, if this screen draws its own chrome, say `navigationBar=\"none\"` on the route";

/// `lower-header-unplaced`: a node in a route's first `header` that the
/// iOS navigation bar does not place (LLP 1075.003 §9.10, LLP 1116 D6). The
/// bar takes one heading as its title, the first text after it as the
/// subtitle (a filled box before it as the avatar), one search field, one
/// tablist and the buttons that press or open a popover; the rest of the
/// header is hidden with it. With no heading or with two, the bar takes
/// nothing and the header is shown as written, so nothing is dropped; that
/// header is `lower-header-no-bar`'s (LLP 1116.003 D2.2). A `when` first
/// among the route's children is read branch by branch: a branch's first
/// node is the route's first child while it is taken, and `after` the
/// route's nodes past that `when`.
fn route_header<'a>(children: &'a [Node], after: &[&'a Node], out: &mut Vec<LowerError>) {
    if let Some(Node::When {
        then, otherwise, ..
    }) = children.first()
    {
        let rest: Vec<&Node> = children[1..].iter().chain(after.iter().copied()).collect();
        route_header(then, &rest, out);
        route_header(otherwise, &rest, out);
        return;
    }
    let Some(header @ Node::Element { tag, attrs, .. }) = children.first() else {
        return;
    };
    if tag != "header" || literal(attrs, "display") == Some("none") || computed(attrs, "display") {
        return;
    }
    let mut bar = Bar::default();
    walk(kids(header), &mut bar);
    if bar.headings != 1 {
        if !bar.doubt {
            out.push(LowerError {
                id: "lower-header-no-bar",
                message: if bar.headings == 0 {
                    format!("this route's `header` holds no heading, so on iOS it is not the navigation bar: {NOT_THE_BAR} Give it one `text role=\"heading\"` (`aria-level=1` for a large title) — its buttons become the bar's items{OR_NO_BAR} (docs/recipes/navigation-stack.md)")
                } else {
                    format!("this route's `header` holds {} headings, so on iOS it is not the navigation bar: {NOT_THE_BAR} Keep one `text role=\"heading\"` and make the others plain `text` (the first text after the heading is the bar's subtitle), or move them below the `header`{OR_NO_BAR} (docs/recipes/navigation-stack.md)", bar.headings)
                },
                span: header.span(),
            });
        }
        return;
    }
    if let Some(title) = bar.title {
        let content: Vec<&Node> = children[1..].iter().chain(after.iter().copied()).collect();
        title::title_twice(title, &content, out);
    }
    for (span, what) in bar.unplaced {
        out.push(LowerError {
            id: "lower-header-unplaced",
            message: format!(
                "{what} in this route's `header` is not shown on iOS: the navigation bar there takes one heading as its title, the first text after it as the subtitle, one `input type=\"search\"`, one `role=\"tablist\"` and the buttons that press, and hides the rest of the header. Move it below the `header`, into the route's content (the scroller's first row), or make it a button that presses (docs/contract-for-agents.md, \"Tabs and stacks\")"
            ),
            span,
        });
    }
}

fn walk<'a>(nodes: &'a [Node], bar: &mut Bar<'a>) {
    for node in nodes {
        let span = node.span();
        match part(node) {
            Part::Doubt => bar.doubt = true,
            Part::Hidden => {}
            Part::Heading => {
                bar.headings += 1;
                bar.title = Some(node);
                bar.passed = true;
            }
            Part::Search if !bar.search => bar.search = true,
            Part::Tablist if !bar.tablist => bar.tablist = true,
            Part::Search => bar
                .unplaced
                .push((span, "a second `input type=\"search\"`".into())),
            Part::Tablist => bar
                .unplaced
                .push((span, "a second `role=\"tablist\"`".into())),
            // A pressable box around the heading is the title, tapped.
            Part::Pressable if !bar.tap && has_press(node) && holds_heading(node) => {
                bar.tap = true;
                walk(kids(node), bar);
            }
            Part::Pressable => {}
            Part::Text if bar.passed && !bar.subtitle => bar.subtitle = true,
            Part::Text => bar.unplaced.push((
                span,
                if bar.passed {
                    "this text, after the subtitle,".into()
                } else {
                    "this text, before the heading,".into()
                },
            )),
            Part::Box if bar.passed && !bar.subtitle && glyph_line(node) => bar.subtitle = true,
            Part::Box if !bar.passed && !bar.avatar && filled(node) && !holds_heading(node) => {
                bar.avatar = true;
            }
            Part::Box if holds_placed(node) => walk(kids(node), bar),
            // A box with nothing in it lays the bar's items out (a spacer).
            Part::Box if kids(node).is_empty() => {}
            Part::Box => bar.unplaced.push((span, "this box".into())),
            Part::Other => {
                let what = match node {
                    Node::Element { tag, .. } => format!("this `{tag}`"),
                    _ => "this node".into(),
                };
                bar.unplaced.push((span, what));
            }
        }
    }
}

fn has_press(node: &Node) -> bool {
    matches!(node, Node::Element { attrs, .. } if has(attrs, "press"))
}

/// The `header`s that may be a route's first child: its first node, or the
/// first node of a branch of the `when`s and `match`es it opens with, each
/// of which may yield nothing and leave the next to be first.
fn first_headers<'a>(children: &'a [Node], out: &mut Vec<&'a Node>) {
    for node in children {
        match node {
            Node::When {
                then, otherwise, ..
            } => {
                first_headers(then, out);
                first_headers(otherwise, out);
            }
            Node::Match { some, none, .. } => {
                first_headers(&some.1, out);
                first_headers(none, out);
            }
            Node::Element { tag, .. } => {
                if tag == "header" {
                    out.push(node);
                }
                return;
            }
            _ => return,
        }
    }
}

/// `lower-back-twice`: a route draws its own Back control (the element
/// whose `id` the root's `navigationBack` names) where iOS shows the system
/// Back too (LLP 1115.000 D5.2). Every pushed route has the navigation bar
/// unless it says `navigationBar="none"`, and the bar's Back presses the
/// authored control (LLP 1115 D5), so a control drawn in the page is a
/// second Back on the same screen.
///
/// A control in the route's first `header` is not drawn there on iOS: when
/// the header is the bar it is the bar's Back button, and a header that is
/// no bar is `lower-header-no-bar`'s to name. A sheet (`modal`,
/// `fullscreen`) is the first route of its own stack, with no system Back.
/// A computed `navigationBar`, `navigationPresentation`, `id` or `display`
/// is given the benefit of the doubt.
fn drawn_back(route: &[Attr], children: &[Node], name: &str, out: &mut Vec<LowerError>) {
    if has(route, "navigationBar") && literal(route, "navigationBar") != Some("auto") {
        return;
    }
    if has(route, "navigationPresentation") && literal(route, "navigationPresentation") != Some("")
    {
        return;
    }
    let mut headers = Vec::new();
    first_headers(children, &mut headers);
    let mut stack: Vec<&Node> = children.iter().rev().collect();
    while let Some(node) = stack.pop() {
        if headers.iter().any(|h| std::ptr::eq(*h, node)) {
            continue;
        }
        match node {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                // Not drawn in the page: hidden, or shown over it when it
                // opens. A `display` the compiler cannot read is the benefit.
                if literal(attrs, "display") == Some("none")
                    || computed(attrs, "display")
                    || has(attrs, "popover")
                    || tag == "dialog"
                {
                    continue;
                }
                if literal(attrs, "id") == Some(name) {
                    out.push(LowerError {
                        id: "lower-back-twice",
                        message: format!("this is the route's Back control (`id=\"{name}\"`, which the root's `navigationBack` names), drawn in the page: on iOS a pushed route's navigation bar has the system Back button, which presses this one, so the screen shows two. Delete it, and the platform's Back still goes back (the bar's button, the edge swipe, the browser's Back), through the root's `navigate` or the router's own back; or move it into the route's `header` beside one `text role=\"heading\"`, where it is the bar's Back button; or, for a full-bleed screen that draws its own chrome, say `navigationBar=\"none\"` on the route (docs/recipes/navigation-stack.md)"),
                        span: node.span(),
                    });
                    continue;
                }
                stack.extend(children.iter().rev());
            }
            Node::When {
                then, otherwise, ..
            } => stack.extend(then.iter().chain(otherwise).rev()),
            Node::Each { body, .. } => stack.extend(body.iter().rev()),
            Node::Match { some, none, .. } => stack.extend(some.1.iter().chain(none).rev()),
            Node::Use { children, .. } => stack.extend(children.iter().rev()),
            Node::Children { .. } => {}
        }
    }
}

/// `lower-single-tab`: a root tab bar (a `role="tablist"` whose tabs name
/// their panels with `aria-controls`) with fewer than two tabs (LLP 1116
/// D6): a bar of one tab under a single screen, which builders copied from
/// a tab skeleton (polls, chat-rooms).
fn single_tab(tablist: &Node, out: &mut Vec<LowerError>) {
    let (mut tabs, mut controls, mut doubt) = (0, false, false);
    let mut stack: Vec<&Node> = kids(tablist).iter().collect();
    while let Some(node) = stack.pop() {
        match node {
            Node::Element {
                attrs, children, ..
            } => {
                if computed(attrs, "role") {
                    doubt = true;
                } else if literal(attrs, "role") == Some("tab") {
                    tabs += 1;
                    controls |= has(attrs, "aria-controls");
                } else {
                    stack.extend(children);
                }
            }
            _ => doubt = true,
        }
    }
    if controls && !doubt && tabs < 2 {
        out.push(LowerError {
            id: "lower-single-tab",
            message: format!(
                "this tab bar has {} tab{}: a tab bar is for moving between two or more sections, and one tab is a bar that does nothing. For a single screen, drop the `role=\"tablist\"` and the `tabpanel` around the routes, so the routes are the navigation root's children (one stack); add the other tabs if the app has them (docs/contract-for-agents.md, \"Tabs and stacks\")",
                tabs,
                if tabs == 1 { "" } else { "s" }
            ),
            span: tablist.span(),
        });
    }
}

/// What the expanded root's names may hold, as far as the clock goes.
struct Clock<'a> {
    fns: &'a [FnDecl],
    /// Names whose value may be a reading of `performanceNow()`: the
    /// milliseconds since this launch, or a number made from one.
    marks: BTreeSet<String>,
    /// Names whose value is the launch's wall-clock origin, `epochAtZero`.
    epochs: BTreeSet<String>,
}

impl Clock<'_> {
    fn epoch(&self, e: &Expr, locals: &[(String, bool)]) -> bool {
        match e {
            Expr::Member(_, field, _) => field == "epochAtZero",
            Expr::Ident(name, _) => {
                !locals.iter().any(|(n, _)| n == name) && self.epochs.contains(name)
            }
            Expr::Typed(inner, _, _) => self.epoch(inner, locals),
            _ => false,
        }
    }

    /// Whether `e` may be a launch-relative time: `performanceNow()`, a name
    /// or a computation holding one, unless `epochAtZero` is added to it
    /// (the wall-clock time) or another reading is taken from it (a
    /// duration). `depth` bounds a `fn`'s expansion.
    fn mark(&self, e: &Expr, locals: &[(String, bool)], depth: u32) -> bool {
        let m = |e: &Expr| self.mark(e, locals, depth);
        match e {
            Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => false,
            Expr::Ident(name, _) => match locals.iter().rev().find(|(n, _)| n == name) {
                Some(&(_, marked)) => marked,
                None => self.marks.contains(name),
            },
            Expr::Member(inner, field, _) => field != "epochAtZero" && m(inner),
            Expr::Call(name, args, _) => {
                if let Some(f) = self.fns.iter().find(|f| &f.name == name) {
                    if depth == 0 {
                        return args.iter().any(m);
                    }
                    let bound: Vec<(String, bool)> = f
                        .params
                        .iter()
                        .zip(args)
                        .map(|(p, a)| (p.name.clone(), m(a)))
                        .collect();
                    return self.mark(&f.body, &bound, depth - 1);
                }
                match Stdlib::from_name(name) {
                    Some(Stdlib::PerformanceNow) => true,
                    // A count, a position or a truth is no time.
                    Some(f) if f.returns() == "bool" => false,
                    Some(Stdlib::Length | Stdlib::IndexOf) => false,
                    _ => args.iter().any(m),
                }
            }
            Expr::Binary(op, a, b, _) => match op {
                BinOp::Add if self.epoch(a, locals) || self.epoch(b, locals) => false,
                BinOp::Sub if m(a) && m(b) => false,
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => m(a) || m(b),
                _ => false,
            },
            Expr::Unary(UnOp::Neg, inner, _) => m(inner),
            Expr::Unary(..) => false,
            Expr::Ternary(_, a, b, _) => m(a) || m(b),
            Expr::Match {
                subject,
                var,
                some,
                none,
                ..
            } => {
                let mut inner = locals.to_vec();
                inner.push((var.clone(), m(subject)));
                self.mark(some, &inner, depth) || m(none)
            }
            Expr::Let {
                name, value, body, ..
            } => {
                let mut inner = locals.to_vec();
                inner.push((name.clone(), m(value)));
                self.mark(body, &inner, depth)
            }
            Expr::Template(parts, _) => parts.iter().any(|p| match p {
                TemplatePart::Expr(e) => m(e),
                TemplatePart::Text(_) => false,
            }),
            Expr::List(items, _) => items.iter().any(m),
            Expr::Some(inner, _) | Expr::NamedArg(_, inner, _) | Expr::Typed(inner, _, _) => {
                m(inner)
            }
            Expr::Arrow { body, .. } => m(body),
        }
    }
}

/// Every state a statement list assigns a mark to, with its `let`s.
fn assigned(
    clock: &Clock<'_>,
    body: &[Stmt],
    locals: &mut Vec<(String, bool)>,
    out: &mut BTreeSet<String>,
) {
    let scope = locals.len();
    for s in body {
        match s {
            Stmt::Let { name, expr, .. } => {
                let marked = clock.mark(expr, locals, 4);
                locals.push((name.clone(), marked));
            }
            Stmt::Assign { target, expr, .. } => {
                if clock.mark(expr, locals, 4) {
                    out.insert(target.clone());
                }
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                assigned(clock, then, locals, out);
                assigned(clock, otherwise, locals, out);
            }
            Stmt::Match {
                subject,
                some,
                none,
                ..
            } => {
                let marked = clock.mark(subject, locals, 4);
                locals.push((some.0.clone(), marked));
                assigned(clock, &some.1, locals, out);
                locals.pop();
                assigned(clock, none, locals, out);
            }
            Stmt::Call { body, .. } => assigned(clock, body, locals, out),
            Stmt::Command { .. } | Stmt::Send { .. } | Stmt::Refresh { .. } => {}
        }
    }
    locals.truncate(scope);
}

/// Every argument a `send` gives a source that may be a mark.
fn sent(
    clock: &Clock<'_>,
    body: &[Stmt],
    locals: &mut Vec<(String, bool)>,
    out: &mut Vec<(Span, String)>,
) {
    let scope = locals.len();
    for s in body {
        match s {
            Stmt::Let { name, expr, .. } => {
                let marked = clock.mark(expr, locals, 4);
                locals.push((name.clone(), marked));
            }
            Stmt::Send { source, args, .. } => {
                for a in args.iter().filter(|a| clock.mark(a, locals, 4)) {
                    out.push((a.span(), source.clone()));
                }
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                sent(clock, then, locals, out);
                sent(clock, otherwise, locals, out);
            }
            Stmt::Match {
                subject,
                some,
                none,
                ..
            } => {
                let marked = clock.mark(subject, locals, 4);
                locals.push((some.0.clone(), marked));
                sent(clock, &some.1, locals, out);
                locals.pop();
                sent(clock, none, locals, out);
            }
            Stmt::Call { body, .. } => sent(clock, body, locals, out),
            Stmt::Assign { .. } | Stmt::Command { .. } | Stmt::Refresh { .. } => {}
        }
    }
    locals.truncate(scope);
}

/// `type-performance-now-persisted`: a value made from `performanceNow()`
/// without `epochAtZero` added, given to a source by a `send` or kept in a
/// persisted state (LLP 1116 D6, D5).
/// `performanceNow()` counts from 0 at each launch, so a start mark a
/// source keeps reads as a time before the next launch began (stopwatch:
/// laps of `0-5:0-41`). A duration, one reading less another, is kept as
/// it is.
fn performance_now(root: &contract_syntax::Component, fns: &[FnDecl], out: &mut Vec<LowerError>) {
    let mut clock = Clock {
        fns,
        marks: BTreeSet::new(),
        epochs: BTreeSet::new(),
    };
    // Settle which states and derives hold marks. Each round recomputes
    // them from the last, so a name marked before what it subtracts was
    // known (a duration, `performanceNow() - start`) is cleared again; the
    // rounds are bounded by the names.
    let names = root.states.len() + root.derives.len() + 1;
    for _ in 0..=names {
        let mut marks = BTreeSet::new();
        let mut epochs = BTreeSet::new();
        for b in root.states.iter().chain(&root.derives) {
            if clock.mark(&b.expr, &[], 4) {
                marks.insert(b.name.clone());
            }
            if clock.epoch(&b.expr, &[]) {
                epochs.insert(b.name.clone());
            }
        }
        for a in &root.actions {
            assigned(&clock, &a.body, &mut Vec::new(), &mut marks);
        }
        if marks == clock.marks && epochs == clock.epochs {
            break;
        }
        (clock.marks, clock.epochs) = (marks, epochs);
    }
    let mut found = Vec::new();
    for a in &root.actions {
        sent(&clock, &a.body, &mut Vec::new(), &mut found);
    }
    // A persisted state outlives the launch as a source's copy does (D5).
    for s in root.states.iter().filter(|s| root.persists(&s.name)) {
        if clock.marks.contains(&s.name) {
            out.push(LowerError {
                id: "type-performance-now-persisted",
                message: format!(
                    "the persisted state `{}` holds a time computed from `performanceNow()`, the milliseconds since this launch, which start again at 0 on every launch, so the kept value is wrong after a relaunch. Keep the wall-clock time, `time.epochAtZero + performanceNow()` (with `resource time = exactTime() as shape T` and `epochAtZero: number` in `T`), or a duration, `performanceNow() - start` (docs/recipes/timer-that-survives-relaunch.md)",
                    s.name
                ),
                span: s.span,
            });
        }
    }
    // An action called from another is in both bodies: once per argument.
    let mut seen = BTreeSet::new();
    for (span, source) in found {
        if !seen.insert((span.line, span.col, span.end_col)) {
            continue;
        }
        out.push(LowerError {
            id: "type-performance-now-persisted",
            message: format!(
                "this argument to `{source}` is computed from `performanceNow()`, the milliseconds since this launch, which start again at 0 on every launch, so a time the source keeps is wrong after a relaunch. Send the wall-clock time, `time.epochAtZero + performanceNow()` (with `resource time = exactTime() as shape T` and `epochAtZero: number` in `T`), or a duration, `performanceNow() - start` (docs/contract-grammar.md, `performanceNow()`)"
            ),
            span,
        });
    }
}

/// What a colour property paints, for the role its repair names.
#[derive(Clone, Copy)]
enum Paints {
    Text,
    Surface,
    Line,
    Accent,
}

impl Paints {
    /// The rows whose colour is the page's own: text, surfaces, lines and
    /// the accent. `fill`, `stroke` and a symbol's palette are artwork's,
    /// and a shadow darkens in either scheme, as UIKit's does.
    fn of(name: &str) -> Option<Paints> {
        let rows = match crate::style_names::lookup(name)? {
            AttrTarget::Styles(rows) => rows,
            AttrTarget::Shorthand => crate::shorthands::rows(name),
            _ => return None,
        };
        rows.iter().find_map(|row| match row {
            StyleId::TextColor | StyleId::CaretColor | StyleId::TextStrokeColor => {
                Some(Paints::Text)
            }
            StyleId::BackgroundColor => Some(Paints::Surface),
            StyleId::BorderColorTop
            | StyleId::BorderColorRight
            | StyleId::BorderColorBottom
            | StyleId::BorderColorLeft
            | StyleId::ColumnRuleColor => Some(Paints::Line),
            StyleId::AccentColor | StyleId::TintColor => Some(Paints::Accent),
            _ => None,
        })
    }

    fn repair(self) -> &'static str {
        match self {
            Paints::Text => "name a role (`-exact-label`, `-exact-secondary-label`, `AccentColor`) or leave it unsaid",
            Paints::Surface => "name a role (`-exact-secondary-grouped-background`, `-exact-fill`, `AccentColor`) or leave it unsaid",
            Paints::Line => "name a role (`-exact-separator`) or leave it unsaid",
            Paints::Accent => "name a role (`-exact-system-teal`) or write one `light-dark(light, dark)` pair",
        }
    }
}

/// Whether one value of a colour property is a literal colour, the same in
/// both schemes: a hex, `rgb()`, `hsl()`, a colour in its own space or a
/// named colour. `transparent` (any colour of no alpha), `currentColor`,
/// the roles (`-exact-label`, `CanvasText`, `AccentColor`), a
/// `light-dark()` pair and `-exact-platform-color()` are not.
fn literal_colour(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if let Some(open) = lower.find('(') {
        let colour = matches!(
            &lower[..open],
            "rgb" | "rgba" | "hsl" | "hsla" | "hwb" | "lab" | "lch" | "oklab" | "oklch" | "color"
        );
        if !colour {
            return false;
        }
    }
    match exact_color::parse(&lower) {
        Some(exact_color::Parsed::Legacy(c)) => c.a > 0.0,
        Some(exact_color::Parsed::Wide(c)) => c.alpha > 0.0,
        _ => false,
    }
}

/// The literal colours in one string a colour property takes: each value
/// at the top level, where white space and commas outside parentheses
/// split them (`1px solid #ddd`), so `light-dark(#fff, #000)` is one.
fn literal_colours(text: &str, out: &mut Vec<String>) {
    let (mut depth, mut start) = (0usize, None);
    let mut take = |s: &str| {
        if literal_colour(s) && !out.iter().any(|c| c == s) {
            out.push(s.to_string());
        }
    };
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if (c.is_whitespace() || c == ',') && depth == 0 => {
                if let Some(s) = start.take() {
                    take(&text[s..i]);
                }
                continue;
            }
            _ => {}
        }
        start.get_or_insert(i);
    }
    if let Some(s) = start {
        take(&text[s..]);
    }
}

/// What the view's names may hold, as far as colour goes.
struct Leaves<'a> {
    fns: &'a [FnDecl],
    derives: &'a [Binding],
}

impl Leaves<'_> {
    /// The literal colours `e` may be: its string leaves, through `?:`,
    /// `match`, a template's text, a `fn`'s body and a derive. A name the
    /// view binds, a state, a field or a call of the stdlib is given the
    /// benefit of the doubt. `depth` bounds the expansion.
    fn colours(&self, e: &Expr, locals: &[String], depth: u32, out: &mut Vec<String>) {
        match e {
            Expr::Str(text, _) => literal_colours(text, out),
            Expr::Template(parts, _) => {
                for p in parts {
                    if let TemplatePart::Text(text) = p {
                        literal_colours(text, out);
                    }
                }
            }
            Expr::Ternary(_, yes, no, _) => {
                self.colours(yes, locals, depth, out);
                self.colours(no, locals, depth, out);
            }
            Expr::Match {
                var, some, none, ..
            } => {
                let mut inner = locals.to_vec();
                inner.push(var.clone());
                self.colours(some, &inner, depth, out);
                self.colours(none, locals, depth, out);
            }
            Expr::Let { name, body, .. } => {
                let mut inner = locals.to_vec();
                inner.push(name.clone());
                self.colours(body, &inner, depth, out);
            }
            Expr::Typed(inner, _, _) | Expr::Some(inner, _) => {
                self.colours(inner, locals, depth, out)
            }
            Expr::Call(name, _, _) if depth > 0 => {
                if let Some(f) = self.fns.iter().find(|f| &f.name == name) {
                    let params: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
                    self.colours(&f.body, &params, depth - 1, out);
                }
            }
            Expr::Ident(name, _) if depth > 0 && !locals.contains(name) => {
                if let Some(d) = self.derives.iter().find(|d| &d.name == name) {
                    self.colours(&d.expr, &[], depth - 1, out);
                }
            }
            _ => {}
        }
    }
}

/// One attribute that sets a literal colour.
struct Site {
    span: Span,
    name: String,
    paints: Paints,
    colours: Vec<String>,
}

/// Every attribute of `attrs` that sets a literal colour on the page's own
/// rows, once a name.
fn colour_sites(attrs: &[Attr], leaves: &Leaves<'_>, locals: &[String], out: &mut Vec<Site>) {
    for (i, a) in attrs.iter().enumerate() {
        if attrs[i + 1..].iter().any(|b| b.name == a.name) {
            continue;
        }
        let Some(paints) = Paints::of(&a.name) else {
            continue;
        };
        let mut colours = Vec::new();
        leaves.colours(&a.value, locals, 4, &mut colours);
        if !colours.is_empty() {
            out.push(Site {
                span: a.span,
                name: a.name.clone(),
                paints,
                colours,
            });
        }
    }
}

/// `lower-colour-one-scheme`: a literal colour (a hex, `rgb()`, `hsl()`, a
/// named colour) on a colour property of the page, outside `light-dark()`
/// (LLP 1116.002 D1). It stays the same in dark mode while the platform's
/// text and controls change: round 4's light cards under white text. The
/// one authored colour is the accent, a role or one `light-dark()` pair;
/// the rest are roles or unsaid. It still fires when the app forces a
/// `color-scheme`, which is the app with no dark mode. Artwork (`fill`,
/// `stroke`, anything in an `svg`, a game's picture) is exempt.
///
/// One line a node and property, each short, so every site is named in
/// one run; with more than one, a last line says why and what the accent
/// may be.
fn one_scheme(root: &Component, styles: &[StyleDecl], fns: &[FnDecl], out: &mut Vec<LowerError>) {
    // A game (LLP 1046) reads its world through `exactSurface`: its HUD and
    // title screen are its art, over a world no scheme repaints.
    if root.resources.iter().any(|r| r.source == "exactSurface") {
        return;
    }
    let leaves = Leaves {
        fns,
        derives: &root.derives,
    };
    let mut sites = Vec::new();
    for s in styles {
        colour_sites(&s.attrs, &leaves, &[], &mut sites);
    }
    let mut stack: Vec<(&Node, Vec<String>)> =
        root.view.iter().rev().map(|n| (n, Vec::new())).collect();
    while let Some((node, locals)) = stack.pop() {
        let (children, locals): (Vec<&Node>, Vec<String>) = match node {
            Node::Element { tag, .. } if tag == "svg" => continue,
            Node::Element {
                attrs, children, ..
            } => {
                colour_sites(attrs, &leaves, &locals, &mut sites);
                (children.iter().collect(), locals)
            }
            Node::When {
                then, otherwise, ..
            } => (then.iter().chain(otherwise).collect(), locals),
            Node::Each {
                var, index, body, ..
            } => {
                let mut inner = locals;
                inner.push(var.clone());
                inner.extend(index.clone());
                (body.iter().collect(), inner)
            }
            Node::Match { some, none, .. } => {
                // The `none` arm cannot see the `some` arm's name; a name
                // bound in either is the benefit of the doubt in both.
                let mut inner = locals;
                inner.push(some.0.clone());
                (some.1.iter().chain(none).collect(), inner)
            }
            Node::Use { children, .. } => (children.iter().collect(), locals),
            Node::Children { .. } => continue,
        };
        stack.extend(children.into_iter().rev().map(|n| (n, locals.clone())));
    }
    // A component used twice is inlined twice: one line for its source.
    let mut seen = BTreeSet::new();
    sites.retain(|s| {
        seen.insert((
            s.span.source_id,
            s.span.line,
            s.span.col,
            s.span.end_col,
            s.name.clone(),
        ))
    });
    let why = "The one authored colour is the accent: a role, or one `light-dark(light, dark)` pair on `accent-color`. A forced `color-scheme` is no repair: the app then has no dark mode (docs/contract-for-agents.md, \"Colours: say a role, not a value\")";
    // `#fff` on `color` stays; `#fff`, `#eee` on `background-color` stay.
    let shown = |s: &Site| {
        let mut shown: Vec<String> = s.colours.iter().take(3).map(|c| format!("`{c}`")).collect();
        if s.colours.len() > 3 {
            shown.push("…".into());
        }
        let stay = if s.colours.len() == 1 {
            "stays"
        } else {
            "stay"
        };
        format!("{} on `{}` {stay}", shown.join(", "), s.name)
    };
    if let [s] = sites.as_slice() {
        out.push(LowerError {
            id: "lower-colour-one-scheme",
            message: format!(
                "{} the same in dark mode while the platform's text and controls change: {}. {why}",
                shown(s),
                s.paints.repair()
            ),
            span: s.span,
        });
        return;
    }
    for s in &sites {
        out.push(LowerError {
            id: "lower-colour-one-scheme",
            message: format!("{} the same in dark mode: {}", shown(s), s.paints.repair()),
            span: s.span,
        });
    }
    if let Some(first) = sites.first() {
        out.push(LowerError {
            id: "lower-colour-one-scheme",
            message: format!(
                "these {} literal colours stay the same in dark mode while the platform's text and controls change, so a light card keeps its colour under the platform's white text: name a role for each or leave it unsaid. {why}",
                sites.len()
            ),
            span: first.span,
        });
    }
}

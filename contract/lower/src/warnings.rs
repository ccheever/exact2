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

use crate::LowerError;
use contract_syntax::{Attr, BinOp, Expr, FnDecl, Node, Span, Stmt, TemplatePart, UnOp};
use contract_types::Checked;
use exact_plan::Stdlib;
use std::collections::BTreeSet;

/// At most this many warnings from one compile.
pub const MAX_WARNINGS: usize = 20;

/// Every warning the checked file earns, in source order of its checks.
pub fn warnings(checked: &Checked<'_>) -> Vec<LowerError> {
    let root = &checked.expanded.root;
    let mut out = Vec::new();
    // Each node, and whether a navigation root (the outermost element with
    // a `navigationKey`) encloses it: an element with a `navigationKey`
    // inside one is a route.
    let mut stack: Vec<(&Node, bool)> = root.view.iter().rev().map(|n| (n, false)).collect();
    while let Some((node, in_root)) = stack.pop() {
        let (children, in_root): (Vec<&Node>, bool) = match node {
            Node::Element {
                attrs, children, ..
            } => {
                let keyed = has(attrs, "navigationKey");
                if in_root && keyed && !has(attrs, "navigationBack") {
                    route_header(children, &mut out);
                }
                if literal(attrs, "role") == Some("tablist") {
                    single_tab(node, &mut out);
                }
                (children.iter().collect(), in_root || keyed)
            }
            Node::When {
                then, otherwise, ..
            } => (then.iter().chain(otherwise).collect(), in_root),
            Node::Each { body, .. } => (body.iter().collect(), in_root),
            Node::Match { some, none, .. } => (some.1.iter().chain(none).collect(), in_root),
            Node::Use { children, .. } => (children.iter().collect(), in_root),
            Node::Children { .. } => (Vec::new(), in_root),
        };
        stack.extend(children.into_iter().rev().map(|n| (n, in_root)));
    }
    performance_now(root, &checked.file.fns, &mut out);
    out.truncate(MAX_WARNINGS);
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
    /// `text role="heading"` (or with `aria-level`): the bar's title.
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
        "text" if role == Some("heading") || has(attrs, "aria-level") => Part::Heading,
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
struct Bar {
    headings: usize,
    /// The heading has been passed, in document order.
    passed: bool,
    subtitle: bool,
    avatar: bool,
    search: bool,
    tablist: bool,
    tap: bool,
    unplaced: Vec<(Span, String)>,
}

/// `lower-header-unplaced`: a node in a route's first `header` that the
/// iOS navigation bar does not place (LLP 1075.003 §9.10, LLP 1116 D6). The
/// bar takes one heading as its title, the first text after it as the
/// subtitle (a filled box before it as the avatar), one search field, one
/// tablist and the buttons that press or open a popover; the rest of the
/// header is hidden with it. With no heading or with two, the bar takes
/// nothing and the header is shown as written, so nothing is dropped.
fn route_header(children: &[Node], out: &mut Vec<LowerError>) {
    let Some(header @ Node::Element { tag, attrs, .. }) = children.first() else {
        return;
    };
    if tag != "header" || literal(attrs, "display") == Some("none") || computed(attrs, "display") {
        return;
    }
    let mut bar = Bar::default();
    walk(kids(header), &mut bar);
    if bar.headings != 1 {
        return;
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

fn walk(nodes: &[Node], bar: &mut Bar) {
    for node in nodes {
        let span = node.span();
        match part(node) {
            Part::Doubt | Part::Hidden => {}
            Part::Heading => {
                bar.headings += 1;
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

//! `lower-title-twice` (LLP 1116.005 D6): a heading in a route's content
//! that says what its `header`'s heading says. On iOS that header is the
//! navigation bar and its heading the bar's title, so the title shows
//! twice, once in the bar and once in the page; on the web the content
//! heading sits under the header that says it. Trivia, in rounds 6 and 7,
//! put "Trivia Quiz" in both (round 5's said "Timed Trivia" under it).

use super::{computed, has, literal};
use crate::LowerError;
use contract_syntax::{Expr, Node, Span, TemplatePart, VisitSpans};

/// What a heading says: its words when they are written, else its
/// expression with its places dropped, so the same name twice is equal.
#[derive(PartialEq)]
enum Says {
    Words(String),
    Bound(Expr),
}

/// A `text` heading's words, from its first positional argument.
fn says(node: &Node) -> Option<Says> {
    let Node::Element { positional, .. } = node else {
        return None;
    };
    let words = |s: &str| Says::Words(s.split_whitespace().collect::<Vec<_>>().join(" "));
    match positional.first()? {
        Expr::Str(s, _) => Some(words(s)),
        Expr::Template(parts, _) if parts.iter().all(|p| matches!(p, TemplatePart::Text(_))) => {
            let text: String = parts
                .iter()
                .map(|p| match p {
                    TemplatePart::Text(t) => t.as_str(),
                    TemplatePart::Expr(_) => "",
                })
                .collect();
            Some(words(&text))
        }
        e => {
            let mut e = e.clone();
            e.visit_spans(&mut |s: &mut Span| *s = Span::default());
            Some(Says::Bound(e))
        }
    }
}

/// A heading is a `text` whose role says so; an `aria-level` alone is not
/// one (LLP 1115.000 D12.5, `lower-level-without-heading`).
fn heading(tag: &str, attrs: &[contract_syntax::Attr]) -> bool {
    tag == "text" && literal(attrs, "role") == Some("heading")
}

/// One walk of a route's content for headings that say its title.
struct Walk<'a> {
    title: &'a Says,
    out: Vec<Span>,
}

impl Walk<'_> {
    /// A heading of `nodes` that says the title and is the first heading
    /// the page shows under the bar. Past one that is always shown before
    /// it, or inside a `section`, it is a section's own name, not the
    /// screen's. An `if`, `for` or `match` may show nothing, so a heading
    /// under one is passed only within it. What the page never shows with
    /// the bar is skipped: a hidden node, a popover or dialog (over the
    /// page when opened) and another route. Under a `for` or `match`, a
    /// bound expression may name another value: the benefit of the doubt.
    fn walk(&mut self, nodes: &[Node], bound: bool, section: bool, passed: &mut bool) {
        for node in nodes {
            match node {
                Node::Element {
                    tag,
                    attrs,
                    children,
                    ..
                } => {
                    if literal(attrs, "display") == Some("none")
                        || computed(attrs, "display")
                        || has(attrs, "popover")
                        || has(attrs, "navigationKey")
                        || tag == "dialog"
                    {
                        continue;
                    }
                    if heading(tag, attrs) {
                        let same = says(node).is_some_and(|s| {
                            &s == self.title && (matches!(s, Says::Words(_)) || !bound)
                        });
                        if same && !*passed && !section {
                            self.out.push(node.span());
                        }
                        *passed = true;
                    }
                    self.walk(children, bound, section || tag == "section", passed);
                }
                Node::When {
                    then, otherwise, ..
                } => {
                    for arm in [then, otherwise] {
                        self.walk(arm, bound, section, &mut passed.clone());
                    }
                }
                Node::Each { body, .. } => self.walk(body, true, section, &mut passed.clone()),
                Node::Match { some, none, .. } => {
                    for arm in [&some.1, none] {
                        self.walk(arm, true, section, &mut passed.clone());
                    }
                }
                Node::Use { children, .. } => self.walk(children, bound, section, passed),
                Node::Children { .. } => {}
            }
        }
    }
}

/// `lower-title-twice`: `title` is the heading of a route's `header` that
/// is its iOS bar (one heading, by `route_header`'s rule), `content` the
/// route's nodes after that header.
pub(super) fn title_twice(title: &Node, content: &[&Node], out: &mut Vec<LowerError>) {
    let Some(says_title) = says(title) else {
        return;
    };
    let large = match title {
        Node::Element { attrs, .. } => {
            matches!(
                attrs.iter().rev().find(|a| a.name == "aria-level").map(|a| &a.value),
                Some(Expr::Number(n, _)) if *n == 1.0
            ) || literal(attrs, "aria-level") == Some("1")
        }
        _ => false,
    };
    let words = match &says_title {
        Says::Words(w) => format!("\"{w}\""),
        Says::Bound(_) => "the same value".into(),
    };
    let bar = if large {
        "once large in the navigation bar"
    } else {
        "once in the navigation bar"
    };
    let mut walk = Walk {
        title: &says_title,
        out: Vec::new(),
    };
    let mut passed = false;
    for node in content {
        walk.walk(std::slice::from_ref(*node), false, false, &mut passed);
    }
    for span in walk.out {
        out.push(LowerError {
            id: "lower-title-twice",
            message: format!(
                "this heading says {words}, as the route's `header` heading does, so the title shows twice: on iOS {bar} and once in the page, and on the web just under the header that says it. Drop this heading, or make it say something else (a section's own name, \"Results\") — the bar's title already names the screen (docs/recipes/navigation-stack.md)"
            ),
            span,
        });
    }
}

//! Paragraph ownership after component expansion (LLP 1126 D6/D7).
use super::{refusal, CompileError};
use contract_syntax::{Expr, File, Node};

pub(super) fn check(file: &File) -> Result<(), Vec<CompileError>> {
    let (expanded, _) = contract_syntax::expand_checked(file, false);
    let mut errors = Vec::new();
    // Expansion errors belong to the ordinary compiler. Its surviving tree
    // still exposes every independent paragraph refusal.
    super::visit_nodes(&expanded.root.view, &mut |node| {
        if matches!(node, Node::Element { tag, .. } if tag == "text" || tag == "time") {
            inline(node, false, &mut errors);
        }
    });
    errors.sort_by_key(|e| (e.span.source_id, e.span.line, e.span.col, e.id.clone()));
    errors.dedup_by(|a, b| a.span == b.span && a.id == b.id);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn own_run(node: &Node) -> bool {
    matches!(node, Node::Element { tag, positional, attrs, .. }
        if tag == "time" || !positional.is_empty() || attrs.iter().any(|a| a.name == "text"))
}

// Maximum simultaneous runs, capped at two. Opposite branches do not share
// a paragraph with each other; iterations of the same region can.
fn count(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            Node::Element { children, .. } => usize::from(own_run(node)) + count(children),
            Node::When {
                then, otherwise, ..
            } => count(then).max(count(otherwise)),
            Node::Match { some, none, .. } => count(&some.1).max(count(none)),
            Node::Each { body, .. } if count(body) > 0 => 2,
            _ => 0,
        })
        .sum::<usize>()
        .min(2)
}

fn children(nodes: &[Node], outside: bool, errors: &mut Vec<CompileError>) {
    for (i, node) in nodes.iter().enumerate() {
        let mixed = outside || count(&nodes[..i]) > 0 || count(&nodes[i + 1..]) > 0;
        inline(node, mixed, errors);
    }
}

fn live(expr: &Expr) -> bool {
    match expr {
        Expr::Str(value, _) => matches!(value.as_str(), "relative" | "timer" | "countdown"),
        Expr::Ternary(_, yes, no, _) => live(yes) || live(no),
        // An unknown format can choose a live presentation at runtime.
        _ => true,
    }
}

fn inline(node: &Node, outside: bool, errors: &mut Vec<CompileError>) {
    match node {
        Node::Element {
            tag,
            attrs,
            children: nested,
            span,
            ..
        } => {
            let mixed = outside || count(nested) > 0;
            if mixed && tag == "time" && attrs.iter().any(|a| a.name == "format" && live(&a.value))
            {
                errors.push(refusal(
                    "snapshot-inline-live-time",
                    "make the live time its own text, so the widget can reserve its width",
                    *span,
                ));
            }
            if outside && (tag == "link" || tag == "a" || attrs.iter().any(|a| a.name == "href")) {
                errors.push(refusal(
                    "snapshot-inline-link",
                    "make the link its own element",
                    *span,
                ));
            }
            children(nested, outside || own_run(node), errors);
        }
        Node::When {
            then, otherwise, ..
        } => {
            children(then, outside, errors);
            children(otherwise, outside, errors);
        }
        Node::Match { some, none, .. } => {
            children(&some.1, outside, errors);
            children(none, outside, errors);
        }
        Node::Each { body, .. } => children(body, outside || count(body) > 0, errors),
        _ => {}
    }
}

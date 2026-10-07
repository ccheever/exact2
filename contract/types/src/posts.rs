//! `postMessage(message, target)` names a surface by literal: the app must
//! declare a canvas with `surface=target(...)`, or the post is refused here
//! rather than dropped by a host at run time.
use super::{Sink, TypeError};
use contract_syntax::{Expr, File, Node, Span, Stmt};
use std::collections::BTreeSet;

/// A call refused as ambiguous (`ambiguous`, LLP 1089 D1) is not checked.
pub(super) fn check_targets(file: &File, ambiguous: &BTreeSet<Span>, sink: &mut Sink) {
    let mut surfaces = BTreeSet::new();
    for c in &file.components {
        view(&c.view, &mut surfaces);
    }
    for c in &file.components {
        for a in &c.actions {
            stmts(&a.body, &surfaces, ambiguous, sink);
        }
    }
}
fn view<'a>(nodes: &'a [Node], out: &mut BTreeSet<&'a str>) {
    for n in nodes {
        match n {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                for a in attrs
                    .iter()
                    .filter(|a| a.name == "surface" && tag == "canvas")
                {
                    if let Expr::Ident(name, _) | Expr::Call(name, _, _) = &a.value {
                        out.insert(name);
                    }
                }
                view(children, out);
            }
            Node::Use { children, .. } => view(children, out),
            Node::When {
                then, otherwise, ..
            } => {
                view(then, out);
                view(otherwise, out);
            }
            Node::Each { body, .. } => view(body, out),
            Node::Match { some, none, .. } => {
                view(&some.1, out);
                view(none, out);
            }
            Node::Children { .. } => {}
        }
    }
}
fn stmts(body: &[Stmt], surfaces: &BTreeSet<&str>, ambiguous: &BTreeSet<Span>, sink: &mut Sink) {
    for s in body {
        match s {
            Stmt::Command { name, args, span }
                if name == "postMessage" && !ambiguous.contains(span) =>
            {
                let Some(Expr::Str(target, span)) = args.get(1) else {
                    continue;
                };
                if !surfaces.contains(target.as_str()) {
                    let known = if surfaces.is_empty() {
                        "this app declares no canvas surface".to_string()
                    } else {
                        let names: Vec<_> = surfaces.iter().map(|s| format!("`{s}`")).collect();
                        format!("its canvases declare {}", names.join(", "))
                    };
                    sink.push(TypeError {
                        id: "type-post-message",
                        message: format!(
                            "`postMessage(message, \"{target}\")` names no surface: {known} (`canvas surface={target}()`)"
                        ),
                        span: *span,
                    });
                }
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                stmts(then, surfaces, ambiguous, sink);
                stmts(otherwise, surfaces, ambiguous, sink);
            }
            Stmt::Match { some, none, .. } => {
                stmts(&some.1, surfaces, ambiguous, sink);
                stmts(none, surfaces, ambiguous, sink);
            }
            _ => {}
        }
    }
}

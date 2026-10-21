//! Posts name a literal canvas surface or a live iframe id. Canvas-only apps
//! retain literal target checking; repeated iframe instances may compute ids.
use super::{Sink, TypeError};
use contract_syntax::{Expr, File, Node, Span, Stmt};
use std::collections::BTreeSet;

#[derive(Default)]
struct Targets<'a> {
    canvases: BTreeSet<&'a str>,
    frames: BTreeSet<&'a str>,
    dynamic_frames: bool,
    has_frames: bool,
}

/// A call refused as ambiguous (`ambiguous`, LLP 1089 D1) is not checked.
pub(super) fn check_targets(file: &File, ambiguous: &BTreeSet<Span>, sink: &mut Sink) {
    let mut targets = Targets::default();
    for c in &file.components {
        view(&c.view, &mut targets);
    }
    for c in &file.components {
        for a in &c.actions {
            stmts(&a.body, &targets, ambiguous, sink);
        }
    }
}
fn view<'a>(nodes: &'a [Node], out: &mut Targets<'a>) {
    for n in nodes {
        match n {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                if tag == "iframe" {
                    out.has_frames = true;
                    for a in attrs.iter().filter(|a| a.name == "id") {
                        if let Expr::Str(id, _) = &a.value {
                            out.frames.insert(id);
                        } else {
                            out.dynamic_frames = true;
                        }
                    }
                }
                for a in attrs
                    .iter()
                    .filter(|a| a.name == "surface" && tag == "canvas")
                {
                    if let Expr::Ident(name, _) | Expr::Call(name, _, _) = &a.value {
                        out.canvases.insert(name);
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
fn stmts(body: &[Stmt], targets: &Targets<'_>, ambiguous: &BTreeSet<Span>, sink: &mut Sink) {
    for s in body {
        match s {
            Stmt::Command { name, args, span }
                if name == "postMessage" && !ambiguous.contains(span) =>
            {
                let Some(Expr::Str(target, target_span)) = args.get(1) else {
                    if !targets.has_frames {
                        sink.push(TypeError {
                            id: "type-post-message",
                            message: "a canvas target is named literally; string expressions address iframe ids".into(),
                            span: *span,
                        });
                    }
                    continue;
                };
                let canvas = targets.canvases.contains(target.as_str());
                let frame = targets.frames.contains(target.as_str());
                let message = if canvas && frame {
                    Some(format!("postMessage target `{target}` is ambiguous: both a canvas surface and an iframe id"))
                } else if !canvas && !frame && !targets.dynamic_frames {
                    Some(format!(
                        "`postMessage(message, \"{target}\")` names no canvas surface or iframe id"
                    ))
                } else {
                    None
                };
                if let Some(message) = message {
                    sink.push(TypeError {
                        id: "type-post-message",
                        message,
                        span: *target_span,
                    });
                }
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                stmts(then, targets, ambiguous, sink);
                stmts(otherwise, targets, ambiguous, sink);
            }
            Stmt::Match { some, none, .. } => {
                stmts(&some.1, targets, ambiguous, sink);
                stmts(none, targets, ambiguous, sink);
            }
            _ => {}
        }
    }
}

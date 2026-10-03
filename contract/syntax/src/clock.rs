//! `animation-timeline=Name` naming a `timeline` declaration (LLP 1055.002
//! D2): the attribute becomes the row's text, `"clock(Name)"`, before types
//! and lowering see it, so neither knows clock timelines exist.

use crate::ast::{Attr, Expr, File, Node};
use crate::parser::SyntaxError;
use std::collections::HashSet;

/// Rewrite every `animation-timeline` (or `animationTimeline`) whose value
/// is a bare name a `timeline` declares into `"clock(Name)"`, on view
/// elements and in `style` declarations. On an element any other value is
/// left as written (a bare name there may be a binding); in a style, where
/// only a timeline's name is admitted, one no `timeline` declares is refused.
pub fn resolve_clock_timelines(file: &mut File) -> Result<(), SyntaxError> {
    let names: HashSet<String> = file.timelines.iter().map(|t| t.name.clone()).collect();
    for style in &mut file.styles {
        attrs(&names, &mut style.attrs);
        if let Some(a) = style
            .attrs
            .iter()
            .find(|a| matches!(a.value, Expr::Ident(..)))
        {
            let Expr::Ident(name, _) = &a.value else {
                unreachable!()
            };
            return Err(SyntaxError {
                id: "contract-timeline-unknown",
                message: format!(
                    "`{}={name}` in `style {}`: no `timeline {name}` is declared or used",
                    a.name, style.name
                ),
                span: a.span,
            });
        }
    }
    if names.is_empty() {
        return Ok(());
    }
    for component in &mut file.components {
        nodes(&names, &mut component.view);
    }
    Ok(())
}

fn attrs(names: &HashSet<String>, attrs: &mut [Attr]) {
    for a in attrs {
        if a.name != "animation-timeline" && a.name != "animationTimeline" {
            continue;
        }
        if let Expr::Ident(name, span) = &a.value {
            if names.contains(name) {
                a.value = Expr::Str(format!("clock({name})"), *span);
            }
        }
    }
}

fn nodes(names: &HashSet<String>, list: &mut [Node]) {
    for node in list {
        match node {
            Node::Element {
                attrs: a, children, ..
            } => {
                attrs(names, a);
                nodes(names, children);
            }
            Node::Use { children, .. } => nodes(names, children),
            Node::Children { .. } => {}
            Node::When {
                then, otherwise, ..
            } => {
                nodes(names, then);
                nodes(names, otherwise);
            }
            Node::Each { body, .. } => nodes(names, body),
            Node::Match { some, none, .. } => {
                nodes(names, &mut some.1);
                nodes(names, none);
            }
        }
    }
}

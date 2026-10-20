//! A component's anatomy (LLP 1139 D2, stage 12 C): the parts it names and
//! the row groups each takes. [`Anatomy`] is the checked data, whatever its
//! spelling; the compiler's checks (`contract_lower::anatomy`) read only it.
//!
//! **The spelling is this layer's alone** (LLP 1139 §9 Q7 is open): r6's is
//! a `parts` section, `name: group group …` a line (`parser/anatomy.rs`),
//! and `part="name"` on a node of the component's own view, which
//! [`take_marks`] lifts out of the view as the component is parsed, so no
//! later pass sees the attribute (nothing reaches a plan, the DOM or a
//! host). A Q7 answer that writes the groups beside `part=` changes this
//! file and the parser's section, never the model or the checks.

use crate::ast::{Expr, Node};
use crate::Span;

/// What a component says of its parts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Anatomy {
    /// Each part declared, with its groups, in order.
    pub parts: Vec<PartDecl>,
    /// Each node named as a part, in view order.
    pub marks: Vec<Mark>,
}

/// One declared part.
#[derive(Debug, Clone, PartialEq)]
pub struct PartDecl {
    /// Its name.
    pub name: String,
    /// The row groups it takes, each with where it is written.
    pub groups: Vec<(String, Span)>,
    /// Where it is declared.
    pub span: Span,
}

/// A node named as a part.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    /// The part it names; empty when the name isn't a string literal.
    pub part: String,
    /// Where the name is written.
    pub span: Span,
    /// The node's own span: what finds it in the component's view.
    pub node: Span,
    /// Whether the node is of the component's own view, not inside a used
    /// component's slot fill (which is the used component's content).
    pub own: bool,
}

impl Anatomy {
    /// Whether the component says anything of parts.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty() && self.marks.is_empty()
    }
}

/// The attribute a node is named as a part by (r6's spelling).
pub const PART: &str = "part";

/// How a part is declared, as a diagnostic quotes it (r6's spelling): the
/// section's name.
pub const SECTION: &str = "parts";

/// How part `name` is declared with `groups`, as a diagnostic quotes it
/// (r6's spelling): `go: control`.
pub fn declared(name: &str, groups: &[&str]) -> String {
    format!("{name}: {}", groups.join(" "))
}

/// How a node is named as part `name`, as a diagnostic quotes it (r6's
/// spelling): `part="surface"`.
pub fn written(name: &str) -> String {
    format!("{PART}=\"{name}\"")
}

/// Lift every `part=` out of `view` into marks: a node of the view's own,
/// or of a used component's slot fill (`own: false`, refused later).
pub fn take_marks(view: &mut [Node]) -> Vec<Mark> {
    let mut marks = Vec::new();
    walk(view, true, &mut marks);
    marks
}

fn walk(nodes: &mut [Node], own: bool, marks: &mut Vec<Mark>) {
    for node in nodes {
        match node {
            Node::Element {
                attrs,
                children,
                span,
                ..
            } => {
                let node_span = *span;
                attrs.retain(|a| {
                    if a.name != PART {
                        return true;
                    }
                    let part = match &a.value {
                        Expr::Str(s, _) => s.clone(),
                        _ => String::new(),
                    };
                    marks.push(Mark {
                        part,
                        span: a.span,
                        node: node_span,
                        own,
                    });
                    false
                });
                walk(children, own, marks);
            }
            Node::Use { children, .. } => walk(children, false, marks),
            Node::Children { .. } => {}
            Node::When {
                then, otherwise, ..
            } => {
                walk(then, own, marks);
                walk(otherwise, own, marks);
            }
            Node::Each { body, .. } => walk(body, own, marks),
            Node::Match { some, none, .. } => {
                walk(&mut some.1, own, marks);
                walk(none, own, marks);
            }
        }
    }
}

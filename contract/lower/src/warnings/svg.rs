use super::literal;
use crate::LowerError;
use contract_syntax::{Expr, Node, Span};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn use_foreign_object(view: &[Node], out: &mut Vec<LowerError>) {
    let mut refs = References::default();
    refs.collect(view, false, false);
    if refs.dynamic_id {
        return;
    }
    let mut seen = BTreeSet::new();
    for (id, span) in refs.uses {
        if refs.targets.get(id) == Some(&Some(true)) && seen.insert(span) {
            out.push(LowerError {
                id: "lower-svg-use-foreign-object",
                message: format!(
                    "the `foreignObject` in `#{id}` draws nothing through SVG `use`. Place it directly in the SVG for the web; for content on every host, position an ordinary box over the SVG"
                ),
                span,
            });
        }
    }
}

#[derive(Default)]
struct References<'a> {
    // Only unique, unconditional SVG ids resolve. Other ids still count as
    // collisions; a dynamic id can collide with any literal target.
    targets: BTreeMap<&'a str, Option<bool>>,
    dynamic_id: bool,
    uses: Vec<(&'a str, Span)>,
}

impl<'a> References<'a> {
    /// Record whether each target contains a foreignObject in one tree walk.
    /// References inside a target are not followed.
    fn collect(&mut self, nodes: &'a [Node], svg: bool, conditional: bool) -> bool {
        let mut contains = false;
        for node in nodes {
            contains |= match node {
                Node::Element {
                    tag,
                    attrs,
                    children,
                    span,
                    ..
                } => {
                    let svg = svg || tag == "svg";
                    let foreign = svg && tag == "foreignObject";
                    let child_foreign = self.collect(children, svg && !foreign, conditional);
                    let contains = foreign || child_foreign;
                    if let Some(id) = attrs.iter().rev().find(|a| a.name == "id") {
                        if let Expr::Str(id, _) = &id.value {
                            self.targets
                                .entry(id)
                                .and_modify(|target| *target = None)
                                .or_insert((svg && !conditional).then_some(contains));
                        } else {
                            self.dynamic_id = true;
                        }
                    }
                    if svg && tag == "use" {
                        if let Some(id) = literal(attrs, "href").and_then(|s| s.strip_prefix('#')) {
                            self.uses.push((id, *span));
                        }
                    }
                    contains
                }
                Node::When {
                    then, otherwise, ..
                } => {
                    let yes = self.collect(then, svg, true);
                    self.collect(otherwise, svg, true) || yes
                }
                Node::Each { body, .. } => self.collect(body, svg, true),
                Node::Match { some, none, .. } => {
                    let yes = self.collect(&some.1, svg, true);
                    self.collect(none, svg, true) || yes
                }
                Node::Use { children, .. } => self.collect(children, svg, true),
                Node::Children { .. } => false,
            };
        }
        contains
    }
}

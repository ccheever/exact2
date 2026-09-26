//! Conservative initial flow shape for opt-in variable-height collections.
use super::{err, values::numeric_literal, LowerError, Lowerer};
use contract_syntax::{Attr, Expr, Node, Span};

impl Lowerer<'_> {
    pub(super) fn check_collection(
        &self,
        tag: &str,
        attrs: &[Attr],
        children: &[Node],
        span: Span,
    ) -> Result<(), LowerError> {
        let Some(opt) = attrs.iter().find(|a| a.name == "virtualized") else {
            return Ok(());
        };
        if tag != "list" || !matches!(opt.value, Expr::Bool(..)) {
            return err(
                "lower-collection-opt-in",
                "`virtualized` requires a literal boolean on `list`",
                opt.span,
            );
        }
        if matches!(opt.value, Expr::Bool(false, _)) {
            return Ok(());
        }
        // Computed bounds are checked by the bake's measured-layout lint.
        // Merely spelling `auto` or a non-growing flex is not a bound.
        if !attrs.iter().any(|a| match a.name.as_str() {
            "height" | "max-height" => !matches!(&a.value, Expr::Str(s, _) if s == "auto"),
            "flex" => numeric_literal(&a.value).is_none_or(|n| n > 0.0),
            _ => false,
        }) {
            return err("lower-collection-unbounded", "a virtualized list needs height, max-height, or flex constraining its vertical scrollport", span);
        }
        self.collection_flow(attrs, true)?;
        let [Node::Each { body, .. }] = children else {
            return err(
                "lower-collection-template",
                "a virtualized list initially supports exactly one direct each",
                span,
            );
        };
        self.reject_nested_collections(body)?;
        let [Node::Element { tag, attrs, .. }] = body.as_slice() else {
            return err("lower-collection-template", "each in a virtualized list needs exactly one element flow root (wrap conditional or multiple roots in column)", span);
        };
        if tag == "dialog" {
            return err(
                "lower-collection-flow",
                "a virtual row root must participate in normal flow",
                span,
            );
        }
        // An unknown class is the node's own refusal; here it has no rows.
        let mut expanded = attrs.clone();
        if let Some((_, rows)) = self.class_rows(attrs).ok().flatten() {
            expanded.extend(rows);
        }
        self.collection_flow(&expanded, false)
    }

    // Component uses and slots have already expanded before lowering. Inspect
    // all arms, even currently inactive ones: an inner focus pin cannot keep
    // an offscreen ancestor row alive under the initial lifetime policy.
    fn reject_nested_collections(&self, nodes: &[Node]) -> Result<(), LowerError> {
        for node in nodes {
            match node {
                Node::Element {
                    attrs, children, ..
                } => {
                    // `virtualized` is a prop, which no `style` may hold.
                    let opt = attrs.iter().find(|a| a.name == "virtualized");
                    if let Some(opt) = opt.filter(|a| matches!(a.value, Expr::Bool(true, _))) {
                        return err("lower-collection-nested", "a virtualized list cannot occur inside another virtualized list's row template until bounded ancestor-row lifetime is supported; use an ordinary container or an eager outer list", opt.span);
                    }
                    self.reject_nested_collections(children)?;
                }
                Node::When {
                    then, otherwise, ..
                } => {
                    self.reject_nested_collections(then)?;
                    self.reject_nested_collections(otherwise)?;
                }
                Node::Match { some, none, .. } => {
                    self.reject_nested_collections(&some.1)?;
                    self.reject_nested_collections(none)?;
                }
                Node::Each { body, .. } | Node::Provide { body, .. } => {
                    self.reject_nested_collections(body)?;
                }
                Node::Use { children, .. } => self.reject_nested_collections(children)?,
                Node::Children { .. } => {}
            }
        }
        Ok(())
    }

    fn collection_flow(&self, attrs: &[Attr], container: bool) -> Result<(), LowerError> {
        for a in attrs {
            if container
                && matches!(
                    a.name.as_str(),
                    "padding" | "padding-top" | "padding-bottom"
                )
                && numeric_literal(&a.value) != Some(0.0)
            {
                return err("lower-collection-flow", format!("`{}` on a virtualized list container requires literal zero; put top/end spacing inside measured rows until a collection inset policy is supported (padding-left and padding-right remain allowed)", a.name), a.span);
            }
            let allowed = match a.name.as_str() {
                "position" => matches!(&a.value, Expr::Str(s, _) if s == "relative"),
                "top" | "bottom" | "left" | "right" | "rotate" => {
                    numeric_literal(&a.value) == Some(0.0)
                }
                "scale" => numeric_literal(&a.value) == Some(1.0),
                "margin" | "margin-top" | "margin-bottom" => {
                    numeric_literal(&a.value).is_some_and(|v| v >= 0.0)
                }
                "display" if container => matches!(&a.value, Expr::Str(s, _) if s == "block"),
                "display" => !matches!(&a.value, Expr::Str(s, _) if s == "contents"),
                "overflow" | "overflow-y" if container => {
                    matches!(&a.value, Expr::Str(s, _) if s == "scroll" || s == "auto")
                }
                "overflow-x" if container => matches!(&a.value, Expr::Str(s, _) if s == "hidden"),
                "gap" | "row-gap" | "column-gap" if container => {
                    numeric_literal(&a.value) == Some(0.0)
                }
                "flex-direction" | "flex-wrap" | "grid-template-rows" | "grid-template-columns"
                    if container =>
                {
                    false
                }
                _ => true,
            };
            if !allowed {
                return err("lower-collection-flow", format!("`{}` is not supported on this virtual collection flow root; absolute/overlapping rows and alternate container layouts are not windowed", a.name), a.span);
            }
        }
        Ok(())
    }
}

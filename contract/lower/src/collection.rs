//! Conservative initial flow shape for opt-in variable-size collections: a
//! block list scrolls vertically; a flex row scrolls horizontally, CSS's
//! carousel (LLP 1070 H2).
use super::{err, values::numeric_literal, LowerError, Lowerer};
use contract_syntax::{Attr, Expr, Node, Span};

fn literal<'a>(attrs: &'a [Attr], name: &str) -> Option<&'a Attr> {
    attrs.iter().find(|a| a.name == name)
}
fn string(a: &Attr) -> Option<&str> {
    match &a.value {
        Expr::Str(s, _) => Some(s),
        _ => None,
    }
}

/// Whether a virtualized list's attributes make it a row list: `display:
/// flex`, whose `flex-direction` is CSS's default `row`. Only literals
/// count; the collection's axis is fixed when it is created.
pub(super) fn row_list(attrs: &[Attr]) -> bool {
    literal(attrs, "display").and_then(string) == Some("flex")
}

impl Lowerer<'_> {
    pub(super) fn check_collection(
        &self,
        tag: &str,
        attrs: &[Attr],
        children: &[Node],
        span: Span,
    ) -> Result<(), LowerError> {
        // Reorder is a collection's (LLP 1010 §6, LLP 1043.000): every host
        // drags only a virtualized list's measured rows, so anywhere else
        // `reorderdrop` would never fire, on any host (the weather diary's
        // F1 met it as the web build's refusal).
        let virtualized = tag == "list"
            && attrs
                .iter()
                .any(|a| a.name == "virtualized" && matches!(a.value, Expr::Bool(true, _)));
        if let Some(reorder) = attrs.iter().find(|a| a.name == "reorderdrop") {
            if !virtualized {
                return err("lower-reorder-collection", format!("`reorderdrop` reorders a virtualized list's rows, and every host drags only those: write `list virtualized=true` with one `each`, and give each row a handle with `reorderFor` naming the list's `id`; on {} it would never fire", if tag == "list" { "a list that is not virtualized".to_string() } else { format!("`{tag}`") }), reorder.span);
            }
        }
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
        let row = row_list(attrs);
        self.collection_axis(attrs, row, span)?;
        // Computed bounds are checked by the bake's measured-layout lint.
        // Merely spelling `auto` or a non-growing flex is not a bound.
        if !row
            && !attrs.iter().any(|a| match a.name.as_str() {
                "height" | "max-height" => !matches!(&a.value, Expr::Str(s, _) if s == "auto"),
                "flex" => numeric_literal(&a.value).is_none_or(|n| n > 0.0),
                _ => false,
            })
        {
            return err("lower-collection-unbounded", "a virtualized list needs height, max-height, or flex constraining its vertical scrollport", span);
        }
        self.collection_flow(attrs, Some(row))?;
        let [Node::Each { body, .. }] = children else {
            return err(
                "lower-collection-template",
                "a virtualized list initially supports exactly one direct each",
                span,
            );
        };
        self.check_nested(body, false)?;
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
        self.collection_flow(&expanded, None)
    }

    /// The rules that make a list's axis CSS's and keep its index's starts
    /// prefix sums (LLP 1070 H2, §8): a row list is a literal-height flex row,
    /// left to right, that neither wraps, reverses nor spreads its items.
    fn collection_axis(&self, attrs: &[Attr], row: bool, span: Span) -> Result<(), LowerError> {
        if let Some(display) = literal(attrs, "display") {
            if !matches!(string(display), Some("block" | "flex")) {
                return err("lower-collection-flow", "a virtualized list's `display` is a literal `block` (it scrolls vertically) or `flex` (a row that scrolls horizontally)", display.span);
            }
        }
        if let Some(direction) = literal(attrs, "flex-direction") {
            if !row {
                return err("lower-collection-flow", "`flex-direction` needs `display=\"flex\"` on a virtualized list; CSS ignores it on a block, and the list would silently scroll vertically", direction.span);
            }
            if string(direction) != Some("row") {
                return err("lower-collection-flow", "a virtualized flex list is a `flex-direction: row`; a reversed row is an inverted list, and a column is `display: block`", direction.span);
            }
        }
        let [own, other] = if row {
            ["estimated-item-width", "estimated-item-height"]
        } else {
            ["estimated-item-height", "estimated-item-width"]
        };
        if let Some(estimate) = literal(attrs, other) {
            return err("lower-collection-estimate", format!("`{other}` estimates the other axis; this list scrolls {}, so its estimate is `{own}`", if row { "horizontally" } else { "vertically" }), estimate.span);
        }
        if let Some(restoration) = literal(attrs, "scroll-restoration") {
            if !matches!(string(restoration), Some("auto" | "manual")) {
                return err("lower-attr-value", "`scroll-restoration` is `auto` (a nested list keeps where its reader left it) or `manual` (the app does)", restoration.span);
            }
        }
        if let Some(start) = literal(attrs, "scroll-start") {
            if !matches!(string(start), Some("start" | "end")) {
                return err("lower-attr-value", "`scroll-start` is `start` (the default) or `end` (the list opens at its last row)", start.span);
            }
        }
        if let Some(reorder) = literal(attrs, "reorderdrop").filter(|_| row) {
            return err("lower-collection-reorder", "reordering is vertical; a virtualized row list refuses `reorderdrop` until a consumer needs it (LLP 1070 §4.7)", reorder.span);
        }
        if !row {
            return Ok(());
        }
        if !literal(attrs, "height")
            .is_some_and(|a| numeric_literal(&a.value).is_some_and(|n| n > 0.0))
        {
            return err("lower-collection-cross", "a virtualized row list needs a literal `height`: an auto height would be its tallest mounted item, which changes as items mount", span);
        }
        for a in attrs {
            let refusal = match a.name.as_str() {
                "flex-wrap" if string(a) != Some("nowrap") => Some("a wrapping virtualized list is a grid, which is not windowed"),
                "justify-content" if !matches!(string(a), Some("flex-start" | "normal")) => Some("main-axis alignment would move the items' starts; space items with a margin on the row root"),
                "direction" if string(a) == Some("rtl") => Some("a right-to-left virtualized row list is not windowed yet; its `scrollLeft` counts from the right edge"),
                _ => None,
            };
            if let Some(reason) = refusal {
                return err(
                    "lower-collection-flow",
                    format!("`{}` on a virtualized row list: {reason}", a.name),
                    a.span,
                );
            }
        }
        Ok(())
    }

    // Component uses and slots have already expanded before lowering. Inspect
    // all arms, even currently inactive ones. A row may hold one virtualized
    // list, one level down, whose lifetime is the row's (LLP 1070 N1, N6); it
    // may not hold another, reorder, or scroll vertically without a literal
    // height, since a row's height is its content's and `flex` alone bounds
    // nothing there.
    fn check_nested(&self, nodes: &[Node], inner: bool) -> Result<(), LowerError> {
        for node in nodes {
            match node {
                Node::Element {
                    attrs,
                    children,
                    span,
                    ..
                } => {
                    // `virtualized` is a prop, which no `style` may hold.
                    let opt = attrs.iter().find(|a| a.name == "virtualized");
                    if let Some(opt) = opt.filter(|a| matches!(a.value, Expr::Bool(true, _))) {
                        if inner {
                            return err("lower-collection-depth", "virtualized lists nest one level deep: this list is inside a virtualized list that is itself in a virtualized list's row", opt.span);
                        }
                        if let Some(reorder) = attrs.iter().find(|a| a.name == "reorderdrop") {
                            return err("lower-collection-reorder", "reordering is not built for a virtualized list in another's row (LLP 1070 §4.7)", reorder.span);
                        }
                        let bounded = attrs.iter().any(|a| {
                            matches!(a.name.as_str(), "height" | "max-height")
                                && numeric_literal(&a.value).is_some_and(|n| n > 0.0)
                        });
                        if !row_list(attrs) && !bounded {
                            return err("lower-collection-unbounded", "a virtualized list in a virtualized list's row needs a literal `height` or `max-height`: the row's height is its content's, so `flex` bounds nothing there", *span);
                        }
                        self.check_nested(children, true)?;
                        continue;
                    }
                    self.check_nested(children, inner)?;
                }
                Node::When {
                    then, otherwise, ..
                } => {
                    self.check_nested(then, inner)?;
                    self.check_nested(otherwise, inner)?;
                }
                Node::Match { some, none, .. } => {
                    self.check_nested(&some.1, inner)?;
                    self.check_nested(none, inner)?;
                }
                Node::Each { body, .. } => {
                    self.check_nested(body, inner)?;
                }
                Node::Use { children, .. } => self.check_nested(children, inner)?,
                Node::Children { .. } => {}
            }
        }
        Ok(())
    }

    fn collection_flow(&self, attrs: &[Attr], row: Option<bool>) -> Result<(), LowerError> {
        let container = row.is_some();
        let horizontal = row == Some(true);
        let main_padding: &[&str] = if horizontal {
            &["padding", "padding-left", "padding-right"]
        } else {
            &["padding", "padding-top", "padding-bottom"]
        };
        for a in attrs {
            if container
                && main_padding.contains(&a.name.as_str())
                && numeric_literal(&a.value) != Some(0.0)
            {
                let (axis, cross) = if horizontal {
                    ("left/right", "padding-top and padding-bottom")
                } else {
                    ("top/end", "padding-left and padding-right")
                };
                return err("lower-collection-flow", format!("`{}` on a virtualized list container requires literal zero; put {axis} spacing inside measured rows until a collection inset policy is supported ({cross} remain allowed)", a.name), a.span);
            }
            let allowed = match a.name.as_str() {
                "position" => {
                    matches!(&a.value, Expr::Str(s, _) if s == "relative" || s == "static")
                }
                "margin" | "margin-top" | "margin-bottom" => {
                    numeric_literal(&a.value).is_some_and(|v| v >= 0.0)
                }
                "display" if container => true, // `collection_axis`
                "flex-direction" | "flex-wrap" | "justify-content" | "direction" if horizontal => {
                    true // `collection_axis`
                }
                "display" => !matches!(&a.value, Expr::Str(s, _) if s == "contents"),
                "overflow" if container => {
                    matches!(&a.value, Expr::Str(s, _) if s == "scroll" || s == "auto")
                }
                // The main axis scrolls; the cross axis is `hidden`, as a
                // vertical list's `overflow-x` has always been.
                "overflow-y" | "overflow-x" if container => {
                    let main = if horizontal {
                        "overflow-x"
                    } else {
                        "overflow-y"
                    };
                    matches!(&a.value, Expr::Str(s, _) if if a.name == main { s == "scroll" || s == "auto" } else { s == "hidden" })
                }
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
            if !allowed
                && a.name == "position"
                && matches!(&a.value, Expr::Str(s, _) if s == "sticky")
            {
                // @ref LLP 1083 D5 — each row is laid out in a wrapper of its own.
                return err("lower-collection-flow", "a windowed row's root would stick only inside its own row: make a section one row, and its header `position: sticky` inside it", a.span);
            }
            if !allowed {
                return err("lower-collection-flow", format!("`{}` is not supported on this virtual collection flow root; absolute/overlapping rows and alternate container layouts are not windowed", a.name), a.span);
            }
        }
        Ok(())
    }
}

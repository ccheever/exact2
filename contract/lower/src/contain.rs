//! The containing block of absolutely positioned descendants (LLP 1074 T1):
//! which attributes make a box one on some host, and the `position:
//! relative` lowering adds where a box is one on every host. Split from
//! `tags.rs`, whose attribute table it reads.
use crate::tags::{attr, AttrTarget, Tag};
use exact_kernel::{NodeType, StyleId};

/// The rows that make a box the containing block of its absolutely
/// positioned descendants on some host, whatever its `position` (LLP 1074
/// T1): a box with one is lowered `position: relative` unless it names a
/// position.
///
/// - `overflow`: a native host clips and scrolls a box's view subtree, so a
///   descendant placed against an ancestor outside it would still be clipped
///   and scrolled by it. CSS does not make a clipping box a containing block;
///   this is the declared deviation.
/// - The transforms, `filter` and `backdrop-filter`: CSS's own rule.
/// - Motion (an animation, a transition, a press scale, a timeline): the
///   browser makes the box a containing block while a transform runs, so it
///   is one at rest too.
pub const CONTAINS_ABSOLUTE: [StyleId; 16] = [
    StyleId::OverflowX,
    StyleId::OverflowY,
    StyleId::Translate,
    StyleId::Scale,
    StyleId::Rotate,
    StyleId::Transform,
    StyleId::Filter,
    StyleId::BackdropFilter,
    StyleId::Animation,
    StyleId::Transition,
    StyleId::LayoutTransition,
    StyleId::ExitAnimation,
    StyleId::PressScale,
    StyleId::DragTimeline,
    StyleId::AnimationTimeline,
    // CSS: `perspective` makes a containing block too (LLP 1077 D8).
    StyleId::Perspective,
];
/// Whether an attribute makes its box a containing block (see
/// [`CONTAINS_ABSOLUTE`]): one of those rows, a material (a backdrop filter),
/// a navigation screen or modal (which the host moves), or a context
/// preview (which the host transforms).
pub fn contains_absolute(name: &str, value: &contract_syntax::Expr) -> bool {
    // A literal that clips nothing and transforms nothing makes no containing block.
    if matches!(value, contract_syntax::Expr::Str(v, _) if v == "visible" || v == "none") {
        return false;
    }
    match attr(name) {
        Some(AttrTarget::Styles(rows)) => rows.iter().any(|row| CONTAINS_ABSOLUTE.contains(row)),
        _ => matches!(
            name,
            "backgroundMaterial" | "navigationKey" | "navigationPresentation" | "contextTarget"
        ),
    }
}

/// An element's attributes with `position: relative` added, when it is the
/// containing block of its absolutely positioned descendants on every host
/// and names no position: it has a [`contains_absolute`] attribute, scrolls
/// by its tag, is a canvas (whose surface the page positions) or a Markdown
/// editor (whose line markers it holds). `None` otherwise. An authored
/// `position: static` there is refused.
pub(crate) fn positioned(
    tag: &Tag,
    attrs: &[contract_syntax::Attr],
    in_svg: bool,
    span: contract_syntax::Span,
    host_transform: bool,
    holds_nothing: bool,
) -> Result<Option<Vec<contract_syntax::Attr>>, crate::LowerError> {
    use contract_syntax::Expr;
    let literal =
        |a: &contract_syntax::Attr, v: &str| matches!(&a.value, Expr::Str(s, _) if s == v);
    let contains = !in_svg
        && !tag.node_type.is_svg_element()
        && (host_transform
            || attrs.iter().any(|a| contains_absolute(&a.name, &a.value))
            || tag.node_type.scrolls_by_default()
            || tag.node_type == NodeType::Canvas
            || attrs
                .iter()
                .any(|a| a.name == "markup" && literal(a, "markdown")));
    if !contains {
        return Ok(None);
    }
    match attrs.iter().find(|a| a.name == "position") {
        Some(a) if literal(a, "static") => crate::err(
            "lower-attr-value",
            "`position: static` on a box that clips, scrolls, transforms or animates: such a box is the containing block of its absolutely positioned descendants on every host, so it is `relative`; remove `position`",
            a.span,
        ),
        // A bound position is fine when every value it can take is positioned.
        Some(a) if !always_positioned(&a.value) => crate::err(
            "lower-attr-value",
            "a bound `position` on a box that clips, scrolls, transforms or animates: such a box is the containing block of its absolutely positioned descendants on every host, so every value its position can take must be `relative`, `absolute` or `sticky`",
            a.span,
        ),
        Some(_) => Ok(None),
        None if tag.fixed_styles.iter().any(|(id, _)| *id == StyleId::PositionType) => Ok(None),
        // Only its own rows (it clips, transforms or animates) and nothing
        // absolute can be under it (`Lowerer::may_hold_absolute`): it is the
        // containing block of nothing, and a positioned box costs a host a
        // layer to paint and hit-test (10,000 grid rows' clipping cells: a 25 ms hit test a pointer event).
        None if holds_nothing
            && !host_transform
            && !tag.node_type.scrolls_by_default()
            && tag.node_type != NodeType::Canvas
            && !attrs.iter().any(|a| {
                (a.name == "markup" && literal(a, "markdown"))
                    || matches!(
                        a.name.as_str(),
                        "backgroundMaterial"
                            | "navigationKey"
                            | "navigationPresentation"
                            | "contextTarget"
                            | "z-index"
                            | "top"
                            | "right"
                            | "bottom"
                            | "left"
                    )
            }) =>
        {
            Ok(None)
        }
        None => {
            let mut attrs = attrs.to_vec();
            attrs.push(contract_syntax::Attr {
                name: "position".into(),
                value: Expr::Str("relative".into(), span),
                span,
            });
            Ok(Some(attrs))
        }
    }
}
/// Whether every value a `position` expression can take is positioned: a
/// `relative`, `absolute` or `sticky` literal, or a choice between such. A
/// value from run time (a state, a field, a call) is not.
fn always_positioned(value: &contract_syntax::Expr) -> bool {
    use contract_syntax::Expr;
    match value {
        Expr::Str(v, _) => v == "relative" || v == "absolute" || v == "sticky",
        Expr::Ternary(_, a, b, _) => always_positioned(a) && always_positioned(b),
        _ => false,
    }
}

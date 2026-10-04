//! Wrapper/spacer views use the same kernel operations and styles as authored UI.
use super::*;
use exact_kernel::{PropValue, StyleId};
pub(super) fn style(
    u: &mut Update<'_>,
    view: ViewId,
    rows: &[(&str, Value)],
) -> Result<(), InstanceError> {
    let patch = patch(u.env.plan, rows)?;
    u.ops.push(Op::SetStyle {
        id: view,
        patch: Box::new(patch),
    });
    Ok(())
}

/// `rows` as a style patch, through the bridge as authored rows go.
pub(super) fn patch(plan: &Plan, rows: &[(&str, Value)]) -> Result<StyleProps, InstanceError> {
    let mut patch = StyleProps::default();
    for (name, value) in rows {
        let id = StyleId::from_name(name).unwrap_or_else(|| panic!("unknown kernel style: {name}"));
        bridge::set_style(&mut patch, id as u16, value, plan.stacks.len())
            .map_err(InstanceError::Bridge)?;
    }
    Ok(patch)
}

/// A row wrapper's rows. A flex formatting context encloses positive root
/// margins on all hosts; no collapsed CSS margin can escape the measured
/// wrapper border box. In a row list the wrapper is a flex item that neither
/// grows nor shrinks (`flex: none`, as an overflowing carousel's cards
/// are), stretched to the row's height by the list's own `align-items`
/// (LLP 1070 §3.1).
fn wrapper_rows(axis: ListAxis) -> Vec<(&'static str, Value)> {
    match axis {
        ListAxis::Vertical => vec![
            ("display", Value::str("flex")),
            ("flex_direction", Value::str("column")),
            ("flex_shrink", Value::Number(0.0)),
            ("min_width", Value::Number(0.0)),
            ("width", Value::str("100%")),
            ("box_sizing", Value::str("border-box")),
        ],
        ListAxis::Horizontal => vec![
            ("display", Value::str("flex")),
            ("flex_direction", Value::str("column")),
            ("flex_grow", Value::Number(0.0)),
            ("flex_shrink", Value::Number(0.0)),
            ("min_height", Value::Number(0.0)),
            ("box_sizing", Value::str("border-box")),
        ],
    }
}

/// The host translates/raises a reorderable list's wrapper during Arrange;
/// it must be the same containing block before, during and after the
/// gesture (1074 D1).
const REORDERABLE: (&str, &str) = ("position_type", "relative");

/// A row wrapper's style, as its ops leave it in a kernel: what a document
/// written without one gives it (LLP 1048.004).
fn wrapper_style(
    plan: &Plan,
    axis: ListAxis,
    reorderable: bool,
) -> Result<StyleProps, InstanceError> {
    let mut style = patch(plan, &wrapper_rows(axis))?;
    if reorderable {
        style.apply_patch(&patch(plan, &[(REORDERABLE.0, Value::str(REORDERABLE.1))])?);
    }
    Ok(style)
}

/// A spacer's style for `size`, as its ops leave it in a kernel.
fn spacer_style(plan: &Plan, axis: ListAxis, size: f64) -> Result<StyleProps, InstanceError> {
    patch(plan, &spacer_rows(axis, size))
}

/// A row's wrapper: its identity (`listItemKey`) and `listitem` role, so a
/// host can select and copy text across rows it has not mounted.
pub(super) fn row_wrapper(
    u: &mut Update<'_>,
    axis: ListAxis,
    children: Vec<ViewId>,
    key: &str,
    reorderable: bool,
) -> Result<ViewId, InstanceError> {
    let view = u.ids.fresh();
    u.ops.push(Op::CreateView {
        id: view,
        node_type: NodeType::View,
    });
    style(u, view, &wrapper_rows(axis))?;
    if reorderable {
        style(u, view, &[(REORDERABLE.0, Value::str(REORDERABLE.1))])?;
    }
    u.ops.push(Op::SetChildren { id: view, children });
    u.ops.push(Op::SetProp {
        id: view,
        prop: PropId::AccessibilityRole,
        value: PropValue::Str("listitem".into()),
    });
    u.ops.push(Op::SetProp {
        id: view,
        prop: PropId::ListItemKey,
        value: PropValue::Str(key.into()),
    });
    Ok(view)
}

/// A row whose item left the data, before the list detaches and destroys its
/// wrapper: an empty `listItemKey` tells the kernel it leaves (and plays its
/// root's `exit-animation`, LLP 1063), where a row that scrolled away simply
/// goes.
pub(crate) fn item_left(u: &mut Update<'_>, wrapper: ViewId) {
    u.ops.push(Op::SetProp {
        id: wrapper,
        prop: PropId::ListItemKey,
        value: PropValue::Str(String::new()),
    });
}

/// A mounted row's place in the whole list, which moves as rows come and go.
pub(super) fn publish_position(u: &mut Update<'_>, wrapper: ViewId, position: usize, count: usize) {
    u.ops.push(Op::SetProp {
        id: wrapper,
        prop: PropId::AccessibilityPosInSet,
        value: PropValue::Int(position as i64 + 1),
    });
    u.ops.push(Op::SetProp {
        id: wrapper,
        prop: PropId::AccessibilitySetSize,
        value: PropValue::Int(count as i64),
    });
}
/// The main-axis extent of rows not mounted: a spacer's height in a
/// vertical list, its width in a row list, stretched across the other.
fn main_size(axis: ListAxis) -> &'static str {
    match axis {
        ListAxis::Vertical => "height",
        ListAxis::Horizontal => "width",
    }
}
/// A spacer's rows for `size`.
fn spacer_rows(axis: ListAxis, size: f64) -> Vec<(&'static str, Value)> {
    match axis {
        ListAxis::Vertical => vec![
            ("height", Value::Number(size)),
            ("flex_shrink", Value::Number(0.0)),
            ("width", Value::str("100%")),
        ],
        ListAxis::Horizontal => vec![
            ("width", Value::Number(size)),
            ("flex_grow", Value::Number(0.0)),
            ("flex_shrink", Value::Number(0.0)),
            ("align_self", Value::str("stretch")),
        ],
    }
}
fn spacer(u: &mut Update<'_>, axis: ListAxis, size: f64) -> Result<ViewId, InstanceError> {
    let view = u.ids.fresh();
    u.ops.push(Op::CreateView {
        id: view,
        node_type: NodeType::View,
    });
    u.ops.push(Op::SetProp {
        id: view,
        prop: PropId::AccessibilityElementsHidden,
        value: PropValue::Bool(true),
    });
    style(u, view, &spacer_rows(axis, size))?;
    Ok(view)
}
impl Collection {
    pub(super) fn emit_children(&mut self, u: &mut Update<'_>) -> Result<(), InstanceError> {
        let mut children = Vec::with_capacity(self.mounted.len() + 4);
        let mut cursor = 0;
        let mut spacer_count = 0;
        for i in 0..=self.mounted.len() {
            let position = self.mounted.get(i).map_or(self.index.len(), |r| r.position);
            let gap = self.index.prefix(position).unwrap() - self.index.prefix(cursor).unwrap();
            if gap > 0.0 {
                if let Some((view, old)) = self.spacers.get_mut(spacer_count) {
                    if *old != gap {
                        style(u, *view, &[(main_size(self.axis), Value::Number(gap))])?;
                        *old = gap;
                    }
                    children.push(*view);
                } else {
                    let view = spacer(u, self.axis, gap)?;
                    self.spacers.push((view, gap));
                    children.push(view);
                }
                spacer_count += 1;
            }
            if let Some(row) = self.mounted.get(i) {
                children.push(row.wrapper);
                cursor = position + 1;
            }
        }
        for (view, _) in self.spacers.drain(spacer_count..) {
            u.ops.push(Op::DestroyView { id: view });
        }
        if children != self.children {
            u.ops.push(Op::SetChildren {
                id: self.view,
                children: children.clone(),
            });
            self.children = children;
        }
        Ok(())
    }
}
/// One child of a virtualized list's view, as a document written without a
/// kernel holds it (LLP 1048.004): a spacer, or a mounted row's wrapper
/// around its root.
pub(in crate::instance) enum ListChild<'a> {
    /// A spacer: its view and style (its props are fixed: hidden).
    Spacer(ViewId, StyleProps),
    /// A mounted row: its wrapper, the wrapper's style and props, and the
    /// row's roots.
    Row(ViewId, StyleProps, exact_kernel::PropList, &'a [Child]),
}

impl Collection {
    /// The list view's children, in order, with what their ops left in a
    /// kernel: each spacer's rows at its size, each wrapper's rows and its
    /// role, key and published place.
    pub(in crate::instance) fn document_children(
        &self,
        plan: &Plan,
    ) -> Result<Vec<ListChild<'_>>, InstanceError> {
        let mut out = Vec::with_capacity(self.children.len());
        for view in &self.children {
            if let Some((_, size)) = self.spacers.iter().find(|(v, _)| v == view) {
                out.push(ListChild::Spacer(
                    *view,
                    spacer_style(plan, self.axis, *size)?,
                ));
                continue;
            }
            let Some(row) = self.mounted.iter().find(|m| m.wrapper == *view) else {
                return Err(invalid("a list child is neither a spacer nor a row"));
            };
            let key = super::super::ident(&row.row.key, row.row.dup)
                .ok_or_else(|| invalid("a row's key has no text"))?;
            let mut props = exact_kernel::PropList::new();
            props.set(PropId::AccessibilityRole, PropValue::Str("listitem".into()));
            props.set(PropId::ListItemKey, PropValue::Str(key));
            if row.published != (usize::MAX, usize::MAX) {
                let (position, count) = row.published;
                props.set(
                    PropId::AccessibilityPosInSet,
                    PropValue::Int(position as i64 + 1),
                );
                props.set(PropId::AccessibilitySetSize, PropValue::Int(count as i64));
            }
            out.push(ListChild::Row(
                *view,
                wrapper_style(plan, self.axis, self.reorderable)?,
                props,
                &row.row.roots,
            ));
        }
        Ok(out)
    }
}
pub(super) fn validate_row(plan: &Plan, roots: &[Child]) -> Result<(), InstanceError> {
    let [Child::Node(root)] = roots else {
        return Err(invalid("collection requires one flow root"));
    };
    let descriptor = plan.node(root.node);
    for (i, binding) in descriptor
        .bindings
        .iter()
        .map(|id| plan.binding(id))
        .enumerate()
    {
        if binding.kind != BindingKind::Style {
            continue;
        }
        let Some(style) = StyleId::from_bit(binding.id as u32) else {
            continue;
        };
        let Some(value) = root.last[i].as_ref() else {
            continue;
        };
        // A transform or a relative offset moves the row's box where it
        // paints and leaves its flow, and so its wrapper's measure, as CSS
        // does: a lifted row being dragged is `translate` and `z-index`
        // (files F14: "0px 0px" was refused and the list drew nothing).
        let allowed = match style.name() {
            "position_type" => value == &Value::str("relative") || value == &Value::str("static"),
            "margin_top" | "margin_bottom" => value.as_number().is_some_and(|n| n >= 0.0),
            _ => true,
        };
        if !allowed {
            return Err(invalid(
                "collection row must remain in nonoverlapping normal flow",
            ));
        }
    }
    Ok(())
}

//! Wrapper/spacer views use the same kernel operations and styles as authored UI.
use super::*;
use exact_kernel::{PropValue, StyleId};
pub(super) fn style(
    u: &mut Update<'_>,
    view: ViewId,
    rows: &[(&str, Value)],
) -> Result<(), InstanceError> {
    let mut patch = StyleProps::default();
    for (name, value) in rows {
        let id = StyleId::from_name(name).unwrap_or_else(|| panic!("unknown kernel style: {name}"));
        bridge::set_style(&mut patch, id as u16, value, u.env.plan.stacks.len())
            .map_err(InstanceError::Bridge)?;
    }
    u.ops.push(Op::SetStyle {
        id: view,
        patch: Box::new(patch),
    });
    Ok(())
}
pub(super) fn row_wrapper(
    u: &mut Update<'_>,
    children: Vec<ViewId>,
) -> Result<ViewId, InstanceError> {
    let view = u.ids.fresh();
    u.ops.push(Op::CreateView {
        id: view,
        node_type: NodeType::View,
    });
    // A flex formatting context encloses positive root margins on all hosts;
    // no collapsed CSS margin can escape the measured wrapper border box.
    style(
        u,
        view,
        &[
            ("display", Value::str("flex")),
            ("flex_direction", Value::str("column")),
            ("flex_shrink", Value::Number(0.0)),
            ("min_width", Value::Number(0.0)),
            ("width", Value::str("100%")),
            ("box_sizing", Value::str("border-box")),
        ],
    )?;
    u.ops.push(Op::SetChildren { id: view, children });
    Ok(view)
}
fn spacer(u: &mut Update<'_>, height: f64) -> Result<ViewId, InstanceError> {
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
    style(
        u,
        view,
        &[
            ("height", Value::Number(height)),
            ("flex_shrink", Value::Number(0.0)),
            ("width", Value::str("100%")),
        ],
    )?;
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
                        style(u, *view, &[("height", Value::Number(gap))])?;
                        *old = gap;
                    }
                    children.push(*view);
                } else {
                    let view = spacer(u, gap)?;
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
        let allowed = match style.name() {
            "position_type" => value == &Value::str("relative"),
            "top" | "bottom" | "left" | "right" | "rotate" => value.as_number() == Some(0.0),
            "scale" => value.as_number() == Some(1.0),
            "translate" => value == &Value::str("0 0"),
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

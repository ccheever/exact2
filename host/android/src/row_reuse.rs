//! Conservative Android carrier admission over collection row bodies only.
use exact_kernel::{NodeType, PropId};
use exact_plan::{BindingKind, NodesId, Opcode, Plan, RegionKind, StrId};

/// Outside-row native navigation and scrollers retain their own lifetime.
/// Any unsupported row body disables this owner's reuse opt-in. Individual
/// shared Runner rows still apply their stronger state/slot admission policy.
pub(super) fn admits(plan: &Plan) -> bool {
    plan.nodes.iter().enumerate().all(|(index, node)| {
        if !under_list_row(plan, NodesId(index as u32)) {
            return true;
        }
        !matches!(
            NodeType::from_wire(node.node_type),
            Some(NodeType::ScrollView | NodeType::Svg)
        ) && node.bindings.iter().map(|id| plan.binding(id)).all(|b| {
            if b.kind != BindingKind::Prop {
                return true;
            }
            match PropId::from_wire(b.id) {
                Some(PropId::NavigationKey) => false,
                Some(PropId::AccessibilityRole) => safe_literal_role(plan, b.expr),
                _ => true,
            }
        })
    })
}

fn under_list_row(plan: &Plan, node: NodesId) -> bool {
    let mut arm = plan.node(node).arm;
    while let Some(current) = arm {
        let region = plan.region(plan.arm(current).region);
        if region.kind == RegionKind::Each
            && region.parent.is_some_and(|parent| {
                NodeType::from_wire(plan.node(parent).node_type) == Some(NodeType::List)
            })
        {
            return true;
        }
        arm = region.arm;
    }
    false
}

fn safe_literal_role(plan: &Plan, expr: exact_plan::Code) -> bool {
    let mut instructions = exact_runner::vm::instructions(plan.code(expr));
    let (Some(Ok(value)), Some(Ok(end)), None) = (
        instructions.next(),
        instructions.next(),
        instructions.next(),
    ) else {
        return false;
    };
    value.op == Opcode::Str
        && end.op == Opcode::Return
        && !matches!(plan.str(StrId(value.args[0] as u32)), "toolbar" | "tablist")
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = "component App\n  resource rows = rows() as shape list<number>\n  view\n    column\n      list virtualized=true height=320\n        each row in rows key=row\n          column height=64\n            text `${row}`\n";
    fn plan(source: &str) -> Plan {
        contract::compile(source).unwrap()
    }
    #[test]
    fn row_reuse_admits_outer_list_and_literal_non_navigation_roles() {
        assert!(admits(&plan(SOURCE)));
        let source = SOURCE.replace(
            "          column height=64",
            "          button height=64 role=\"button\"",
        );
        assert!(admits(&plan(&source)));
    }
    #[test]
    fn row_reuse_refuses_scroll_and_native_navigation_in_every_row_arm() {
        for row in [
            "scroll height=64 overflow-y=\"scroll\"",
            "column height=64 navigationKey=`page-${row}`",
            "column height=64 role=\"toolbar\"",
            "column height=64 role=\"tablist\"",
            "column height=64 role=(row > 30 ? \"toolbar\" : \"button\")",
        ] {
            assert!(
                !admits(&plan(&SOURCE.replace("column height=64", row))),
                "{row}"
            );
        }
        let source=SOURCE.replace("          column height=64\n            text `${row}`",
            "          column height=64\n            when row > 30\n              scroll height=320 overflow-y=\"scroll\"\n                text `${row}`\n            else\n              text \"first\"");
        assert!(
            !admits(&plan(&source)),
            "inactive nested arms must also be checked"
        );
    }
    #[test]
    fn row_reuse_refuses_svg_carriers_inside_collection_rows() {
        let mut p = plan(SOURCE);
        let at = p
            .nodes
            .iter()
            .enumerate()
            .find_map(|(index, node)| {
                (node.node_type == NodeType::View as u8
                    && under_list_row(&p, NodesId(index as u32)))
                .then_some(index)
            })
            .unwrap();
        p.nodes[at].node_type = NodeType::Svg as u8;
        assert!(!admits(&p), "SVG has no resettable Android carrier");
    }
    #[test]
    fn row_reuse_default_policy_is_off_and_can_be_configured_before_boot() {
        let mut bridge = super::super::Bridge::<()>::new();
        assert!(!bridge.row_reuse);
        bridge.set_row_reuse(true);
        assert!(bridge.row_reuse);
        bridge.set_row_reuse(false);
        assert!(!bridge.row_reuse);
    }
}

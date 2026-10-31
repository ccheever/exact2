//! Which of a plan's nodes are rows (LLP 1048.007 D5): repeated by a `for`
//! that lists items, not one that shows pages.

use exact_plan::Plan;

/// Whether plan node `node` is repeated by a `for`, at any depth: a row of
/// a list or grid, whose image loads as the row comes into view (LLP
/// 1048.007 D5), as a platform list's cells load theirs. A `for` over the
/// navigation stack (`stack(nav)`) or over one item (`[route]`, which
/// keys a page) shows pages, not rows.
pub fn repeated(plan: &Plan, node: exact_plan::NodesId) -> bool {
    use exact_plan::{Opcode, RegionKind, Stdlib};
    let rows = |subject| {
        let code: Vec<_> = crate::vm::instructions(plan.code(subject))
            .flatten()
            .filter(|i| i.op != Opcode::Return)
            .collect();
        !code.last().is_some_and(|i| {
            i.op == Opcode::List && i.args[0] <= 1
                || i.op == Opcode::Call && Stdlib::from_wire(i.args[0] as u8) == Some(Stdlib::Stack)
        })
    };
    let (mut parent, mut arm) = (Some(node), None);
    loop {
        if let Some(n) = parent.take() {
            let row = plan.node(n);
            (parent, arm) = (row.parent, row.arm);
            continue;
        }
        let Some(a) = arm.take() else { return false };
        let region = &plan.regions[plan.arms[a.0 as usize].region.0 as usize];
        if region.kind == RegionKind::Each && rows(region.subject) {
            return true;
        }
        (parent, arm) = (region.parent, region.arm);
    }
}

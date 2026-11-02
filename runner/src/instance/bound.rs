//! What a realized node's bindings last emitted.
use super::*;

impl NodeInst {
    /// Where binding `i` of `row` keeps its last value in `last`: past the
    /// bindings before it that can change. A plan's bindings are mostly
    /// constant (euxy's first screen: 3,652 of 3,974), and those are kept once
    /// for the plan's life in [`SiteIndex::constants`], not per instance.
    pub(super) fn slot(sites: &SiteIndex, row: &exact_plan::NodesRow, i: usize) -> usize {
        (0..i).filter(|&j| !Self::constant(sites, row, j)).count()
    }

    pub(super) fn constant(sites: &SiteIndex, row: &exact_plan::NodesRow, i: usize) -> bool {
        sites.deps.bindings[(row.bindings.start + i as u32) as usize].is_constant()
    }

    /// The value binding `i` last emitted.
    pub(super) fn emitted(&self, plan: &Plan, sites: &SiteIndex, i: usize) -> Option<Value> {
        let row = plan.node(self.node);
        if Self::constant(sites, &row, i) {
            let b = row.bindings.start + i as u32;
            return sites.constants.borrow().get(&b).cloned();
        }
        self.last[Self::slot(sites, &row, i)].clone()
    }

    /// The value a style binding last emitted.
    pub(super) fn bound_style(
        &self,
        plan: &Plan,
        sites: &SiteIndex,
        style: exact_kernel::StyleId,
    ) -> Option<Value> {
        self.bound(plan, sites, BindingKind::Style, style as u16)
    }

    /// The value a prop binding last emitted.
    pub(super) fn bound_prop(
        &self,
        plan: &Plan,
        sites: &SiteIndex,
        prop: exact_kernel::PropId,
    ) -> Option<Value> {
        self.bound(plan, sites, BindingKind::Prop, prop as u16)
    }

    fn bound(&self, plan: &Plan, sites: &SiteIndex, kind: BindingKind, id: u16) -> Option<Value> {
        plan.node(self.node)
            .bindings
            .iter()
            .enumerate()
            .find_map(|(i, b)| {
                let b = plan.binding(b);
                (b.kind == kind && b.id == id)
                    .then(|| self.emitted(plan, sites, i))
                    .flatten()
            })
    }
}

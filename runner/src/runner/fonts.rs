//! Session-scoped installed-family lists beside the plan's compiled stacks.
use super::{DataSource, Runner};
use exact_plan::Plan;

impl<D: DataSource> Runner<D> {
    /// The plan.
    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    /// Runtime installed-family lists in their assigned stack-id order.
    pub fn runtime_font_stacks(&self) -> Vec<String> {
        self.fonts.borrow().names().to_vec()
    }

    /// Number of runtime stacks in this session.
    pub fn runtime_font_count(&self) -> usize {
        self.fonts.borrow().names().len()
    }
}

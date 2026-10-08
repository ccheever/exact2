//! Explicit source contract plus whole-plan carrier selection at bake time.

use exact_plan::Plan;
use exact_runner::DataSource;

/// Whether an authored plan may omit the general fallback owner.
/// Existing `host!(Data, ..)` retains that provider and the full owner by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DataContract {
    /// Keep construction, binding, activation, identity and all runtime behavior.
    #[default]
    Runtime,
    /// The author explicitly omits the general fallback for the fully baked plan.
    /// The runtime provider and construction/bind behavior are retained;
    /// the provider promises this plan never needs grants or native/executor work.
    /// This is a plan contract rather than automatic omission for arbitrary data.
    CorePlan,
}

/// Select a core carrier only after an explicit authored data contract.
/// Whole-plan runtime eligibility is checked again when this carrier boots.
/// Unproven runtime sources retain their original provider and general owner.
pub fn baked_core_eligible<D: DataSource>(plan: &Plan, data: &D, contract: DataContract) -> bool {
    contract == DataContract::CorePlan
        && (plan.app_id.is_empty() || data.app_id().is_empty() || plan.app_id == data.app_id())
        && crate::core_eligible(plan, data)
}

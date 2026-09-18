//! State carried across a restart (LLP 1007 §6, LLP 1027 D5).
use exact_plan::Value;

/// What survives a reload: slots whose names/types still fit, settled
/// resources with matching arguments and logic identity, app secrets, and
/// the clock. A logic edit re-asks resources rather than preserving old answers.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Carried {
    /// Router slot name and its value decoded through the old plan's shapes.
    /// @ref LLP 1038 D5 — preserves named params when their field order changes.
    pub router: Option<(String, exact_route::Router)>,
    /// The replaceable data module that produced these resource answers.
    pub data_revision: Option<String>,
    /// Preserve kept-answer persistence when a live candidate is already loaded.
    pub keeps_answers: bool,
    /// Slot name → value.
    pub slots: Vec<(String, Value)>,
    /// Resource name → (arguments, value).
    pub resources: Vec<(String, Vec<Value>, Value)>,
    /// Names of carried resources whose answer depends on the store.
    pub store_readers: Vec<String>,
    /// The clock, milliseconds.
    pub now_ms: f64,
    /// The store's kept values (LLP 1018): what the host has persisted.
    pub store: Vec<(String, String)>,
}

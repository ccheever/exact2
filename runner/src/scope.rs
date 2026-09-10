//! Cells owned by scope frames in the existing instance tree.
use crate::runner::ResourceState;
use crate::vm::RowSlots;
use exact_plan::{RegionsId, Value};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

/// Shared storage reached through a mounted scope's frame.
pub type ScopeRef = Rc<RefCell<ScopeState>>;

/// One mounted component's state. The tree owns its lifetime.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopeState {
    pub(crate) lifetime: u64,
    pub(crate) region: RegionsId,
    pub(crate) path: Vec<Value>,
    pub(crate) slots: RowSlots,
    pub(crate) resources: BTreeMap<usize, OwnedResource>,
    pub(crate) mutations: BTreeMap<usize, bool>,
    pub(crate) timers: BTreeMap<usize, f64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct OwnedResource {
    pub(crate) state: Option<ResourceState>,
    /// A real answer survives argument changes; a pending fallback does not.
    pub(crate) settled: Option<Value>,
    pub(crate) reader: bool,
    pub(crate) pending: bool,
    pub(crate) stale: bool,
    pub(crate) refresh: bool,
}

impl ScopeState {
    pub(crate) fn new(lifetime: u64, region: RegionsId, path: Vec<Value>) -> Self {
        Self {
            lifetime,
            region,
            path,
            slots: Rc::new(RefCell::new(BTreeMap::new())),
            resources: BTreeMap::new(),
            mutations: BTreeMap::new(),
            timers: BTreeMap::new(),
        }
    }
}

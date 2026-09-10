//! Pre-walk rollback: root values plus only the action's reached storage cells.
use super::*;
use crate::{
    scope::{ScopeRef, ScopeState},
    store::StoreCheckpoint,
};
use std::collections::BTreeMap;

pub(super) struct Checkpoint {
    store: StoreCheckpoint,
    slots: Vec<Value>,
    derives: Vec<Option<Value>>,
    derive_readers: Vec<bool>,
    resources: Vec<Option<ResourceState>>,
    resource_values: Vec<Option<Value>>,
    readers: Vec<bool>,
    stale: Vec<bool>,
    pending: Vec<PendingReq>,
    contexts: std::collections::BTreeSet<u64>,
    pending_res: Vec<bool>,
    pending_mut: Vec<bool>,
    requests: Vec<RequestOut>,
    refresh: Vec<usize>,
    commands: usize,
    cells: Vec<(ScopeRef, ScopeState)>,
    rows: Vec<(RowSlots, BTreeMap<u32, Value>)>,
}

impl<D: DataSource> Runner<D> {
    pub(super) fn checkpoint(&self, frames: &[Frame]) -> Checkpoint {
        Checkpoint {
            store: self.store.checkpoint(),
            slots: self.slots.clone(),
            derives: self.derives.clone(),
            derive_readers: self.derive_readers.clone(),
            resources: self.resources.clone(),
            resource_values: self.resource_values.clone(),
            readers: self.store_readers.clone(),
            stale: self.stale.clone(),
            pending: self.pending.clone(),
            contexts: self.contexts.clone(),
            pending_res: self.pending_res.clone(),
            pending_mut: self.pending_mut.clone(),
            requests: self.requests.clone(),
            refresh: self.refresh_next.clone(),
            commands: self.commands.len(),
            cells: frames
                .iter()
                .filter_map(|f| f.scope.as_ref().map(|s| (s.clone(), s.borrow().clone())))
                .collect(),
            rows: frames
                .iter()
                .filter_map(|f| f.row.as_ref().map(|r| (r.clone(), r.borrow().clone())))
                .collect(),
        }
    }

    pub(super) fn restore(&mut self, saved: Checkpoint) {
        self.store.restore(saved.store);
        // No transactional copy of the instance tree: after a walk refusal,
        // the kernel is unchanged and all outgoing work is discarded by poison.
        if self.poisoned {
            return;
        }
        self.retain_contexts(&saved.contexts);
        self.slots = saved.slots;
        self.derives = saved.derives;
        self.derive_readers = saved.derive_readers;
        self.resources = saved.resources;
        self.resource_values = saved.resource_values;
        self.store_readers = saved.readers;
        self.stale = saved.stale;
        self.pending = saved.pending;
        self.pending_res = saved.pending_res;
        self.pending_mut = saved.pending_mut;
        self.requests = saved.requests;
        self.refresh_next = saved.refresh;
        self.commands.truncate(saved.commands);
        for (cell, state) in saved.cells {
            *cell.borrow_mut() = state;
        }
        for (row, slots) in saved.rows {
            *row.borrow_mut() = slots;
        }
    }
}

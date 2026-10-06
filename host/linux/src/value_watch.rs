//! The `value`s the presenter keeps a typed text or choice against (LLP
//! 1069.001 D4): one a commit changes, removes or renews is replaced, even
//! if a later commit writes the old value back, as the web build's write of
//! `value` replaces the element's own.
use exact_kernel::ViewId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(crate) struct ValueWatch {
    watched: BTreeMap<ViewId, String>,
    replaced: BTreeSet<ViewId>,
}

impl ValueWatch {
    /// Watch `view`'s `value` from `at`; `None` stops.
    pub(crate) fn watch(&mut self, view: ViewId, at: Option<String>) {
        match at {
            Some(at) => self.watched.insert(view, at),
            None => self.watched.remove(&view),
        };
    }

    /// A commit left `view` with `value`, or removed or renewed it (`None`).
    pub(crate) fn committed(&mut self, view: ViewId, value: Option<&str>) {
        if self
            .watched
            .get(&view)
            .is_some_and(|at| Some(at.as_str()) != value)
        {
            self.watched.remove(&view);
            self.replaced.insert(view);
        }
    }

    /// Watched views replaced since the last call.
    pub(crate) fn take_replaced(&mut self) -> BTreeSet<ViewId> {
        std::mem::take(&mut self.replaced)
    }
}

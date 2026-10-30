//! A realized node's state that few nodes have, behind one pointer: a
//! `Child` is 112 bytes, a size class of Apple's allocator, and most child
//! vectors hold one.

use super::{collection, NodeInst};
use exact_plan::Value;

/// A [`NodeInst`]'s state that few nodes have.
#[derive(Debug, Default)]
pub(super) struct Rare {
    /// A virtualized list's engine.
    pub(super) collection: Option<Box<collection::Collection>>,
    /// Last published surface inputs (a canvas node).
    pub(super) last_surface: Option<Vec<Value>>,
    /// A mode boundary's implied rows, by [`super::theme::INHERITED`] (D7).
    pub(super) implied: Option<[Option<Value>; 8]>,
    /// The bindings, by `BindingsId`, whose last evaluation trapped
    /// (`trapped.rs`): unset and journaled once.
    pub(super) trapped: Vec<u32>,
}

/// A node's list engine, borrowing only its [`Rare`].
pub(super) fn list_mut(rare: &mut Option<Box<Rare>>) -> Option<&mut collection::Collection> {
    rare.as_mut()?.collection.as_deref_mut()
}

impl NodeInst {
    pub(super) fn rare(&mut self) -> &mut Rare {
        self.rare.get_or_insert_with(Box::default)
    }

    /// Its list engine, when it is a virtualized list.
    pub(super) fn collection(&self) -> Option<&collection::Collection> {
        self.rare.as_ref()?.collection.as_deref()
    }

    pub(super) fn collection_mut(&mut self) -> Option<&mut collection::Collection> {
        list_mut(&mut self.rare)
    }

    pub(super) fn set_collection(&mut self, made: Option<Box<collection::Collection>>) {
        if made.is_some() || self.rare.is_some() {
            self.rare().collection = made;
        }
    }
}

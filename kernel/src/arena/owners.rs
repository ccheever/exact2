//! The content owners a host publishes values for (LLP 1123 D9), and the
//! environment each node's rows resolve against.

use super::NodeArena;
use crate::style::Env;

impl NodeArena {
    /// Each owner's environments onto a new document environment, keeping
    /// the owner's values and container.
    pub(super) fn rebase_owners(&mut self, env: &Env) {
        for (_, owned, lifted) in &mut self.owners {
            *owned = Env {
                owner: owned.owner.take(),
                container: owned.container,
                ..env.clone()
            };
            *lifted = Env {
                container: lifted.container,
                ..env.clone()
            };
        }
    }

    /// Whether a host published values for any content owner.
    pub(crate) fn has_owners(&self) -> bool {
        !self.owners.is_empty()
    }

    /// The environment `slot`'s rows resolve against: the document's,
    /// carrying the values of the nearest content owner at or above it
    /// (LLP 1123 D9), or none outside every owner. A top-layer box, a
    /// popover or a `dialog`, is the document's, so one written in an owner
    /// reads the document's values, while its container units keep the
    /// owner's size (D12).
    pub fn env_for(&self, slot: u32) -> &Env {
        if self.owners.is_empty() {
            return &self.env;
        }
        let (mut at, mut lifted) = (Some(slot), false);
        while let Some(s) = at {
            if let Some((_, env, top)) = self.owners.iter().find(|(o, _, _)| *o == s) {
                return if lifted { top } else { env };
            }
            // A `dialog` too: Exact shows one only modally, in the top layer
            lifted |= self.top_layer_slots.contains(s)
                || self.props(s).str(crate::PropId::SemanticTag) == Some("dialog");
            at = self.parent(s);
        }
        &self.env
    }

    /// The values a host published for the owner rooted at `slot`.
    pub fn owner_values(&self, slot: u32) -> Option<&crate::style::OwnerValues> {
        self.owners
            .iter()
            .find(|(s, _, _)| *s == slot)
            .and_then(|(_, env, _)| env.owner.as_deref())
    }

    /// Publish (or, `None`, take back) the values of the owner rooted at
    /// `slot`; whether they changed.
    pub(crate) fn set_owner(
        &mut self,
        slot: u32,
        values: Option<crate::style::OwnerValues>,
    ) -> bool {
        if self.owner_values(slot) == values.as_ref() {
            return false;
        }
        self.owners.retain(|(s, _, _)| *s != slot);
        if let Some(v) = values {
            let container = Some(v.size);
            let env = Env {
                owner: Some(std::sync::Arc::new(v)),
                container,
                ..self.env.clone()
            };
            let lifted = Env {
                container,
                ..self.env.clone()
            };
            self.owners.push((slot, env, lifted));
        }
        true
    }
}

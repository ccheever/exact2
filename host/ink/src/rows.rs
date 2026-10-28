//! Kept instances interleaved with composition and image-slot markers. @ref LLP 1149 D4.
use crate::instance::{Bindings, Instance, TextureId};
use std::ops::Range;

#[derive(Clone)]
pub enum Command<M> {
    Instances(Range<usize>),
    Marker(M),
}

#[derive(Clone)]
pub struct Drawing<M> {
    pub instances: Vec<(Instance, Bindings)>,
    pub commands: Vec<Command<M>>,
}

impl<M> Default for Drawing<M> {
    fn default() -> Self {
        Self {
            instances: Vec::new(),
            commands: Vec::new(),
        }
    }
}

impl<M> Drawing<M> {
    pub fn push(&mut self, instance: Instance, bindings: Bindings) {
        let at = self.instances.len();
        self.instances.push((instance, bindings));
        if let Some(Command::Instances(range)) = self.commands.last_mut() {
            range.end = at + 1;
        } else {
            self.commands.push(Command::Instances(at..at + 1));
        }
    }

    pub fn marker(&mut self, marker: M) {
        self.commands.push(Command::Marker(marker));
    }

    /// The owner pins each returned texture once and adds its markers' references.
    pub fn textures(&self) -> Vec<TextureId> {
        let mut ids: Vec<_> = self
            .instances
            .iter()
            .flat_map(|(_, b)| {
                [
                    b.glyph,
                    b.color,
                    b.path,
                    b.ramp,
                    b.image,
                    b.mask,
                    b.destination,
                ]
            })
            .filter(|id| id.0 != 0)
            .collect();
        ids.sort_unstable_by_key(|id| id.0);
        ids.dedup();
        ids
    }
}

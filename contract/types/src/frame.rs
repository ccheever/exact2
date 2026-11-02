//! One frame of a [`Scope`](crate::Scope): the names it binds, in order.
use crate::{Ref, Ty};

#[derive(Debug, Clone)]
pub(crate) struct Frame {
    pub(crate) names: Vec<(String, Ref, Ty)>,
    /// Whether this frame is a region scope (counts toward `Item`/`Bound` depth).
    pub(crate) region: bool,
    /// Each name's first position, built by the first lookup in a frame of
    /// at least [`Frame::INDEXED`] names (a component's declarations), so a
    /// lookup costs the same in a component of ten thousand as of ten.
    index: std::sync::OnceLock<std::collections::HashMap<String, usize>>,
}

impl Frame {
    const INDEXED: usize = 16;

    pub(crate) fn new(names: Vec<(String, Ref, Ty)>, region: bool) -> Self {
        Frame {
            names,
            region,
            index: std::sync::OnceLock::new(),
        }
    }

    /// The position of `name`'s first entry, as a scan from the front finds it.
    pub(crate) fn find(&self, name: &str) -> Option<usize> {
        if self.names.len() < Self::INDEXED {
            return self.names.iter().position(|(n, _, _)| n == name);
        }
        let index = self.index.get_or_init(|| {
            let mut index = std::collections::HashMap::with_capacity(self.names.len());
            for (i, (n, _, _)) in self.names.iter().enumerate() {
                index.entry(n.clone()).or_insert(i);
            }
            index
        });
        index.get(name).copied()
    }

    /// Add a name at the end. Every later entry goes through here, so a
    /// built index takes it too (where the name is new: the first still wins).
    pub(crate) fn push_name(&mut self, entry: (String, Ref, Ty)) {
        if let Some(index) = self.index.get_mut() {
            index.entry(entry.0.clone()).or_insert(self.names.len());
        }
        self.names.push(entry);
    }
}

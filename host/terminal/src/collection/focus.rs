//! Logical Tab/arrow order through virtual rows, including rows not mounted.
//! @ref LLP 1101 D6 — every control remains reachable in document order.
use super::*;
use exact_kernel::NodeType;

#[derive(Clone, Copy)]
enum Entry {
    Control(ViewId),
    List(ViewId),
}

impl<D: DataSource> Host<D> {
    /// Collections stay one entry in the outer focus order. Their rows are
    /// traversed by logical position only when the key enters or leaves one.
    fn focus_entries(&self, roots: &[ViewId]) -> Vec<Entry> {
        let mut out = Vec::new();
        let mut stack: Vec<_> = roots.iter().rev().copied().collect();
        while let Some(view) = stack.pop() {
            let Some(n) = self.kernel().node(view) else {
                continue;
            };
            if n.style.display == Display::None
                || n.props.bool(PropId::Disabled) == Some(true)
                || (Self::is_layer(&n) && !self.layers.iter().any(|(open, _)| *open == view))
            {
                continue;
            }
            if matches!(
                n.node_type,
                NodeType::Pressable | NodeType::TextInput | NodeType::Control
            ) {
                out.push(Entry::Control(view));
            }
            if n.node_type == NodeType::List && n.props.bool(PropId::Virtualized) == Some(true) {
                out.push(Entry::List(view));
            } else if n.node_type != NodeType::Text {
                stack.extend(n.children().into_iter().rev());
            }
        }
        out
    }

    /// Find the next control in one lexical subtree, without wrapping out of
    /// it. A containing list delegates to its row before advancing past it.
    fn next_in(&mut self, roots: &[ViewId], current: Option<ViewId>, back: bool) -> Option<ViewId> {
        let entries = self.focus_entries(roots);
        let at = current.and_then(|current| {
            entries.iter().position(|entry| match entry {
                Entry::Control(view) => *view == current,
                Entry::List(view) => self.inside(current, *view),
            })
        });
        if let Some(index) = at {
            if let Entry::List(view) = entries[index] {
                if let Some(next) = self.next_in_list(view, current, back) {
                    return Some(next);
                }
            }
        }
        let candidates: Box<dyn Iterator<Item = usize>> = if back {
            Box::new((0..at.unwrap_or(entries.len())).rev())
        } else {
            Box::new(at.map_or(0, |i| i + 1)..entries.len())
        };
        for index in candidates {
            match entries[index] {
                Entry::Control(view) => {
                    if self.kernel().node(view).is_some() {
                        return Some(view);
                    }
                }
                Entry::List(view) => {
                    if let Some(next) = self.next_in_list(view, None, back) {
                        return Some(next);
                    }
                }
            }
        }
        None
    }

    fn next_in_list(
        &mut self,
        view: ViewId,
        current: Option<ViewId>,
        back: bool,
    ) -> Option<ViewId> {
        let snapshot = self.runner.collection(view)?;
        let row = current.and_then(|current| {
            snapshot
                .rows
                .iter()
                .find(|row| self.inside(current, row.view))
        });
        if let Some(row) = row {
            if let Some(next) = self.next_in(&[row.root], current, back) {
                return Some(next);
            }
        }
        let indices: Box<dyn Iterator<Item = usize>> = if back {
            Box::new((0..row.map_or(snapshot.count, |r| r.index)).rev())
        } else {
            Box::new(row.map_or(0, |r| r.index + 1)..snapshot.count)
        };
        for index in indices {
            let root = self
                .runner
                .collection(view)?
                .rows
                .iter()
                .find(|row| row.index == index)
                .map(|row| row.root);
            // Even an already mounted row takes over an outstanding seek:
            // several keyboard steps can fit within one prefetched window.
            match self.runner.reveal_collection_index(view, index) {
                Ok(Some(_)) => {
                    if root.is_none() {
                        self.after_commit();
                    }
                }
                Ok(None) => return None,
                Err(error) => {
                    eprintln!("terminal list #{view}: {error:?}");
                    return None;
                }
            }
            let root = match root {
                Some(root) => root,
                None => {
                    // Suppressed feedback keeps this provisional row present
                    // until the eventual target can carry its real focus pin.
                    // This also reaches a focusable zero-height row.
                    self.runner
                        .collection(view)?
                        .rows
                        .iter()
                        .find(|row| row.index == index)?
                        .root
                }
            };
            if let Some(next) = self.next_in(&[root], None, back) {
                return Some(next);
            }
        }
        None
    }

    pub(crate) fn step_collection_focus(&mut self, back: bool) -> bool {
        if self.collections.cursors.is_empty() {
            return false;
        }
        let settling = self.collections.settling;
        self.collections.settling = true;
        let roots = match self.layers.last() {
            Some((top, _)) => vec![*top],
            None => self.runner.roots(),
        };
        let next = self
            .next_in(&roots, self.focus, back)
            .or_else(|| self.next_in(&roots, None, back));
        if let Some(next) = next {
            self.focus(Some(next));
        }
        self.collections.settling = settling;
        self.settle_collections();
        true
    }
}

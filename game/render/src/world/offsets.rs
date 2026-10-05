//! Presentation offsets (`exact_game::Offset`) by row. A present writes a
//! row's revision only when its content changes, so the feed visits the rows
//! that changed, and an offset that changed moves the drawn poses of its own
//! subtree only: a walk cycle costs its walkers, not the world.
use exact_game::{Entity, Offset, Parent, World, PAGE};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Change {
    None,
    /// Only values changed: the same entities carry an offset.
    Values,
    /// An offset row appeared or went: the unparented overrides change too.
    Rows,
}

#[derive(Default)]
pub(super) struct Offsets {
    // The Offset column's revision and membership at the last feed; none
    // after a reset, when every row is new.
    since: Option<(u64, u64)>,
    // Every Parent edge as (parent index, child), sorted: descendants without a
    // world scan. Rebuilt with the feed's parented overrides.
    children: Vec<(u32, Entity)>,
    // Entities whose offset changed, appeared or went at this feed.
    roots: Vec<Entity>,
    stack: Vec<Entity>,
    /// The roots and their descendants, in entity order: every drawn pose an
    /// offset change moved at this feed.
    pub moved: Vec<Entity>,
    // Per history buffer: pages an offset moved that it has not rewritten yet.
    pending: [Vec<usize>; 2],
}

impl Offsets {
    pub fn reset(&mut self) {
        self.since = None;
        self.children.clear();
        self.roots.clear();
        self.moved.clear();
        self.pending.iter_mut().for_each(Vec::clear);
    }
    /// The entities whose offset changed, appeared or went since the last
    /// feed. O(rows changed).
    pub fn diff(&mut self, w: &World) -> Change {
        self.roots.clear();
        self.moved.clear();
        let now = (w.revision::<Offset>(), w.membership::<Offset>());
        let Some((revision, membership)) = self.since.replace(now) else {
            self.roots
                .extend(w.query::<&Offset>().iter().map(|(e, _)| e));
            return if self.roots.is_empty() {
                Change::None
            } else {
                Change::Rows
            };
        };
        if now.0 == revision {
            return Change::None;
        }
        // A slot whose row went with its entity reports its next occupant (or
        // none): the subtree of a living one is drawn without the old offset.
        self.roots
            .extend(w.changed::<Offset>(revision).filter(|&e| w.contains(e)));
        match (self.roots.is_empty(), now.1 != membership) {
            (_, true) => Change::Rows,
            (true, false) => Change::None,
            (false, false) => Change::Values,
        }
    }
    /// Index the hierarchy's edges; the feed calls this whenever it rebuilds
    /// its parented overrides (parents, membership or offset rows changed).
    pub fn index(&mut self, w: &World) {
        self.children.clear();
        self.children
            .extend(w.query::<&Parent>().iter().map(|(e, p)| (p.0.index(), e)));
        self.children.sort_unstable_by_key(|&(p, e)| (p, e.index()));
    }
    /// Fill `moved` with the changed roots and their descendants, bounded even
    /// if tools edited a cycle, and queue their pages for both history buffers.
    pub fn subtrees(&mut self, w: &World) {
        self.moved.clear();
        self.stack.clear();
        self.stack.extend_from_slice(&self.roots);
        let mut budget = w.len() + self.roots.len();
        while let Some(e) = self.stack.pop() {
            if budget == 0 {
                break;
            }
            budget -= 1;
            self.moved.push(e);
            let at = self.children.partition_point(|&(p, _)| p < e.index());
            for &(p, child) in &self.children[at..] {
                if p != e.index() {
                    break;
                }
                // An edge recorded for the slot's previous occupant is not this
                // entity's: confirm it against the live row.
                if w.get::<Parent>(child).is_some_and(|q| q.0 == e) {
                    self.stack.push(child);
                }
            }
        }
        self.moved.sort_unstable_by_key(|e| e.index());
        self.moved.dedup();
        for pending in &mut self.pending {
            pending.extend(self.moved.iter().map(|e| e.index() as usize / PAGE));
            pending.sort_unstable();
            pending.dedup();
        }
    }
    /// Pages history buffer `buffer` must rewrite before it is drawn again:
    /// what offsets moved at this feed and at feeds since it was last written.
    pub fn take_pending(&mut self, buffer: usize) -> std::vec::Drain<'_, usize> {
        self.pending[buffer].drain(..)
    }
    #[cfg(test)]
    pub fn roots(&self) -> &[Entity] {
        &self.roots
    }
}

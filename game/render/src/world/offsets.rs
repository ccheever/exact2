//! Presentation offsets (`exact_game::Offset`) by content. `Game::present`
//! rewrites every row each tick, so the feed compares values, and an offset that
//! changed moves the drawn poses of its own subtree only: a walk cycle costs its
//! walkers, not the world.
use exact_game::{Entity, Parent, Transform, World, PAGE};

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
    // Last fed rows in entity order, as bits so NaN compares equal to itself.
    rows: Vec<(Entity, [u32; 10])>,
    next: Vec<(Entity, [u32; 10])>,
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

fn bits(t: Transform) -> [u32; 10] {
    super::floats(t).map(f32::to_bits)
}

impl Offsets {
    pub fn reset(&mut self) {
        self.rows.clear();
        self.children.clear();
        self.roots.clear();
        self.moved.clear();
        self.pending.iter_mut().for_each(Vec::clear);
    }
    /// Compare this boundary's offsets with the last feed's, collecting the
    /// entities whose offset changed. O(offset rows).
    pub fn diff(&mut self, w: &World) -> Change {
        self.next.clear();
        self.next.extend(
            w.query::<&exact_game::Offset>()
                .iter()
                .map(|(e, o)| (e, bits(o.0))),
        );
        self.roots.clear();
        self.moved.clear();
        let (old, new) = (&self.rows, &self.next);
        let mut rows = old.len() != new.len();
        let (mut i, mut j) = (0, 0);
        while i < old.len() || j < new.len() {
            let a = old.get(i).map(|r| r.0);
            let b = new.get(j).map(|r| r.0);
            if a.is_some() && a == b {
                if old[i].1 != new[j].1 {
                    self.roots.push(old[i].0);
                }
                i += 1;
                j += 1;
                continue;
            }
            rows = true;
            match (a, b) {
                // Gone (or a different generation in the slot): its subtree is
                // drawn without it, if it still lives.
                (Some(a), b) if b.is_none_or(|b| a.index() <= b.index()) => {
                    if w.contains(a) {
                        self.roots.push(a);
                    }
                    i += 1;
                }
                (_, Some(b)) => {
                    self.roots.push(b);
                    j += 1;
                }
                _ => unreachable!("the loop runs while either list has rows"),
            }
        }
        std::mem::swap(&mut self.rows, &mut self.next);
        match (self.roots.is_empty(), rows) {
            (true, false) => Change::None,
            (_, true) => Change::Rows,
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

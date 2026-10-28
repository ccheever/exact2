//! The dependency table: what every binding, site, derive and resource
//! argument reads, scanned once from the plan when a runner boots.
//!
//! @ref LLP 1005 §8 (the Deps table and dirty-set sweep)
//!
//! An input is a root slot, a derive, a resource, a status flag or the
//! clock: one bit each in [`Bits`]. What a code body reads of its enclosing
//! scopes is a mask of relative frame depths (bit 0 is the innermost frame in
//! force where it runs; bit 63 stands for every frame from there out). A row
//! slot is a slot bit too, but its value lives in one row: [`RowWrites`]
//! names the rows an action wrote. A site's reads are the union over its subtree,
//! with frame masks shifted out of the scopes the subtree introduces, so an
//! update can skip a site when nothing its subtree reads has changed.

use crate::held::Held;
use crate::vm::{self, Env, RowSlots};
use exact_plan::{Code, Opcode, Plan, Stdlib, Value};

/// A set of input bits, held up to its last word with a bit set: most of a
/// plan's bodies read nothing that changes (a literal style row), and those
/// hold no words at all.
#[derive(Debug, Clone, Default, Eq)]
pub struct Bits(Box<[u64]>);

impl Bits {
    fn grow(&mut self, words: usize) {
        if words > self.0.len() {
            let mut grown = std::mem::take(&mut self.0).into_vec();
            grown.resize(words, 0);
            self.0 = grown.into_boxed_slice();
        }
    }
    pub(crate) fn set(&mut self, i: usize) {
        self.grow(i / 64 + 1);
        self.0[i / 64] |= 1 << (i % 64);
    }
    pub(crate) fn get(&self, i: usize) -> bool {
        self.0.get(i / 64).is_some_and(|w| w & (1 << (i % 64)) != 0)
    }
    pub(crate) fn union(&mut self, other: &Bits) {
        self.grow(
            other
                .0
                .iter()
                .rposition(|w| *w != 0)
                .map_or(0, |last| last + 1),
        );
        for (a, b) in self.0.iter_mut().zip(other.0.iter()) {
            *a |= b;
        }
    }
    pub(crate) fn intersects(&self, other: &Bits) -> bool {
        self.0.iter().zip(other.0.iter()).any(|(a, b)| a & b != 0)
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.0.iter().all(|w| *w == 0)
    }
}

/// Hashed as [`PartialEq`] compares: up to the last word with a bit set.
impl std::hash::Hash for Bits {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let words = self
            .0
            .iter()
            .rposition(|w| *w != 0)
            .map_or(0, |last| last + 1);
        self.0[..words].hash(state);
    }
}

/// Equal when they hold the same bits, whatever words each holds.
impl PartialEq for Bits {
    fn eq(&self, other: &Bits) -> bool {
        let (short, long) = match self.0.len() <= other.0.len() {
            true => (&self.0, &other.0),
            false => (&other.0, &self.0),
        };
        short[..] == long[..short.len()] && long[short.len()..].iter().all(|w| *w == 0)
    }
}

/// Where each kind of input sits in [`Bits`].
#[derive(Debug, Clone, Copy)]
struct Layout {
    slots: usize,
    derives: usize,
    resources: usize,
    mutations: usize,
}

impl Layout {
    fn derive(&self, i: usize) -> usize {
        self.slots + i
    }
    fn resource(&self, i: usize) -> usize {
        self.slots + self.derives + i
    }
    fn pending_resource(&self, i: usize) -> usize {
        self.slots + self.derives + self.resources + i
    }
    fn pending_mutation(&self, i: usize) -> usize {
        self.slots + self.derives + 2 * self.resources + i
    }
    fn failed_resource(&self, i: usize) -> usize {
        self.slots + self.derives + 2 * self.resources + self.mutations + i
    }
    fn clock(&self) -> usize {
        self.slots + self.derives + 3 * self.resources + self.mutations
    }
    fn width(&self) -> usize {
        self.clock() + 1
    }
}

/// What one code body, or the union over a subtree, reads.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Reads {
    /// Inputs, by [`Layout`].
    pub(crate) bits: Bits,
    /// Enclosing frames read, by relative depth (bit 63: that far or farther).
    pub(crate) frames: u64,
    /// The fields of the innermost frame's value read (bit `k` for field
    /// `k`, bit 63 for 63 and past; every bit when read other than one
    /// field at a time): LLP 1017.003 D6. Meaningful with bit 0 of `frames`.
    pub(crate) fields: u64,
    /// Enclosing frames whose POSITION is read (`each item, i in list`,
    /// LoadIndex), by relative depth as `frames`: bit 0 says a row's body
    /// reads its own position, at any depth, so a row that moved must
    /// re-evaluate (LLP 1062 D8); one that reads none keeps its bindings.
    pub(crate) positions: u64,
    /// Whether any row slot is among `bits`.
    pub(crate) row_slots: bool,
    /// Reads a parameter or has an effect: never skipped.
    pub(crate) opaque: bool,
}

impl Reads {
    /// Reads no input, frame, row slot or parameter: the same value always.
    pub(crate) fn is_constant(&self) -> bool {
        !self.opaque && !self.row_slots && self.frames == 0 && self.bits.is_empty()
    }

    fn union(&mut self, other: &Reads, shift: u32) {
        self.bits.union(&other.bits);
        let frames = out_of(other.frames, shift);
        if shift == 0 {
            self.fields |= other.fields;
        } else if frames & 1 != 0 {
            // A scope further in read this frame as an outer one: which
            // fields is not recorded that far out.
            self.fields = !0;
        }
        self.frames |= frames;
        self.positions |= out_of(other.positions, shift);
        self.row_slots |= other.row_slots;
        self.opaque |= other.opaque;
    }
    /// Reads nothing that can change: the value it had at creation stands.
    #[cfg(test)]
    pub(crate) fn constant(&self) -> bool {
        self.bits.is_empty() && self.frames == 0 && !self.opaque
    }
    /// Whether these reads, a row's body's, read the row's own position.
    pub(crate) fn reads_position(&self) -> bool {
        self.positions & 1 != 0
    }
    /// The same reads seen from `shift` scopes further out.
    pub(crate) fn frames_outside(&self, shift: u32) -> u64 {
        out_of(self.frames, shift)
    }
}

/// A frame mask seen from `shift` scopes further out. Bit 63 means "that far
/// or farther", so it spreads over every depth it may now stand for.
fn out_of(frames: u64, shift: u32) -> u64 {
    if shift == 0 {
        return frames;
    }
    let shift = shift.min(63);
    let far = if frames >> 63 != 0 {
        !0 << (63 - shift)
    } else {
        0
    };
    (frames >> shift) | far
}

/// A dirty-frame mask seen from one scope further in, whose own frame is
/// `dirty`; bit 63 stays set once set.
pub(crate) fn into_scope(frames: u64, dirty: bool) -> u64 {
    (frames << 1) | (frames & (1 << 63)) | u64::from(dirty)
}

/// Read sets by index, each distinct set held once: most of a plan's
/// bindings are literal rows, which read nothing, and most of the rest read
/// what another reads (euxy: 5,651 bindings, 85 distinct sets).
#[derive(Debug, Default)]
pub(crate) struct ReadsTable {
    /// Each index's place in `reads`; 0 is the empty set.
    at: Vec<u32>,
    reads: Vec<Reads>,
}

impl FromIterator<Reads> for ReadsTable {
    fn from_iter<I: IntoIterator<Item = Reads>>(all: I) -> Self {
        let all = all.into_iter();
        let mut at = Vec::with_capacity(all.size_hint().0);
        let mut places: std::collections::HashMap<Reads, u32> = Default::default();
        for reads in all {
            let place = match reads == Reads::default() {
                true => 0,
                false => {
                    let next = places.len() as u32 + 1;
                    *places.entry(reads).or_insert(next)
                }
            };
            at.push(place);
        }
        let mut reads = vec![Reads::default(); places.len() + 1];
        for (set, place) in places {
            reads[place as usize] = set;
        }
        ReadsTable { at, reads }
    }
}

impl std::ops::Index<usize> for ReadsTable {
    type Output = Reads;
    fn index(&self, i: usize) -> &Reads {
        &self.reads[self.at[i] as usize]
    }
}

/// Per binding, surface argument, site, derive and resource argument list.
#[derive(Debug, Default)]
pub struct Deps {
    layout: Option<Layout>,
    pub(crate) bindings: ReadsTable,
    pub(crate) surface_args: ReadsTable,
    /// A node's own reads and every descendant's, relative to the node.
    pub(crate) nodes: ReadsTable,
    /// A region's subject, relative to the region.
    pub(crate) subjects: ReadsTable,
    /// A region's key, relative to the row (bit 0 is the item).
    pub(crate) keys: ReadsTable,
    /// Exactly the innermost row's position; identities survive item changes.
    pub(crate) index_keys: Vec<bool>,
    /// A region's arms' sites, relative to the row or arm frame.
    pub(crate) bodies: ReadsTable,
    /// Subject, key and body, relative to the region.
    pub(crate) regions: ReadsTable,
    /// Each derive's body.
    pub(crate) derives: Vec<Reads>,
    /// Each resource's arguments together.
    pub(crate) resource_args: Vec<Reads>,
}

/// One input a settlement body reads, decoded from [`Reads::bits`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Input {
    Slot(usize),
    Derive(usize),
    Resource(usize),
    PendingResource(usize),
    PendingMutation(usize),
    FailedResource(usize),
    Clock,
}

impl Deps {
    /// Scan every body once and fold sites bottom-up over `children`.
    pub(crate) fn new(plan: &Plan, children: &super::SiteIndex) -> Self {
        let layout = Layout {
            slots: plan.slots.len(),
            derives: plan.derives.len(),
            resources: plan.resources.len(),
            mutations: plan.mutations.len(),
        };
        let scan = |code: Code| scan(plan, layout, code);
        let mut deps = Deps {
            layout: Some(layout),
            bindings: plan.bindings.iter().map(|b| scan(b.expr)).collect(),
            surface_args: plan.surface_args.iter().map(|a| scan(a.expr)).collect(),
            nodes: ReadsTable::default(),
            subjects: plan.regions.iter().map(|r| scan(r.subject)).collect(),
            keys: plan.regions.iter().map(|r| scan(r.key)).collect(),
            index_keys: plan
                .regions
                .iter()
                .map(|r| is_index_key(plan.code(r.key)))
                .collect(),
            bodies: ReadsTable::default(),
            regions: ReadsTable::default(),
            derives: plan.derives.iter().map(|d| scan(d.body)).collect(),
            resource_args: plan
                .resources
                .iter()
                .map(|r| {
                    let mut reads = Reads::default();
                    for a in r.args.iter() {
                        reads.union(&scan(plan.arg(a).expr), 0);
                    }
                    reads
                })
                .collect(),
        };
        // Post-order over the site tree without recursing to the plan's depth;
        // the sites' reads are tabled once every one is known.
        let mut nodes = vec![Reads::default(); plan.nodes.len()];
        let mut bodies = vec![Reads::default(); plan.regions.len()];
        let mut regions = vec![Reads::default(); plan.regions.len()];
        let mut stack: Vec<(super::Site, bool)> = children
            .children(None, None)
            .iter()
            .map(|(_, s)| (*s, false))
            .collect();
        while let Some((site, done)) = stack.pop() {
            let kids = |site| match site {
                super::Site::Node(n) => children.children(Some(n), plan.node(n).arm).to_vec(),
                super::Site::Region(r) => plan
                    .region(r)
                    .arms
                    .iter()
                    .flat_map(|arm| children.children(None, Some(arm)).iter().copied())
                    .collect(),
            };
            if !done {
                stack.push((site, true));
                stack.extend(kids(site).into_iter().map(|(_, s)| (s, false)));
                continue;
            }
            let of = |site| match site {
                super::Site::Node(n) => &nodes[n.0 as usize],
                super::Site::Region(r) => &regions[r.0 as usize],
            };
            match site {
                super::Site::Node(n) => {
                    let node = plan.node(n);
                    let mut reads = Reads::default();
                    for b in node.bindings.iter() {
                        reads.union(&deps.bindings[b.0 as usize], 0);
                    }
                    if let Some(surface) = node.surface {
                        for a in plan.surface(surface).args.iter() {
                            reads.union(&deps.surface_args[a.0 as usize], 0);
                        }
                    }
                    for (_, kid) in kids(site) {
                        reads.union(of(kid), 0);
                    }
                    nodes[n.0 as usize] = reads;
                }
                super::Site::Region(r) => {
                    let i = r.0 as usize;
                    let mut body = Reads::default();
                    for (_, kid) in kids(site) {
                        body.union(of(kid), 0);
                    }
                    let mut all = deps.subjects[i].clone();
                    all.union(&deps.keys[i], 1);
                    all.union(&body, 1);
                    bodies[i] = body;
                    regions[i] = all;
                }
            }
        }
        deps.nodes = nodes.into_iter().collect();
        deps.bodies = bodies.into_iter().collect();
        deps.regions = regions.into_iter().collect();
        deps
    }

    /// What one code body reads, as the table's rows are scanned.
    pub(crate) fn reads_of(&self, plan: &Plan, code: Code) -> Reads {
        scan(plan, self.layout.expect("built from a plan"), code)
    }

    /// The inputs `reads` names, in bit order.
    pub(crate) fn inputs<'a>(&self, reads: &'a Reads) -> impl Iterator<Item = Input> + 'a {
        let l = self.layout.expect("built from a plan");
        (0..l.width()).filter(|i| reads.bits.get(*i)).map(move |i| {
            let (d, r, m) = (l.derive(0), l.resource(0), l.pending_resource(0));
            let (pm, clock) = (l.pending_mutation(0), l.clock());
            let failed = l.failed_resource(0);
            match i {
                _ if i < d => Input::Slot(i),
                _ if i < r => Input::Derive(i - d),
                _ if i < m => Input::Resource(i - r),
                _ if i < pm => Input::PendingResource(i - m),
                _ if i < failed => Input::PendingMutation(i - pm),
                _ if i < clock => Input::FailedResource(i - failed),
                _ => Input::Clock,
            }
        })
    }

    /// The inputs that differ between what the tree last showed and `env`.
    pub(crate) fn changed(&self, seen: &Seen, env: &Env<'_>) -> Bits {
        let layout = self.layout.expect("built from a plan");
        let mut bits = Bits::default();
        for (i, (old, new)) in seen.slots.iter().zip(env.slots).enumerate() {
            if !crate::compare::equivalent(old, new) {
                bits.set(i);
            }
        }
        let same = |a: &Option<Value>, b: &Option<Value>| match (a, b) {
            (Some(a), Some(b)) => crate::compare::same(a, b),
            (None, None) => true,
            _ => false,
        };
        for (i, (old, new)) in seen.derives.iter().zip(env.derives).enumerate() {
            if !same(old, new) {
                bits.set(layout.derive(i));
            }
        }
        for (i, (old, new)) in seen.resources.iter().zip(env.resources).enumerate() {
            let same = match (old, new) {
                (Some(a), Some(b)) => Held::same(a, b),
                (None, None) => true,
                _ => false,
            };
            if !same {
                bits.set(layout.resource(i));
            }
        }
        for (i, (old, new)) in seen
            .pending_resources
            .iter()
            .zip(env.pending_resources)
            .enumerate()
        {
            if old != new {
                bits.set(layout.pending_resource(i));
            }
        }
        for (i, (old, new)) in seen
            .pending_mutations
            .iter()
            .zip(env.pending_mutations)
            .enumerate()
        {
            if old != new || seen.failed_mutations.get(i) != env.failed_mutations.get(i) {
                bits.set(layout.pending_mutation(i));
            }
        }
        for (i, old) in seen.failed_resources.iter().enumerate() {
            // Its message too: `failure(x)` reads it (LLP 1109 D3).
            if *old != crate::failure::state(env.failed_resources, env.failed_why, i) {
                bits.set(layout.failed_resource(i));
            }
        }
        if seen.now_ms.to_bits() != env.now_ms.to_bits() {
            bits.set(layout.clock());
        }
        bits
    }
}

fn is_index_key(code: &[u8]) -> bool {
    let mut code = vm::instructions(code);
    matches!(code.next(), Some(Ok(i)) if i.op == Opcode::LoadIndex && i.args[0] == 0)
        && matches!(code.next(), Some(Ok(i)) if i.op == Opcode::Return)
        && code.next().is_none()
}

fn scan(plan: &Plan, layout: Layout, code: Code) -> Reads {
    let mut reads = Reads::default();
    let mut code = vm::instructions(plan.code(code)).peekable();
    while let Some(instruction) = code.next() {
        let Ok(i) = instruction else {
            reads.opaque = true;
            return reads;
        };
        let index = i.args[0] as usize;
        match i.op {
            Opcode::LoadSlot => {
                reads.bits.set(index);
                reads.row_slots |= plan.slots.get(index).is_none_or(|s| s.owner.is_some());
            }
            Opcode::LoadDerive => reads.bits.set(layout.derive(index)),
            Opcode::LoadResource => reads.bits.set(layout.resource(index)),
            Opcode::PendingResource => {
                // Known only once the resource settled, like its value.
                reads.bits.set(layout.resource(index));
                reads.bits.set(layout.pending_resource(index));
            }
            Opcode::PendingMutation | Opcode::FailedMutation | Opcode::FailureMutation => {
                reads.bits.set(layout.pending_mutation(index))
            }
            Opcode::FailedResource | Opcode::FailureResource => {
                reads.bits.set(layout.resource(index));
                reads.bits.set(layout.failed_resource(index));
            }
            Opcode::Call if Stdlib::from_wire(index as u8) == Some(Stdlib::PerformanceNow) => {
                reads.bits.set(layout.clock())
            }
            // A share's files arrive by `set_shared`, after the location (LLP
            // 1133.024 D3): no input the scan sees changes with them.
            Opcode::Call if Stdlib::from_wire(index as u8) == Some(Stdlib::SharedFiles) => {
                reads.opaque = true
            }
            Opcode::LoadItem | Opcode::LoadBound | Opcode::LoadIndex => {
                reads.frames |= 1 << index.min(63);
                if i.op == Opcode::LoadIndex {
                    reads.positions |= 1 << index.min(63);
                }
                if index == 0 {
                    // One field of the innermost frame's value, or all of it
                    // (its position, LLP 1062 D8, counts as all of it).
                    reads.fields |= match code.peek() {
                        Some(Ok(next)) if i.op != Opcode::LoadIndex && next.op == Opcode::Field => {
                            1 << next.args[0].min(63)
                        }
                        _ => !0,
                    };
                }
            }
            Opcode::LoadParam
            | Opcode::StoreSlot
            | Opcode::Command
            | Opcode::Send
            | Opcode::Refresh => reads.opaque = true,
            _ => {}
        }
    }
    reads
}

/// What the tree last showed: the inputs an update compares against.
#[derive(Debug, Default)]
pub struct Seen {
    slots: Vec<Value>,
    derives: Vec<Option<Value>>,
    resources: Vec<Option<Held>>,
    pending_resources: Vec<bool>,
    pending_mutations: Vec<bool>,
    failed_mutations: Vec<Option<crate::failure::Failure>>,
    failed_resources: Vec<Option<crate::failure::Failure>>,
    now_ms: f64,
}

impl Seen {
    /// Resource `i` is now `held`, the same value for every reader
    /// ([`Held::released`]): what the tree showed holds it no more.
    pub(crate) fn release(&mut self, i: usize, held: &Held) {
        if let Some(Some(old)) = self.resources.get_mut(i) {
            if Held::same(old, held) {
                *old = held.clone();
            }
        }
    }

    pub(crate) fn of(env: &Env<'_>) -> Self {
        Seen {
            slots: env.slots.to_vec(),
            derives: env.derives.to_vec(),
            resources: env.resources.to_vec(),
            pending_resources: env.pending_resources.to_vec(),
            pending_mutations: env.pending_mutations.to_vec(),
            failed_mutations: env.failed_mutations.to_vec(),
            failed_resources: crate::failure::states(env.failed_resources, env.failed_why),
            now_ms: env.now_ms,
        }
    }
}

/// Rows an action wrote since the last update: each row's slots and the
/// rows enclosing it, by identity. A row-slot write dirties only its row.
#[derive(Debug, Default, Clone)]
pub struct RowWrites {
    /// Every row on the written rows' scope chains.
    pub(crate) path: Vec<usize>,
    /// `(row, slot)` for each write.
    pub(crate) writes: Vec<(usize, u32)>,
}

impl RowWrites {
    /// The identity of a row's slot storage.
    pub(crate) fn id(row: &RowSlots) -> usize {
        std::rc::Rc::as_ptr(row) as *const () as usize
    }
    /// Record that `slot` of the innermost row in `frames` owning it was written.
    pub(crate) fn record(&mut self, frames: &[vm::Frame], row: &RowSlots, slot: u32) {
        for frame in frames {
            if let Some(r) = &frame.row {
                let id = Self::id(r);
                if !self.path.contains(&id) {
                    self.path.push(id);
                }
            }
        }
        let id = Self::id(row);
        if !self.path.contains(&id) {
            self.path.push(id);
        }
        self.writes.push((id, slot));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_masks_shift_with_a_sticky_far_bit() {
        assert_eq!(out_of(0b110, 1), 0b11);
        assert_eq!(out_of(0b1, 1), 0);
        assert_eq!(out_of(1 << 63, 1), 0b11 << 62);
        assert_eq!(into_scope(0b1, true), 0b11);
        assert_eq!(into_scope(0b1, false), 0b10);
        assert_eq!(into_scope(1 << 62, false), 1 << 63);
        assert_eq!(into_scope(1 << 63, false), 1 << 63);
    }
}

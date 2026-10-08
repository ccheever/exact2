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

/// A fixed-width set of input bits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bits(Box<[u64]>);

impl Bits {
    fn new(width: usize) -> Self {
        Bits(vec![0; width.div_ceil(64)].into_boxed_slice())
    }
    pub(crate) fn set(&mut self, i: usize) {
        self.0[i / 64] |= 1 << (i % 64);
    }
    pub(crate) fn get(&self, i: usize) -> bool {
        self.0.get(i / 64).is_some_and(|w| w & (1 << (i % 64)) != 0)
    }
    pub(crate) fn union(&mut self, other: &Bits) {
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reads {
    /// Inputs, by [`Layout`].
    pub(crate) bits: Bits,
    /// Enclosing frames read, by relative depth (bit 63: that far or farther).
    pub(crate) frames: u64,
    /// The fields of the innermost frame's value read (bit `k` for field
    /// `k`, bit 63 for 63 and past; every bit when read other than one
    /// field at a time): LLP 1017.003 D6. Meaningful with bit 0 of `frames`.
    pub(crate) fields: u64,
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
        self.row_slots |= other.row_slots;
        self.opaque |= other.opaque;
    }
    /// Reads nothing that can change: the value it had at creation stands.
    #[cfg(test)]
    pub(crate) fn constant(&self) -> bool {
        self.bits.is_empty() && self.frames == 0 && !self.opaque
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

/// Per binding, surface argument, site, derive and resource argument list.
#[derive(Debug, Default)]
pub struct Deps {
    layout: Option<Layout>,
    pub(crate) bindings: Vec<Reads>,
    pub(crate) surface_args: Vec<Reads>,
    /// A node's own reads and every descendant's, relative to the node.
    pub(crate) nodes: Vec<Reads>,
    /// A region's subject, relative to the region.
    pub(crate) subjects: Vec<Reads>,
    /// A region's key, relative to the row (bit 0 is the item).
    pub(crate) keys: Vec<Reads>,
    /// A region's arms' sites, relative to the row or arm frame.
    pub(crate) bodies: Vec<Reads>,
    /// Subject, key and body, relative to the region.
    pub(crate) regions: Vec<Reads>,
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
            nodes: vec![Reads::default(); plan.nodes.len()],
            subjects: plan.regions.iter().map(|r| scan(r.subject)).collect(),
            keys: plan.regions.iter().map(|r| scan(r.key)).collect(),
            bodies: vec![Reads::default(); plan.regions.len()],
            regions: vec![Reads::default(); plan.regions.len()],
            derives: plan.derives.iter().map(|d| scan(d.body)).collect(),
            resource_args: plan
                .resources
                .iter()
                .map(|r| {
                    let mut reads = empty(layout);
                    for a in r.args.iter() {
                        reads.union(&scan(plan.arg(a).expr), 0);
                    }
                    reads
                })
                .collect(),
        };
        // Post-order over the site tree without recursing to the plan's depth.
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
            match site {
                super::Site::Node(n) => {
                    let node = plan.node(n);
                    let mut reads = empty(layout);
                    for b in node.bindings.iter() {
                        reads.union(&deps.bindings[b.0 as usize], 0);
                    }
                    if let Some(surface) = node.surface {
                        for a in plan.surface(surface).args.iter() {
                            reads.union(&deps.surface_args[a.0 as usize], 0);
                        }
                    }
                    for (_, kid) in kids(site) {
                        reads.union(deps.site(kid), 0);
                    }
                    deps.nodes[n.0 as usize] = reads;
                }
                super::Site::Region(r) => {
                    let i = r.0 as usize;
                    let mut body = empty(layout);
                    for (_, kid) in kids(site) {
                        body.union(deps.site(kid), 0);
                    }
                    let mut all = deps.subjects[i].clone();
                    all.union(&deps.keys[i], 1);
                    all.union(&body, 1);
                    deps.bodies[i] = body;
                    deps.regions[i] = all;
                }
            }
        }
        deps
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

    fn site(&self, site: super::Site) -> &Reads {
        match site {
            super::Site::Node(n) => &self.nodes[n.0 as usize],
            super::Site::Region(r) => &self.regions[r.0 as usize],
        }
    }

    /// The inputs that differ between what the tree last showed and `env`.
    pub(crate) fn changed(&self, seen: &Seen, env: &Env<'_>) -> Bits {
        let layout = self.layout.expect("built from a plan");
        let mut bits = Bits::new(layout.width());
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
            if old != new {
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

fn empty(layout: Layout) -> Reads {
    Reads {
        bits: Bits::new(layout.width()),
        ..Reads::default()
    }
}

fn scan(plan: &Plan, layout: Layout, code: Code) -> Reads {
    let mut reads = empty(layout);
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
            Opcode::PendingMutation => reads.bits.set(layout.pending_mutation(index)),
            Opcode::FailedResource | Opcode::FailureResource => {
                reads.bits.set(layout.resource(index));
                reads.bits.set(layout.failed_resource(index));
            }
            Opcode::Call if Stdlib::from_wire(index as u8) == Some(Stdlib::PerformanceNow) => {
                reads.bits.set(layout.clock())
            }
            Opcode::LoadItem | Opcode::LoadBound | Opcode::LoadIndex => {
                reads.frames |= 1 << index.min(63);
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

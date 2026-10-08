//! EXACT PATCH 29: a subtree's layout, remembered by what the algorithms read.
//!
//! A list row rebound to another item (or a new row of the same shape) is
//! laid out by the same algorithms over the same styles; only its leaves'
//! answers can differ, and for most rows they do not. Laying out a marked
//! root records a trace: every call its subtree's algorithms make through
//! the tree, in order. A box an algorithm computes is a [`Event::Visit`] (its
//! style, and each child's style and child count: what a parent reads of a
//! child); an answer that comes from outside the computation (a leaf's
//! measure, a cache hit, a hidden or replaced box) is a [`Event::Query`]; what
//! the computation writes is a [`Event::Store`], [`Event::Layout`] or
//! [`Event::Static`].
//!
//! The algorithms are deterministic in what they read, so a later layout of
//! an equal root under the same input makes the same first call, and, given
//! the same answer, the same second. Replaying a trace therefore proves each
//! step before taking it: a visit compares styles, a query is asked for real
//! of the box at that place and its answer compared, and the writes between
//! are the writes the algorithms would make. At the first difference the
//! replay stops and the root is computed (and recorded) as before; what the
//! replay wrote until then is what that computation writes first.
//!
//! One part of an answer is not compared: a box's scrollable overflow that
//! lies inside its own border box (a text's ink is narrower than its box by
//! an amount every string differs in). Every algorithm does two things with
//! a child's overflow: it copies it into the child's layout, and it gives it
//! to `compute_scrollable_overflow_contribution`, which answers the border
//! box alone for any overflow inside it. So such answers are one outcome,
//! and a replay writes the child's layout with the overflow the child gave
//! this time. A recorded layout that did otherwise is not kept.
//!
//! Nor is what no algorithm reads of an answer: of a size asked for
//! (`ComputeSize`), anything but the size and its collapsing margins, which
//! is all the cache keeps of one.
//!
//! A call the trace already holds an answer for is not recorded again: a
//! leaf answers it the same, from its cache or measured again, and nothing
//! but the cache's own order of use is touched by asking.
//!
//! Traces share their common starts in a trie (`alt` links the outcomes seen
//! at one place), so a lookup is one walk whatever the number of rows seen.
//! Nothing here holds a node: places are indices in the order boxes were
//! first read, so a trace made by one row serves any other.
use super::{Layout, LayoutInput, LayoutOutput, StaticPosition};
use crate::geometry::{Rect, Size};
use crate::style::Style;
use std::rc::Rc;
use std::vec::Vec;

/// No step.
pub(crate) const NONE: u32 = u32::MAX;

/// Steps kept before the memo starts over: every shape and answer a list
/// showed, a few megabytes at most.
const MAX_STEPS: usize = 24_000;

/// Misses in a row after which a tree's roots are computed without a replay
/// or a record, but for one in [`RETRY`] (rows whose every text is new).
const GIVE_UP: u32 = 24;
const RETRY: u32 = 16;

/// What a parent's algorithm reads of a child before asking it anything.
#[derive(Debug, Clone)]
pub(crate) struct Kid {
    pub(crate) style: Rc<Style>,
    pub(crate) children: u32,
}

/// One call through the tree during a recorded layout. `node` is the box's
/// index in the order the trace first read it (the root is 0).
#[derive(Debug, Clone)]
pub(crate) enum Event {
    /// A container computed under `input`: its children take the indices
    /// from `first` on ([`NONE`] and no `kids` when an earlier visit read
    /// them).
    Visit { node: u32, input: LayoutInput, style: Rc<Style>, first: u32, kids: Rc<[Kid]> },
    /// An answer from outside the computation.
    Query { node: u32, input: LayoutInput, output: LayoutOutput },
    /// A visited container's result, as its cache takes it.
    Store { node: u32, input: LayoutInput, output: LayoutOutput },
    /// A layout written.
    Layout { node: u32, layout: Layout },
    /// A static position written.
    Static { node: u32, position: StaticPosition },
    /// The root's result.
    End { output: LayoutOutput },
}

/// Whether an overflow lies inside the border box of `size`.
fn inside(overflow: Rect<f32>, size: Size<f32>) -> bool {
    overflow.left >= 0.0 && overflow.top >= 0.0 && overflow.right <= size.width && overflow.bottom <= size.height
}

/// Whether two answers to `input` are one outcome. A size asked for is its
/// size and margins: the cache answers a size with those alone
/// (`LayoutOutput::from_outer_size`), so no algorithm reads more of one. A
/// final layout is all of it, but for overflows inside their boxes.
pub(crate) fn same_answer(input: &LayoutInput, a: &LayoutOutput, b: &LayoutOutput) -> bool {
    if input.run_mode == super::RunMode::ComputeSize {
        return a.size == b.size
            && a.top_margin == b.top_margin
            && a.bottom_margin == b.bottom_margin
            && a.margins_can_collapse_through == b.margins_can_collapse_through;
    }
    a == b
        || (inside(a.scrollable_overflow_rect, a.size)
            && inside(b.scrollable_overflow_rect, b.size)
            && LayoutOutput { scrollable_overflow_rect: b.scrollable_overflow_rect, ..*a } == *b)
}

/// The same for two layouts written.
fn same_layout(a: &Layout, b: &Layout) -> bool {
    a == b
        || (inside(a.scrollable_overflow_rect, a.size)
            && inside(b.scrollable_overflow_rect, b.size)
            && Layout { scrollable_overflow_rect: b.scrollable_overflow_rect, ..*a } == *b)
}

pub(crate) fn same_style(a: &Rc<Style>, b: &Rc<Style>) -> bool {
    Rc::ptr_eq(a, b) || a == b
}

impl Event {
    /// Whether `self` and `other` are the same step of a trace.
    fn same(&self, other: &Event) -> bool {
        match (self, other) {
            (
                Event::Visit { node: a, input: ai, style: s, first: af, kids: ak },
                Event::Visit { node: b, input: bi, style: t, first: bf, kids: bk },
            ) => {
                a == b
                    && ai == bi
                    && af == bf
                    && same_style(s, t)
                    && ak.len() == bk.len()
                    && ak.iter().zip(bk.iter()).all(|(x, y)| x.children == y.children && same_style(&x.style, &y.style))
            }
            (
                Event::Query { node: a, input: ai, output: ao },
                Event::Query { node: b, input: bi, output: bo },
            ) => a == b && ai == bi && same_answer(ai, ao, bo),
            (
                Event::Store { node: a, input: ai, output: ao },
                Event::Store { node: b, input: bi, output: bo },
            ) => a == b && ai == bi && same_answer(ai, ao, bo),
            (Event::Layout { node: a, layout: x }, Event::Layout { node: b, layout: y }) => a == b && same_layout(x, y),
            (Event::Static { node: a, position: x }, Event::Static { node: b, position: y }) => a == b && x == y,
            (Event::End { output: a }, Event::End { output: b }) => a == b,
            _ => false,
        }
    }

    /// The call this step is an outcome of, where another outcome may stand
    /// beside it: the box asked and under what.
    pub(crate) fn call(&self) -> Option<(u32, &LayoutInput)> {
        match self {
            Event::Visit { node, input, .. } | Event::Query { node, input, .. } => Some((*node, input)),
            _ => None,
        }
    }
}

/// A step of the trie: the step after it, and another outcome at its place.
#[derive(Debug, Clone)]
pub(crate) struct Step {
    pub(crate) event: Event,
    pub(crate) next: u32,
    pub(crate) alt: u32,
}

/// The traces of one tree's marked roots.
#[derive(Debug, Clone)]
pub(crate) struct Memo {
    pub(crate) steps: Vec<Step>,
    /// The first step of every trace (its alternatives are the other root
    /// styles and inputs seen).
    pub(crate) first: u32,
    /// Misses since the last hit.
    misses: u32,
    /// Roots laid out since the memo gave up.
    skipped: u32,
    /// Marked roots are laid out as any other box.
    pub(crate) off: bool,
    /// A replayed root's boxes by index, kept for its capacity.
    pub(crate) nodes: Vec<super::NodeId>,
    /// The overflow each box last answered a final layout with, where the
    /// replay asked it.
    pub(crate) overflows: Vec<Option<Rect<f32>>>,
    /// The containers a replay has stored a result for.
    pub(crate) stored: Vec<super::NodeId>,
    /// Replays that reached their end, and layouts recorded.
    pub(crate) hits: usize,
    pub(crate) records: usize,
}

impl Default for Memo {
    fn default() -> Self {
        Memo {
            steps: Vec::new(),
            first: NONE,
            misses: 0,
            skipped: 0,
            off: false,
            nodes: Vec::new(),
            overflows: Vec::new(),
            stored: Vec::new(),
            hits: 0,
            records: 0,
        }
    }
}

impl Memo {
    /// Whether this root is worth a replay and a record: always, until
    /// nothing has matched for a while.
    pub(crate) fn attempt(&mut self) -> bool {
        if self.misses < GIVE_UP {
            return true;
        }
        self.skipped += 1;
        if self.skipped >= RETRY {
            self.skipped = 0;
            return true;
        }
        false
    }

    /// A replay reached its end.
    pub(crate) fn hit(&mut self) {
        self.misses = 0;
        self.hits += 1;
    }

    /// Forget every trace.
    pub(crate) fn clear(&mut self) {
        self.steps.clear();
        self.first = NONE;
    }

    /// Add a recorded trace: it joins the trie where it first differs from
    /// the traces held, as another outcome at that place. (Where the
    /// difference is not an outcome of one call, a container whose cache
    /// had dropped an answer the other trace found there, a replay never
    /// takes it: it follows the first, which computes the same.)
    pub(crate) fn insert(&mut self, events: Vec<Event>) {
        self.misses = self.misses.saturating_add(1);
        self.records += 1;
        if self.misses >= GIVE_UP {
            // Given up: the traces held matched nothing for a while, and
            // their storage goes with them (crypto's feed, whose rows never
            // replay, held all 24,000 steps: 4.2 MB). This trace alone is
            // kept, for the next root attempted to be tried against.
            self.steps = Vec::new();
            self.first = NONE;
        } else if self.steps.len() + events.len() > MAX_STEPS {
            self.clear();
        }
        // The step whose `next` leads to this place (none: `first`).
        let mut prev: Option<u32> = None;
        let mut events = events.into_iter();
        while let Some(event) = events.next() {
            let mut at = prev.map_or(self.first, |s| self.steps[s as usize].next);
            let mut found = None;
            let mut last = NONE;
            while at != NONE {
                let step = &self.steps[at as usize];
                if step.event.same(&event) {
                    found = Some(at);
                    break;
                }
                last = at;
                at = step.alt;
            }
            if let Some(step) = found {
                prev = Some(step);
                continue;
            }
            // New from here: this step beside the outcomes held, the rest
            // one after another.
            let mut index = self.steps.len() as u32;
            self.steps.push(Step { event, next: NONE, alt: NONE });
            match (last, prev) {
                (NONE, None) => self.first = index,
                (NONE, Some(s)) => self.steps[s as usize].next = index,
                (last, _) => self.steps[last as usize].alt = index,
            }
            for event in events.by_ref() {
                self.steps.push(Step { event, next: NONE, alt: NONE });
                self.steps[index as usize].next = index + 1;
                index += 1;
            }
            return;
        }
    }
}

/// A layout being recorded: its events, and each box's index.
#[derive(Debug)]
pub(crate) struct Recorder {
    pub(crate) events: Vec<Event>,
    /// Each read box's index, and whether it was visited (its children read).
    index: std::collections::HashMap<super::NodeId, (u32, bool), core::hash::BuildHasherDefault<IdHasher>>,
    count: u32,
    /// The overflow each box last gave a final layout with, by index.
    overflows: Vec<Option<Rect<f32>>>,
    /// The calls the trace holds an answer or a result for.
    asked: Vec<(u32, LayoutInput)>,
    /// Something was read that the trace cannot hold: it is not kept.
    pub(crate) poisoned: bool,
}

/// Node ids are slot keys: their bits are spread, not hashed.
#[derive(Default)]
pub(crate) struct IdHasher(u64);
impl core::hash::Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

impl Recorder {
    pub(crate) fn new(root: super::NodeId) -> Self {
        let mut index = std::collections::HashMap::default();
        index.insert(root, (0, false));
        Recorder { events: Vec::with_capacity(256), index, count: 1, overflows: Vec::new(), asked: Vec::new(), poisoned: false }
    }

    /// The index of a box the trace has read, if it has.
    pub(crate) fn index(&mut self, node: super::NodeId) -> Option<u32> {
        let found = self.index.get(&node).map(|(i, _)| *i);
        self.poisoned |= found.is_none();
        found
    }

    /// Whether `node`'s children were read into the trace (it was visited).
    pub(crate) fn visited(&self, node: super::NodeId) -> bool {
        self.index.get(&node).is_some_and(|(_, visited)| *visited)
    }

    /// Whether the trace has read `node` (a visited box or a child of one).
    pub(crate) fn read(&self, node: super::NodeId) -> bool {
        self.index.contains_key(&node)
    }

    /// Note `node` visited; the index its first child takes, `None` when its
    /// children already have theirs (a second visit under another input).
    pub(crate) fn visit(&mut self, node: super::NodeId) -> Option<u32> {
        let entry = self.index.get_mut(&node)?;
        if entry.1 {
            return None;
        }
        entry.1 = true;
        Some(self.count)
    }

    /// Whether the trace already holds what `node` gives under `input`: a
    /// cache then answers the same again, and a replay need not ask twice.
    pub(crate) fn repeats(&self, node: u32, input: &LayoutInput) -> bool {
        self.asked.iter().any(|(n, i)| *n == node && i == input)
    }

    /// Note a final layout's overflow, as a box answered or was computed.
    pub(crate) fn answered(&mut self, node: u32, input: &LayoutInput, output: &LayoutOutput) {
        self.asked.push((node, *input));
        if input.run_mode == super::RunMode::PerformLayout {
            if self.overflows.len() <= node as usize {
                self.overflows.resize(node as usize + 1, None);
            }
            self.overflows[node as usize] = Some(output.scrollable_overflow_rect);
        }
    }

    /// A layout written for `node`: it carries the overflow the box last
    /// gave, or the trace is not one a replay can stand in for.
    pub(crate) fn wrote(&mut self, node: u32, layout: &Layout) {
        if let Some(Some(overflow)) = self.overflows.get(node as usize) {
            self.poisoned |= *overflow != layout.scrollable_overflow_rect;
        }
        self.events.push(Event::Layout { node, layout: *layout });
    }

    /// Boxes read so far.
    pub(crate) fn count(&self) -> u32 {
        self.count
    }

    /// Give a visited box's child the next index.
    pub(crate) fn child(&mut self, node: super::NodeId) {
        // A box read twice (it cannot be: a child has one parent) keeps the
        // trace out rather than its indices wrong.
        self.poisoned |= self.index.insert(node, (self.count, false)).is_some();
        self.count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trace of `len` steps no other trace shares a start with.
    fn trace(seed: u32, len: usize) -> Vec<Event> {
        (0..len).map(|k| Event::Layout { node: seed.wrapping_mul(1000) + k as u32, layout: Layout::new() }).collect()
    }

    #[test]
    fn a_memo_that_gave_up_holds_one_trace() {
        let mut memo = Memo::default();
        for seed in 0..GIVE_UP - 1 {
            memo.insert(trace(seed, 100));
        }
        assert_eq!(memo.steps.len(), 100 * (GIVE_UP as usize - 1), "every trace until it gives up");
        memo.insert(trace(GIVE_UP, 100));
        assert_eq!(memo.steps.len(), 100, "then the last alone");
        assert!(memo.steps.capacity() < 400, "and the others' storage is freed");
        memo.insert(trace(GIVE_UP + 1, 100));
        assert_eq!(memo.steps.len(), 100);
        // A hit starts it keeping traces again.
        memo.hit();
        memo.insert(trace(GIVE_UP + 2, 100));
        memo.insert(trace(GIVE_UP + 3, 100));
        assert_eq!(memo.steps.len(), 300);
    }
}

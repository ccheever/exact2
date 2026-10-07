//! Named timeline lookup (LLP 1057.003 D4; Scroll-driven Animations 1 §4.2).
//!
//! A named timeline is referenceable by the node that declares it and its
//! descendants; `timeline-scope: <name>` on a node declares the name in
//! scope for its subtree, attached to the one descendant that declares it
//! and is not already captured by a nearer `timeline-scope`, or else to an
//! inactive timeline; `all` does that for every name below it. So a
//! consumer's lookup walks up from itself, and the first node that declares
//! the name or scopes it decides; nothing found is no timeline. Chrome 154
//! agrees case by case (the phase 3 probe), `all` aside: it parses none.
//!
//! The kernel keeps a registry of the few nodes that bear a timeline row
//! (sources, consumers, scopes), kept from each commit's receipt, and after
//! a commit re-resolves every consumer; the receipt names those whose
//! answer changed, and `motion_sync` sends them (`MotionSync::timelines`).

use super::{Axis, TimelineScope, LINKED};
use crate::arena::NodeArena;
use crate::generated::StyleProps;
use crate::id::NodeKey;
use crate::motion::{motion_node, TimelineRows};
use crate::sorted::SlotSet;
use crate::txn::CommitReceipt;
use exact_motion::NamedTimeline;

/// The linked lookup's signature: [`resolve`].
pub(super) type Resolve = fn(&mut NodeArena, &CommitReceipt) -> Vec<NodeKey>;

/// The timeline-bearing nodes: each slot whose style declares a drag
/// timeline, scopes a name or follows one, and what each follower's name
/// resolved to at the last commit. A page has a handful.
#[derive(Debug, Clone, Default)]
pub(crate) struct Registry {
    slots: SlotSet,
    resolved: Vec<(NodeKey, NamedTimeline)>,
}

impl Registry {
    fn resolved(&self, consumer: NodeKey) -> Option<NamedTimeline> {
        self.resolved
            .iter()
            .find(|(k, _)| *k == consumer)
            .map(|(_, t)| *t)
    }
}

impl crate::kernel::Kernel {
    /// What `consumer`'s `animation-timeline` name resolved to at the last
    /// commit (LLP 1057.003 D4); `None` for a node that follows no name, or
    /// while the lookup is unlinked.
    pub fn timeline_of(&self, consumer: NodeKey) -> Option<NamedTimeline> {
        self.arena().timelines.resolved(consumer)
    }

    /// Whether any node bears a timeline row: a source, a scope or a
    /// consumer. A host whose page seeks consumers itself (the web's glue)
    /// follows every commit while one does.
    pub fn has_timelines(&self) -> bool {
        !self.arena().timelines.slots.is_empty()
    }
}

/// After a commit, the consumers whose timeline now resolves differently,
/// leaving out any the commit created or touched (their rows reach the
/// engine anyway). Nothing while unlinked.
pub(crate) fn refresh(arena: &mut NodeArena, receipt: &CommitReceipt) -> Vec<NodeKey> {
    match LINKED.get() {
        Some(grammar) => (grammar.resolve)(arena, receipt),
        None => Vec::new(),
    }
}

fn bears(s: &StyleProps) -> bool {
    s.rare.drag_timeline.name.is_some()
        || s.rare.animation_timeline.name().is_some()
        || s.rare.timeline_scope != TimelineScope::None
}

/// [`refresh`], linked: keep the registry, then re-resolve every consumer.
/// Any change can move a scope (a node inserted, removed or moved, a row
/// set), so any commit that finds the registry in use re-resolves; each
/// lookup walks one consumer's ancestors and, at a scope, the registry's
/// declarations, never the tree.
pub(super) fn resolve(arena: &mut NodeArena, receipt: &CommitReceipt) -> Vec<NodeKey> {
    for key in &receipt.destroyed {
        arena.timelines.slots.remove(key.index);
    }
    for key in receipt.created.iter().chain(&receipt.touched) {
        let Some(slot) = arena.resolve(*key) else {
            continue;
        };
        if bears(arena.style(slot)) {
            arena.timelines.slots.insert(slot);
        } else {
            arena.timelines.slots.remove(slot);
        }
    }
    if arena.timelines.slots.is_empty() && arena.timelines.resolved.is_empty() {
        return Vec::new();
    }
    let at = &*arena;
    let resolved: Vec<(NodeKey, NamedTimeline)> = at
        .timelines
        .slots
        .iter()
        .filter_map(|slot| {
            let name = at.style(slot).rare.animation_timeline.name()?;
            Some((at.key(slot), lookup(at, slot, name)))
        })
        .collect();
    let changed = resolved
        .iter()
        .filter(|(key, timeline)| {
            at.timelines.resolved(*key) != Some(*timeline)
                && !receipt.created.contains(key)
                && receipt.touched.binary_search(key).is_err()
        })
        .map(|(key, _)| *key)
        .collect();
    arena.timelines.resolved = resolved;
    changed
}

/// What `name` resolves to for the consumer at `slot`: walking up from it,
/// the first node that declares the name (its own `drag-timeline`, first)
/// or scopes it (`timeline-scope`) decides.
fn lookup(arena: &NodeArena, consumer: u32, name: &str) -> NamedTimeline {
    let slots = &arena.timelines.slots;
    let declares = |slot: u32| arena.style(slot).rare.drag_timeline.name.as_deref() == Some(name);
    let mut at = Some(consumer);
    while let Some(slot) = at {
        if slots.contains(slot) {
            if declares(slot) {
                return NamedTimeline::Source(motion_node(arena.key(slot)));
            }
            let scope = &arena.style(slot).rare.timeline_scope;
            if scope.scopes(name) {
                // What this scope captures: the declarations whose nearest
                // scope of the name, themselves included, is this one. One
                // is the timeline; none or several are an inactive one, but
                // `all` declares only the names below it.
                let mut captured = slots
                    .iter()
                    .filter(|&s| declares(s) && scope_of(arena, s, name) == Some(slot));
                match (captured.next(), captured.next()) {
                    (Some(source), None) => {
                        return NamedTimeline::Source(motion_node(arena.key(source)))
                    }
                    (None, _) if *scope == TimelineScope::All => {}
                    _ => return NamedTimeline::Inactive,
                }
            }
        }
        at = arena.parent(slot);
    }
    NamedTimeline::Missing
}

/// The nearest node at or above `slot` whose `timeline-scope` names `name`.
fn scope_of(arena: &NodeArena, slot: u32, name: &str) -> Option<u32> {
    let mut at = Some(slot);
    while let Some(s) = at {
        if arena.timelines.slots.contains(s) && arena.style(s).rare.timeline_scope.scopes(name) {
            return Some(s);
        }
        at = arena.parent(s);
    }
    None
}

/// One node's timeline rows as the engine takes them: whether the timeline
/// it drives reads `x`, and what its animations follow over their range.
pub(crate) fn rows(arena: &NodeArena, slot: u32) -> TimelineRows {
    let key = arena.key(slot);
    let style = arena.style(slot);
    let source = style
        .rare
        .drag_timeline
        .name
        .as_ref()
        .map(|_| style.rare.drag_timeline.axis == Axis::X);
    // `normal` has no length range to map a drag onto: unbound.
    let binding = style.rare.animation_timeline.name().and_then(|_| {
        let [a, b] = style.animation_range.0?;
        let timeline = arena
            .timelines
            .resolved(key)
            .unwrap_or(NamedTimeline::Missing);
        Some((timeline, [a as f64, b as f64]))
    });
    (motion_node(key), source, binding)
}

//! The gesture graph: authored recognizers and compositions, built into a
//! runnable topology and driven by the arena.
//!
//! @ref LLP 0099#gesture-recognizers
//! @ref LLP 0099#gesture-composition
//!
//! A plan names recognizers and compositions on view nodes. Building the graph
//! turns those records into per-node roots, resolves each root's axis and
//! direction, and declares one arena claim per root. From then on the
//! controller is the whole loop: frames in, arbitration, typed outputs out.
//!
//! Every structural rule is checked once, here, at build time — a composition
//! is a tree, every descendant of a root sits on the same view node, no leaf
//! belongs to two roots, and nesting is bounded. The runtime may then assume
//! all of it.

use super::*;
use crate::plan::{MotionPlan, PlanNodeKind};
use std::collections::{BTreeMap, BTreeSet};

/// Nesting depth one composition tree may reach.
pub const MAX_COMPOSITION_DEPTH: usize = 64;
/// Concurrent invocations of one composition root on one node.
pub const MAX_COMPOSITION_INSTANCES_PER_ROOT: usize = 256;
/// Outputs one controller retains between drains.
///
/// The buffer is bounded so that a host that stops draining is refused rather
/// than allowed to grow the frame path without limit. Every mutating entry
/// point pre-flights the worst case it could produce, so a call is refused
/// whole instead of applied halfway.
pub const MAX_RETAINED_OUTPUTS: usize = u16::MAX as usize;

/// What a gesture output is reporting.
///
/// One output can carry several of these at once: a stream's last event is
/// commonly a lifecycle event, a terminal, and a stream terminal together.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GestureEffects {
    /// An author-visible lifecycle event: began, changed, ended, cancelled.
    pub lifecycle: bool,
    /// A recognizer reached its terminal.
    pub terminal: bool,
    /// Recognition ended, but the commit and dispatch checkpoints still own
    /// the outcome: the caller must resolve this stream before it is final.
    pub needs_resolution: bool,
    /// The output carries an arena or composition receipt.
    pub receipt: bool,
    /// The one output that ends a pointer stream's author-visible lifecycle.
    pub stream_terminal: bool,
    /// A physical pointer stream retired without a semantic terminal — an
    /// unclaimed recognizer, or an intermediate member of a sequence.
    pub stream_retired: bool,
}

/// The author-visible phase of a gesture output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GestureOutputPhase {
    /// A receipt or control record with no author-visible phase.
    #[default]
    None,
    /// The gesture started.
    Began,
    /// The gesture moved.
    Changed,
    /// The gesture completed.
    Ended,
    /// The gesture was cancelled after it had started.
    Cancelled,
    /// The gesture failed before it ever started.
    Failed,
}

/// Why a stream resolved the way it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureReceipt {
    /// The arena made the decision.
    Arena(ArenaDecisionReason),
    /// A composition made the decision.
    Composition(CompositionDecisionKind),
}

/// One record the controller produces.
///
/// This is the typed outcome record: lifecycle events, terminals, and receipts
/// all arrive on this one shape, so a caller drains a single ordered stream
/// rather than correlating several.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GestureOutput {
    /// What this record reports.
    pub effects: GestureEffects,
    /// Author-visible phase, if any.
    pub phase: GestureOutputPhase,
    /// Which recognizer kind produced it, for a leaf record.
    pub gesture_kind: Option<RecognizerKind>,
    /// Why recognition ended, for a terminal record.
    pub terminal_reason: Option<GestureTerminalReason>,
    /// The arena or composition decision behind this record.
    pub receipt: Option<GestureReceipt>,
    /// Whether a resolved stream committed rather than cancelled.
    pub committed: bool,
    /// Plan generation of the descriptor that produced the record.
    pub descriptor_generation: u32,
    /// View node the gesture is mounted on.
    pub node_id: u32,
    /// Motion root the controller serves.
    pub root_id: u64,
    /// Plan id of the recognizer or composition this record is about.
    pub gesture_id: u64,
    /// Plan id of the root that owns `gesture_id`.
    pub root_gesture_id: u64,
    /// Plan id of the decided child, on a composition receipt.
    pub decided_child_id: u64,
    /// One-based ordinal of the decided child, on a composition receipt.
    pub decided_child_ordinal: u32,
    /// The pointer stream.
    pub stream_id: u64,
    /// Which invocation of the composition this record belongs to.
    pub composition_instance_id: u64,
    /// Monotonic receipt counter, so a caller can order records across streams.
    pub receipt_sequence: u64,
    /// The arena lease in force when the record was produced.
    pub lease_sequence: u64,
    /// Host timestamp of the frame that produced it, in milliseconds.
    pub timestamp_ms: f64,
    /// Pointers in the gesture's frozen pointer set, or members of a sequence.
    pub pointer_count: u32,
    /// Contact centroid, in node coordinates.
    pub position: MotionPoint,
    /// Contact centroid, in screen coordinates.
    pub absolute_position: MotionPoint,
    /// Centroid movement since the previous frame, in points.
    pub delta: MotionPoint,
    /// Mean contact pressure, where one is nominal.
    pub pressure: f64,
    /// The recognizer's typed payload: translation, scale, rotation, taps.
    pub payload: GesturePayload,
}

/// One presenter's answer for one claim, at one presenter generation.
///
/// Eligibility fails closed: a claim the presenter does not publish cannot win,
/// and a record stamped with a superseded presenter generation is ignored
/// rather than trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimEligibilityRecord {
    /// The claim this record answers for.
    pub claim_id: u64,
    /// Presenter generation the answer was produced at.
    pub presenter_generation: u32,
    /// Whether the presenter can consume this claim's input right now.
    pub can_consume: bool,
    /// Whether the presenter holds the scene lease this claim needs. Only a
    /// router-history claim may assert it.
    pub scene_lease_acquired: bool,
}

/// One native scroll presenter's live capacity, tied to the claims mounted on
/// the view node it sits on.
///
/// The node association is what lets an ancestor scroll view with remaining
/// travel keep a gesture on the same axis from winning, without pretending the
/// scroll view is part of the authored plan.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollCapacityPublication {
    /// View node whose claims this scroll presenter competes with.
    pub claim_node_id: u32,
    /// The presenter's capacity.
    pub capacity: ScrollCapacityRecord,
}

/// One host input sample addressed to a controller.
///
/// The identity fields fence the frame to exactly the plan revision it was
/// produced against: a frame from a superseded topology is refused, never
/// applied to its successor.
#[derive(Debug, Clone, PartialEq)]
pub struct GestureFrame {
    /// Motion root instance the frame belongs to.
    pub root_instance: u32,
    /// Slab epoch the frame belongs to.
    pub epoch: u32,
    /// Plan sequence the frame was produced against.
    pub motion_sequence: u64,
    /// View node the pointer is over.
    pub node_id: u32,
    /// The pointer stream.
    pub stream_id: u64,
    /// Host timestamp, in milliseconds.
    pub timestamp_ms: f64,
    /// Where the stream is in its lifecycle.
    pub phase: PointerPhase,
    /// What kind of input produced it.
    pub input_kind: ArenaInputKind,
    /// The contacts in this sample.
    pub contacts: Vec<PointerContact>,
}

/// Why a controller operation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerError {
    /// A root, epoch, or generation identity was zero.
    InvalidIdentity,
    /// A plan node was not a valid recognizer or composition record.
    InvalidDescriptor,
    /// The graph's structure is not runnable: a cycle, a shared child, a root
    /// whose descendants straddle view nodes, or nesting past the depth bound.
    InvalidGraph,
    /// A frame's contacts, phase, or timestamp were not usable.
    InvalidFrame,
    /// The frame or plan belongs to a superseded revision.
    StaleFrame,
    /// The arena refused the operation.
    Arena,
    /// The interaction-state publications are missing or inconsistent.
    InteractionState,
    /// A recognizer refused the frame.
    Recognizer,
    /// More than one invocation of this composition is waiting on this node,
    /// so the caller must name which one the frame belongs to.
    CompositionInstanceRequired,
    /// A monotonic counter is exhausted.
    SequenceExhausted,
    /// The retained-output buffer has no room for what this call could emit.
    /// The call was refused whole; drain and retry.
    OutputBackpressure,
}

/// One recognizer of a built graph, with the arena facts derived from it.
#[derive(Debug, Clone)]
pub struct LeafRuntime {
    pub(crate) node_id: u32,
    pub(crate) root_gesture_id: u64,
    pub(crate) claim_direction: Option<ArenaDirection>,
    /// The recognizer this leaf instantiates for each stream.
    pub recognizer: GestureRecognizer,
}

/// One pointer stream in flight.
#[derive(Debug, Clone)]
pub struct StreamRuntime {
    pub(crate) node_id: u32,
    pub(crate) input_kind: ArenaInputKind,
    pub(crate) roots: Vec<u64>,
    pub(crate) recognizers: BTreeMap<u64, GestureRecognizer>,
    pub(crate) automata: BTreeMap<u64, CompositionRuntimeNode>,
    pub(crate) composition_instances: BTreeMap<u64, CompositionInstanceRuntime>,
    pub(crate) active_root: Option<u64>,
    pub(crate) visible_leaves: BTreeSet<u64>,
    pub(crate) terminal_leaf: Option<u64>,
    pub(crate) terminal_candidate: Option<u64>,
}

/// A validated gesture graph: everything the controller needs to run a plan.
#[derive(Debug, Clone)]
pub struct GestureGraph {
    pub(crate) leaves: BTreeMap<u64, LeafRuntime>,
    pub(crate) compositions: BTreeMap<u64, CompositionRecord>,
    pub(crate) roots_by_node: BTreeMap<u32, Vec<u64>>,
    pub(crate) leaves_by_node: BTreeMap<u32, Vec<u64>>,
    pub(crate) leaf_to_root: BTreeMap<u64, u64>,
    pub(crate) claim_directions: BTreeMap<u64, Option<ArenaDirection>>,
    pub(crate) arena: GestureArena,
}

impl GestureGraph {
    /// Build the runnable graph for the gesture half of a plan.
    ///
    /// Plan nodes that are not recognizers or compositions are ignored, so the
    /// same plan drives the value graph and this one without being split first.
    pub fn build(
        plan: &MotionPlan,
        profile_kind: ArenaProfileKind,
    ) -> Result<Self, ControllerError> {
        let mut leaves = BTreeMap::new();
        let mut compositions = BTreeMap::new();
        let mut all_ids = BTreeSet::new();
        for node in plan.gesture_nodes() {
            if node.id == 0 || node.generation == 0 || node.node_id == 0 || !all_ids.insert(node.id)
            {
                return Err(ControllerError::InvalidDescriptor);
            }
            match &node.kind {
                PlanNodeKind::Composition { kind, children } => {
                    let mut seen = BTreeSet::new();
                    if children.len() < 2 || !children.iter().all(|child| seen.insert(*child)) {
                        return Err(ControllerError::InvalidGraph);
                    }
                    compositions.insert(
                        node.id,
                        CompositionRecord {
                            node_id: node.node_id,
                            generation: node.generation,
                            kind: *kind,
                            children: children.clone(),
                        },
                    );
                }
                PlanNodeKind::Recognizer {
                    descriptor,
                    claim_direction,
                } => {
                    let mut descriptor = *descriptor;
                    descriptor.set_generation(node.generation);
                    descriptor
                        .validate()
                        .map_err(|_| ControllerError::InvalidDescriptor)?;
                    if descriptor.common().id.0 != node.id {
                        return Err(ControllerError::InvalidDescriptor);
                    }
                    let recognizer = GestureRecognizer::new(descriptor)
                        .map_err(|_| ControllerError::InvalidDescriptor)?;
                    leaves.insert(
                        node.id,
                        LeafRuntime {
                            node_id: node.node_id,
                            root_gesture_id: node.id,
                            claim_direction: *claim_direction,
                            recognizer,
                        },
                    );
                }
                _ => return Err(ControllerError::InvalidDescriptor),
            }
        }
        Self::assemble(leaves, compositions, all_ids, profile_kind)
    }

    fn assemble(
        mut leaves: BTreeMap<u64, LeafRuntime>,
        compositions: BTreeMap<u64, CompositionRecord>,
        all_ids: BTreeSet<u64>,
        profile_kind: ArenaProfileKind,
    ) -> Result<Self, ControllerError> {
        for composition in compositions.values() {
            if composition.children.iter().any(|id| !all_ids.contains(id)) {
                return Err(ControllerError::InvalidGraph);
            }
        }
        detect_cycles(&compositions)?;
        let depended: BTreeSet<_> = compositions
            .values()
            .flat_map(|composition| composition.children.iter().copied())
            .collect();
        let roots: Vec<_> = all_ids
            .iter()
            .copied()
            .filter(|id| !depended.contains(id))
            .collect();
        let mut roots_by_node: BTreeMap<u32, Vec<u64>> = BTreeMap::new();
        let mut leaves_by_node: BTreeMap<u32, Vec<u64>> = BTreeMap::new();
        let mut leaf_to_root = BTreeMap::new();
        for (id, leaf) in &leaves {
            leaves_by_node.entry(leaf.node_id).or_default().push(*id);
        }
        for root in roots {
            let node_id = descriptor_node(root, &leaves, &compositions)
                .ok_or(ControllerError::InvalidGraph)?;
            if !descendant_nodes_match(root, node_id, &leaves, &compositions)? {
                return Err(ControllerError::InvalidGraph);
            }
            let descendants = descendant_leaves(root, &leaves, &compositions)?;
            if descendants.is_empty() {
                return Err(ControllerError::InvalidGraph);
            }
            roots_by_node.entry(node_id).or_default().push(root);
            for leaf_id in descendants {
                if leaf_to_root.insert(leaf_id, root).is_some() {
                    return Err(ControllerError::InvalidGraph);
                }
                if let Some(leaf) = leaves.get_mut(&leaf_id) {
                    leaf.root_gesture_id = root;
                }
            }
        }
        for values in roots_by_node.values_mut() {
            values.sort_unstable();
        }
        for values in leaves_by_node.values_mut() {
            values.sort_unstable();
        }
        // A graph whose single-feed worst case cannot fit in the retained
        // inventory could be permanently backpressured even when fully
        // drained. Refuse it at build time rather than at the first frame.
        if leaves_by_node.keys().copied().any(|node_id| {
            feed_output_bound_for_node(node_id, &leaves_by_node, &compositions)
                > MAX_RETAINED_OUTPUTS
        }) {
            return Err(ControllerError::InvalidGraph);
        }
        let claim_directions = roots_by_node
            .values()
            .flatten()
            .copied()
            .map(|id| (id, root_direction(id, &leaves, &compositions)))
            .collect::<BTreeMap<_, _>>();
        let claims = roots_by_node
            .values()
            .flat_map(|values| values.iter().copied())
            .enumerate()
            .map(|(order, id)| ArenaClaimDescriptor {
                id: ArenaClaimId(id),
                gesture_id: Some(GestureId(id)),
                kind: ArenaClaimKind::Gesture,
                axis: root_axis(id, &leaves, &compositions),
                direction: claim_directions
                    .get(&id)
                    .copied()
                    .flatten()
                    .unwrap_or(ArenaDirection::Positive),
                depth: 0,
                priority: 0,
                declaration_order: order as u16,
                compound: None,
            })
            .collect::<Vec<_>>();
        let profile = ArenaProfile::new(profile_kind, claims, [], [])
            .map_err(|_| ControllerError::InvalidGraph)?;
        Ok(Self {
            leaves,
            compositions,
            roots_by_node,
            leaves_by_node,
            leaf_to_root,
            claim_directions,
            arena: GestureArena::new(profile),
        })
    }

    /// The arena claims this graph declares, one per root, in claim order.
    pub fn claims(&self) -> impl Iterator<Item = ArenaClaimId> + '_ {
        self.roots_by_node
            .values()
            .flatten()
            .copied()
            .map(ArenaClaimId)
    }
}

/// Worst-case outputs one `feed` on `node_id` can enqueue.
///
/// * A recognizer produces at most one raw event per sample, lifecycle
///   normalization may insert a discrete start before its end, and exclusive
///   buffering retains at most start + latest update + terminal — so no leaf
///   contributes more than three lifecycle outputs to one feed.
/// * A failure or arena-loss path can additionally cancel each visible leaf
///   once, so a fourth slot per leaf is conservative.
/// * Race and exclusive emit at most one decision per child; simultaneous and
///   sequence emit one. Every composition has at least two children, so nested
///   decisions are bounded by authored edges, plus one slot per composition.
/// * At most two control records remain: a retirement or continuation, and a
///   terminal fallback.
pub(crate) fn feed_output_bound_for_node(
    node_id: u32,
    leaves_by_node: &BTreeMap<u32, Vec<u64>>,
    compositions: &BTreeMap<u64, CompositionRecord>,
) -> usize {
    let leaf_count = leaves_by_node.get(&node_id).map_or(0, Vec::len);
    if leaf_count == 0 {
        return 0;
    }
    let (composition_count, composition_edges) = compositions
        .values()
        .filter(|composition| composition.node_id == node_id)
        .fold((0_usize, 0_usize), |(count, edges), composition| {
            (
                count.saturating_add(1),
                edges.saturating_add(composition.children.len()),
            )
        });
    leaf_count
        .saturating_mul(4)
        .saturating_add(composition_edges)
        .saturating_add(composition_count)
        .saturating_add(2)
}

fn detect_cycles(compositions: &BTreeMap<u64, CompositionRecord>) -> Result<(), ControllerError> {
    let mut state = BTreeMap::<u64, u8>::new();
    let mut completion_order = Vec::with_capacity(compositions.len());
    for root in compositions.keys().copied() {
        if state.get(&root) == Some(&2) {
            continue;
        }
        state.insert(root, 1);
        let mut stack = vec![(root, 0_usize)];
        while let Some((id, next_child)) = stack.last_mut() {
            let composition = compositions.get(id).ok_or(ControllerError::InvalidGraph)?;
            if *next_child == composition.children.len() {
                state.insert(*id, 2);
                completion_order.push(*id);
                stack.pop();
                continue;
            }
            let child = composition.children[*next_child];
            *next_child += 1;
            if !compositions.contains_key(&child) {
                continue;
            }
            match state.get(&child).copied().unwrap_or(0) {
                0 => {
                    state.insert(child, 1);
                    stack.push((child, 0));
                }
                1 => return Err(ControllerError::InvalidGraph),
                2 => {}
                _ => {}
            }
        }
    }
    // Completion order is child-before-parent even when descriptor ids put
    // the innermost composition first. Compute the authored longest path
    // independently of map iteration order so hostile ids cannot evade the
    // renderer's existing depth contract.
    let mut depths = BTreeMap::<u64, usize>::new();
    for id in completion_order {
        let composition = compositions.get(&id).ok_or(ControllerError::InvalidGraph)?;
        let depth = composition
            .children
            .iter()
            .filter_map(|child| depths.get(child).copied())
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        if depth > MAX_COMPOSITION_DEPTH {
            return Err(ControllerError::InvalidGraph);
        }
        depths.insert(id, depth);
    }
    Ok(())
}

fn descriptor_node(
    id: u64,
    leaves: &BTreeMap<u64, LeafRuntime>,
    compositions: &BTreeMap<u64, CompositionRecord>,
) -> Option<u32> {
    leaves
        .get(&id)
        .map(|leaf| leaf.node_id)
        .or_else(|| compositions.get(&id).map(|composition| composition.node_id))
}

fn descendant_leaves(
    id: u64,
    leaves: &BTreeMap<u64, LeafRuntime>,
    compositions: &BTreeMap<u64, CompositionRecord>,
) -> Result<Vec<u64>, ControllerError> {
    let mut result = Vec::new();
    let mut stack = vec![id];
    let mut seen = BTreeSet::new();
    while let Some(candidate) = stack.pop() {
        if !seen.insert(candidate) {
            // Gesture compositions are trees: sharing a descriptor would
            // process one recognizer more than once per physical sample.
            return Err(ControllerError::InvalidGraph);
        }
        if leaves.contains_key(&candidate) {
            result.push(candidate);
            continue;
        }
        let composition = compositions
            .get(&candidate)
            .ok_or(ControllerError::InvalidGraph)?;
        stack.extend(composition.children.iter().rev().copied());
    }
    Ok(result)
}

fn descendant_nodes_match(
    id: u64,
    node_id: u32,
    leaves: &BTreeMap<u64, LeafRuntime>,
    compositions: &BTreeMap<u64, CompositionRecord>,
) -> Result<bool, ControllerError> {
    let mut stack = vec![id];
    let mut seen = BTreeSet::new();
    while let Some(candidate) = stack.pop() {
        if !seen.insert(candidate) {
            return Err(ControllerError::InvalidGraph);
        }
        if let Some(leaf) = leaves.get(&candidate) {
            if leaf.node_id != node_id {
                return Ok(false);
            }
            continue;
        }
        let composition = compositions
            .get(&candidate)
            .ok_or(ControllerError::InvalidGraph)?;
        if composition.node_id != node_id {
            return Ok(false);
        }
        stack.extend(composition.children.iter().rev().copied());
    }
    Ok(true)
}

fn root_axis(
    id: u64,
    leaves: &BTreeMap<u64, LeafRuntime>,
    compositions: &BTreeMap<u64, CompositionRecord>,
) -> ArenaAxis {
    descendant_leaves(id, leaves, compositions)
        .ok()
        .and_then(|ids| ids.into_iter().next())
        .and_then(|leaf| leaves.get(&leaf))
        .map(|leaf| match leaf.recognizer.descriptor() {
            RecognizerDescriptor::Pan(value) => value.axis,
            RecognizerDescriptor::Fling(value) => value.axis,
            _ => GestureAxis::Any,
        })
        .map(|axis| match axis {
            GestureAxis::Vertical => ArenaAxis::Vertical,
            GestureAxis::Any | GestureAxis::Horizontal => ArenaAxis::Horizontal,
        })
        .unwrap_or(ArenaAxis::Horizontal)
}

/// A compound root is directional only when every descendant leaf declares
/// the same physical direction. Missing or mixed declarations stay
/// direction-agnostic so a scroll presenter with capacity on either side wins
/// conservatively instead of inheriting an arbitrary first child.
fn root_direction(
    id: u64,
    leaves: &BTreeMap<u64, LeafRuntime>,
    compositions: &BTreeMap<u64, CompositionRecord>,
) -> Option<ArenaDirection> {
    let descendants = descendant_leaves(id, leaves, compositions).ok()?;
    let mut direction = None;
    for leaf_id in descendants {
        let candidate = leaves.get(&leaf_id)?.claim_direction?;
        match direction {
            Some(existing) if existing != candidate => return None,
            None => direction = Some(candidate),
            _ => {}
        }
    }
    direction
}

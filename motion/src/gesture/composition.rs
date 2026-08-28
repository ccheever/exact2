//! Recognizer composition: how several recognizers on one node relate.
//!
//! @ref LLP 0099#gesture-composition
//!
//! Four relations, each a small automaton over the same pointer frames:
//!
//! - **Exclusive** — the first child to succeed wins, but only once every child
//!   before it has failed. Losers' events are buffered until then, so a winner
//!   that arrives late still starts from the beginning of the gesture.
//! - **Race** — the first child to succeed wins outright and the rest fail.
//! - **Simultaneous** — every child runs, and one lease covers them all.
//! - **Sequence** — children run in order, each on its own pointer stream; the
//!   composition survives the gap between them as a continuation.
//!
//! A composition is a tree, never a graph: sharing a child would run one
//! recognizer twice for one physical sample.

use super::*;
use std::collections::BTreeMap;

/// How a composition's children relate to one another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionKind {
    /// The first child to succeed wins, once every earlier child has failed.
    Exclusive,
    /// The first child to succeed wins outright; the rest are cancelled.
    Race,
    /// Every child runs, under one shared arena lease.
    Simultaneous,
    /// Children run in order, each on its own pointer stream.
    Sequence,
}

/// One composition node of a built gesture graph.
#[derive(Debug, Clone)]
pub struct CompositionRecord {
    pub(crate) node_id: u32,
    pub(crate) generation: u32,
    pub(crate) kind: CompositionKind,
    pub(crate) children: Vec<u64>,
}

/// Where a composition or recognizer is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionRuntimeStatus {
    /// Nothing has happened yet.
    Idle,
    /// Recognition has started but nothing has committed.
    Possible,
    /// The gesture is running and emitting.
    Active,
    /// The gesture completed.
    Ended,
    /// The gesture was cancelled after it had started emitting.
    Cancelled,
    /// The gesture failed before it ever emitted.
    Failed,
}

impl CompositionRuntimeStatus {
    pub(crate) fn succeeds(self) -> bool {
        matches!(self, Self::Active | Self::Ended)
    }

    pub(crate) fn is_failure(self) -> bool {
        matches!(self, Self::Cancelled | Self::Failed)
    }

    pub(crate) fn is_terminal(self) -> bool {
        matches!(self, Self::Ended | Self::Cancelled | Self::Failed)
    }
}

/// What a composition decided, as it appears on a receipt output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionDecisionKind {
    /// A simultaneous composition took the stream for all of its children.
    SimultaneousLease,
    /// This child won a race.
    RaceWinner,
    /// This child lost a race and was cancelled.
    RaceLoser,
    /// This child won an exclusive composition.
    ExclusiveWinner,
    /// This child lost an exclusive composition and was cancelled.
    ExclusiveLoser,
    /// This child completed and the sequence moved to the next one.
    SequenceAdvance,
    /// This child completed and the whole sequence is done.
    SequenceComplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompositionDecision {
    pub(crate) composition_id: u64,
    pub(crate) child_id: u64,
    pub(crate) child_index: Option<usize>,
    pub(crate) kind: CompositionDecisionKind,
}

#[derive(Debug, Clone)]
pub(crate) struct CompositionStep {
    pub(crate) status: CompositionRuntimeStatus,
    pub(crate) emissions: Vec<(u64, RecognizerEvent)>,
    pub(crate) decisions: Vec<CompositionDecision>,
    pub(crate) terminal_leaf: Option<u64>,
    pub(crate) awaits_next_stream: bool,
}

impl CompositionStep {
    pub(crate) fn empty(status: CompositionRuntimeStatus) -> Self {
        Self {
            status,
            emissions: Vec::new(),
            decisions: Vec::new(),
            terminal_leaf: None,
            awaits_next_stream: false,
        }
    }
}

/// One node of a running composition automaton.
#[derive(Debug, Clone)]
pub enum CompositionRuntimeNode {
    /// One recognizer.
    Leaf {
        /// Plan id of the recognizer.
        id: u64,
    },
    /// A composition of other nodes.
    Composition {
        /// Plan id of the composition.
        id: u64,
        /// How its children relate.
        kind: CompositionKind,
        /// The running children, in authored order.
        children: Vec<CompositionRuntimeNode>,
        /// Where the composition is in its own lifecycle.
        status: CompositionRuntimeStatus,
        /// Index of the child that won, once one has.
        winner: Option<usize>,
        /// Index of the sequence member currently running.
        sequence_index: usize,
        /// Whether the composition has ever succeeded, which decides whether a
        /// later failure reads as a cancel or as a plain failure.
        has_activated: bool,
        /// Per-child event buffers, held until an exclusive winner is known.
        buffered: Vec<Vec<(u64, RecognizerEvent)>>,
    },
}

/// A sequence composition waiting between two pointer streams.
///
/// The finger has lifted and the arena stream is gone, but the composition is
/// not over: it keeps its automaton and its recognizers until the next stream
/// on the same node arrives, or the topology detaches.
#[derive(Debug, Clone)]
pub struct CompositionContinuation {
    pub(crate) instance: CompositionInstanceRuntime,
    pub(crate) automaton: CompositionRuntimeNode,
    pub(crate) recognizers: BTreeMap<u64, GestureRecognizer>,
}

/// Stable identity for one invocation of a compound automaton. It is
/// deliberately distinct from a physical pointer stream: a sequence can
/// retire one arena stream and continue on another, while two invocations of
/// the same descriptor can be waiting at the same node at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CompositionInstanceId(pub u64);

#[derive(Debug, Clone)]
pub(crate) struct CompositionInstanceRuntime {
    pub(crate) id: CompositionInstanceId,
    /// Every physical member keeps its own arena lease identity. A zero lease
    /// is possible only before activation; it is replaced when the compound
    /// wins that stream.
    pub(crate) members: BTreeMap<u64, u64>,
}

impl CompositionInstanceRuntime {
    pub(crate) fn new(id: CompositionInstanceId, stream_id: u64) -> Self {
        Self {
            id,
            members: BTreeMap::from([(stream_id, 0)]),
        }
    }

    pub(crate) fn record_member(&mut self, stream_id: u64, lease_sequence: u64) {
        self.members.insert(stream_id, lease_sequence);
    }
}

impl CompositionRuntimeNode {
    /// Instantiate the automaton for one root of a built graph.
    pub fn build(
        id: u64,
        leaves: &BTreeMap<u64, LeafRuntime>,
        compositions: &BTreeMap<u64, CompositionRecord>,
    ) -> Result<Self, ControllerError> {
        if leaves.contains_key(&id) {
            return Ok(Self::Leaf { id });
        }
        let composition = compositions.get(&id).ok_or(ControllerError::InvalidGraph)?;
        let children = composition
            .children
            .iter()
            .map(|child| Self::build(*child, leaves, compositions))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::Composition {
            id,
            kind: composition.kind,
            buffered: vec![Vec::new(); children.len()],
            children,
            status: CompositionRuntimeStatus::Idle,
            winner: None,
            sequence_index: 0,
            has_activated: false,
        })
    }

    /// The plan id of this node.
    pub fn descriptor_id(&self) -> u64 {
        match self {
            Self::Leaf { id } | Self::Composition { id, .. } => *id,
        }
    }

    /// Advance the automaton with one pointer frame.
    pub(crate) fn feed(
        &mut self,
        frame: &PointerFrame,
        recognizers: &mut BTreeMap<u64, GestureRecognizer>,
    ) -> Result<CompositionStep, ControllerError> {
        match self {
            Self::Leaf { id } => {
                let recognizer = recognizers
                    .get_mut(id)
                    .ok_or(ControllerError::InvalidGraph)?;
                let previous_state = recognizer.state();
                let mut events = match recognizer.process(frame) {
                    Ok(step) => step.events,
                    Err(GestureInputError::TerminalState) => Vec::new(),
                    Err(_) => return Err(ControllerError::Recognizer),
                };
                let state = recognizer.state();
                // The lower recognizer uses Began for its down-sample
                // bookkeeping and Changed for several first threshold
                // crossings. Normalize that internal edge to the author
                // lifecycle: possible emits nothing, and the first successful
                // edge emits start (plus end for a discrete recognizer).
                events.retain(|event| {
                    event.phase != GestureEventPhase::Began
                        || event.state == RecognizerState::Active
                });
                if previous_state == RecognizerState::Began
                    && matches!(state, RecognizerState::Active | RecognizerState::Ended)
                    && events
                        .first()
                        .is_some_and(|event| event.phase != GestureEventPhase::Began)
                {
                    let mut start = events[0].clone();
                    start.phase = GestureEventPhase::Began;
                    start.state = RecognizerState::Active;
                    start.terminal_reason = None;
                    if state == RecognizerState::Active
                        && events[0].phase == GestureEventPhase::Changed
                    {
                        events[0] = start;
                    } else {
                        events.insert(0, start);
                    }
                }
                let events = events.into_iter().map(|event| (*id, event)).collect();
                let status = recognizer_status(state);
                let terminal_leaf = (status == CompositionRuntimeStatus::Ended).then_some(*id);
                Ok(CompositionStep {
                    status,
                    emissions: events,
                    decisions: Vec::new(),
                    terminal_leaf,
                    awaits_next_stream: false,
                })
            }
            Self::Composition {
                id,
                kind,
                children,
                status,
                winner,
                sequence_index,
                has_activated,
                buffered,
            } => {
                if status.is_terminal() {
                    return Ok(CompositionStep::empty(*status));
                }
                if *status == CompositionRuntimeStatus::Idle {
                    *status = CompositionRuntimeStatus::Possible;
                }
                match kind {
                    CompositionKind::Simultaneous => {
                        feed_simultaneous(*id, children, status, has_activated, frame, recognizers)
                    }
                    CompositionKind::Race => feed_race(
                        *id,
                        children,
                        status,
                        winner,
                        has_activated,
                        frame,
                        recognizers,
                    ),
                    CompositionKind::Exclusive => feed_exclusive(
                        *id,
                        children,
                        status,
                        winner,
                        has_activated,
                        buffered,
                        frame,
                        recognizers,
                    ),
                    CompositionKind::Sequence => feed_sequence(
                        *id,
                        children,
                        status,
                        sequence_index,
                        has_activated,
                        frame,
                        recognizers,
                    ),
                }
            }
        }
    }

    /// Cancel every recognizer under this node.
    pub fn fail(&mut self, recognizers: &mut BTreeMap<u64, GestureRecognizer>, timestamp_ms: f64) {
        match self {
            Self::Leaf { id } => {
                if let Some(recognizer) = recognizers.get_mut(id) {
                    let _ = recognizer.cancel(GestureTerminalReason::AuthorCancelled, timestamp_ms);
                }
            }
            Self::Composition {
                children,
                status,
                has_activated,
                ..
            } => {
                let was_active = *has_activated || *status == CompositionRuntimeStatus::Active;
                for child in children {
                    child.fail(recognizers, timestamp_ms);
                }
                *status = if was_active {
                    CompositionRuntimeStatus::Cancelled
                } else {
                    CompositionRuntimeStatus::Failed
                };
            }
        }
    }
}

fn recognizer_status(status: RecognizerState) -> CompositionRuntimeStatus {
    match status {
        RecognizerState::Idle => CompositionRuntimeStatus::Idle,
        RecognizerState::Began => CompositionRuntimeStatus::Possible,
        RecognizerState::Active => CompositionRuntimeStatus::Active,
        RecognizerState::Ended => CompositionRuntimeStatus::Ended,
        RecognizerState::Cancelled => CompositionRuntimeStatus::Cancelled,
        RecognizerState::Failed => CompositionRuntimeStatus::Failed,
    }
}

fn feed_simultaneous(
    composition_id: u64,
    children: &mut [CompositionRuntimeNode],
    status: &mut CompositionRuntimeStatus,
    has_activated: &mut bool,
    frame: &PointerFrame,
    recognizers: &mut BTreeMap<u64, GestureRecognizer>,
) -> Result<CompositionStep, ControllerError> {
    let mut steps = Vec::with_capacity(children.len());
    for child in children {
        steps.push(child.feed(frame, recognizers)?);
    }
    let mut decisions: Vec<_> = steps
        .iter_mut()
        .flat_map(|step| std::mem::take(&mut step.decisions))
        .collect();
    let emissions = steps
        .iter_mut()
        .flat_map(|step| std::mem::take(&mut step.emissions))
        .collect();
    if !*has_activated && steps.iter().any(|step| step.status.succeeds()) {
        *has_activated = true;
        *status = CompositionRuntimeStatus::Active;
        decisions.push(CompositionDecision {
            composition_id,
            child_id: 0,
            child_index: None,
            kind: CompositionDecisionKind::SimultaneousLease,
        });
    }
    if steps.iter().all(|step| step.status.is_terminal()) {
        *status = if steps
            .iter()
            .any(|step| step.status == CompositionRuntimeStatus::Ended)
        {
            CompositionRuntimeStatus::Ended
        } else if *has_activated
            || steps
                .iter()
                .any(|step| step.status == CompositionRuntimeStatus::Cancelled)
        {
            CompositionRuntimeStatus::Cancelled
        } else {
            CompositionRuntimeStatus::Failed
        };
    } else if *has_activated {
        *status = CompositionRuntimeStatus::Active;
    }
    Ok(CompositionStep {
        status: *status,
        emissions,
        decisions,
        terminal_leaf: steps.iter().rev().find_map(|step| step.terminal_leaf),
        awaits_next_stream: steps.iter().any(|step| step.awaits_next_stream),
    })
}

fn feed_race(
    composition_id: u64,
    children: &mut [CompositionRuntimeNode],
    status: &mut CompositionRuntimeStatus,
    winner: &mut Option<usize>,
    has_activated: &mut bool,
    frame: &PointerFrame,
    recognizers: &mut BTreeMap<u64, GestureRecognizer>,
) -> Result<CompositionStep, ControllerError> {
    if let Some(index) = *winner {
        let step = children[index].feed(frame, recognizers)?;
        *status = step.status;
        return Ok(step);
    }
    let mut steps = Vec::with_capacity(children.len());
    for child in children.iter_mut() {
        steps.push(child.feed(frame, recognizers)?);
    }
    if let Some(index) = steps.iter().position(|step| step.status.succeeds()) {
        *winner = Some(index);
        *has_activated = true;
        *status = if steps[index].status == CompositionRuntimeStatus::Ended {
            CompositionRuntimeStatus::Ended
        } else {
            CompositionRuntimeStatus::Active
        };
        let mut selected = std::mem::replace(
            &mut steps[index],
            CompositionStep::empty(CompositionRuntimeStatus::Failed),
        );
        for (loser_index, child) in children.iter_mut().enumerate() {
            if loser_index == index {
                continue;
            }
            child.fail(recognizers, frame.timestamp_ms);
            selected.decisions.push(CompositionDecision {
                composition_id,
                child_id: child.descriptor_id(),
                child_index: Some(loser_index),
                kind: CompositionDecisionKind::RaceLoser,
            });
        }
        selected.decisions.push(CompositionDecision {
            composition_id,
            child_id: children[index].descriptor_id(),
            child_index: Some(index),
            kind: CompositionDecisionKind::RaceWinner,
        });
        selected.status = *status;
        return Ok(selected);
    }
    if steps.iter().all(|step| step.status.is_terminal()) {
        *status = CompositionRuntimeStatus::Failed;
    }
    Ok(CompositionStep {
        status: *status,
        emissions: Vec::new(),
        decisions: steps
            .iter_mut()
            .flat_map(|step| std::mem::take(&mut step.decisions))
            .collect(),
        terminal_leaf: None,
        awaits_next_stream: false,
    })
}

#[allow(clippy::too_many_arguments)]
fn feed_exclusive(
    composition_id: u64,
    children: &mut [CompositionRuntimeNode],
    status: &mut CompositionRuntimeStatus,
    winner: &mut Option<usize>,
    has_activated: &mut bool,
    buffered: &mut [Vec<(u64, RecognizerEvent)>],
    frame: &PointerFrame,
    recognizers: &mut BTreeMap<u64, GestureRecognizer>,
) -> Result<CompositionStep, ControllerError> {
    if let Some(index) = *winner {
        let step = children[index].feed(frame, recognizers)?;
        *status = step.status;
        return Ok(step);
    }
    let mut steps = Vec::with_capacity(children.len());
    for (index, child) in children.iter_mut().enumerate() {
        let mut step = child.feed(frame, recognizers)?;
        buffered[index].append(&mut step.emissions);
        coalesce_composition_updates(&mut buffered[index]);
        steps.push(step);
    }
    let eligible = steps.iter().enumerate().position(|(index, step)| {
        step.status.succeeds()
            && steps[..index]
                .iter()
                .all(|predecessor| predecessor.status.is_failure())
    });
    if let Some(index) = eligible {
        *winner = Some(index);
        *has_activated = true;
        *status = if steps[index].status == CompositionRuntimeStatus::Ended {
            CompositionRuntimeStatus::Ended
        } else {
            CompositionRuntimeStatus::Active
        };
        let mut selected = std::mem::replace(
            &mut steps[index],
            CompositionStep::empty(CompositionRuntimeStatus::Failed),
        );
        selected.emissions = std::mem::take(&mut buffered[index]);
        for (loser_index, child) in children.iter_mut().enumerate() {
            if loser_index == index {
                continue;
            }
            child.fail(recognizers, frame.timestamp_ms);
            buffered[loser_index].clear();
            selected.decisions.push(CompositionDecision {
                composition_id,
                child_id: child.descriptor_id(),
                child_index: Some(loser_index),
                kind: CompositionDecisionKind::ExclusiveLoser,
            });
        }
        selected.decisions.push(CompositionDecision {
            composition_id,
            child_id: children[index].descriptor_id(),
            child_index: Some(index),
            kind: CompositionDecisionKind::ExclusiveWinner,
        });
        selected.status = *status;
        return Ok(selected);
    }
    if steps.iter().all(|step| step.status.is_failure()) {
        *status = CompositionRuntimeStatus::Failed;
    }
    Ok(CompositionStep {
        status: *status,
        emissions: Vec::new(),
        decisions: steps
            .iter_mut()
            .flat_map(|step| std::mem::take(&mut step.decisions))
            .collect(),
        terminal_leaf: None,
        awaits_next_stream: false,
    })
}

fn feed_sequence(
    composition_id: u64,
    children: &mut [CompositionRuntimeNode],
    status: &mut CompositionRuntimeStatus,
    sequence_index: &mut usize,
    has_activated: &mut bool,
    frame: &PointerFrame,
    recognizers: &mut BTreeMap<u64, GestureRecognizer>,
) -> Result<CompositionStep, ControllerError> {
    let mut step = children[*sequence_index].feed(frame, recognizers)?;
    if step.status == CompositionRuntimeStatus::Ended {
        *has_activated = true;
        let completed = *sequence_index;
        if completed == children.len() - 1 {
            *status = CompositionRuntimeStatus::Ended;
            step.status = *status;
            step.decisions.push(CompositionDecision {
                composition_id,
                child_id: children[completed].descriptor_id(),
                child_index: Some(completed),
                kind: CompositionDecisionKind::SequenceComplete,
            });
        } else {
            *sequence_index += 1;
            *status = CompositionRuntimeStatus::Active;
            step.status = *status;
            step.awaits_next_stream = true;
            step.decisions.push(CompositionDecision {
                composition_id,
                child_id: children[completed].descriptor_id(),
                child_index: Some(completed),
                kind: CompositionDecisionKind::SequenceAdvance,
            });
        }
    } else if step.status.is_failure() {
        *status = step.status;
    } else if step.status == CompositionRuntimeStatus::Active || *has_activated {
        *has_activated = true;
        *status = CompositionRuntimeStatus::Active;
    } else {
        *status = CompositionRuntimeStatus::Possible;
    }
    step.status = *status;
    Ok(step)
}

fn coalesce_composition_updates(events: &mut Vec<(u64, RecognizerEvent)>) {
    let mut last_updates = BTreeMap::new();
    for (index, (leaf_id, event)) in events.iter().enumerate() {
        if event.phase == GestureEventPhase::Changed {
            last_updates.insert(*leaf_id, index);
        }
    }
    let mut index = 0;
    events.retain(|(leaf_id, event)| {
        let keep = event.phase != GestureEventPhase::Changed
            || last_updates.get(leaf_id).copied() == Some(index);
        index += 1;
        keep
    });
}

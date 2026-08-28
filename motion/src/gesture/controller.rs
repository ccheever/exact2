//! The gesture controller: the loop from host frames to arbitrated events.
//!
//! @ref LLP 0099#gesture-recognizers
//! @ref LLP 0099#gesture-composition
//!
//! One controller serves one motion root. It owns the built graph, the arena,
//! the interaction-state publications arbitration reads, the streams in flight,
//! and the sequence continuations waiting between streams.
//!
//! The loop per frame: fence the frame to the current plan revision, admit or
//! find the stream, feed the composition automata, let the arena pick a winner
//! the first time any root succeeds, and emit typed outputs for the winner
//! only. A stream that ends recognition is not finished: the commit and
//! dispatch checkpoints still own the outcome, which is why an `Ended` event
//! carries `needs_resolution` and the caller must call
//! [`GestureGraphController::resolve_terminal`].

use super::*;
use crate::plan::MotionPlan;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy)]
pub(crate) struct PublishedClaimEligibility {
    pub(crate) eligibility: ClaimEligibility,
    pub(crate) presenter_generation: u32,
}

/// The gesture half of the evaluator for one motion root.
#[derive(Debug, Clone)]
pub struct GestureGraphController {
    pub(crate) root_id: u64,
    pub(crate) root_instance: u32,
    pub(crate) epoch: u32,
    pub(crate) motion_sequence: u64,
    pub(crate) profile_generation: u32,
    pub(crate) profile_kind: ArenaProfileKind,
    pub(crate) leaves: BTreeMap<u64, LeafRuntime>,
    pub(crate) compositions: BTreeMap<u64, CompositionRecord>,
    pub(crate) roots_by_node: BTreeMap<u32, Vec<u64>>,
    pub(crate) leaves_by_node: BTreeMap<u32, Vec<u64>>,
    pub(crate) leaf_to_root: BTreeMap<u64, u64>,
    pub(crate) claim_directions: BTreeMap<u64, Option<ArenaDirection>>,
    pub(crate) arena: GestureArena,
    pub(crate) interaction_state: InteractionStateStore,
    pub(crate) published_environment: Option<InteractionEnvironmentSnapshot>,
    pub(crate) topology_context_published: bool,
    pub(crate) published_claim_eligibility: BTreeMap<ArenaClaimId, PublishedClaimEligibility>,
    pub(crate) scroll_ids_by_node: BTreeMap<u32, BTreeSet<u64>>,
    pub(crate) streams: BTreeMap<u64, StreamRuntime>,
    pub(crate) continuations: BTreeMap<(u32, u64, CompositionInstanceId), CompositionContinuation>,
    pub(crate) outputs: Vec<GestureOutput>,
    pub(crate) output_capacity: usize,
    pub(crate) receipt_sequence: u64,
}

impl GestureGraphController {
    /// Open a controller for one motion root, with an empty graph installed.
    ///
    /// Every identity is nonzero: a zero root, instance, epoch, or profile
    /// generation is a caller bug, not an "unset" value.
    pub fn new(
        root_id: u64,
        root_instance: u32,
        epoch: u32,
        profile_generation: u32,
        profile_kind: ArenaProfileKind,
    ) -> Result<Self, ControllerError> {
        if root_id == 0 || root_instance == 0 || epoch == 0 || profile_generation == 0 {
            return Err(ControllerError::InvalidIdentity);
        }
        let arena = GestureArena::new(
            ArenaProfile::new(profile_kind, [], [], []).map_err(|_| ControllerError::Arena)?,
        );
        Ok(Self {
            root_id,
            root_instance,
            epoch,
            motion_sequence: 0,
            profile_generation,
            profile_kind,
            leaves: BTreeMap::new(),
            compositions: BTreeMap::new(),
            roots_by_node: BTreeMap::new(),
            leaves_by_node: BTreeMap::new(),
            leaf_to_root: BTreeMap::new(),
            claim_directions: BTreeMap::new(),
            arena,
            interaction_state: InteractionStateStore::default(),
            published_environment: None,
            topology_context_published: false,
            published_claim_eligibility: BTreeMap::new(),
            scroll_ids_by_node: BTreeMap::new(),
            streams: BTreeMap::new(),
            continuations: BTreeMap::new(),
            outputs: Vec::new(),
            output_capacity: MAX_RETAINED_OUTPUTS,
            receipt_sequence: 0,
        })
    }

    /// The motion root this controller serves.
    pub fn root_id(&self) -> u64 {
        self.root_id
    }

    /// The root instance the installed graph belongs to.
    pub fn root_instance(&self) -> u32 {
        self.root_instance
    }

    /// The slab epoch the installed graph belongs to.
    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    /// The plan sequence of the installed graph.
    pub fn motion_sequence(&self) -> u64 {
        self.motion_sequence
    }

    /// The arena arbitrating this root's claims.
    pub fn arena(&self) -> &GestureArena {
        &self.arena
    }

    /// The arena claims the installed graph declares, one per root.
    pub fn claims(&self) -> impl Iterator<Item = ArenaClaimId> + '_ {
        self.roots_by_node
            .values()
            .flatten()
            .copied()
            .map(ArenaClaimId)
    }

    /// The recognizers of the installed graph, by plan id.
    pub fn leaves(&self) -> &BTreeMap<u64, LeafRuntime> {
        &self.leaves
    }

    /// Streams currently in flight.
    pub fn active_stream_count(&self) -> usize {
        self.streams.len()
    }

    /// Sequence compositions waiting between pointer streams.
    pub fn continuation_count(&self) -> usize {
        self.continuations.len()
    }

    /// Install a new plan, ending every stream the old one owned.
    ///
    /// A plan is only ever replaced forward: an older epoch, a different root
    /// instance within an epoch, or a sequence that has already been installed
    /// is refused as stale rather than applied out of order.
    pub fn replace(
        &mut self,
        root_instance: u32,
        epoch: u32,
        motion_sequence: u64,
        plan: &MotionPlan,
        timestamp_ms: f64,
    ) -> Result<(), ControllerError> {
        if root_instance == 0 || epoch == 0 || motion_sequence == 0 || !timestamp_ms.is_finite() {
            return Err(ControllerError::InvalidIdentity);
        }
        if epoch < self.epoch
            || (epoch == self.epoch && root_instance != self.root_instance)
            || (epoch == self.epoch && motion_sequence <= self.motion_sequence)
        {
            return Err(ControllerError::StaleFrame);
        }
        let graph = GestureGraph::build(plan, self.profile_kind)?;
        self.require_output_headroom(self.cancel_all_output_bound())?;
        let environment = self
            .published_environment
            .as_ref()
            .cloned()
            .ok_or(ControllerError::InteractionState)?;
        self.cancel_all(ArenaCancelReason::StructuralInvalidation, timestamp_ms)?;
        let _ = self
            .interaction_state
            .detach_topology(self.root_id, self.root_instance as u64)
            .map_err(|_| ControllerError::InteractionState)?;
        self.clear_scroll_capacity();
        let source = InteractionSourceIdentity {
            epoch,
            root_id: self.root_id,
            root_instance: root_instance as u64,
            motion_seq: motion_sequence,
            profile_generation: self.profile_generation,
            // A gesture plan carries neither a modal barrier nor a
            // RouterHistory disposition. Zero is the truthful absence value;
            // those publishers extend the topology record rather than have a
            // generation fabricated here.
            modal_generation: 0,
            presenter_generation: environment.presenter_generation,
            back_structural_revision: 0,
            back_policy_revision: 0,
        };
        let mounted_claims = graph.claims().collect();
        self.interaction_state
            .attach_topology(InteractionTopologySnapshot {
                source,
                mounted_claims,
                modal_barrier_id: None,
                declared_layout_direction: environment.effective_layout_direction,
                back_mode: BackMode::None,
            })
            .map_err(|_| ControllerError::InteractionState)?;
        self.root_instance = root_instance;
        self.epoch = epoch;
        self.motion_sequence = motion_sequence;
        self.install(graph);
        self.topology_context_published = false;
        self.published_claim_eligibility.clear();
        self.streams.clear();
        self.continuations.clear();
        Ok(())
    }

    /// Advance the graph with one host input sample.
    pub fn feed(&mut self, frame: &GestureFrame) -> Result<(), ControllerError> {
        self.feed_for_instance(frame, None)
    }

    /// Advance the graph with one sample, naming which composition invocation
    /// it belongs to.
    ///
    /// Required when more than one invocation of the same composition is
    /// waiting on the node — a double tap begun twice, say — because there is
    /// then no truthful node-and-root-only answer.
    pub fn feed_for_instance(
        &mut self,
        frame: &GestureFrame,
        requested_instance: Option<CompositionInstanceId>,
    ) -> Result<(), ControllerError> {
        if frame.root_instance != self.root_instance
            || frame.epoch != self.epoch
            || frame.motion_sequence != self.motion_sequence
        {
            return Err(ControllerError::StaleFrame);
        }
        let phase = frame.phase;
        let input_kind = frame.input_kind;
        let pointer_frame = PointerFrame::new(
            GestureStreamId(frame.stream_id),
            phase,
            frame.timestamp_ms,
            frame.contacts.iter().copied(),
        )
        .map_err(|_| ControllerError::InvalidFrame)?;
        let output_bound = self.feed_output_bound(frame.node_id).max(
            self.streams
                .get(&frame.stream_id)
                .map_or(0, Self::stream_cancel_output_bound),
        );
        self.require_output_headroom(output_bound)?;

        if phase == PointerPhase::Down {
            self.begin_stream(frame, input_kind, requested_instance)?;
        } else if let Some(requested_instance) = requested_instance {
            let matches_bound_instance = self.streams.get(&frame.stream_id).is_some_and(|stream| {
                stream
                    .composition_instances
                    .values()
                    .any(|instance| instance.id == requested_instance)
            });
            if !matches_bound_instance {
                return Err(ControllerError::InvalidFrame);
            }
        }
        let Some(stream) = self.streams.get(&frame.stream_id) else {
            return Ok(());
        };
        if stream.node_id != frame.node_id {
            self.cancel_stream(
                frame.stream_id,
                ArenaCancelReason::StructuralInvalidation,
                GestureTerminalReason::StructuralInvalidation,
                frame.timestamp_ms,
            )?;
            return Ok(());
        }

        let roots_to_feed = self
            .streams
            .get(&frame.stream_id)
            .map(|stream| {
                stream
                    .active_root
                    .map_or_else(|| stream.roots.clone(), |root| vec![root])
            })
            .unwrap_or_default();
        let mut steps = Vec::with_capacity(roots_to_feed.len());
        if let Some(stream) = self.streams.get_mut(&frame.stream_id) {
            let (automata, recognizers) = (&mut stream.automata, &mut stream.recognizers);
            for root in roots_to_feed {
                let automaton = automata
                    .get_mut(&root)
                    .ok_or(ControllerError::InvalidGraph)?;
                steps.push((root, automaton.feed(&pointer_frame, recognizers)?));
            }
        }
        self.consume_steps(frame.stream_id, phase, frame.timestamp_ms, steps)?;
        Ok(())
    }

    fn begin_stream(
        &mut self,
        frame: &GestureFrame,
        input_kind: ArenaInputKind,
        requested_instance: Option<CompositionInstanceId>,
    ) -> Result<(), ControllerError> {
        if let Some(existing) = self.streams.get(&frame.stream_id) {
            let instance_matches = match requested_instance {
                None => true,
                Some(requested) => existing
                    .composition_instances
                    .values()
                    .any(|instance| instance.id == requested),
            };
            if existing.node_id == frame.node_id && instance_matches {
                return Ok(());
            }
            return Err(ControllerError::InvalidFrame);
        }
        let roots = self
            .roots_by_node
            .get(&frame.node_id)
            .cloned()
            .unwrap_or_default();
        if roots.is_empty() {
            return Ok(());
        }
        // Admitting a stream commits the controller to being able to cancel
        // every stream it now holds. Reserve that worst case up front so a
        // later structural invalidation can never be refused midway.
        let cancellation_reservation = self
            .streams
            .values()
            .map(|stream| self.node_cancel_output_reservation(stream.node_id))
            .fold(0_usize, usize::saturating_add)
            .saturating_add(self.node_cancel_output_reservation(frame.node_id));
        if cancellation_reservation > self.output_capacity {
            return Err(ControllerError::OutputBackpressure);
        }
        let mut recognizers: BTreeMap<_, _> = self
            .leaves_by_node
            .get(&frame.node_id)
            .into_iter()
            .flatten()
            .filter_map(|leaf_id| {
                self.leaves
                    .get(leaf_id)
                    .map(|leaf| (*leaf_id, leaf.recognizer.clone()))
            })
            .collect();
        let mut automata = roots
            .iter()
            .copied()
            .map(|root| {
                CompositionRuntimeNode::build(root, &self.leaves, &self.compositions)
                    .map(|automaton| (root, automaton))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let continuation_keys = roots
            .iter()
            .copied()
            .map(|root| {
                self.continuation_key(frame.node_id, root, requested_instance)
                    .map(|key| (root, key))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let new_instance_id = requested_instance.unwrap_or(CompositionInstanceId(frame.stream_id));
        for root in &roots {
            if continuation_keys.get(root).is_some_and(Option::is_some) {
                continue;
            }
            if new_instance_id.0 == 0
                || self.composition_instance_is_active(frame.node_id, *root, new_instance_id)
                || self.composition_instance_count(frame.node_id, *root)
                    >= MAX_COMPOSITION_INSTANCES_PER_ROOT
            {
                return Err(ControllerError::InvalidFrame);
            }
        }
        let eligibility = self.eligibility(input_kind)?;
        self.arena
            .begin_stream(
                GestureStreamId(frame.stream_id),
                input_kind,
                eligibility.source,
                roots.iter().copied().map(ArenaClaimId),
            )
            .map_err(|_| ControllerError::Arena)?;
        let mut composition_instances = BTreeMap::new();
        for root in &roots {
            if let Some(key) = continuation_keys.get(root).copied().flatten() {
                let continuation = self
                    .continuations
                    .remove(&key)
                    .ok_or(ControllerError::InvalidGraph)?;
                let mut instance = continuation.instance;
                instance.record_member(frame.stream_id, 0);
                composition_instances.insert(*root, instance);
                automata.insert(*root, continuation.automaton);
                for (leaf_id, recognizer) in continuation.recognizers {
                    recognizers.insert(leaf_id, recognizer);
                }
            } else {
                composition_instances.insert(
                    *root,
                    CompositionInstanceRuntime::new(new_instance_id, frame.stream_id),
                );
            }
        }
        self.streams.insert(
            frame.stream_id,
            StreamRuntime {
                node_id: frame.node_id,
                input_kind,
                roots,
                recognizers,
                automata,
                composition_instances,
                active_root: None,
                visible_leaves: BTreeSet::new(),
                terminal_leaf: None,
                terminal_candidate: None,
            },
        );
        self.bump_receipt()?;
        Ok(())
    }

    fn continuation_key(
        &self,
        node_id: u32,
        root_id: u64,
        requested_instance: Option<CompositionInstanceId>,
    ) -> Result<Option<(u32, u64, CompositionInstanceId)>, ControllerError> {
        if let Some(instance_id) = requested_instance {
            let key = (node_id, root_id, instance_id);
            return Ok(self.continuations.contains_key(&key).then_some(key));
        }

        let mut matches = self
            .continuations
            .keys()
            .filter(|(candidate_node, candidate_root, _)| {
                *candidate_node == node_id && *candidate_root == root_id
            })
            .copied();
        let first = matches.next();
        if matches.next().is_some() {
            // With more than one waiting invocation there is no truthful
            // node/root-only answer. Require the caller's stable instance
            // token instead of silently selecting FIFO/BTree order.
            return Err(ControllerError::CompositionInstanceRequired);
        }
        Ok(first)
    }

    fn composition_instance_is_active(
        &self,
        node_id: u32,
        root_id: u64,
        instance_id: CompositionInstanceId,
    ) -> bool {
        self.streams.values().any(|stream| {
            stream.node_id == node_id
                && stream
                    .composition_instances
                    .get(&root_id)
                    .is_some_and(|instance| instance.id == instance_id)
        })
    }

    fn composition_instance_count(&self, node_id: u32, root_id: u64) -> usize {
        let active = self
            .streams
            .values()
            .filter(|stream| {
                stream.node_id == node_id && stream.composition_instances.contains_key(&root_id)
            })
            .count();
        let waiting = self
            .continuations
            .keys()
            .filter(|(candidate_node, candidate_root, _)| {
                *candidate_node == node_id && *candidate_root == root_id
            })
            .count();
        active.saturating_add(waiting)
    }

    fn consume_steps(
        &mut self,
        stream_id: u64,
        phase: PointerPhase,
        timestamp_ms: f64,
        mut steps: Vec<(u64, CompositionStep)>,
    ) -> Result<(), ControllerError> {
        let activation_roots: Vec<_> = steps
            .iter()
            .filter_map(|(root, step)| step.status.succeeds().then_some(*root))
            .collect();
        if !activation_roots.is_empty()
            && self
                .streams
                .get(&stream_id)
                .is_some_and(|stream| stream.active_root.is_none())
        {
            let input_kind = self
                .streams
                .get(&stream_id)
                .map(|stream| stream.input_kind)
                .ok_or(ControllerError::InvalidFrame)?;
            let eligibility = self.eligibility(input_kind)?;
            let resolution = self
                .arena
                .activate(
                    GestureStreamId(stream_id),
                    activation_roots.iter().copied().map(ArenaClaimId),
                    &eligibility,
                )
                .map_err(|_| ControllerError::Arena)?;
            self.bump_receipt()?;
            let winner = resolution.active_members.first().map(|claim| claim.0);
            let lease_sequence = self
                .arena
                .lease(GestureStreamId(stream_id))
                .map(|lease| lease.sequence)
                .unwrap_or(0);
            if let Some(stream) = self.streams.get_mut(&stream_id) {
                stream.active_root = winner;
                if let Some(instance) =
                    winner.and_then(|root| stream.composition_instances.get_mut(&root))
                {
                    instance.record_member(stream_id, lease_sequence);
                }
            }
        }
        let active_root = self
            .streams
            .get(&stream_id)
            .and_then(|stream| stream.active_root);
        let Some(root_id) = active_root else {
            let all_terminal = steps.iter().all(|(_, step)| step.status.is_terminal());
            if all_terminal {
                self.retire_unclaimed_stream(stream_id, timestamp_ms)?;
            }
            return Ok(());
        };
        let Some((_, mut step)) = steps.drain(..).find(|(candidate, _)| *candidate == root_id)
        else {
            return Ok(());
        };

        for decision in &step.decisions {
            self.push_composition_decision(root_id, stream_id, timestamp_ms, *decision)?;
        }

        let root_terminal = step.status == CompositionRuntimeStatus::Ended;
        let terminal_leaf = step.terminal_leaf;
        for (leaf_id, event) in step.emissions.drain(..) {
            let was_visible = self
                .streams
                .get(&stream_id)
                .is_some_and(|stream| stream.visible_leaves.contains(&leaf_id));
            let losing_terminal = matches!(
                event.phase,
                GestureEventPhase::Cancelled | GestureEventPhase::Failed
            ) && !was_visible;
            if losing_terminal {
                continue;
            }
            if matches!(
                event.phase,
                GestureEventPhase::Began | GestureEventPhase::Changed
            ) {
                if let Some(stream) = self.streams.get_mut(&stream_id) {
                    stream.visible_leaves.insert(leaf_id);
                }
            }
            if event.phase == GestureEventPhase::Changed {
                let _ = self.arena.mark_visible_motion(GestureStreamId(stream_id));
            }
            self.push_event(
                root_id,
                leaf_id,
                &event,
                root_terminal && terminal_leaf == Some(leaf_id),
            )?;
            if matches!(
                event.phase,
                GestureEventPhase::Ended | GestureEventPhase::Cancelled | GestureEventPhase::Failed
            ) {
                if let Some(stream) = self.streams.get_mut(&stream_id) {
                    stream.visible_leaves.remove(&leaf_id);
                }
            }
        }

        if step.awaits_next_stream && phase == PointerPhase::Up {
            self.continue_composition(stream_id, root_id, timestamp_ms)?;
            return Ok(());
        }

        if root_terminal {
            let input_kind = self
                .streams
                .get(&stream_id)
                .map(|stream| stream.input_kind)
                .ok_or(ControllerError::InvalidFrame)?;
            let eligibility = self.eligibility(input_kind)?;
            match self
                .arena
                .commit_checkpoint(GestureStreamId(stream_id), &eligibility)
            {
                Ok(resolution) if resolution.reason == ArenaDecisionReason::CommitRecheck => {
                    self.bump_receipt()?;
                    if let Some(stream) = self.streams.get_mut(&stream_id) {
                        stream.terminal_candidate = Some(root_id);
                        stream.terminal_leaf = terminal_leaf;
                    }
                }
                Ok(_) | Err(_) => {
                    self.cancel_stream(
                        stream_id,
                        ArenaCancelReason::HolderIneligible,
                        GestureTerminalReason::StructuralInvalidation,
                        timestamp_ms,
                    )?;
                    return Ok(());
                }
            }
        } else if step.status.is_failure() {
            self.cancel_stream(
                stream_id,
                ArenaCancelReason::AuthorCancelled,
                GestureTerminalReason::AuthorCancelled,
                timestamp_ms,
            )?;
            return Ok(());
        }
        let should_retire = self.streams.get(&stream_id).is_some_and(|stream| {
            stream.active_root.is_none()
                && stream
                    .recognizers
                    .values()
                    .all(|recognizer| recognizer.state().is_terminal())
        });
        if should_retire {
            if let Some(stream) = self.streams.remove(&stream_id) {
                for root in stream.roots {
                    let _ = self.arena.fail_claim(
                        GestureStreamId(stream_id),
                        ArenaClaimId(root),
                        ClaimFailureReason::Ineligible,
                    );
                }
            }
            self.arena.retire_stream(GestureStreamId(stream_id));
        }
        Ok(())
    }

    fn retire_unclaimed_stream(
        &mut self,
        stream_id: u64,
        timestamp_ms: f64,
    ) -> Result<(), ControllerError> {
        if let Some(stream) = self.streams.remove(&stream_id) {
            let root_gesture_id = stream.roots.first().copied().unwrap_or(0);
            for root in &stream.roots {
                let _ = self.arena.fail_claim(
                    GestureStreamId(stream_id),
                    ArenaClaimId(*root),
                    ClaimFailureReason::Ineligible,
                );
            }
            self.enqueue_output(GestureOutput {
                effects: GestureEffects {
                    stream_retired: true,
                    ..GestureEffects::default()
                },
                node_id: stream.node_id,
                root_id: self.root_id,
                root_gesture_id,
                stream_id,
                timestamp_ms,
                ..GestureOutput::default()
            })?;
        }
        self.arena.retire_stream(GestureStreamId(stream_id));
        Ok(())
    }

    fn continue_composition(
        &mut self,
        stream_id: u64,
        root_id: u64,
        timestamp_ms: f64,
    ) -> Result<(), ControllerError> {
        let input_kind = self
            .streams
            .get(&stream_id)
            .map(|stream| stream.input_kind)
            .ok_or(ControllerError::InvalidFrame)?;
        let eligibility = self.eligibility(input_kind)?;
        if !matches!(
            self
            .arena
            .commit_checkpoint(GestureStreamId(stream_id), &eligibility),
            Ok(resolution) if resolution.reason == ArenaDecisionReason::CommitRecheck
        ) {
            self.cancel_stream(
                stream_id,
                ArenaCancelReason::HolderIneligible,
                GestureTerminalReason::StructuralInvalidation,
                timestamp_ms,
            )?;
            return Ok(());
        }
        self.bump_receipt()?;
        self.arena
            .dispatch_checkpoint(GestureStreamId(stream_id), false)
            .map_err(|_| ControllerError::Arena)?;
        self.bump_receipt()?;
        let lease_sequence = self
            .arena
            .lease(GestureStreamId(stream_id))
            .map(|lease| lease.sequence)
            .unwrap_or(0);
        if let Some(lease) = self.arena.lease(GestureStreamId(stream_id)).cloned() {
            self.arena
                .acknowledge_settle(GestureStreamId(stream_id), lease.sequence)
                .map_err(|_| ControllerError::Arena)?;
            self.bump_receipt()?;
        }
        self.arena.retire_stream(GestureStreamId(stream_id));

        let Some(mut stream) = self.streams.remove(&stream_id) else {
            return Ok(());
        };
        let automaton = stream
            .automata
            .remove(&root_id)
            .ok_or(ControllerError::InvalidGraph)?;
        let mut instance = stream
            .composition_instances
            .remove(&root_id)
            .ok_or(ControllerError::InvalidGraph)?;
        instance.record_member(stream_id, lease_sequence);
        let composition_instance_id = instance.id;
        let member_count = u32::try_from(instance.members.len())
            .map_err(|_| ControllerError::SequenceExhausted)?;
        let recognizers = std::mem::take(&mut stream.recognizers)
            .into_iter()
            .filter(|(leaf_id, _)| self.leaf_to_root.get(leaf_id).copied() == Some(root_id))
            .collect();
        self.continuations.insert(
            (stream.node_id, root_id, composition_instance_id),
            CompositionContinuation {
                instance,
                automaton,
                recognizers,
            },
        );
        self.enqueue_output(GestureOutput {
            effects: GestureEffects {
                stream_retired: true,
                ..GestureEffects::default()
            },
            node_id: stream.node_id,
            root_id: self.root_id,
            root_gesture_id: root_id,
            stream_id,
            // Continuation metadata: the caller correlates the next physical
            // member of the sequence by this invocation identity.
            composition_instance_id: composition_instance_id.0,
            pointer_count: member_count,
            timestamp_ms,
            ..GestureOutput::default()
        })?;
        Ok(())
    }

    fn push_composition_decision(
        &mut self,
        root_id: u64,
        stream_id: u64,
        timestamp_ms: f64,
        decision: CompositionDecision,
    ) -> Result<(), ControllerError> {
        let instance = self
            .streams
            .get(&stream_id)
            .and_then(|stream| stream.composition_instances.get(&root_id))
            .ok_or(ControllerError::InvalidGraph)?;
        let instance_id = instance.id.0;
        let member_count = u32::try_from(instance.members.len())
            .map_err(|_| ControllerError::SequenceExhausted)?;
        self.bump_receipt()?;
        let composition = self
            .compositions
            .get(&decision.composition_id)
            .ok_or(ControllerError::InvalidGraph)?;
        let output = GestureOutput {
            effects: GestureEffects {
                receipt: true,
                ..GestureEffects::default()
            },
            receipt: Some(GestureReceipt::Composition(decision.kind)),
            descriptor_generation: composition.generation,
            node_id: composition.node_id,
            root_id: self.root_id,
            gesture_id: decision.composition_id,
            root_gesture_id: root_id,
            decided_child_id: decision.child_id,
            decided_child_ordinal: decision.child_index.map_or(0, |index| index as u32 + 1),
            stream_id,
            composition_instance_id: instance_id,
            receipt_sequence: self.receipt_sequence,
            lease_sequence: self
                .arena
                .lease(GestureStreamId(stream_id))
                .map(|lease| lease.sequence)
                .unwrap_or(0),
            timestamp_ms,
            pointer_count: member_count,
            ..GestureOutput::default()
        };
        self.enqueue_output(output)?;
        Ok(())
    }

    /// Commit or cancel a stream whose recognition ended.
    ///
    /// Recognition ending is not the outcome. This is where the commit and
    /// dispatch checkpoints run, and where a stream that the arena has since
    /// found ineligible is cancelled instead of committed.
    pub fn resolve_terminal(
        &mut self,
        stream_id: u64,
        commit: bool,
        dispatch_blocked: bool,
        timestamp_ms: f64,
    ) -> Result<(), ControllerError> {
        if !timestamp_ms.is_finite() {
            return Err(ControllerError::InvalidFrame);
        }
        let Some(stream) = self.streams.get(&stream_id).cloned() else {
            return Ok(());
        };
        let Some(root_id) = stream.terminal_candidate else {
            return Ok(());
        };
        let output_bound = if !commit || dispatch_blocked {
            Self::stream_cancel_output_bound(&stream)
        } else {
            1
        };
        self.require_output_headroom(output_bound)?;
        if !commit {
            self.cancel_stream(
                stream_id,
                ArenaCancelReason::AuthorCancelled,
                GestureTerminalReason::AuthorCancelled,
                timestamp_ms,
            )?;
            return Ok(());
        }
        let resolution = self
            .arena
            .dispatch_checkpoint(GestureStreamId(stream_id), dispatch_blocked)
            .map_err(|_| ControllerError::Arena)?;
        self.bump_receipt()?;
        if dispatch_blocked {
            if let Some(mut stream) = self.streams.remove(&stream_id) {
                for recognizer in stream.recognizers.values_mut() {
                    let _ = recognizer.cancel(GestureTerminalReason::Blocked, timestamp_ms);
                }
                let lease_sequence = self
                    .arena
                    .lease(GestureStreamId(stream_id))
                    .map(|lease| lease.sequence)
                    .unwrap_or(0);
                self.push_cancel_outputs(
                    stream_id,
                    &stream,
                    GestureTerminalReason::Blocked,
                    timestamp_ms,
                    lease_sequence,
                )?;
                self.arena.retire_stream(GestureStreamId(stream_id));
            }
            return Ok(());
        }
        let composition_instance_id = stream
            .composition_instances
            .get(&root_id)
            .map(|instance| instance.id.0)
            .unwrap_or(0);
        let (gesture_id, generation, node_id) =
            self.terminal_leaf_details(&stream)
                .unwrap_or((root_id, 0, stream.node_id));
        self.enqueue_output(GestureOutput {
            effects: GestureEffects {
                terminal: true,
                stream_terminal: true,
                receipt: true,
                ..GestureEffects::default()
            },
            phase: GestureOutputPhase::Ended,
            gesture_kind: self
                .leaves
                .get(&gesture_id)
                .map(|leaf| leaf.recognizer.descriptor().kind()),
            committed: true,
            descriptor_generation: generation,
            node_id,
            root_id: self.root_id,
            gesture_id,
            root_gesture_id: root_id,
            stream_id,
            composition_instance_id,
            receipt_sequence: self.receipt_sequence,
            lease_sequence: self
                .arena
                .lease(GestureStreamId(stream_id))
                .map(|lease| lease.sequence)
                .unwrap_or(0),
            timestamp_ms,
            receipt: Some(GestureReceipt::Arena(resolution.reason)),
            ..GestureOutput::default()
        })?;
        if let Some(lease) = self.arena.lease(GestureStreamId(stream_id)).cloned() {
            let _ = self
                .arena
                .acknowledge_settle(GestureStreamId(stream_id), lease.sequence);
            self.bump_receipt()?;
        }
        self.arena.retire_stream(GestureStreamId(stream_id));
        self.streams.remove(&stream_id);
        Ok(())
    }

    /// Report that something outside this graph took the stream — a platform
    /// recognizer, a system edge gesture.
    ///
    /// The platform can win before the first sample ever reaches this crate, so
    /// the stream may be one this controller has never seen.
    pub fn report_external_owner(
        &mut self,
        node_id: u32,
        stream_id: u64,
        input_kind: ArenaInputKind,
        platform_reason_code: u16,
        timestamp_ms: f64,
    ) -> Result<(), ControllerError> {
        if !timestamp_ms.is_finite() || stream_id == 0 {
            return Err(ControllerError::InvalidFrame);
        }
        let output_bound = self
            .streams
            .get(&stream_id)
            .map_or(1, Self::stream_cancel_output_bound);
        self.require_output_headroom(output_bound)?;
        if let Some(mut stream) = self.streams.remove(&stream_id) {
            for recognizer in stream.recognizers.values_mut() {
                let _ = recognizer.cancel(GestureTerminalReason::ExternalOwner, timestamp_ms);
            }
            let lease_sequence = self
                .arena
                .lease(GestureStreamId(stream_id))
                .map(|lease| lease.sequence)
                .unwrap_or(0);
            self.arena
                .cancel_for_external_owner(GestureStreamId(stream_id), platform_reason_code)
                .map_err(|_| ControllerError::Arena)?;
            self.bump_receipt()?;
            self.push_cancel_outputs(
                stream_id,
                &stream,
                GestureTerminalReason::ExternalOwner,
                timestamp_ms,
                lease_sequence,
            )?;
            self.arena.retire_stream(GestureStreamId(stream_id));
            return Ok(());
        }
        let contenders = self
            .roots_by_node
            .get(&node_id)
            .cloned()
            .unwrap_or_default();
        let eligibility = self.eligibility(input_kind)?;
        self.arena
            .report_external_owner(
                GestureStreamId(stream_id),
                input_kind,
                eligibility.source,
                contenders.iter().copied().map(ArenaClaimId),
                None,
                platform_reason_code,
            )
            .map_err(|_| ControllerError::Arena)?;
        self.bump_receipt()?;
        self.enqueue_output(GestureOutput {
            // No invocation and no author-visible lifecycle exist for a stream
            // that was never admitted, so this retires the stream without
            // fabricating a terminal whose composition identity would be zero.
            effects: GestureEffects {
                stream_retired: true,
                receipt: true,
                ..GestureEffects::default()
            },
            phase: GestureOutputPhase::Cancelled,
            terminal_reason: Some(GestureTerminalReason::ExternalOwner),
            root_id: self.root_id,
            root_gesture_id: contenders.first().copied().unwrap_or(0),
            stream_id,
            node_id,
            receipt_sequence: self.receipt_sequence,
            timestamp_ms,
            ..GestureOutput::default()
        })?;
        Ok(())
    }

    /// Detach the presenter topology, ending every stream and continuation.
    pub fn detach_topology(&mut self, timestamp_ms: f64) -> Result<(), ControllerError> {
        if !timestamp_ms.is_finite() {
            return Err(ControllerError::InvalidFrame);
        }
        self.require_output_headroom(self.cancel_all_output_bound())?;
        self.cancel_all(ArenaCancelReason::StructuralInvalidation, timestamp_ms)?;
        let _ = self
            .interaction_state
            .detach_topology(self.root_id, self.root_instance as u64)
            .map_err(|_| ControllerError::InteractionState)?;
        self.clear_scroll_capacity();
        self.topology_context_published = false;
        self.published_claim_eligibility.clear();
        // A continuation is logical recognizer state even though it has no
        // currently active physical stream. Once presenter topology detaches,
        // it must not resume into the successor presenter or keep consuming a
        // composition-instance slot.
        self.continuations.clear();
        Ok(())
    }

    /// End every physical stream while keeping the installed topology and the
    /// presenter-owned eligibility.
    ///
    /// A resumed surface can then admit a fresh stream without waiting for a
    /// redundant graph replacement.
    pub fn suspend_active_streams(&mut self, timestamp_ms: f64) -> Result<(), ControllerError> {
        if !timestamp_ms.is_finite() {
            return Err(ControllerError::InvalidFrame);
        }
        self.require_output_headroom(self.cancel_all_output_bound())?;
        self.cancel_all(ArenaCancelReason::StructuralInvalidation, timestamp_ms)?;
        // Sequence/multi-tap continuations are physical-interaction state.
        // A presentation gap cannot carry them into a fresh platform stream.
        self.continuations.clear();
        Ok(())
    }

    /// Retire the whole root into a new epoch, dropping the installed graph.
    pub fn reset(
        &mut self,
        root_instance: u32,
        epoch: u32,
        timestamp_ms: f64,
    ) -> Result<(), ControllerError> {
        if root_instance == 0 || epoch <= self.epoch || !timestamp_ms.is_finite() {
            return Err(ControllerError::InvalidIdentity);
        }
        self.require_output_headroom(self.cancel_all_output_bound())?;
        self.cancel_all(ArenaCancelReason::Reset, timestamp_ms)?;
        let _ = self
            .interaction_state
            .detach_topology(self.root_id, self.root_instance as u64)
            .map_err(|_| ControllerError::InteractionState)?;
        self.clear_scroll_capacity();
        self.root_instance = root_instance;
        self.epoch = epoch;
        self.motion_sequence = 0;
        self.leaves.clear();
        self.compositions.clear();
        self.roots_by_node.clear();
        self.leaves_by_node.clear();
        self.leaf_to_root.clear();
        self.claim_directions.clear();
        self.topology_context_published = false;
        self.published_claim_eligibility.clear();
        self.continuations.clear();
        self.arena.reset();
        Ok(())
    }

    fn cancel_all(
        &mut self,
        reason: ArenaCancelReason,
        timestamp_ms: f64,
    ) -> Result<(), ControllerError> {
        let terminal_reason = match reason {
            ArenaCancelReason::Reset => GestureTerminalReason::Reset,
            ArenaCancelReason::ExternalOwner => GestureTerminalReason::ExternalOwner,
            _ => GestureTerminalReason::StructuralInvalidation,
        };
        let stream_ids: Vec<_> = self.streams.keys().copied().collect();
        for stream_id in stream_ids {
            self.cancel_stream(stream_id, reason, terminal_reason, timestamp_ms)?;
        }
        Ok(())
    }

    fn cancel_stream(
        &mut self,
        stream_id: u64,
        reason: ArenaCancelReason,
        terminal_reason: GestureTerminalReason,
        timestamp_ms: f64,
    ) -> Result<(), ControllerError> {
        let Some(mut stream) = self.streams.remove(&stream_id) else {
            return Ok(());
        };
        for recognizer in stream.recognizers.values_mut() {
            let _ = recognizer.cancel(terminal_reason, timestamp_ms);
        }
        let lease_sequence = self
            .arena
            .lease(GestureStreamId(stream_id))
            .map(|lease| lease.sequence)
            .unwrap_or(0);
        if lease_sequence != 0 {
            let _ = self.arena.cancel(GestureStreamId(stream_id), reason);
        } else {
            for root in &stream.roots {
                let _ = self.arena.fail_claim(
                    GestureStreamId(stream_id),
                    ArenaClaimId(*root),
                    ClaimFailureReason::Ineligible,
                );
            }
        }
        let _ = self.bump_receipt();
        self.push_cancel_outputs(
            stream_id,
            &stream,
            terminal_reason,
            timestamp_ms,
            lease_sequence,
        )?;
        self.arena.retire_stream(GestureStreamId(stream_id));
        Ok(())
    }

    fn push_cancel_outputs(
        &mut self,
        stream_id: u64,
        stream: &StreamRuntime,
        reason: GestureTerminalReason,
        timestamp_ms: f64,
        lease_sequence: u64,
    ) -> Result<(), ControllerError> {
        let root = stream
            .active_root
            .or_else(|| stream.roots.first().copied())
            .unwrap_or(0);
        let mut leaf_ids: Vec<_> = stream.visible_leaves.iter().copied().collect();
        if leaf_ids.is_empty() {
            leaf_ids.push(
                stream
                    .terminal_leaf
                    .or_else(|| {
                        stream
                            .recognizers
                            .keys()
                            .copied()
                            .find(|leaf| self.leaf_to_root.get(leaf).copied() == Some(root))
                    })
                    .or_else(|| stream.recognizers.keys().next().copied())
                    .unwrap_or(root),
            );
        }
        let composition_instance_id = stream
            .composition_instances
            .get(&root)
            .map(|instance| instance.id.0)
            .unwrap_or(0);
        for (index, leaf_id) in leaf_ids.into_iter().enumerate() {
            let leaf = self.leaves.get(&leaf_id);
            self.enqueue_output(GestureOutput {
                effects: GestureEffects {
                    terminal: true,
                    stream_terminal: index == 0,
                    receipt: true,
                    ..GestureEffects::default()
                },
                phase: if stream.active_root.is_some() {
                    GestureOutputPhase::Cancelled
                } else {
                    GestureOutputPhase::Failed
                },
                gesture_kind: leaf.map(|value| value.recognizer.descriptor().kind()),
                terminal_reason: Some(reason),
                descriptor_generation: leaf
                    .map(|value| value.recognizer.descriptor().common().generation)
                    .unwrap_or(0),
                node_id: stream.node_id,
                root_id: self.root_id,
                gesture_id: leaf_id,
                root_gesture_id: root,
                stream_id,
                composition_instance_id,
                receipt_sequence: self.receipt_sequence,
                lease_sequence,
                timestamp_ms,
                ..GestureOutput::default()
            })?;
        }
        Ok(())
    }

    fn push_event(
        &mut self,
        root_id: u64,
        leaf_id: u64,
        event: &RecognizerEvent,
        needs_resolution: bool,
    ) -> Result<(), ControllerError> {
        let composition_instance_id = self
            .streams
            .get(&event.stream_id.0)
            .and_then(|stream| stream.composition_instances.get(&root_id))
            .map(|instance| instance.id.0)
            .unwrap_or(0);
        let Some(leaf) = self.leaves.get(&leaf_id) else {
            return Ok(());
        };
        let phase = match event.phase {
            GestureEventPhase::Began => GestureOutputPhase::Began,
            GestureEventPhase::Changed => GestureOutputPhase::Changed,
            GestureEventPhase::Ended => GestureOutputPhase::Ended,
            GestureEventPhase::Cancelled => GestureOutputPhase::Cancelled,
            GestureEventPhase::Failed => GestureOutputPhase::Failed,
        };
        let terminal = matches!(
            event.phase,
            GestureEventPhase::Cancelled | GestureEventPhase::Failed
        );
        let output = GestureOutput {
            effects: GestureEffects {
                lifecycle: true,
                receipt: true,
                // Recognition ended, but the commit and dispatch checkpoints
                // still own the one terminal outcome.
                needs_resolution: event.phase == GestureEventPhase::Ended && needs_resolution,
                terminal,
                ..GestureEffects::default()
            },
            phase,
            gesture_kind: Some(leaf.recognizer.descriptor().kind()),
            terminal_reason: event.terminal_reason,
            descriptor_generation: event.descriptor_generation,
            node_id: leaf.node_id,
            root_id: self.root_id,
            gesture_id: leaf_id,
            root_gesture_id: root_id,
            stream_id: event.stream_id.0,
            composition_instance_id,
            receipt_sequence: self.receipt_sequence,
            lease_sequence: self
                .arena
                .lease(event.stream_id)
                .map(|lease| lease.sequence)
                .unwrap_or(0),
            timestamp_ms: event.timestamp_ms,
            pointer_count: event.pointer_set.len() as u32,
            position: event.position,
            absolute_position: event.absolute_position,
            delta: event.delta,
            pressure: event.pressure,
            payload: event.payload.clone(),
            ..GestureOutput::default()
        };
        self.enqueue_output(output)?;
        Ok(())
    }

    fn terminal_leaf_details(&self, stream: &StreamRuntime) -> Option<(u64, u32, u32)> {
        let leaf_id = stream.terminal_leaf?;
        let leaf = self.leaves.get(&leaf_id)?;
        Some((
            leaf_id,
            leaf.recognizer.descriptor().common().generation,
            leaf.node_id,
        ))
    }

    fn bump_receipt(&mut self) -> Result<(), ControllerError> {
        self.receipt_sequence = self
            .receipt_sequence
            .checked_add(1)
            .ok_or(ControllerError::SequenceExhausted)?;
        Ok(())
    }

    fn install(&mut self, graph: GestureGraph) {
        self.leaves = graph.leaves;
        self.compositions = graph.compositions;
        self.roots_by_node = graph.roots_by_node;
        self.leaves_by_node = graph.leaves_by_node;
        self.leaf_to_root = graph.leaf_to_root;
        self.claim_directions = graph.claim_directions;
        self.arena = graph.arena;
    }

    /// Refuse a call whose worst case does not fit in the retained inventory.
    ///
    /// Pre-flighted at the head of every mutating entry point, so a refusal
    /// leaves the controller untouched instead of half-applied.
    fn require_output_headroom(&self, additional: usize) -> Result<(), ControllerError> {
        if additional > self.output_capacity.saturating_sub(self.outputs.len()) {
            Err(ControllerError::OutputBackpressure)
        } else {
            Ok(())
        }
    }

    fn stream_cancel_output_bound(stream: &StreamRuntime) -> usize {
        stream.visible_leaves.len().max(1)
    }

    fn cancel_all_output_bound(&self) -> usize {
        self.streams
            .values()
            .map(Self::stream_cancel_output_bound)
            .fold(0_usize, usize::saturating_add)
    }

    fn node_cancel_output_reservation(&self, node_id: u32) -> usize {
        self.leaves_by_node.get(&node_id).map_or(1, |leaves| {
            // A stream selects at most one root, but reserving every leaf on
            // the node avoids coupling cancellation safety to arena choice.
            leaves.len().max(1)
        })
    }

    fn feed_output_bound(&self, node_id: u32) -> usize {
        feed_output_bound_for_node(node_id, &self.leaves_by_node, &self.compositions)
    }

    fn enqueue_output(&mut self, output: GestureOutput) -> Result<(), ControllerError> {
        debug_assert!(
            !(output.effects.lifecycle || output.effects.terminal)
                || output.composition_instance_id != 0,
            "a semantic gesture output requires a nonzero composition instance identity"
        );
        self.require_output_headroom(1)?;
        self.outputs.push(output);
        Ok(())
    }

    /// Take everything produced since the last drain, in production order.
    pub fn drain(&mut self) -> Vec<GestureOutput> {
        std::mem::take(&mut self.outputs)
    }

    /// Borrow the undrained outputs without taking them.
    pub fn outputs(&self) -> &[GestureOutput] {
        &self.outputs
    }
}

#[cfg(test)]
#[path = "controller_tests.rs"]
mod tests;

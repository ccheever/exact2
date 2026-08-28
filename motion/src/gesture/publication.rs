//! The interaction-state publications the arena reads at a checkpoint.
//!
//! @ref LLP 0099#gesture-composition
//!
//! Arbitration needs facts from three different publishers: the runtime
//! owns the topology, the host owns the environment, and the presenter owns
//! scroll capacity and per-claim eligibility. They arrive separately, with
//! independent generations, so this half of the controller is about joining
//! them without pretending they were one transaction — and about failing
//! closed when they disagree.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

impl GestureGraphController {
    /// Publish the host environment facts for this root.
    pub fn publish_environment(
        &mut self,
        snapshot: InteractionEnvironmentSnapshot,
    ) -> Result<(), ControllerError> {
        if snapshot.root_id != self.root_id {
            return Err(ControllerError::InteractionState);
        }
        self.interaction_state
            .publish_environment(snapshot.clone())
            .map_err(|_| ControllerError::InteractionState)?;
        self.published_environment = Some(snapshot);
        Ok(())
    }

    /// Publish the runtime-owned topology facts for the installed graph.
    pub fn publish_topology_context(
        &mut self,
        context: InteractionTopologyContext,
    ) -> Result<(), ControllerError> {
        self.validate_topology_context_identity(&context)?;
        self.interaction_state
            .publish_topology_context(context)
            .map_err(|_| ControllerError::InteractionState)?;
        self.topology_context_published = true;
        Ok(())
    }

    fn validate_topology_context_identity(
        &self,
        context: &InteractionTopologyContext,
    ) -> Result<(), ControllerError> {
        if context.root_id != self.root_id
            || context.root_instance != self.root_instance as u64
            || context.epoch != self.epoch
            || context.motion_seq != self.motion_sequence
        {
            return Err(ControllerError::InteractionState);
        }
        Ok(())
    }

    /// Publish whole-root claim eligibility.
    ///
    /// Every mounted claim must appear: an omitted claim fails closed rather
    /// than inheriting a previous publication's answer.
    pub fn publish_claim_eligibility(
        &mut self,
        records: &[ClaimEligibilityRecord],
    ) -> Result<(), ControllerError> {
        self.published_claim_eligibility = self.validated_claim_eligibility(records)?;
        Ok(())
    }

    fn validated_claim_eligibility(
        &self,
        records: &[ClaimEligibilityRecord],
    ) -> Result<BTreeMap<ArenaClaimId, PublishedClaimEligibility>, ControllerError> {
        let presenter_generation = self
            .published_environment
            .as_ref()
            .map(|snapshot| snapshot.presenter_generation)
            .ok_or(ControllerError::InteractionState)?;
        let expected = self
            .roots_by_node
            .values()
            .flatten()
            .copied()
            .map(ArenaClaimId)
            .collect::<BTreeSet<_>>();
        let mut published = BTreeMap::new();
        for record in records {
            let claim = ArenaClaimId(record.claim_id);
            let descriptor = self
                .arena
                .profile()
                .claim(claim)
                .ok_or(ControllerError::InteractionState)?;
            if record.presenter_generation != presenter_generation
                || (record.scene_lease_acquired && descriptor.kind != ArenaClaimKind::RouterHistory)
                || published.contains_key(&claim)
            {
                return Err(ControllerError::InteractionState);
            }
            published.insert(
                claim,
                PublishedClaimEligibility {
                    eligibility: ClaimEligibility {
                        can_consume: record.can_consume,
                        scene_lease_acquired: record.scene_lease_acquired,
                    },
                    presenter_generation,
                },
            );
        }
        if published.keys().copied().collect::<BTreeSet<_>>() != expected {
            return Err(ControllerError::InteractionState);
        }
        Ok(published)
    }

    /// Publish the live capacity of every native scroll presenter on this root.
    pub fn publish_scroll_capacity(
        &mut self,
        records: &[ScrollCapacityPublication],
    ) -> Result<(), ControllerError> {
        let mut staged_store = self.interaction_state.clone();
        let staged_by_node = self.stage_scroll_capacity(records, &mut staged_store)?;
        self.interaction_state = staged_store;
        self.scroll_ids_by_node = staged_by_node;
        Ok(())
    }

    fn stage_scroll_capacity(
        &self,
        records: &[ScrollCapacityPublication],
        staged_store: &mut InteractionStateStore,
    ) -> Result<BTreeMap<u32, BTreeSet<u64>>, ControllerError> {
        let presenter_generation = self
            .published_environment
            .as_ref()
            .map(|snapshot| snapshot.presenter_generation)
            .ok_or(ControllerError::InteractionState)?;
        for ids in self.scroll_ids_by_node.values() {
            for id in ids {
                staged_store.tombstone_scroll_capacity(*id);
            }
        }
        let mut staged_by_node: BTreeMap<u32, BTreeSet<u64>> = BTreeMap::new();
        let mut staged_scroll_ids = BTreeSet::new();
        for record in records {
            let capacity = record.capacity;
            if capacity.root_id != self.root_id
                || record.claim_node_id == 0
                || !self.roots_by_node.contains_key(&record.claim_node_id)
                || capacity.presenter_generation != presenter_generation
                || !staged_scroll_ids.insert(capacity.scroll_id)
            {
                return Err(ControllerError::InteractionState);
            }
            staged_by_node
                .entry(record.claim_node_id)
                .or_default()
                .insert(capacity.scroll_id);
            staged_store
                .publish_scroll_capacity(capacity)
                .map_err(|_| ControllerError::InteractionState)?;
        }
        Ok(staged_by_node)
    }

    /// Replace all three joined interaction-state publications at once.
    ///
    /// Validation runs against a cloned store and local publisher maps: no live
    /// field changes until the topology, the complete claim set, and the
    /// whole-root scroll capacity are all accepted together.
    pub fn publish_interaction_state(
        &mut self,
        context: InteractionTopologyContext,
        claim_records: &[ClaimEligibilityRecord],
        scroll_records: &[ScrollCapacityPublication],
    ) -> Result<(), ControllerError> {
        self.validate_topology_context_identity(&context)?;
        let staged_claims = self.validated_claim_eligibility(claim_records)?;
        let mut staged_store = self.interaction_state.clone();
        staged_store
            .publish_topology_context(context)
            .map_err(|_| ControllerError::InteractionState)?;
        let staged_scroll = self.stage_scroll_capacity(scroll_records, &mut staged_store)?;

        self.interaction_state = staged_store;
        self.published_claim_eligibility = staged_claims;
        self.scroll_ids_by_node = staged_scroll;
        self.topology_context_published = true;
        Ok(())
    }

    pub(crate) fn clear_scroll_capacity(&mut self) {
        for ids in self.scroll_ids_by_node.values() {
            for id in ids {
                self.interaction_state.tombstone_scroll_capacity(*id);
            }
        }
        self.scroll_ids_by_node.clear();
    }

    /// Resolve the eligibility snapshot the arena reads at a checkpoint.
    ///
    /// A claim is eligible only if the presenter published it as consumable at
    /// the current generation and no ancestor scroll presenter on the same node
    /// still has capacity on the claim's axis and direction.
    pub fn eligibility(
        &self,
        input_kind: ArenaInputKind,
    ) -> Result<ArenaEligibilitySnapshot, ControllerError> {
        if !self.topology_context_published {
            return Err(ControllerError::InteractionState);
        }
        let environment = self
            .published_environment
            .as_ref()
            .ok_or(ControllerError::InteractionState)?;
        let presenter_generation = environment.presenter_generation;
        // An AppKit NSScrollView consumes wheel/trackpad scrolling, not a
        // mouse-drag stream owned by an NSPanGestureRecognizer. Applying its
        // same-axis capacity to that recognizer stream makes draggable
        // content inside a horizontal scroller permanently ineligible (and
        // lets descendant text selection steal the drag). Other input kinds
        // and display modes retain the conservative scroll-capacity gate.
        let ancestor_scroll_competes = !matches!(
            (self.profile_kind, environment.display_mode, input_kind),
            (
                ArenaProfileKind::MacosTrackpad,
                InteractionDisplayMode::Desktop,
                ArenaInputKind::PlatformRecognizer
            )
        );
        let claims = self
            .roots_by_node
            .iter()
            .flat_map(|(node_id, ids)| ids.iter().copied().map(move |id| (*node_id, id)))
            .map(|(node_id, id)| {
                let claim_id = ArenaClaimId(id);
                let mut eligibility = self
                    .published_claim_eligibility
                    .get(&claim_id)
                    .filter(|record| record.presenter_generation == presenter_generation)
                    .map(|record| record.eligibility)
                    .unwrap_or(ClaimEligibility {
                        can_consume: false,
                        scene_lease_acquired: false,
                    });
                if eligibility.can_consume && ancestor_scroll_competes {
                    if let Some(descriptor) = self.arena.profile().claim(claim_id) {
                        let direction = self.claim_directions.get(&id).copied().flatten();
                        eligibility.can_consume &= !self
                            .scroll_ids_by_node
                            .get(&node_id)
                            .into_iter()
                            .flatten()
                            .filter_map(|id| self.interaction_state.scroll_capacity(*id))
                            .any(|scroll| {
                                scroll.presenter_generation == presenter_generation
                                    && scroll.axis == descriptor.axis
                                    && direction.map_or(
                                        scroll.can_consume_negative || scroll.can_consume_positive,
                                        |value| scroll.can_consume(value),
                                    )
                            });
                    }
                }
                (claim_id, eligibility)
            })
            .collect::<BTreeMap<_, _>>();
        self.interaction_state
            .arena_snapshot(self.root_id, self.arena.profile(), &claims)
            .map_err(|_| ControllerError::InteractionState)
    }
}

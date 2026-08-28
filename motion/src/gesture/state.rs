//! The interaction state store: the join point for the separately published
//! facts an arbitration decision reads.
//!
//! @ref LLP 0099#gesture-composition
//!
//! Topology, host environment, and presenter scroll capacity arrive from
//! different publishers with independent generations. The store joins them
//! without pretending they are one transaction.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

/// Frozen identity of the split-publisher interaction store. `motion_seq` and
/// policy revision may advance while eligibility is re-read; the remaining
/// fields define structural stream identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteractionSourceIdentity {
    /// Runtime epoch of the publishing graph. A new epoch is a new world;
    /// nothing frozen carries across one.
    pub epoch: u32,
    /// The root this identity describes. Never zero.
    pub root_id: u64,
    /// Instance counter for the root, bumped whenever a root is torn down and
    /// replaced. It is what stops a late commit from an old root from touching
    /// its replacement.
    pub root_instance: u64,
    /// Sequence number of the topology publication. It advances while a stream
    /// runs, so it is deliberately not part of structural identity; a value
    /// below the attached snapshot's is still rejected as stale.
    pub motion_seq: u64,
    /// Generation of the arbitration profile in force. Structural: a reprofiled
    /// root cannot keep a stream frozen against the previous claim table.
    pub profile_generation: u32,
    /// Generation of the modal barrier state. Structural, so a modal opening or
    /// closing invalidates a stream mid-flight instead of racing it.
    pub modal_generation: u32,
    /// Generation of the host-environment publication joined onto this identity.
    /// Not structural: the presenter may republish while a stream runs.
    pub presenter_generation: u32,
    /// Revision of the back-navigation topology, meaning what a back gesture
    /// would target. Structural.
    pub back_structural_revision: u64,
    /// Revision of the back-navigation policy, meaning whether and how back may
    /// be claimed. Not structural: a policy change is picked up at the next
    /// checkpoint rather than cancelling the stream.
    pub back_policy_revision: u64,
}

impl InteractionSourceIdentity {
    pub(crate) fn is_valid(self) -> bool {
        self.epoch != 0
            && self.root_id != 0
            && self.root_instance != 0
            && self.profile_generation != 0
            && self.presenter_generation != 0
    }

    pub(crate) fn structurally_matches(self, current: Self) -> bool {
        self.epoch == current.epoch
            && self.root_id == current.root_id
            && self.root_instance == current.root_instance
            && self.profile_generation == current.profile_generation
            && self.modal_generation == current.modal_generation
            && self.back_structural_revision == current.back_structural_revision
    }
}

/// Whether one claim may consume a stream, as re-read at an arbitration
/// checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimEligibility {
    /// Whether the claim may take the stream at all. The store only narrows the
    /// publisher's value: an unmounted claim, an unfocused root, or a scroll
    /// with no remaining travel cannot consume regardless of what was published.
    pub can_consume: bool,
    /// Required only for an interactive RouterHistory claim. Failure is a
    /// deterministic downgrade, never a stale interactive lease.
    pub scene_lease_acquired: bool,
}

/// One checkpoint's view of eligibility across a root's whole claim table,
/// carried together with the source identity it was joined from. The arena
/// compares that identity against the one the stream froze on entry, so a
/// snapshot is only usable against a structurally unchanged graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArenaEligibilitySnapshot {
    /// Identity of the published facts this snapshot was joined from.
    pub source: InteractionSourceIdentity,
    /// How back navigation may be claimed for this root, before any per-claim
    /// scene-lease downgrade is applied.
    pub back_mode: BackMode,
    /// Per-claim eligibility keyed by claim id, covering every claim in the
    /// root's profile. A claim missing here is treated as unable to consume.
    pub claims: BTreeMap<ArenaClaimId, ClaimEligibility>,
}

impl ArenaEligibilitySnapshot {
    pub(crate) fn eligible(&self, claim: &ArenaClaimDescriptor) -> bool {
        let record = self
            .claims
            .get(&claim.id)
            .copied()
            .unwrap_or(ClaimEligibility {
                can_consume: false,
                scene_lease_acquired: false,
            });
        if !record.can_consume {
            return false;
        }
        if claim.kind != ArenaClaimKind::RouterHistory {
            return true;
        }
        if !self.back_mode.is_claim_capable() {
            return false;
        }
        // A failed interactive SceneLease acquisition downgrades this stream
        // to a discrete claim; it never turns claim-capable Back into a stale
        // interactive lease, and it does not expose the edge to app content.
        true
    }

    /// The back mode `claim` actually receives. An interactive claim whose scene
    /// lease was not acquired is downgraded to a discrete one; every other mode
    /// is returned unchanged. The downgrade is deterministic, and never yields a
    /// stale interactive lease.
    pub fn effective_back_mode(&self, claim: ArenaClaimId) -> BackMode {
        let scene_lease_acquired = self
            .claims
            .get(&claim)
            .is_some_and(|record| record.scene_lease_acquired);
        if self.back_mode == BackMode::InteractiveClaim && !scene_lease_acquired {
            BackMode::DiscreteClaim
        } else {
            self.back_mode
        }
    }
}

/// Writing direction, which is what fixes the meaning of the positive direction
/// along the horizontal axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutDirection {
    /// Left-to-right writing order.
    LeftToRight,
    /// Right-to-left writing order; horizontal claim directions mirror.
    RightToLeft,
}

/// The kind of host surface a root is presented on. It selects input
/// expectations, not appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionDisplayMode {
    /// A touch-first surface: contacts come from fingers and host chrome is
    /// minimal.
    Touch,
    /// A native desktop window driven by pointer and wheel input.
    Desktop,
    /// A web document, where the user agent may claim a gesture before the arena
    /// ever sees it.
    Browser,
}

/// The runtime-published half of a root's interaction facts: what exists and
/// what is allowed to claim. It is attached and detached whole, never patched
/// field by field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionTopologySnapshot {
    /// Identity and generations of the publication this snapshot came from.
    pub source: InteractionSourceIdentity,
    /// Claim ids currently mounted in the graph. A claim absent here cannot
    /// consume, whatever its published eligibility says.
    pub mounted_claims: BTreeSet<ArenaClaimId>,
    /// Id of the modal barrier in force, or `None` when nothing is modal. A
    /// present barrier requires a non-zero modal generation.
    pub modal_barrier_id: Option<u64>,
    /// The direction the graph declares. The host may resolve a different
    /// effective direction; arbitration reads both.
    pub declared_layout_direction: LayoutDirection,
    /// How back navigation may be claimed for this root.
    pub back_mode: BackMode,
}

/// The host-published half of a root's interaction facts: where the root is
/// presented and whether it currently owns input. It is published on its own
/// generation, independently of topology.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionEnvironmentSnapshot {
    /// The root this environment describes. Never zero.
    pub root_id: u64,
    /// Generation of this publication. A publication older than the stored one
    /// is rejected rather than applied.
    pub presenter_generation: u32,
    /// The root that currently holds focus, or `None` when none does. A claim
    /// can consume only when this names its own root.
    pub focused_root_id: Option<u64>,
    /// The platform window hosting the root, or `None` when it is not in one.
    /// Not being in a window blocks consumption even while focused.
    pub window_id: Option<u64>,
    /// The direction the host actually resolved, which may differ from the
    /// direction the graph declared.
    pub effective_layout_direction: LayoutDirection,
    /// The kind of surface this root is presented on.
    pub display_mode: InteractionDisplayMode,
    /// Whether host chrome is showing, such as a browser's own back affordance
    /// or a system edge, which can shadow an edge gesture.
    pub host_chrome_present: bool,
}

/// How far one scroll view can still travel on one axis. The presenter
/// publishes it, and it is the last gate a scroll claim's eligibility passes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollCapacityRecord {
    /// Identity of the scroll view, which is also the id of the scroll claim
    /// this record gates.
    pub scroll_id: u64,
    /// The root the scroll view belongs to. A record that changes its root is
    /// rejected rather than reinterpreted.
    pub root_id: u64,
    /// The axis this record describes. A scroll claim on another axis is not
    /// gated by it.
    pub axis: ArenaAxis,
    /// Current scroll offset along `axis`, in points.
    pub offset: f64,
    /// Total scrollable content length along `axis`, in points. Never negative.
    pub extent: f64,
    /// Visible length along `axis`, in points. Never negative.
    pub viewport: f64,
    /// Whether travel remains in the negative direction along `axis`.
    pub can_consume_negative: bool,
    /// Whether travel remains in the positive direction along `axis`.
    pub can_consume_positive: bool,
    /// Generation of the presenter publication that produced this record. An
    /// older generation is rejected as stale.
    pub presenter_generation: u32,
}

/// The runtime-owned fields that naturally arrive beside a committed
/// navigation or topology sidecar. It updates an already attached root; it
/// never creates one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteractionTopologyContext {
    /// The attached root to update.
    pub root_id: u64,
    /// Must equal the attached snapshot's instance, so a publication aimed at a
    /// replaced root is rejected instead of mutating its successor.
    pub root_instance: u64,
    /// Must equal the attached snapshot's epoch.
    pub epoch: u32,
    /// Must equal the attached snapshot's sequence. This update rides the
    /// existing publication rather than advancing it.
    pub motion_seq: u64,
    /// The modal barrier now in force, or `None` when nothing is modal. A
    /// present barrier requires a non-zero `modal_generation`.
    pub modal_barrier_id: Option<u64>,
    /// Generation of the modal state. It may not move backward.
    pub modal_generation: u32,
    /// The layout direction the graph now declares.
    pub declared_layout_direction: LayoutDirection,
    /// How back navigation may now be claimed. Any mode other than
    /// `BackMode::None` requires both back revisions to be non-zero.
    pub back_mode: BackMode,
    /// Revision of the back-navigation topology. It may not move backward, and
    /// a change to it structurally invalidates in-flight streams.
    pub back_structural_revision: u64,
    /// Revision of the back-navigation policy. It may not move backward, and is
    /// re-read at checkpoints instead of invalidating a stream.
    pub back_policy_revision: u64,
}

impl ScrollCapacityRecord {
    /// Whether travel remains in `direction` along this record's axis.
    pub fn can_consume(self, direction: ArenaDirection) -> bool {
        match direction {
            ArenaDirection::Negative => self.can_consume_negative,
            ArenaDirection::Positive => self.can_consume_positive,
        }
    }
}

/// Why a publication into the interaction store, or a join read out of it, was
/// refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractionStateError {
    /// A root id or root instance was zero, which is reserved to mean "absent".
    InvalidRoot,
    /// A source identity or topology context failed its non-zero and coherence
    /// checks.
    InvalidSource,
    /// An environment publication carried a zero root or presenter generation,
    /// or an explicitly zero focused root or window id.
    InvalidEnvironment,
    /// A scroll record carried a zero identity, a non-finite measurement, or a
    /// negative extent or viewport.
    InvalidScrollRecord,
    /// Nothing is attached for the named root, so there is nothing to update or
    /// join against.
    UnknownRoot(u64),
    /// The publication names a root instance, epoch, or sequence other than the
    /// attached snapshot's. A superseded publisher cannot mutate its successor.
    RootInstanceMismatch,
    /// The publication's generation is older than what is already stored. The
    /// store keeps what it has rather than letting a publisher move backward.
    StalePublisherGeneration,
}

/// Main-owned join point for the split natural publishers adjudicated in LLP
/// 0336 OQ16/OQ17. It is not a transport: runtime topology, host environment,
/// and presenter scroll capacity retain distinct update methods and source
/// generations.
#[derive(Debug, Clone, Default)]
pub struct InteractionStateStore {
    topology: BTreeMap<u64, InteractionTopologySnapshot>,
    environment: BTreeMap<u64, InteractionEnvironmentSnapshot>,
    scroll_capacity: BTreeMap<u64, ScrollCapacityRecord>,
}

impl InteractionStateStore {
    /// Runtime-side sidecar detach phase. The root instance must match so a
    /// late old-root commit cannot remove a replacement root's topology.
    pub fn detach_topology(
        &mut self,
        root_id: u64,
        root_instance: u64,
    ) -> Result<bool, InteractionStateError> {
        if root_id == 0 || root_instance == 0 {
            return Err(InteractionStateError::InvalidRoot);
        }
        if self
            .topology
            .get(&root_id)
            .is_some_and(|snapshot| snapshot.source.root_instance != root_instance)
        {
            return Err(InteractionStateError::RootInstanceMismatch);
        }
        Ok(self.topology.remove(&root_id).is_some())
    }

    /// Runtime-side sidecar attach phase, called only after presenter apply.
    /// Whole-root omission is represented by calling `detach_topology`
    /// without a matching attach after the watermark applies.
    pub fn attach_topology(
        &mut self,
        snapshot: InteractionTopologySnapshot,
    ) -> Result<(), InteractionStateError> {
        if !snapshot.source.is_valid() || snapshot.source.root_id == 0 {
            return Err(InteractionStateError::InvalidSource);
        }
        if let Some(previous) = self.topology.get(&snapshot.source.root_id) {
            if previous.source.root_instance == snapshot.source.root_instance
                && snapshot.source.motion_seq < previous.source.motion_seq
            {
                return Err(InteractionStateError::StalePublisherGeneration);
            }
        }
        self.topology.insert(snapshot.source.root_id, snapshot);
        Ok(())
    }

    /// Update the runtime-owned fields that naturally arrive beside the
    /// committed navigation/topology sidecar. Identity fields fence the
    /// update to the currently attached whole-root snapshot; older router or
    /// modal publications therefore cannot mutate a successor graph.
    pub fn publish_topology_context(
        &mut self,
        context: InteractionTopologyContext,
    ) -> Result<(), InteractionStateError> {
        if context.root_id == 0
            || context.root_instance == 0
            || context.epoch == 0
            || context.motion_seq == 0
            || (context.modal_barrier_id.is_some() && context.modal_generation == 0)
            || (context.back_mode != BackMode::None
                && (context.back_structural_revision == 0 || context.back_policy_revision == 0))
        {
            return Err(InteractionStateError::InvalidSource);
        }
        let snapshot = self
            .topology
            .get_mut(&context.root_id)
            .ok_or(InteractionStateError::UnknownRoot(context.root_id))?;
        if snapshot.source.root_instance != context.root_instance
            || snapshot.source.epoch != context.epoch
            || snapshot.source.motion_seq != context.motion_seq
        {
            return Err(InteractionStateError::RootInstanceMismatch);
        }
        if context.modal_generation < snapshot.source.modal_generation
            || context.back_structural_revision < snapshot.source.back_structural_revision
            || context.back_policy_revision < snapshot.source.back_policy_revision
        {
            return Err(InteractionStateError::StalePublisherGeneration);
        }
        snapshot.source.modal_generation = context.modal_generation;
        snapshot.source.back_structural_revision = context.back_structural_revision;
        snapshot.source.back_policy_revision = context.back_policy_revision;
        snapshot.modal_barrier_id = context.modal_barrier_id;
        snapshot.declared_layout_direction = context.declared_layout_direction;
        snapshot.back_mode = context.back_mode;
        Ok(())
    }

    /// Publish the host-owned half of a root's facts. It is independent of
    /// topology attachment, so an environment may arrive before or after the
    /// topology for the same root. A publication older than the stored one is
    /// rejected rather than applied.
    pub fn publish_environment(
        &mut self,
        snapshot: InteractionEnvironmentSnapshot,
    ) -> Result<(), InteractionStateError> {
        if snapshot.root_id == 0
            || snapshot.presenter_generation == 0
            || snapshot.focused_root_id == Some(0)
            || snapshot.window_id == Some(0)
        {
            return Err(InteractionStateError::InvalidEnvironment);
        }
        if self
            .environment
            .get(&snapshot.root_id)
            .is_some_and(|previous| snapshot.presenter_generation < previous.presenter_generation)
        {
            return Err(InteractionStateError::StalePublisherGeneration);
        }
        self.environment.insert(snapshot.root_id, snapshot);
        Ok(())
    }

    /// Publish one scroll view's remaining travel. Records are keyed by scroll
    /// id, which is also the scroll claim's id. A record that moves the view to
    /// a different root, or that moves the presenter generation backward, is
    /// rejected.
    pub fn publish_scroll_capacity(
        &mut self,
        record: ScrollCapacityRecord,
    ) -> Result<(), InteractionStateError> {
        if record.scroll_id == 0
            || record.root_id == 0
            || record.presenter_generation == 0
            || !record.offset.is_finite()
            || !record.extent.is_finite()
            || !record.viewport.is_finite()
            || record.extent < 0.0
            || record.viewport < 0.0
        {
            return Err(InteractionStateError::InvalidScrollRecord);
        }
        if self
            .scroll_capacity
            .get(&record.scroll_id)
            .is_some_and(|previous| {
                previous.root_id != record.root_id
                    || record.presenter_generation < previous.presenter_generation
            })
        {
            return Err(InteractionStateError::StalePublisherGeneration);
        }
        self.scroll_capacity.insert(record.scroll_id, record);
        Ok(())
    }

    /// Presenter detach tombstones before native-view reuse.
    pub fn tombstone_scroll_capacity(&mut self, scroll_id: u64) -> bool {
        self.scroll_capacity.remove(&scroll_id).is_some()
    }

    /// Join the three publications into one checkpoint view of eligibility for
    /// `root_id`. `base_eligibility` is the caller's starting point and is only
    /// ever narrowed here: the root must be focused and in a window, the claim
    /// must be mounted, and a scroll claim must have capacity on its own axis
    /// and direction. The joined source carries topology's identity with the
    /// environment's presenter generation, because the two publishers advance
    /// separately.
    pub fn arena_snapshot(
        &self,
        root_id: u64,
        profile: &ArenaProfile,
        base_eligibility: &BTreeMap<ArenaClaimId, ClaimEligibility>,
    ) -> Result<ArenaEligibilitySnapshot, InteractionStateError> {
        let topology = self
            .topology
            .get(&root_id)
            .ok_or(InteractionStateError::UnknownRoot(root_id))?;
        let environment = self
            .environment
            .get(&root_id)
            .ok_or(InteractionStateError::UnknownRoot(root_id))?;
        let root_focused =
            environment.focused_root_id == Some(root_id) && environment.window_id.is_some();
        let mut claims = BTreeMap::new();
        for descriptor in profile.claims.values() {
            let mut record =
                base_eligibility
                    .get(&descriptor.id)
                    .copied()
                    .unwrap_or(ClaimEligibility {
                        can_consume: false,
                        scene_lease_acquired: false,
                    });
            record.can_consume &= root_focused && topology.mounted_claims.contains(&descriptor.id);
            if descriptor.kind == ArenaClaimKind::Scroll {
                record.can_consume &=
                    self.scroll_capacity
                        .get(&descriptor.id.0)
                        .is_some_and(|scroll| {
                            scroll.root_id == root_id
                                && scroll.axis == descriptor.axis
                                && scroll.can_consume(descriptor.direction)
                        });
            }
            claims.insert(descriptor.id, record);
        }
        let mut source = topology.source;
        source.presenter_generation = environment.presenter_generation;
        Ok(ArenaEligibilitySnapshot {
            source,
            back_mode: topology.back_mode,
            claims,
        })
    }

    /// The topology attached for a root, or `None` when none is attached.
    pub fn topology(&self, root_id: u64) -> Option<&InteractionTopologySnapshot> {
        self.topology.get(&root_id)
    }

    /// The last published capacity for a scroll view, or `None` when none was
    /// published or it has been tombstoned.
    pub fn scroll_capacity(&self, scroll_id: u64) -> Option<ScrollCapacityRecord> {
        self.scroll_capacity.get(&scroll_id).copied()
    }
}

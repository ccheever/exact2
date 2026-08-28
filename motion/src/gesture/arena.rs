//! The gesture arena: claim arbitration, leases, and bounded receipts.
//!
//! @ref LLP 0099#gesture-composition
//!
//! This is the one arena. A stream freezes its claim topology on entry and
//! only re-reads eligibility at three named checkpoints, so a decision can
//! never be re-litigated midstream.

use super::*;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Arbitration receipts one arena retains before the oldest is dropped.
pub const DEFAULT_ARENA_RECEIPT_CAPACITY: usize = 100;

/// Where one claim stands in a single stream. A claim leaves `Possible`
/// exactly once and never returns to it, so a decision is never re-litigated
/// midstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimPhase {
    /// Still contending: frozen into this stream's topology at entry and
    /// neither won nor lost yet.
    Possible,
    /// Dropped out before any lease existed. Nothing was moved on screen, so
    /// there is nothing to undo.
    Failed,
    /// Holds or shares this stream's lease. Only an active claim may produce
    /// visible motion.
    Active,
    /// Held the lease and finished normally.
    Ended,
    /// Held the lease and lost it; the lease's cancel reason says why, and any
    /// visible motion it produced has to be undone.
    Cancelled,
}

/// Why a claim left `Possible` without ever activating. Recorded per claim so
/// a receipt can tell a lost race apart from a claim that was never eligible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimFailureReason {
    /// Another contender took this stream's lease.
    LostRace,
    /// A claim this one declared it required to fail activated instead, so it
    /// is blocked for the rest of the stream.
    RequiredClaimActive,
    /// The eligibility snapshot read at activation did not let this claim
    /// consume.
    Ineligible,
    /// Stood down one-way to a same-axis scroll claim while both were still
    /// `Possible`.
    PreLockHandoff,
}

/// The externally visible owner of a stream's one lease. A compound presents a
/// single identity to the arena however many of its members are running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseHolder {
    /// One claim holds the lease on its own.
    Claim(ArenaClaimId),
    /// A compound holds the lease; the members actually running are listed
    /// separately on the lease.
    Compound(CompoundClaimId),
}

/// How far a granted lease has advanced. The three checkpoints move it
/// forward; nothing moves it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeasePhase {
    /// Granted and running. Visible motion may be marked from here.
    Active,
    /// Passed the commit re-check: the frozen source identity and every active
    /// member's eligibility still hold.
    CommitChecked,
    /// Dispatch was allowed and the effect is in flight. The lease is held
    /// until a settle acknowledgement quoting its sequence arrives.
    CommittedBusy,
    /// Retired normally after its settle acknowledgement.
    Ended,
    /// Retired by cancellation; the lease carries the reason.
    Cancelled,
}

/// Why an active lease was retired without completing. Carried on the lease
/// and on the stream's terminal receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArenaCancelReason {
    /// The live source identity no longer structurally matches the one frozen
    /// at stream entry: a descriptor, reset, modal, or profile change.
    StructuralInvalidation,
    /// The platform or browser took the stream over.
    ExternalOwner,
    /// A holding member stopped being able to consume at the commit
    /// checkpoint.
    HolderIneligible,
    /// The dispatch checkpoint was refused by a router or action blocker; the
    /// effect snaps back instead of committing.
    Blocked,
    /// An engine reset or epoch bump retired the stream. No events follow the
    /// fence.
    Reset,
    /// The lease expired without reaching its terminal acknowledgement.
    Timeout,
    /// The claiming node left the tree while the lease was held.
    Detached,
    /// Author code cancelled the gesture explicitly.
    AuthorCancelled,
}

/// The single exclusive lease a stream may hold. A lease is never transferred:
/// a contender wanting a different outcome has to stand down before one
/// exists, and after visible motion not even that is allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionLease {
    /// Per-arena grant number, increasing and never reused. A settle
    /// acknowledgement has to quote this exact value, so an acknowledgement
    /// aimed at a retired lease cannot release a later one.
    pub sequence: u64,
    /// The stream this lease was granted for.
    pub stream_id: GestureStreamId,
    /// The externally visible owner: one claim, or the compound identity
    /// covering several.
    pub holder: LeaseHolder,
    /// The claims actually running behind the holder, sorted. A simultaneous
    /// compound may gain members while the lease is active.
    pub active_members: Vec<ArenaClaimId>,
    /// The source identity copied from the stream at entry. Checkpoints
    /// compare the live identity against this, never against a re-read.
    pub frozen_source: InteractionSourceIdentity,
    /// How far the lease has advanced through the checkpoints.
    pub phase: LeasePhase,
    /// Set once the holder has moved something on screen. Past this point the
    /// lease can only end or cancel; there is no handoff.
    pub visible_motion: bool,
    /// Set exactly when the phase is `Cancelled`.
    pub cancel_reason: Option<ArenaCancelReason>,
}

/// The rule that produced a resolution. Every receipt carries one, so an
/// outcome can be explained without replaying the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArenaDecisionReason {
    /// A `System` claim won; it outranks every other kind in every profile.
    SystemTakeover,
    /// An iOS touch profile gave a claim-capable router-history claim
    /// precedence; edge zones are not claimable by app content.
    ReservedEdge,
    /// A macOS trackpad profile preferred a pager over the scrollers around
    /// it.
    MarkedPagerBeforeScroll,
    /// Same-axis nesting resolved to the deepest claim able to consume.
    DeepestConsumer,
    /// Declared priority separated claims that policy and depth had tied.
    ExplicitPriority,
    /// Declaration order broke the last tie, so the winner never depends on
    /// the order samples arrive.
    DeclarationOrder,
    /// Nothing could activate yet because a required claim is still
    /// `Possible`. A later attempt on the same stream may still succeed.
    RequireFailurePending,
    /// No candidate was eligible under the snapshot read at this checkpoint.
    Ineligible,
    /// A simultaneous compound holds the lease and its eligible members run
    /// together behind that one identity.
    SimultaneousCompound,
    /// An exclusive compound holds the lease and one member runs: the first
    /// eligible in declaration order.
    ExclusiveCompound,
    /// The platform owned the stream, whether it took over before or after
    /// the samples reached the arena.
    ExternalOwner,
    /// The commit checkpoint re-read eligibility and the holder survived it.
    CommitRecheck,
    /// The dispatch checkpoint was not blocked, so the effect may commit.
    DispatchAllowed,
    /// The stream was cancelled; the lease's cancel reason says why.
    Cancelled,
    /// A matching settle acknowledgement released a busy lease.
    Settled,
}

/// What the arena decided at one checkpoint. Returned to the caller and copied
/// into the receipt ring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArenaResolution {
    /// Owner of the lease after this decision, or `None` when nothing holds
    /// the stream.
    pub holder: Option<LeaseHolder>,
    /// The claims running behind that holder, sorted; empty when there is no
    /// holder.
    pub active_members: Vec<ArenaClaimId>,
    /// The rule that produced this outcome.
    pub reason: ArenaDecisionReason,
}

/// How a stream finished. A receipt carries one only when that receipt closed
/// the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArenaTerminalOutcome {
    /// The effect was dispatched and, where a settle is required, settled.
    Committed,
    /// The lease was retired before completing, for the given reason.
    Cancelled(ArenaCancelReason),
    /// The platform owned the outcome; the arena recorded it rather than
    /// deciding it.
    ExternalOwner,
}

/// One immutable record of an arbitration decision. Receipts are the
/// diagnostic trail: the arena keeps a bounded ring and drops the oldest, and
/// retiring a stream leaves its receipts behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArbitrationReceiptV2 {
    /// The stream this decision applies to.
    pub stream_id: GestureStreamId,
    /// The input class frozen at stream entry.
    pub input_kind: ArenaInputKind,
    /// The source identity as frozen at stream entry, not as re-read here.
    pub frozen_source: InteractionSourceIdentity,
    /// The claims weighed for this decision: the requests presented at a
    /// checkpoint, or the stream's whole frozen topology for a terminal
    /// record.
    pub contenders: Vec<ArenaClaimId>,
    /// The subset that passed the eligibility snapshot and could still
    /// contend.
    pub eligible_contenders: Vec<ArenaClaimId>,
    /// The lease owner after the decision, if any.
    pub holder: Option<LeaseHolder>,
    /// The claims running behind that holder.
    pub active_members: Vec<ArenaClaimId>,
    /// The rule that produced the decision.
    pub reason: ArenaDecisionReason,
    /// Set when this receipt closed the stream; `None` for an intermediate
    /// checkpoint.
    pub terminal: Option<ArenaTerminalOutcome>,
    /// Opaque code reported by the platform when it owned the outcome, and
    /// `None` otherwise. The arena does not interpret it.
    pub platform_reason_code: Option<u16>,
}

/// Rejections from arena calls. Every variant is a caller error against a
/// stated precondition, never a gesture outcome; a losing claim is a
/// resolution, not an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArenaRuntimeError {
    /// Stream ids are non-zero; zero is reserved for absent.
    ZeroStreamId,
    /// The source identity has a zero in a field that must be set, so it
    /// cannot be frozen.
    InvalidSourceIdentity,
    /// A stream with this id is already open in this arena.
    DuplicateStream(GestureStreamId),
    /// No open stream has this id: it was never begun, or it has been
    /// retired.
    UnknownStream(GestureStreamId),
    /// The arena's profile does not declare this claim.
    UnknownClaim(ArenaClaimId),
    /// The same claim appeared twice in one contender or request list.
    DuplicateContender(ArenaClaimId),
    /// The profile declares this claim, but it was not among the contenders
    /// frozen at stream entry, so it cannot join midstream.
    ClaimNotInFrozenTopology(ArenaClaimId),
    /// The stream already holds a lease this request cannot join: the holder
    /// is not a simultaneous compound, or the request is not one of its
    /// members.
    AlreadyLeased,
    /// The operation needs a granted lease and the stream has none.
    NoLease,
    /// The lease is not in the phase this operation requires. The checkpoints
    /// run in order, and a settle has to quote the current sequence.
    InvalidLeasePhase,
    /// A standdown or failure was attempted after the claim had locked in.
    /// There is no handoff once a lease exists or a claim is active.
    TransferAfterAxisLock,
    /// The requested standdown is not the one permitted case: both claims
    /// still `Possible`, on the same axis, with a scroll claim as target.
    InvalidHandoff,
    /// The frozen topology no longer describes the live tree. Structural
    /// mismatch normally surfaces as a cancelling resolution instead.
    StructuralInvalidation,
    /// The per-arena lease sequence would overflow. The arena refuses to
    /// grant rather than reuse a number a stale settle could quote.
    SequenceExhausted,
}

#[derive(Debug, Clone)]
struct ArenaStream {
    input_kind: ArenaInputKind,
    frozen_source: InteractionSourceIdentity,
    contenders: Vec<ArenaClaimId>,
    phases: BTreeMap<ArenaClaimId, ClaimPhase>,
    failure_reasons: BTreeMap<ArenaClaimId, ClaimFailureReason>,
    lease: Option<InteractionLease>,
}

/// Per-root typed arbitration arena. Streams freeze claim topology on entry;
/// only eligibility snapshots are re-read at the three named checkpoints.
#[derive(Debug, Clone)]
pub struct GestureArena {
    profile: ArenaProfile,
    streams: BTreeMap<GestureStreamId, ArenaStream>,
    receipts: VecDeque<ArbitrationReceiptV2>,
    receipt_capacity: usize,
    next_lease_sequence: u64,
}

impl GestureArena {
    /// Build an arena over an already validated profile, retaining
    /// [`DEFAULT_ARENA_RECEIPT_CAPACITY`] receipts.
    pub fn new(profile: ArenaProfile) -> Self {
        Self::with_receipt_capacity(profile, DEFAULT_ARENA_RECEIPT_CAPACITY)
    }

    /// Build an arena with an explicit receipt ring size. A capacity of zero
    /// keeps no receipts at all; arbitration itself is unaffected either way.
    pub fn with_receipt_capacity(profile: ArenaProfile, receipt_capacity: usize) -> Self {
        Self {
            profile,
            streams: BTreeMap::new(),
            receipts: VecDeque::new(),
            receipt_capacity,
            next_lease_sequence: 0,
        }
    }

    /// The arena owns the validated profile used for every checkpoint. The
    /// interaction-state join reads this same profile rather than rebuilding
    /// a second claim table beside the arbiter.
    pub fn profile(&self) -> &ArenaProfile {
        &self.profile
    }

    /// Open a stream and freeze its arbitration topology: the input kind, the
    /// source identity, and the exact set of claims allowed to contend. Every
    /// contender starts `Possible`, and no claim outside `contenders` can join
    /// later.
    ///
    /// Rejects a zero stream id, an unusable source identity, a stream id
    /// already open, a contender the profile does not declare, and a repeated
    /// contender.
    pub fn begin_stream(
        &mut self,
        stream_id: GestureStreamId,
        input_kind: ArenaInputKind,
        source: InteractionSourceIdentity,
        contenders: impl IntoIterator<Item = ArenaClaimId>,
    ) -> Result<(), ArenaRuntimeError> {
        if stream_id.0 == 0 {
            return Err(ArenaRuntimeError::ZeroStreamId);
        }
        if !source.is_valid() {
            return Err(ArenaRuntimeError::InvalidSourceIdentity);
        }
        if self.streams.contains_key(&stream_id) {
            return Err(ArenaRuntimeError::DuplicateStream(stream_id));
        }
        let contenders: Vec<_> = contenders.into_iter().collect();
        let mut seen = BTreeSet::new();
        for contender in &contenders {
            if !self.profile.claims.contains_key(contender) {
                return Err(ArenaRuntimeError::UnknownClaim(*contender));
            }
            if !seen.insert(*contender) {
                return Err(ArenaRuntimeError::DuplicateContender(*contender));
            }
        }
        let phases = contenders
            .iter()
            .copied()
            .map(|claim| (claim, ClaimPhase::Possible))
            .collect();
        self.streams.insert(
            stream_id,
            ArenaStream {
                input_kind,
                frozen_source: source,
                contenders,
                phases,
                failure_reasons: BTreeMap::new(),
                lease: None,
            },
        );
        Ok(())
    }

    /// Record that a claim dropped out before activation: slop exceeded, the
    /// wrong axis, whatever the recognizer decided. Only a `Possible` claim
    /// moves; failing an already terminal claim does nothing, and failing an
    /// `Active` one is rejected, because a lease is never revoked from outside.
    pub fn fail_claim(
        &mut self,
        stream_id: GestureStreamId,
        claim: ArenaClaimId,
        reason: ClaimFailureReason,
    ) -> Result<(), ArenaRuntimeError> {
        let stream = self
            .streams
            .get_mut(&stream_id)
            .ok_or(ArenaRuntimeError::UnknownStream(stream_id))?;
        let phase = stream
            .phases
            .get_mut(&claim)
            .ok_or(ArenaRuntimeError::ClaimNotInFrozenTopology(claim))?;
        if *phase == ClaimPhase::Active {
            return Err(ArenaRuntimeError::TransferAfterAxisLock);
        }
        if *phase == ClaimPhase::Possible {
            *phase = ClaimPhase::Failed;
            stream.failure_reasons.insert(claim, reason);
        }
        Ok(())
    }

    /// The only v1 handoff: before any lease/visible motion, a candidate may
    /// stand down one-way to a same-axis scroll claim. There is no transfer of
    /// an active lease and no reassignment after structural cancellation.
    pub fn stand_down_to_scroll(
        &mut self,
        stream_id: GestureStreamId,
        from: ArenaClaimId,
        to: ArenaClaimId,
    ) -> Result<(), ArenaRuntimeError> {
        let stream = self
            .streams
            .get_mut(&stream_id)
            .ok_or(ArenaRuntimeError::UnknownStream(stream_id))?;
        if stream.lease.is_some() {
            return Err(ArenaRuntimeError::TransferAfterAxisLock);
        }
        let from_descriptor = self
            .profile
            .claim(from)
            .ok_or(ArenaRuntimeError::UnknownClaim(from))?;
        let to_descriptor = self
            .profile
            .claim(to)
            .ok_or(ArenaRuntimeError::UnknownClaim(to))?;
        if !stream.phases.contains_key(&from)
            || !stream.phases.contains_key(&to)
            || to_descriptor.kind != ArenaClaimKind::Scroll
            || from_descriptor.axis != to_descriptor.axis
            || stream.phases.get(&from) != Some(&ClaimPhase::Possible)
            || stream.phases.get(&to) != Some(&ClaimPhase::Possible)
        {
            return Err(ArenaRuntimeError::InvalidHandoff);
        }
        stream.phases.insert(from, ClaimPhase::Failed);
        stream
            .failure_reasons
            .insert(from, ClaimFailureReason::PreLockHandoff);
        Ok(())
    }

    /// Run the activation checkpoint: re-read eligibility, resolve the race
    /// among the requesting claims, and grant this stream's one lease. Losing
    /// requests that were still `Possible` are failed here.
    ///
    /// If the stream already holds a lease, the requests are instead offered to
    /// a simultaneous compound holder. A live source identity that no longer
    /// structurally matches the frozen one cancels the stream rather than
    /// activating anything. An empty eligible set is not an error: the
    /// resolution carries no holder and says whether a required failure is
    /// still pending.
    pub fn activate(
        &mut self,
        stream_id: GestureStreamId,
        requests: impl IntoIterator<Item = ArenaClaimId>,
        eligibility: &ArenaEligibilitySnapshot,
    ) -> Result<ArenaResolution, ArenaRuntimeError> {
        let requests: Vec<_> = requests.into_iter().collect();
        let stream = self
            .streams
            .get(&stream_id)
            .ok_or(ArenaRuntimeError::UnknownStream(stream_id))?;
        if stream.lease.is_some() {
            return self.join_simultaneous(stream_id, &requests, eligibility);
        }
        let mut seen = BTreeSet::new();
        for request in &requests {
            if !seen.insert(*request) {
                return Err(ArenaRuntimeError::DuplicateContender(*request));
            }
            if !stream.phases.contains_key(request) {
                return Err(ArenaRuntimeError::ClaimNotInFrozenTopology(*request));
            }
        }
        if !stream
            .frozen_source
            .structurally_matches(eligibility.source)
        {
            return self.cancel_before_activation(stream_id, &requests);
        }

        let mut eligible = Vec::new();
        let mut pending_require_failure = false;
        for request in requests.iter().copied() {
            if stream.phases.get(&request) != Some(&ClaimPhase::Possible) {
                continue;
            }
            let descriptor = self.profile.claim(request).expect("frozen claim exists");
            if !eligibility.eligible(descriptor) {
                continue;
            }
            match self.requirement_readiness(stream, request) {
                RequirementReadiness::Ready => {}
                RequirementReadiness::Pending => {
                    pending_require_failure = true;
                    continue;
                }
                RequirementReadiness::Blocked => continue,
            }
            if self.exclusive_predecessor_ready(stream, request, eligibility) {
                continue;
            }
            eligible.push(request);
        }

        if eligible.is_empty() {
            let reason = if pending_require_failure {
                ArenaDecisionReason::RequireFailurePending
            } else {
                ArenaDecisionReason::Ineligible
            };
            let resolution = ArenaResolution {
                holder: None,
                active_members: Vec::new(),
                reason,
            };
            self.record_resolution(stream_id, &requests, &[], &resolution, None, None)?;
            return Ok(resolution);
        }

        let winner = self.select_winner(&eligible, eligibility);
        let (holder, mut active_members, reason) = self.expand_holder(winner, &eligible);
        active_members.sort_unstable();
        self.next_lease_sequence = self
            .next_lease_sequence
            .checked_add(1)
            .ok_or(ArenaRuntimeError::SequenceExhausted)?;
        let stream = self
            .streams
            .get_mut(&stream_id)
            .expect("stream retained during activation");
        for request in &requests {
            if active_members.contains(request) {
                stream.phases.insert(*request, ClaimPhase::Active);
            } else if stream.phases.get(request) == Some(&ClaimPhase::Possible) {
                stream.phases.insert(*request, ClaimPhase::Failed);
                stream
                    .failure_reasons
                    .insert(*request, ClaimFailureReason::LostRace);
            }
        }
        stream.lease = Some(InteractionLease {
            sequence: self.next_lease_sequence,
            stream_id,
            holder,
            active_members: active_members.clone(),
            frozen_source: stream.frozen_source,
            phase: LeasePhase::Active,
            visible_motion: false,
            cancel_reason: None,
        });
        let resolution = ArenaResolution {
            holder: Some(holder),
            active_members,
            reason,
        };
        self.record_resolution(stream_id, &requests, &eligible, &resolution, None, None)?;
        Ok(resolution)
    }

    /// Note that the holder has moved something on screen. This is the point of
    /// no return for handoff: afterwards the lease can only end or cancel.
    /// Valid only while the lease is still `Active`.
    pub fn mark_visible_motion(
        &mut self,
        stream_id: GestureStreamId,
    ) -> Result<(), ArenaRuntimeError> {
        let lease = self
            .streams
            .get_mut(&stream_id)
            .ok_or(ArenaRuntimeError::UnknownStream(stream_id))?
            .lease
            .as_mut()
            .ok_or(ArenaRuntimeError::NoLease)?;
        if lease.phase != LeasePhase::Active {
            return Err(ArenaRuntimeError::InvalidLeasePhase);
        }
        lease.visible_motion = true;
        Ok(())
    }

    /// Run the commit checkpoint: re-compare the live source identity against
    /// the frozen one and re-read eligibility for the active members. A
    /// structural mismatch, or a member that can no longer consume, cancels the
    /// stream; otherwise the lease advances to `CommitChecked` and the effect
    /// may be dispatched.
    pub fn commit_checkpoint(
        &mut self,
        stream_id: GestureStreamId,
        eligibility: &ArenaEligibilitySnapshot,
    ) -> Result<ArenaResolution, ArenaRuntimeError> {
        let (input_kind, contenders, frozen, holder, members, phase) = {
            let stream = self
                .streams
                .get(&stream_id)
                .ok_or(ArenaRuntimeError::UnknownStream(stream_id))?;
            let lease = stream.lease.as_ref().ok_or(ArenaRuntimeError::NoLease)?;
            (
                stream.input_kind,
                stream.contenders.clone(),
                stream.frozen_source,
                lease.holder,
                lease.active_members.clone(),
                lease.phase,
            )
        };
        if phase != LeasePhase::Active {
            return Err(ArenaRuntimeError::InvalidLeasePhase);
        }
        if !frozen.structurally_matches(eligibility.source) {
            return self.cancel_stream(
                stream_id,
                ArenaCancelReason::StructuralInvalidation,
                ArenaDecisionReason::Cancelled,
            );
        }
        let still_eligible = members.iter().all(|member| {
            self.profile
                .claim(*member)
                .is_some_and(|claim| eligibility.eligible(claim))
        });
        if !still_eligible {
            return self.cancel_stream(
                stream_id,
                ArenaCancelReason::HolderIneligible,
                ArenaDecisionReason::Cancelled,
            );
        }
        let stream = self.streams.get_mut(&stream_id).expect("known stream");
        stream.lease.as_mut().expect("known lease").phase = LeasePhase::CommitChecked;
        let resolution = ArenaResolution {
            holder: Some(holder),
            active_members: members.clone(),
            reason: ArenaDecisionReason::CommitRecheck,
        };
        self.push_receipt(ArbitrationReceiptV2 {
            stream_id,
            input_kind,
            frozen_source: frozen,
            contenders,
            eligible_contenders: members,
            holder: Some(holder),
            active_members: resolution.active_members.clone(),
            reason: resolution.reason,
            terminal: None,
            platform_reason_code: None,
        });
        Ok(resolution)
    }

    /// Run the dispatch checkpoint: the last chance for a router or action
    /// blocker to refuse the committed effect. A `blocked` call cancels the
    /// stream and snaps back; otherwise the lease advances to `CommittedBusy`
    /// and the receipt records the stream as committed. Requires a lease that
    /// has passed the commit checkpoint.
    pub fn dispatch_checkpoint(
        &mut self,
        stream_id: GestureStreamId,
        blocked: bool,
    ) -> Result<ArenaResolution, ArenaRuntimeError> {
        let phase = self
            .streams
            .get(&stream_id)
            .ok_or(ArenaRuntimeError::UnknownStream(stream_id))?
            .lease
            .as_ref()
            .ok_or(ArenaRuntimeError::NoLease)?
            .phase;
        if phase != LeasePhase::CommitChecked {
            return Err(ArenaRuntimeError::InvalidLeasePhase);
        }
        if blocked {
            return self.cancel_stream(
                stream_id,
                ArenaCancelReason::Blocked,
                ArenaDecisionReason::Cancelled,
            );
        }
        let stream = self.streams.get_mut(&stream_id).expect("known stream");
        let lease = stream.lease.as_mut().expect("known lease");
        lease.phase = LeasePhase::CommittedBusy;
        let resolution = ArenaResolution {
            holder: Some(lease.holder),
            active_members: lease.active_members.clone(),
            reason: ArenaDecisionReason::DispatchAllowed,
        };
        let receipt = ArbitrationReceiptV2 {
            stream_id,
            input_kind: stream.input_kind,
            frozen_source: stream.frozen_source,
            contenders: stream.contenders.clone(),
            eligible_contenders: lease.active_members.clone(),
            holder: Some(lease.holder),
            active_members: lease.active_members.clone(),
            reason: ArenaDecisionReason::DispatchAllowed,
            terminal: Some(ArenaTerminalOutcome::Committed),
            platform_reason_code: None,
        };
        self.push_receipt(receipt);
        Ok(resolution)
    }

    /// A matching token-bearing descriptor acknowledgement releases a busy
    /// pager/transition lease. Ordinary descriptor changes cannot call this.
    pub fn acknowledge_settle(
        &mut self,
        stream_id: GestureStreamId,
        lease_sequence: u64,
    ) -> Result<ArenaResolution, ArenaRuntimeError> {
        let stream = self
            .streams
            .get_mut(&stream_id)
            .ok_or(ArenaRuntimeError::UnknownStream(stream_id))?;
        let lease = stream.lease.as_mut().ok_or(ArenaRuntimeError::NoLease)?;
        if lease.phase != LeasePhase::CommittedBusy || lease.sequence != lease_sequence {
            return Err(ArenaRuntimeError::InvalidLeasePhase);
        }
        lease.phase = LeasePhase::Ended;
        for member in &lease.active_members {
            stream.phases.insert(*member, ClaimPhase::Ended);
        }
        let resolution = ArenaResolution {
            holder: Some(lease.holder),
            active_members: lease.active_members.clone(),
            reason: ArenaDecisionReason::Settled,
        };
        let receipt = ArbitrationReceiptV2 {
            stream_id,
            input_kind: stream.input_kind,
            frozen_source: stream.frozen_source,
            contenders: stream.contenders.clone(),
            eligible_contenders: lease.active_members.clone(),
            holder: Some(lease.holder),
            active_members: lease.active_members.clone(),
            reason: ArenaDecisionReason::Settled,
            terminal: Some(ArenaTerminalOutcome::Committed),
            platform_reason_code: None,
        };
        self.push_receipt(receipt);
        Ok(resolution)
    }

    /// Retire an active lease for a reason the caller has already classified:
    /// reset, timeout, detach, or author cancellation. Active members become
    /// `Cancelled` and a terminal receipt records the reason. Platform takeover
    /// goes through
    /// [`cancel_for_external_owner`](Self::cancel_for_external_owner) instead,
    /// which keeps its own terminal class and reason code.
    pub fn cancel(
        &mut self,
        stream_id: GestureStreamId,
        reason: ArenaCancelReason,
    ) -> Result<ArenaResolution, ArenaRuntimeError> {
        self.cancel_stream(stream_id, reason, ArenaDecisionReason::Cancelled)
    }

    /// Record platform takeover for a stream whose raw samples already
    /// entered the arena. This preserves the frozen topology and platform
    /// reason code while retiring any active lease as external-owned.
    pub fn cancel_for_external_owner(
        &mut self,
        stream_id: GestureStreamId,
        platform_reason_code: u16,
    ) -> Result<ArenaResolution, ArenaRuntimeError> {
        let stream = self
            .streams
            .get_mut(&stream_id)
            .ok_or(ArenaRuntimeError::UnknownStream(stream_id))?;
        let (holder, active_members) = if let Some(lease) = stream.lease.as_mut() {
            if matches!(lease.phase, LeasePhase::Ended | LeasePhase::Cancelled) {
                return Err(ArenaRuntimeError::InvalidLeasePhase);
            }
            lease.phase = LeasePhase::Cancelled;
            lease.cancel_reason = Some(ArenaCancelReason::ExternalOwner);
            for member in &lease.active_members {
                stream.phases.insert(*member, ClaimPhase::Cancelled);
            }
            (Some(lease.holder), lease.active_members.clone())
        } else {
            for phase in stream.phases.values_mut() {
                if *phase == ClaimPhase::Possible {
                    *phase = ClaimPhase::Failed;
                }
            }
            (None, Vec::new())
        };
        let resolution = ArenaResolution {
            holder,
            active_members: active_members.clone(),
            reason: ArenaDecisionReason::ExternalOwner,
        };
        let receipt = ArbitrationReceiptV2 {
            stream_id,
            input_kind: stream.input_kind,
            frozen_source: stream.frozen_source,
            contenders: stream.contenders.clone(),
            eligible_contenders: active_members.clone(),
            holder,
            active_members,
            reason: ArenaDecisionReason::ExternalOwner,
            terminal: Some(ArenaTerminalOutcome::ExternalOwner),
            platform_reason_code: Some(platform_reason_code),
        };
        self.push_receipt(receipt);
        Ok(resolution)
    }

    /// Drop terminal per-stream state after its immutable receipt has been
    /// copied to the owning outcome seam. Receipt history remains bounded and
    /// queryable independently.
    pub(crate) fn retire_stream(&mut self, stream_id: GestureStreamId) -> bool {
        self.streams.remove(&stream_id).is_some()
    }

    /// Record a platform/browser decision made before raw samples reached the
    /// arena. No synthetic losing decision or lease is created.
    pub fn report_external_owner(
        &mut self,
        stream_id: GestureStreamId,
        input_kind: ArenaInputKind,
        source: InteractionSourceIdentity,
        contenders: impl IntoIterator<Item = ArenaClaimId>,
        platform_winner: Option<ArenaClaimId>,
        platform_reason_code: u16,
    ) -> Result<(), ArenaRuntimeError> {
        if stream_id.0 == 0 {
            return Err(ArenaRuntimeError::ZeroStreamId);
        }
        if !source.is_valid() {
            return Err(ArenaRuntimeError::InvalidSourceIdentity);
        }
        let contenders: Vec<_> = contenders.into_iter().collect();
        let mut seen = BTreeSet::new();
        for contender in &contenders {
            if !self.profile.claims.contains_key(contender) {
                return Err(ArenaRuntimeError::UnknownClaim(*contender));
            }
            if !seen.insert(*contender) {
                return Err(ArenaRuntimeError::DuplicateContender(*contender));
            }
        }
        if platform_winner.is_some_and(|winner| !seen.contains(&winner)) {
            return Err(ArenaRuntimeError::ClaimNotInFrozenTopology(
                platform_winner.expect("checked some"),
            ));
        }
        self.push_receipt(ArbitrationReceiptV2 {
            stream_id,
            input_kind,
            frozen_source: source,
            contenders,
            eligible_contenders: platform_winner.into_iter().collect(),
            holder: platform_winner.map(LeaseHolder::Claim),
            active_members: platform_winner.into_iter().collect(),
            reason: ArenaDecisionReason::ExternalOwner,
            terminal: Some(ArenaTerminalOutcome::ExternalOwner),
            platform_reason_code: Some(platform_reason_code),
        });
        Ok(())
    }

    /// This stream's lease, if one was granted. A cancelled or ended lease is
    /// still returned, carrying its terminal phase and cancel reason; an
    /// unknown or retired stream returns `None`.
    pub fn lease(&self, stream_id: GestureStreamId) -> Option<&InteractionLease> {
        self.streams
            .get(&stream_id)
            .and_then(|stream| stream.lease.as_ref())
    }

    /// Where one claim stands in one stream. `None` for an unknown stream, or
    /// for a claim outside that stream's frozen topology.
    pub fn claim_phase(
        &self,
        stream_id: GestureStreamId,
        claim: ArenaClaimId,
    ) -> Option<ClaimPhase> {
        self.streams
            .get(&stream_id)
            .and_then(|stream| stream.phases.get(&claim).copied())
    }

    /// The bounded receipt ring, oldest first. Retiring a stream leaves its
    /// receipts in place, so history stays queryable after the stream is gone.
    pub fn receipts(&self) -> &VecDeque<ArbitrationReceiptV2> {
        &self.receipts
    }

    /// Drop every open stream, for an engine reset or epoch bump. Receipts and
    /// the lease sequence survive, so a post-reset grant cannot reuse a
    /// sequence number an in-flight settle might still quote.
    pub fn reset(&mut self) {
        self.streams.clear();
    }

    fn requirement_readiness(
        &self,
        stream: &ArenaStream,
        claim: ArenaClaimId,
    ) -> RequirementReadiness {
        let Some(required) = self.profile.requires.get(&claim) else {
            return RequirementReadiness::Ready;
        };
        let mut pending = false;
        for dependency in required {
            match stream.phases.get(dependency).copied() {
                Some(ClaimPhase::Failed | ClaimPhase::Cancelled) => {}
                Some(ClaimPhase::Active | ClaimPhase::Ended) => {
                    return RequirementReadiness::Blocked
                }
                Some(ClaimPhase::Possible) => pending = true,
                // A declared dependency outside this stream's frozen hit
                // topology has already failed to contend for this stream.
                None => {}
            }
        }
        if pending {
            RequirementReadiness::Pending
        } else {
            RequirementReadiness::Ready
        }
    }

    fn join_simultaneous(
        &mut self,
        stream_id: GestureStreamId,
        requests: &[ArenaClaimId],
        eligibility: &ArenaEligibilitySnapshot,
    ) -> Result<ArenaResolution, ArenaRuntimeError> {
        let stream = self
            .streams
            .get(&stream_id)
            .ok_or(ArenaRuntimeError::UnknownStream(stream_id))?;
        let lease = stream.lease.as_ref().ok_or(ArenaRuntimeError::NoLease)?;
        let LeaseHolder::Compound(compound_id) = lease.holder else {
            return Err(ArenaRuntimeError::AlreadyLeased);
        };
        let compound = self
            .profile
            .compounds
            .get(&compound_id)
            .ok_or(ArenaRuntimeError::AlreadyLeased)?;
        if compound.policy != CompoundPolicy::Simultaneous || lease.phase != LeasePhase::Active {
            return Err(ArenaRuntimeError::AlreadyLeased);
        }
        let mut seen = BTreeSet::new();
        for request in requests {
            if !seen.insert(*request) {
                return Err(ArenaRuntimeError::DuplicateContender(*request));
            }
            if !stream.phases.contains_key(request) {
                return Err(ArenaRuntimeError::ClaimNotInFrozenTopology(*request));
            }
            if self
                .profile
                .claim(*request)
                .and_then(|claim| claim.compound)
                != Some(compound_id)
            {
                return Err(ArenaRuntimeError::AlreadyLeased);
            }
        }
        if !stream
            .frozen_source
            .structurally_matches(eligibility.source)
        {
            return self.cancel_stream(
                stream_id,
                ArenaCancelReason::StructuralInvalidation,
                ArenaDecisionReason::Cancelled,
            );
        }
        let joinable: Vec<_> = requests
            .iter()
            .copied()
            .filter(|request| stream.phases.get(request) == Some(&ClaimPhase::Possible))
            .filter(|request| {
                self.profile
                    .claim(*request)
                    .is_some_and(|claim| eligibility.eligible(claim))
            })
            .filter(|request| {
                self.requirement_readiness(stream, *request) == RequirementReadiness::Ready
            })
            .collect();
        if joinable.is_empty() {
            let resolution = ArenaResolution {
                holder: Some(LeaseHolder::Compound(compound_id)),
                active_members: lease.active_members.clone(),
                reason: ArenaDecisionReason::Ineligible,
            };
            self.record_resolution(stream_id, requests, &[], &resolution, None, None)?;
            return Ok(resolution);
        }
        let stream = self.streams.get_mut(&stream_id).expect("known stream");
        let lease = stream.lease.as_mut().expect("known lease");
        for member in &joinable {
            stream.phases.insert(*member, ClaimPhase::Active);
            if !lease.active_members.contains(member) {
                lease.active_members.push(*member);
            }
        }
        lease.active_members.sort_unstable();
        let resolution = ArenaResolution {
            holder: Some(lease.holder),
            active_members: lease.active_members.clone(),
            reason: ArenaDecisionReason::SimultaneousCompound,
        };
        self.record_resolution(stream_id, requests, &joinable, &resolution, None, None)?;
        Ok(resolution)
    }

    fn exclusive_predecessor_ready(
        &self,
        stream: &ArenaStream,
        claim: ArenaClaimId,
        eligibility: &ArenaEligibilitySnapshot,
    ) -> bool {
        self.profile
            .exclusive_predecessors
            .get(&claim)
            .is_some_and(|predecessors| {
                predecessors.iter().any(|predecessor| {
                    stream.phases.get(predecessor) == Some(&ClaimPhase::Possible)
                        && self
                            .profile
                            .claim(*predecessor)
                            .is_some_and(|descriptor| eligibility.eligible(descriptor))
                })
            })
    }

    fn select_winner(
        &self,
        eligible: &[ArenaClaimId],
        eligibility: &ArenaEligibilitySnapshot,
    ) -> ArenaClaimId {
        let mut candidates = eligible.to_vec();
        candidates.sort_by(|left, right| {
            let left = self.profile.claim(*left).expect("eligible claim exists");
            let right = self.profile.claim(*right).expect("eligible claim exists");
            claim_rank(self.profile.kind, left, eligibility)
                .cmp(&claim_rank(self.profile.kind, right, eligibility))
                .then_with(|| left.id.cmp(&right.id))
        });
        candidates[0]
    }

    fn expand_holder(
        &self,
        winner: ArenaClaimId,
        eligible: &[ArenaClaimId],
    ) -> (LeaseHolder, Vec<ArenaClaimId>, ArenaDecisionReason) {
        let claim = self.profile.claim(winner).expect("winner exists");
        let Some(compound_id) = claim.compound else {
            return (
                LeaseHolder::Claim(winner),
                vec![winner],
                decision_reason_for(self.profile.kind, claim),
            );
        };
        let compound = self
            .profile
            .compounds
            .get(&compound_id)
            .expect("validated compound");
        match compound.policy {
            CompoundPolicy::Simultaneous => {
                let members = compound
                    .members
                    .iter()
                    .copied()
                    .filter(|member| eligible.contains(member))
                    .collect();
                (
                    LeaseHolder::Compound(compound_id),
                    members,
                    ArenaDecisionReason::SimultaneousCompound,
                )
            }
            CompoundPolicy::Exclusive => {
                let member = compound
                    .members
                    .iter()
                    .copied()
                    .find(|member| eligible.contains(member))
                    .unwrap_or(winner);
                (
                    LeaseHolder::Compound(compound_id),
                    vec![member],
                    ArenaDecisionReason::ExclusiveCompound,
                )
            }
        }
    }

    fn cancel_stream(
        &mut self,
        stream_id: GestureStreamId,
        reason: ArenaCancelReason,
        decision_reason: ArenaDecisionReason,
    ) -> Result<ArenaResolution, ArenaRuntimeError> {
        let stream = self
            .streams
            .get_mut(&stream_id)
            .ok_or(ArenaRuntimeError::UnknownStream(stream_id))?;
        let lease = stream.lease.as_mut().ok_or(ArenaRuntimeError::NoLease)?;
        if matches!(lease.phase, LeasePhase::Ended | LeasePhase::Cancelled) {
            return Err(ArenaRuntimeError::InvalidLeasePhase);
        }
        lease.phase = LeasePhase::Cancelled;
        lease.cancel_reason = Some(reason);
        for member in &lease.active_members {
            stream.phases.insert(*member, ClaimPhase::Cancelled);
        }
        let resolution = ArenaResolution {
            holder: Some(lease.holder),
            active_members: lease.active_members.clone(),
            reason: decision_reason,
        };
        let receipt = ArbitrationReceiptV2 {
            stream_id,
            input_kind: stream.input_kind,
            frozen_source: stream.frozen_source,
            contenders: stream.contenders.clone(),
            eligible_contenders: lease.active_members.clone(),
            holder: Some(lease.holder),
            active_members: lease.active_members.clone(),
            reason: decision_reason,
            terminal: Some(ArenaTerminalOutcome::Cancelled(reason)),
            platform_reason_code: None,
        };
        self.push_receipt(receipt);
        Ok(resolution)
    }

    fn cancel_before_activation(
        &mut self,
        stream_id: GestureStreamId,
        requests: &[ArenaClaimId],
    ) -> Result<ArenaResolution, ArenaRuntimeError> {
        let stream = self
            .streams
            .get_mut(&stream_id)
            .ok_or(ArenaRuntimeError::UnknownStream(stream_id))?;
        for phase in stream.phases.values_mut() {
            if *phase == ClaimPhase::Possible {
                *phase = ClaimPhase::Failed;
            }
        }
        let resolution = ArenaResolution {
            holder: None,
            active_members: Vec::new(),
            reason: ArenaDecisionReason::Cancelled,
        };
        let receipt = ArbitrationReceiptV2 {
            stream_id,
            input_kind: stream.input_kind,
            frozen_source: stream.frozen_source,
            contenders: requests.to_vec(),
            eligible_contenders: Vec::new(),
            holder: None,
            active_members: Vec::new(),
            reason: ArenaDecisionReason::Cancelled,
            terminal: Some(ArenaTerminalOutcome::Cancelled(
                ArenaCancelReason::StructuralInvalidation,
            )),
            platform_reason_code: None,
        };
        self.push_receipt(receipt);
        Ok(resolution)
    }

    fn record_resolution(
        &mut self,
        stream_id: GestureStreamId,
        requests: &[ArenaClaimId],
        eligible: &[ArenaClaimId],
        resolution: &ArenaResolution,
        terminal: Option<ArenaTerminalOutcome>,
        platform_reason_code: Option<u16>,
    ) -> Result<(), ArenaRuntimeError> {
        let stream = self
            .streams
            .get(&stream_id)
            .ok_or(ArenaRuntimeError::UnknownStream(stream_id))?;
        let receipt = ArbitrationReceiptV2 {
            stream_id,
            input_kind: stream.input_kind,
            frozen_source: stream.frozen_source,
            contenders: requests.to_vec(),
            eligible_contenders: eligible.to_vec(),
            holder: resolution.holder,
            active_members: resolution.active_members.clone(),
            reason: resolution.reason,
            terminal,
            platform_reason_code,
        };
        self.push_receipt(receipt);
        Ok(())
    }

    fn push_receipt(&mut self, receipt: ArbitrationReceiptV2) {
        if self.receipt_capacity == 0 {
            return;
        }
        self.receipts.push_back(receipt);
        while self.receipts.len() > self.receipt_capacity {
            self.receipts.pop_front();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequirementReadiness {
    Ready,
    Pending,
    Blocked,
}

fn claim_rank(
    profile: ArenaProfileKind,
    claim: &ArenaClaimDescriptor,
    eligibility: &ArenaEligibilitySnapshot,
) -> (u8, std::cmp::Reverse<u16>, std::cmp::Reverse<i16>, u16) {
    let policy_rank = if claim.kind == ArenaClaimKind::System {
        0
    } else if profile == ArenaProfileKind::IosTouch
        && claim.kind == ArenaClaimKind::RouterHistory
        && eligibility.back_mode.is_claim_capable()
    {
        1
    } else if profile == ArenaProfileKind::MacosTrackpad && claim.kind == ArenaClaimKind::Pager {
        2
    } else {
        3
    };
    (
        policy_rank,
        std::cmp::Reverse(claim.depth),
        std::cmp::Reverse(claim.priority),
        claim.declaration_order,
    )
}

fn decision_reason_for(
    profile: ArenaProfileKind,
    claim: &ArenaClaimDescriptor,
) -> ArenaDecisionReason {
    if claim.kind == ArenaClaimKind::System {
        ArenaDecisionReason::SystemTakeover
    } else if profile == ArenaProfileKind::IosTouch && claim.kind == ArenaClaimKind::RouterHistory {
        ArenaDecisionReason::ReservedEdge
    } else if profile == ArenaProfileKind::MacosTrackpad && claim.kind == ArenaClaimKind::Pager {
        ArenaDecisionReason::MarkedPagerBeforeScroll
    } else {
        ArenaDecisionReason::DeepestConsumer
    }
}

#[cfg(test)]
#[path = "arena_tests.rs"]
mod tests;

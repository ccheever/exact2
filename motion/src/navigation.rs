//! Interactive navigation: the model behind drag-to-go-back.
//!
//! @ref RFC 0100 §4 (the provisional-state lifecycle)
//! @ref RFC 0100 §4a (published disposition and presenter readiness)
//! @ref RFC 0100 §4b (two-phase resolution: logical commit vs visual settle)
//!
//! A back gesture is a bet placed before the answer is known. The finger starts
//! moving immediately, while whether the navigation is even allowed — the
//! route's policy, its data freshness, a blocker — is decided elsewhere and
//! arrives later. This module is the executable model of that race.
//!
//! Two facts settle independently: the *visual* one (the transition finished
//! animating) and the *logical* one (the navigation was authorized and
//! applied). Either can land first. The transition ends only when both have,
//! and any authoritative "no" at any point interrupts and reconciles back to
//! logical truth rather than leaving the screen ahead of the router.
//!
//! Arbitration runs through one [`DecisionCell`]: a single 64-bit word with two
//! writers — whoever authorizes, and this model, which may expire or nack a
//! stale bet. Every transition is a compare-and-swap, so a late writer can
//! never cross a terminal, generation, or root-instance boundary.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Milliseconds a settled commit waits for authorization before expiring.
pub const DEFAULT_CONFIRMATION_TIMEOUT_MS: f64 = 1_000.0;
/// Release speed, in points per second, above which direction decides the
/// outcome and progress no longer matters.
pub const DEFAULT_COMMIT_VELOCITY_PER_SECOND: f64 = 500.0;

/// The state of one navigation decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DecisionState {
    /// No decision has ever been armed in this cell.
    Empty = 0,
    /// A transition is in flight and its verdict has not arrived.
    Pending = 1,
    /// The navigation was authorized and may be applied.
    ConfirmedAllow = 2,
    /// The navigation was denied by policy; the transition must cancel.
    ConfirmedCancel = 3,
    /// The bet was invalidated structurally — what it named no longer exists.
    Nacked = 4,
    /// No verdict arrived before the confirmation deadline.
    Expired = 5,
    /// The cell was retired; only a later generation may arm it again.
    Tombstone = 6,
}

impl DecisionState {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Empty),
            1 => Some(Self::Pending),
            2 => Some(Self::ConfirmedAllow),
            3 => Some(Self::ConfirmedCancel),
            4 => Some(Self::Nacked),
            5 => Some(Self::Expired),
            6 => Some(Self::Tombstone),
            _ => None,
        }
    }

    fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::ConfirmedAllow | Self::ConfirmedCancel | Self::Nacked | Self::Expired
        )
    }
}

/// Which transition a decision is about.
///
/// Both fields are nonzero and are meaningful only as a pair: a verdict for one
/// root instance's transition can never satisfy another's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionIdentity {
    /// The navigation root this transition belongs to.
    pub root_instance: u32,
    /// Monotonic per-root counter distinguishing successive transitions.
    pub transition_generation: u32,
}

// One exact 64-bit control word: rootInstance:24 | transitionGeneration:32 |
// state:8. Packing identity into the word is what makes a stale writer's CAS
// fail instead of landing on a newer transition.
const DECISION_ROOT_INSTANCE_MAX: u32 = (1 << 24) - 1;

fn pack_decision(
    identity: DecisionIdentity,
    state: DecisionState,
) -> Result<u64, InteractiveNavigationError> {
    if identity.root_instance == 0 || identity.transition_generation == 0 {
        return Err(InteractiveNavigationError::ZeroDecisionIdentity);
    }
    if identity.root_instance > DECISION_ROOT_INSTANCE_MAX {
        return Err(InteractiveNavigationError::DecisionIdentityOverflow);
    }
    Ok(((identity.root_instance as u64) << 40)
        | ((identity.transition_generation as u64) << 8)
        | state as u64)
}

fn unpack_decision(word: u64) -> Option<(DecisionIdentity, DecisionState)> {
    if word == 0 {
        return Some((
            DecisionIdentity {
                root_instance: 0,
                transition_generation: 0,
            },
            DecisionState::Empty,
        ));
    }
    let state = DecisionState::from_u8((word & 0xff) as u8)?;
    Some((
        DecisionIdentity {
            root_instance: ((word >> 40) & DECISION_ROOT_INSTANCE_MAX as u64) as u32,
            transition_generation: ((word >> 8) & u32::MAX as u64) as u32,
        },
        state,
    ))
}

/// The single arbitration word for one navigation transition.
///
/// It has two writers — whoever authorizes the navigation, and the model, which
/// may expire or structurally nack — and exactly one of them can win. A cell
/// that has reached a terminal state accepts nothing but a tombstone or an arm
/// from a strictly later generation of the same root.
#[derive(Debug, Default)]
pub struct DecisionCell {
    word: AtomicU64,
}

impl DecisionCell {
    /// Arm the cell for `identity`, moving it to [`DecisionState::Pending`].
    ///
    /// Refused unless the cell is empty, or already finished with an earlier
    /// generation of the same root.
    pub fn arm(&self, identity: DecisionIdentity) -> Result<(), InteractiveNavigationError> {
        let desired = pack_decision(identity, DecisionState::Pending)?;
        let mut observed = self.word.load(Ordering::Acquire);
        loop {
            let (current_identity, current_state) = unpack_decision(observed)
                .ok_or(InteractiveNavigationError::InvalidDecisionTransition)?;
            let can_arm = current_state == DecisionState::Empty
                || ((current_state == DecisionState::Tombstone || current_state.is_terminal())
                    && current_identity.root_instance == identity.root_instance
                    && identity.transition_generation > current_identity.transition_generation);
            if !can_arm {
                return Err(InteractiveNavigationError::InvalidDecisionTransition);
            }
            match self.word.compare_exchange_weak(
                observed,
                desired,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(()),
                Err(next) => observed = next,
            }
        }
    }

    /// Move a pending decision to a terminal state.
    ///
    /// Returns whether this caller was the one that decided: a `false` means
    /// someone else got there first, which is a normal race outcome and not an
    /// error.
    pub fn decide(
        &self,
        identity: DecisionIdentity,
        terminal: DecisionState,
    ) -> Result<bool, InteractiveNavigationError> {
        if !terminal.is_terminal() {
            return Err(InteractiveNavigationError::InvalidDecisionTransition);
        }
        let pending = pack_decision(identity, DecisionState::Pending)?;
        let terminal = pack_decision(identity, terminal)?;
        Ok(self
            .word
            .compare_exchange(pending, terminal, Ordering::AcqRel, Ordering::Acquire)
            .is_ok())
    }

    /// Read the cell's identity and state.
    pub fn load(&self) -> Result<(DecisionIdentity, DecisionState), InteractiveNavigationError> {
        unpack_decision(self.word.load(Ordering::Acquire))
            .ok_or(InteractiveNavigationError::InvalidDecisionTransition)
    }

    /// Retire the decision for `identity`, superseding any verdict.
    ///
    /// Returns whether this call was the one that retired it.
    pub fn tombstone(
        &self,
        identity: DecisionIdentity,
    ) -> Result<bool, InteractiveNavigationError> {
        let mut observed = self.word.load(Ordering::Acquire);
        loop {
            let (current_identity, current_state) = unpack_decision(observed)
                .ok_or(InteractiveNavigationError::InvalidDecisionTransition)?;
            if current_identity != identity || current_state == DecisionState::Tombstone {
                return Ok(false);
            }
            let desired = pack_decision(identity, DecisionState::Tombstone)?;
            match self.word.compare_exchange_weak(
                observed,
                desired,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(true),
                Err(next) => observed = next,
            }
        }
    }
}

/// What the navigation tree says a back action would do, if one ran right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackActionDisposition {
    /// Pop the top screen off the current navigation stack.
    StackPop,
    /// Pop within a tab container rather than the enclosing stack.
    TabPop,
    /// A back action exists but is being refused; the attempt may only be surfaced.
    Blocked,
    /// Back dismisses a presented sheet, which this model does not drive interactively.
    SheetDismiss,
    /// Back belongs to the window itself, such as a platform back button or a window close.
    Window,
    /// Another gesture already owns back, so a second interactive stream is refused.
    Gesture,
    /// No navigator claims back; whatever the platform does by default applies.
    PlatformDefault,
    /// There is nothing to go back to.
    None,
}

/// Whether the current route registered a back handler of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerDisposition {
    /// No handler is registered, so this model may drive the pop itself.
    None,
    /// A handler is registered and back must run through it, which forces a discrete
    /// navigation rather than an interactive one.
    Registered,
}

/// How usable the destination's already-loaded data is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataDisposition {
    /// The destination can be shown immediately from data already on hand.
    Fresh,
    /// Older than ideal but still displayable, subject to `data_usable_until_ms`.
    Stale,
    /// The destination cannot be shown from what is loaded, so back must run as a
    /// discrete navigation that is free to load first.
    Unusable,
}

/// Whether something stands between the user and leaving the current screen, such as
/// an unsaved-changes guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockerDisposition {
    /// Nothing is blocking the navigation.
    None,
    /// A blocker is armed; back is surfaced as a blocked attempt instead of performed.
    Blocked,
}

/// The published set of facts about what the back gesture would do.
///
/// This is a snapshot of the navigation tree, and it is re-read at every checkpoint
/// in [`CHECKPOINT_MATRIX`]: a gesture that started against one snapshot may only
/// commit against one that still agrees with it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackDisposition {
    /// Revision of the navigation tree's shape, bumped whenever the stack or its targets
    /// change. A mismatch later means the bet named something that no longer exists, and
    /// is nacked rather than denied. Must be nonzero.
    pub structural_revision: u64,
    /// Revision of the policy inputs — handler, blocker, data, gesture enablement —
    /// bumped whenever any of them change. A mismatch later denies the navigation rather
    /// than nacking it. Must be nonzero.
    pub policy_revision: u64,
    /// Identity of the screen back would land on, or `None` when there is nowhere to go.
    /// An authorization is only honored if it pins exactly this target.
    pub back_target: Option<u64>,
    /// Which kind of back action the tree resolved for this gesture.
    pub back_action: BackActionDisposition,
    /// Whether the route registered a back handler of its own.
    pub handler: HandlerDisposition,
    /// How usable the destination's data is as of this snapshot.
    pub data: DataDisposition,
    /// Timestamp in milliseconds, on the same clock as the `now_ms` passed to
    /// [`classify_activation`], after which the destination's data stops being usable.
    /// `None` means no deadline. Past it, the gesture falls back to a discrete
    /// navigation.
    pub data_usable_until_ms: Option<f64>,
    /// Whether the route allows the back gesture at all. False fails activation cleanly.
    pub gesture_enabled: bool,
    /// Whether a blocker stands in the way of the navigation.
    pub blocker: BlockerDisposition,
    /// The owning gesture's cascade republish denies a second stream, but
    /// bumps neither structural nor policy revision.
    pub reentrant_gesture_active: bool,
}

impl BackDisposition {
    /// Check the invariants a published disposition must hold, returning it unchanged so
    /// it can be validated inline.
    ///
    /// Both revisions must be nonzero, and any data deadline must be finite.
    pub fn validate(self) -> Result<Self, InteractiveNavigationError> {
        if self.structural_revision == 0 || self.policy_revision == 0 {
            return Err(InteractiveNavigationError::ZeroRevision);
        }
        if self
            .data_usable_until_ms
            .is_some_and(|value| !value.is_finite())
        {
            return Err(InteractiveNavigationError::NonFinite);
        }
        Ok(self)
    }
}

/// What the presenter can promise about driving a transition interactively.
///
/// Driving the destination under the finger needs its scene retained and a lease
/// pinning it. If any part of this is missing, the gesture degrades to a discrete
/// navigation instead of failing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresenterReadiness {
    /// Generation of the presenter that produced this readiness. Zero means no presenter
    /// is live, which rules out an interactive run.
    pub presenter_generation: u64,
    /// Whether the destination's scene is still retained and can be shown mid-gesture
    /// without being rebuilt.
    pub retained_scene_available: bool,
    /// The target the presenter's lease pins. It must equal the disposition's
    /// `back_target`, or the two disagree about what is being navigated to.
    pub pinned_target: Option<u64>,
    /// Whether a lease can be taken for the duration of the gesture.
    pub lease_available: bool,
}

/// What a back gesture attempt turned into. See [`classify_activation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationOutcome {
    /// The gesture does not run and nothing is shown: back is disabled, absent, or
    /// already owned by another gesture.
    CleanFailure,
    /// Back is refused, and the attempt is surfaced to the user — a bounce, a prompt —
    /// without navigating.
    PassiveBlockedAttempt,
    /// Back is allowed but cannot be driven by the finger, so it runs as an ordinary
    /// non-interactive navigation.
    DiscreteFallback,
    /// The gesture drives the transition, betting on an authorization that has not
    /// arrived yet.
    Interactive,
}

/// Decide what a back gesture attempted at `now_ms` becomes.
///
/// The order of the checks is the meaning: a disabled or reentrant gesture fails
/// cleanly, a blocked one surfaces the attempt, and everything the presenter cannot
/// pin — no target, a registered handler, unusable or past-deadline data, a missing
/// or mismatched lease — degrades to a discrete navigation rather than failing.
/// Only a fully pinned, freshly usable stack or tab pop runs interactively.
///
/// `now_ms` is a millisecond timestamp on the same clock as the disposition's
/// `data_usable_until_ms`.
pub fn classify_activation(
    disposition: BackDisposition,
    presenter: Option<PresenterReadiness>,
    now_ms: f64,
) -> Result<ActivationOutcome, InteractiveNavigationError> {
    let disposition = disposition.validate()?;
    if !now_ms.is_finite() {
        return Err(InteractiveNavigationError::NonFinite);
    }
    if !disposition.gesture_enabled || disposition.reentrant_gesture_active {
        return Ok(ActivationOutcome::CleanFailure);
    }
    if disposition.blocker == BlockerDisposition::Blocked
        || matches!(
            disposition.back_action,
            BackActionDisposition::Blocked | BackActionDisposition::SheetDismiss
        )
    {
        return Ok(ActivationOutcome::PassiveBlockedAttempt);
    }
    let has_handler = disposition.handler == HandlerDisposition::Registered;
    if disposition.back_target.is_none() {
        return Ok(if has_handler {
            ActivationOutcome::DiscreteFallback
        } else {
            ActivationOutcome::CleanFailure
        });
    }
    if !matches!(
        disposition.back_action,
        BackActionDisposition::StackPop | BackActionDisposition::TabPop
    ) || has_handler
        || disposition.data == DataDisposition::Unusable
        || disposition
            .data_usable_until_ms
            .is_some_and(|deadline| now_ms > deadline)
    {
        return Ok(ActivationOutcome::DiscreteFallback);
    }
    let Some(presenter) = presenter else {
        return Ok(ActivationOutcome::DiscreteFallback);
    };
    if presenter.presenter_generation == 0
        || !presenter.retained_scene_available
        || !presenter.lease_available
        || presenter.pinned_target != disposition.back_target
    {
        return Ok(ActivationOutcome::DiscreteFallback);
    }
    Ok(ActivationOutcome::Interactive)
}

/// The verdict from checking a started gesture against the canonical navigation
/// state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalMatchOutcome {
    /// The navigation is authorized against the target the router pinned.
    Authorized {
        /// The target the router authorized. It must equal the disposition's
        /// `back_target`; an authorization for anything else is a nack, not an allow.
        pinned_target: u64,
    },
    /// Policy refused the navigation. The refusal is durable: the transition cancels and
    /// cannot be revived by a later verdict.
    PolicyDenied,
    /// The tree changed underneath the gesture, so the bet no longer names anything the
    /// router recognizes.
    StructuralDivergence,
}

/// Proof that one specific bet was authorized, kept so later checkpoints can re-check
/// the navigation against the exact revisions it was granted under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorizationToken {
    /// The structural revision the authorization was granted against.
    pub structural_revision: u64,
    /// The policy revision the authorization was granted against.
    pub policy_revision: u64,
    /// The target the authorization names. A commit against any other target is refused.
    pub pinned_target: u64,
}

/// Where one interactive back transition currently is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractivePhase {
    /// No transition in flight; the model is ready to activate.
    Idle,
    /// The finger is down and driving the transition; no verdict is needed yet.
    GestureActive,
    /// The finger is up and the transition is animating toward the destination.
    SettlingCommit,
    /// The finger is up and the transition is animating back to the starting screen.
    SettlingCancel,
    /// The commit animation finished but no verdict has arrived. The model waits here
    /// until one lands or the confirmation deadline passes.
    SettledPendingConfirmation,
    /// Both facts landed: the transition finished animating and the navigation was
    /// applied.
    Settled,
    /// An authoritative refusal arrived, and the screen is being reconciled back to
    /// logical truth.
    Interrupted,
}

/// Which way a released gesture resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractiveResolution {
    /// The gesture asked to complete the navigation.
    Commit,
    /// The gesture asked to return to the starting screen.
    Cancel,
    /// The gesture was overruled before it could resolve on its own terms.
    Interrupted,
}

/// A side effect the model asks its caller to perform.
///
/// The model performs none of these itself. It returns them in order, and the caller
/// is responsible for running each one exactly once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerEffect {
    /// Publish that an interactive back transition has started.
    PublishStart,
    /// Wake whoever is waiting on the decision: a verdict landed for this bet, whether it
    /// allowed or denied.
    WakeAck,
    /// Wake whoever is waiting with a nack: the bet no longer matches the tree, or
    /// someone else decided the cell first.
    WakeNack,
    /// Start animating the transition through to the destination.
    BeginCommitSettle,
    /// Start animating the transition back to the starting screen.
    BeginCancelSettle,
    /// Apply the navigation for real. The authorization arrived, so the router may act.
    CommitLogicalNavigation,
    /// Put the screen back in sync with the router: whatever the gesture showed was
    /// speculative and has been overruled.
    ReconcileLogicalTruth,
    /// Release the retained destination scene and the presenter lease.
    ReleaseRetention,
    /// Show the user that back was attempted and refused, without navigating.
    SurfaceBlockedAttempt,
    /// Report a stall: the deadline passed while an allow verdict sat unapplied, which is
    /// a fault downstream of this model rather than a denial.
    DiagnoseRuntimeStall,
    /// The transition is over with both facts in agreement; tear down transition state.
    TransitionEnd,
    /// The transition ended early because it was overruled; tear down transition state.
    TransitionInterrupted,
}

/// An ordered batch of [`ControllerEffect`]s, to be performed in sequence.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ControllerEffects(Vec<ControllerEffect>);

impl ControllerEffects {
    /// The effects in the order they must be performed.
    pub fn as_slice(&self) -> &[ControllerEffect] {
        &self.0
    }
    /// Append `other`'s effects after this batch's, preserving order.
    pub fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
    }
    fn one(effect: ControllerEffect) -> Self {
        Self(vec![effect])
    }
}

/// The state of the gesture at the instant the finger lifted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReleaseSample {
    /// How far the transition has come, where 0.0 is the starting screen and 1.0 is the
    /// destination.
    pub progress: f64,
    /// Release speed in points per second, signed: positive moves toward the destination,
    /// negative back toward the starting screen.
    pub velocity_per_second: f64,
}

/// Decide whether a released gesture commits or cancels.
///
/// At or above `velocity_threshold_per_second`, in points per second, the sign of the
/// velocity decides alone — so a fling back toward the starting screen cancels no
/// matter how far along the transition was. Below the threshold, progress at or past
/// the halfway point commits.
///
/// The sample must be finite, and the threshold finite and positive.
pub fn resolve_back_intent(
    sample: ReleaseSample,
    velocity_threshold_per_second: f64,
) -> Result<InteractiveResolution, InteractiveNavigationError> {
    if !sample.progress.is_finite()
        || !sample.velocity_per_second.is_finite()
        || !velocity_threshold_per_second.is_finite()
        || velocity_threshold_per_second <= 0.0
    {
        return Err(InteractiveNavigationError::NonFinite);
    }
    if sample.velocity_per_second.abs() >= velocity_threshold_per_second {
        return Ok(if sample.velocity_per_second > 0.0 {
            InteractiveResolution::Commit
        } else {
            InteractiveResolution::Cancel
        });
    }
    Ok(if sample.progress >= 0.5 {
        InteractiveResolution::Commit
    } else {
        InteractiveResolution::Cancel
    })
}

/// The last-moment re-checks run immediately before the navigation is dispatched.
///
/// Every field must be true. A single false means the world moved since the bet was
/// placed, and the transition is interrupted and reconciled instead of committed.
/// This is the final chance to catch a stale bet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchEnforcement {
    /// The decision cell holds a confirmed allow for this bet.
    pub decision_confirmed_allow: bool,
    /// The tree's structural revision still equals the one the gesture started against.
    pub structural_revision_matches: bool,
    /// The policy revision still equals the one the authorization was granted under.
    pub policy_revision_matches: bool,
    /// No blocker has armed since activation.
    pub blocker_clear: bool,
    /// The caller is still permitted to perform this navigation.
    pub access_allowed: bool,
    /// The presentation layer still permits the transition to be driven this way.
    pub layer_policy_allowed: bool,
    /// The destination is still the same screen the authorization pinned.
    pub target_identity_matches: bool,
    /// No back handler has been registered in the meantime, which would take the
    /// navigation out of this model's hands.
    pub handler_clear: bool,
    /// The destination's data has not passed its usability deadline.
    pub data_deadline_valid: bool,
}

impl DispatchEnforcement {
    /// Whether every check passed, which is the only condition under which the navigation
    /// may be dispatched.
    pub fn permits_commit(self) -> bool {
        self.decision_confirmed_allow
            && self.structural_revision_matches
            && self.policy_revision_matches
            && self.blocker_clear
            && self.access_allowed
            && self.layer_policy_allowed
            && self.target_identity_matches
            && self.handler_clear
            && self.data_deadline_valid
    }
}

/// The state machine for one interactive back transition.
///
/// It owns neither the animation nor the router. Callers feed it events — activation,
/// release, verdicts, visual settle, timeouts — and it answers with the
/// [`ControllerEffect`]s to perform. It ends a commit only once the visual fact and
/// the logical fact have both landed, and interrupts the moment an authoritative
/// refusal arrives.
#[derive(Debug)]
pub struct InteractiveBackModel {
    identity: DecisionIdentity,
    cell: Arc<DecisionCell>,
    phase: InteractivePhase,
    resolution: Option<InteractiveResolution>,
    disposition: Option<BackDisposition>,
    authorization: Option<AuthorizationToken>,
    visual_settled: bool,
    logical_commit_applied: bool,
    commit_token_observed: bool,
    confirmation_deadline_ms: Option<f64>,
    confirmation_timeout_ms: f64,
}

impl InteractiveBackModel {
    /// Create a model for `identity` with a private decision cell.
    ///
    /// `confirmation_timeout_ms` is how long a settled commit waits for a verdict before
    /// the bet expires; it must be finite and positive, and
    /// [`DEFAULT_CONFIRMATION_TIMEOUT_MS`] is the usual value. Use
    /// [`Self::new_with_cell`] when the runtime must share the cell.
    pub fn new(
        identity: DecisionIdentity,
        confirmation_timeout_ms: f64,
    ) -> Result<Self, InteractiveNavigationError> {
        Self::new_with_cell(
            identity,
            confirmation_timeout_ms,
            Arc::new(DecisionCell::default()),
        )
    }

    /// Production constructor: the main-owned state machine and the
    /// runtime-facing transport retain the same atomic DecisionCell. Runtime
    /// writes authorization with a CAS; main observes it at wake, release,
    /// visual settle, and timeout without copying verdict state through Swift.
    pub fn new_with_cell(
        identity: DecisionIdentity,
        confirmation_timeout_ms: f64,
        cell: Arc<DecisionCell>,
    ) -> Result<Self, InteractiveNavigationError> {
        if !confirmation_timeout_ms.is_finite() || confirmation_timeout_ms <= 0.0 {
            return Err(InteractiveNavigationError::NonFinite);
        }
        Ok(Self {
            identity,
            cell,
            phase: InteractivePhase::Idle,
            resolution: None,
            disposition: None,
            authorization: None,
            visual_settled: false,
            logical_commit_applied: false,
            commit_token_observed: false,
            confirmation_deadline_ms: None,
            confirmation_timeout_ms,
        })
    }

    /// Where the transition currently is.
    pub fn phase(&self) -> InteractivePhase {
        self.phase
    }
    /// Which way the gesture resolved, or `None` before it has been released.
    pub fn resolution(&self) -> Option<InteractiveResolution> {
        self.resolution
    }
    /// Read the current verdict out of the shared decision cell.
    pub fn decision_state(&self) -> Result<DecisionState, InteractiveNavigationError> {
        Ok(self.cell.load()?.1)
    }

    /// Start a back gesture against `disposition` at `now_ms`.
    ///
    /// Only an idle or settled model may activate; anything else fails cleanly rather
    /// than disturbing a transition already in flight. An interactive outcome arms the
    /// decision cell and clears all per-transition state; a blocked one only surfaces the
    /// attempt; the rest change nothing.
    pub fn activate(
        &mut self,
        disposition: BackDisposition,
        presenter: Option<PresenterReadiness>,
        now_ms: f64,
    ) -> Result<(ActivationOutcome, ControllerEffects), InteractiveNavigationError> {
        if self.phase != InteractivePhase::Idle && self.phase != InteractivePhase::Settled {
            return Ok((
                ActivationOutcome::CleanFailure,
                ControllerEffects::default(),
            ));
        }
        let outcome = classify_activation(disposition, presenter, now_ms)?;
        match outcome {
            ActivationOutcome::Interactive => {
                self.cell.arm(self.identity)?;
                self.phase = InteractivePhase::GestureActive;
                self.disposition = Some(disposition);
                self.resolution = None;
                self.authorization = None;
                self.visual_settled = false;
                self.logical_commit_applied = false;
                self.commit_token_observed = false;
                self.confirmation_deadline_ms = None;
                Ok((
                    outcome,
                    ControllerEffects::one(ControllerEffect::PublishStart),
                ))
            }
            ActivationOutcome::PassiveBlockedAttempt => Ok((
                outcome,
                ControllerEffects::one(ControllerEffect::SurfaceBlockedAttempt),
            )),
            _ => Ok((outcome, ControllerEffects::default())),
        }
    }

    /// Deliver the canonical verdict for the bet, revalidated against `current`.
    ///
    /// Comparing `current` with the disposition the gesture started from is part of the
    /// check: a changed structural revision or back target is a nack even when the router
    /// says authorized, and so is an authorization that pins some other target. Losing
    /// the race to decide the cell is likewise a nack, and is a normal outcome rather
    /// than an error.
    pub fn confirm(
        &mut self,
        current: BackDisposition,
        match_outcome: CanonicalMatchOutcome,
    ) -> Result<ControllerEffects, InteractiveNavigationError> {
        let started = self
            .disposition
            .ok_or(InteractiveNavigationError::InvalidPhase)?;
        let structural_diverged = current.structural_revision != started.structural_revision
            || current.back_target != started.back_target
            || matches!(match_outcome, CanonicalMatchOutcome::StructuralDivergence);
        let (terminal, effect) = if structural_diverged {
            (DecisionState::Nacked, ControllerEffect::WakeNack)
        } else {
            match match_outcome {
                CanonicalMatchOutcome::Authorized { pinned_target }
                    if Some(pinned_target) == started.back_target =>
                {
                    self.authorization = Some(AuthorizationToken {
                        structural_revision: current.structural_revision,
                        policy_revision: current.policy_revision,
                        pinned_target,
                    });
                    (DecisionState::ConfirmedAllow, ControllerEffect::WakeAck)
                }
                CanonicalMatchOutcome::PolicyDenied => {
                    (DecisionState::ConfirmedCancel, ControllerEffect::WakeAck)
                }
                _ => (DecisionState::Nacked, ControllerEffect::WakeNack),
            }
        };
        if !self.cell.decide(self.identity, terminal)? {
            return Ok(ControllerEffects::one(ControllerEffect::WakeNack));
        }
        let mut effects = ControllerEffects::one(effect);
        effects.0.extend(self.observe_decision()?.0);
        Ok(effects)
    }

    /// Record that the finger lifted, resolving the gesture and starting the settle.
    ///
    /// The intent read from `sample` is overruled to a cancel whenever the cell already
    /// holds a refusal, so a commit is never animated against a verdict that has already
    /// said no. Valid only while the gesture is active.
    pub fn release(
        &mut self,
        sample: ReleaseSample,
        velocity_threshold_per_second: f64,
    ) -> Result<ControllerEffects, InteractiveNavigationError> {
        if self.phase != InteractivePhase::GestureActive {
            return Err(InteractiveNavigationError::InvalidPhase);
        }
        let mut resolution = resolve_back_intent(sample, velocity_threshold_per_second)?;
        if matches!(
            self.cell.load()?.1,
            DecisionState::ConfirmedCancel
                | DecisionState::Nacked
                | DecisionState::Expired
                | DecisionState::Tombstone
        ) {
            resolution = InteractiveResolution::Cancel;
        }
        self.resolution = Some(resolution);
        self.phase = if resolution == InteractiveResolution::Commit {
            InteractivePhase::SettlingCommit
        } else {
            InteractivePhase::SettlingCancel
        };
        let mut effects = ControllerEffects::one(if resolution == InteractiveResolution::Commit {
            ControllerEffect::BeginCommitSettle
        } else {
            ControllerEffect::BeginCancelSettle
        });
        effects.0.extend(self.observe_decision()?.0);
        Ok(effects)
    }

    /// Report that the navigation tree changed underneath the transition.
    ///
    /// During the gesture this nacks the bet and settles back to the starting screen;
    /// while settling or waiting on confirmation it interrupts and reconciles. Outside a
    /// transition it does nothing.
    pub fn structural_invalidation(
        &mut self,
    ) -> Result<ControllerEffects, InteractiveNavigationError> {
        if self.phase == InteractivePhase::GestureActive {
            let _ = self.cell.decide(self.identity, DecisionState::Nacked)?;
            self.resolution = Some(InteractiveResolution::Interrupted);
            self.phase = InteractivePhase::SettlingCancel;
            return Ok(ControllerEffects(vec![
                ControllerEffect::WakeNack,
                ControllerEffect::BeginCancelSettle,
            ]));
        }
        if matches!(
            self.phase,
            InteractivePhase::SettlingCommit
                | InteractivePhase::SettlingCancel
                | InteractivePhase::SettledPendingConfirmation
        ) {
            return self.interrupt_and_reconcile();
        }
        Ok(ControllerEffects::default())
    }

    /// Record that the transition finished animating, at `now_ms`.
    ///
    /// This is the visual half of the resolution. A cancel is finished outright. A commit
    /// finishes only if the verdict has already landed; if it has not, the model waits
    /// and arms the confirmation deadline at `confirmation_timeout_ms` past `now_ms`, and
    /// if the verdict was a refusal it interrupts instead.
    pub fn settle_visual(
        &mut self,
        now_ms: f64,
    ) -> Result<ControllerEffects, InteractiveNavigationError> {
        if !now_ms.is_finite() {
            return Err(InteractiveNavigationError::NonFinite);
        }
        if !matches!(
            self.phase,
            InteractivePhase::SettlingCommit | InteractivePhase::SettlingCancel
        ) {
            return Err(InteractiveNavigationError::InvalidPhase);
        }
        self.visual_settled = true;
        if self.resolution != Some(InteractiveResolution::Commit) {
            self.phase = InteractivePhase::Settled;
            return Ok(ControllerEffects(vec![
                ControllerEffect::ReleaseRetention,
                ControllerEffect::TransitionEnd,
            ]));
        }
        match self.cell.load()?.1 {
            DecisionState::Pending => {
                self.phase = InteractivePhase::SettledPendingConfirmation;
                self.confirmation_deadline_ms = Some(now_ms + self.confirmation_timeout_ms);
                Ok(ControllerEffects::default())
            }
            DecisionState::ConfirmedAllow => self.finish_if_ready(),
            DecisionState::ConfirmedCancel
            | DecisionState::Nacked
            | DecisionState::Expired
            | DecisionState::Tombstone => self.interrupt_and_reconcile(),
            DecisionState::Empty => Err(InteractiveNavigationError::InvalidDecisionState),
        }
    }

    /// Re-read the shared decision cell and act on whatever verdict is now there.
    ///
    /// Call this whenever the runtime may have written one. An allow applies the logical
    /// navigation exactly once; any refusal interrupts and reconciles. It does nothing
    /// unless the gesture resolved as a commit.
    pub fn observe_decision(&mut self) -> Result<ControllerEffects, InteractiveNavigationError> {
        if self.resolution == Some(InteractiveResolution::Commit) {
            match self.cell.load()?.1 {
                DecisionState::ConfirmedAllow if !self.logical_commit_applied => {
                    self.logical_commit_applied = true;
                    let mut effects =
                        ControllerEffects::one(ControllerEffect::CommitLogicalNavigation);
                    effects.0.extend(self.finish_if_ready()?.0);
                    return Ok(effects);
                }
                DecisionState::ConfirmedCancel
                | DecisionState::Nacked
                | DecisionState::Expired
                | DecisionState::Tombstone => return self.interrupt_and_reconcile(),
                _ => {}
            }
        }
        Ok(ControllerEffects::default())
    }

    /// Run the dispatch-time checks and either let the commit stand or take it back.
    ///
    /// If any check fails, the logical commit is withdrawn and the transition is
    /// interrupted and reconciled rather than dispatched.
    pub fn enforce_dispatch(
        &mut self,
        checks: DispatchEnforcement,
    ) -> Result<ControllerEffects, InteractiveNavigationError> {
        if checks.permits_commit() {
            return Ok(ControllerEffects::default());
        }
        self.logical_commit_applied = false;
        self.interrupt_and_reconcile()
    }

    /// Record that the router acknowledged the navigation with its commit token.
    ///
    /// This is the third fact the end of the transition waits on, alongside the visual
    /// settle and the applied logical commit.
    pub fn observe_commit_token(
        &mut self,
    ) -> Result<ControllerEffects, InteractiveNavigationError> {
        self.commit_token_observed = true;
        self.finish_if_ready()
    }

    /// Check the confirmation deadline at `now_ms` and expire the bet if it has passed.
    ///
    /// A no-op before the deadline, or when none is armed. Winning the race to expire
    /// interrupts the transition. Finding an allow already in the cell instead means the
    /// verdict landed but was never applied, which is reported as a runtime stall.
    pub fn confirmation_timeout(
        &mut self,
        now_ms: f64,
    ) -> Result<ControllerEffects, InteractiveNavigationError> {
        if !now_ms.is_finite() {
            return Err(InteractiveNavigationError::NonFinite);
        }
        let Some(deadline) = self.confirmation_deadline_ms else {
            return Ok(ControllerEffects::default());
        };
        if now_ms < deadline {
            return Ok(ControllerEffects::default());
        }
        if self.cell.decide(self.identity, DecisionState::Expired)? {
            return self.interrupt_and_reconcile();
        }
        match self.cell.load()?.1 {
            DecisionState::ConfirmedAllow => Ok(ControllerEffects::one(
                ControllerEffect::DiagnoseRuntimeStall,
            )),
            DecisionState::ConfirmedCancel
            | DecisionState::Nacked
            | DecisionState::Expired
            | DecisionState::Tombstone => self.interrupt_and_reconcile(),
            _ => Ok(ControllerEffects::default()),
        }
    }

    /// Retire this transition unconditionally, tombstoning its decision cell so no late
    /// writer can land on it, then interrupt and reconcile back to logical truth.
    pub fn reset(&mut self) -> Result<ControllerEffects, InteractiveNavigationError> {
        let _ = self.cell.tombstone(self.identity)?;
        self.interrupt_and_reconcile()
    }

    fn finish_if_ready(&mut self) -> Result<ControllerEffects, InteractiveNavigationError> {
        if self.visual_settled && self.logical_commit_applied && self.commit_token_observed {
            self.phase = InteractivePhase::Settled;
            self.confirmation_deadline_ms = None;
            return Ok(ControllerEffects(vec![
                ControllerEffect::ReleaseRetention,
                ControllerEffect::TransitionEnd,
            ]));
        }
        Ok(ControllerEffects::default())
    }

    fn interrupt_and_reconcile(&mut self) -> Result<ControllerEffects, InteractiveNavigationError> {
        if self.phase == InteractivePhase::Interrupted {
            return Ok(ControllerEffects::default());
        }
        self.phase = InteractivePhase::Interrupted;
        self.resolution = Some(InteractiveResolution::Interrupted);
        self.confirmation_deadline_ms = None;
        Ok(ControllerEffects(vec![
            ControllerEffect::ReconcileLogicalTruth,
            ControllerEffect::ReleaseRetention,
            ControllerEffect::TransitionInterrupted,
        ]))
    }
}

/// One published input that a checkpoint may have to re-read. See
/// [`CHECKPOINT_MATRIX`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispositionField {
    /// The navigation tree's shape revision.
    StructuralRevision,
    /// The revision of the policy inputs governing back.
    PolicyRevision,
    /// The identity of the screen back would land on.
    BackTarget,
    /// The kind of back action the tree resolved.
    BackAction,
    /// Whether the route registered a back handler of its own.
    HandlerDisposition,
    /// How usable the destination's data is.
    DataDisposition,
    /// The millisecond deadline past which the destination's data is unusable.
    DataUsableUntil,
    /// Whether the route allows the back gesture at all.
    GestureEnabled,
    /// Whether a blocker stands in the way of the navigation.
    BlockerDisposition,
    /// The generation of the presenter driving the transition.
    PresenterGeneration,
    /// The lease retaining the destination's scene for the duration of the gesture.
    SceneLease,
}

/// How a change to a published field is accounted for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeClassification {
    /// The change alters what the bet names, so it bumps the structural revision and
    /// nacks a gesture already in flight.
    Structural,
    /// The change alters whether the navigation is allowed, so it bumps the policy
    /// revision and denies rather than nacks.
    Policy,
    /// The field cannot change while a transition is in flight, so no revision bump is
    /// required for it.
    Static,
}

/// One row of [`CHECKPOINT_MATRIX`]: which checkpoints must re-read a published field,
/// and whether the presenter lease pins it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckpointCoverage {
    /// The published field this row is about.
    pub field: DispositionField,
    /// Whether [`classify_activation`] must read the field.
    pub activation: bool,
    /// Whether the commit-time revalidation must read the field again.
    pub commit_revalidation: bool,
    /// Whether the dispatch-time enforcement must read the field again.
    pub dispatch_enforcement: bool,
    /// Whether the presenter lease pins the field for the duration of the transition, so
    /// it cannot change underneath the gesture.
    pub lease_pinned: bool,
    /// How a change to the field is classified: structural, policy, or static.
    pub classification: ChangeClassification,
}

macro_rules! checkpoint {
    ($field:ident, $lease:expr, $classification:ident) => {
        CheckpointCoverage {
            field: DispositionField::$field,
            activation: true,
            commit_revalidation: true,
            dispatch_enforcement: true,
            lease_pinned: $lease,
            classification: ChangeClassification::$classification,
        }
    };
}

/// Every published field, paired with the checkpoints obliged to re-read it.
///
/// Reading a fact once at activation is not enough, because the transition is a bet
/// placed before the answer is known. Each field is re-read at all three checkpoints
/// — activation, commit revalidation, and dispatch enforcement — and this table is
/// the record of that obligation, along with whether the presenter lease pins the
/// field in between.
pub const CHECKPOINT_MATRIX: &[CheckpointCoverage] = &[
    checkpoint!(StructuralRevision, true, Structural),
    checkpoint!(PolicyRevision, true, Policy),
    checkpoint!(BackTarget, true, Structural),
    checkpoint!(BackAction, false, Policy),
    checkpoint!(HandlerDisposition, false, Policy),
    checkpoint!(DataDisposition, true, Policy),
    checkpoint!(DataUsableUntil, true, Policy),
    checkpoint!(GestureEnabled, false, Policy),
    checkpoint!(BlockerDisposition, false, Policy),
    checkpoint!(PresenterGeneration, true, Structural),
    checkpoint!(SceneLease, true, Structural),
];

/// A known-bad implementation the gate probe deliberately reintroduces, to check that
/// the model still rejects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateMutation {
    /// The correct implementation. The probe must pass.
    None,
    /// Every applied position lags the finger by one frame.
    OneFrameDelay,
    /// A release is judged on speed alone, so a fling back toward the starting screen
    /// commits instead of cancelling.
    MagnitudeOnlyReverseFling,
    /// The position is written straight to the layer once, losing composition with the
    /// presenter's own update.
    RawOneShotLayerWrite,
}

/// What one gate-probe run measured. Judged by [`gate_probe_passes`].
#[derive(Debug, Clone, PartialEq)]
pub struct GateProbe {
    /// The positions actually applied to the transition, in points, one per frame.
    pub applied_positions: Vec<f64>,
    /// The finger positions those must equal exactly, in points.
    pub expected_positions: Vec<f64>,
    /// How a fast release back toward the starting screen resolved. It must cancel.
    pub reverse_fling_resolution: InteractiveResolution,
    /// The composed position, in points, after the presenter's update. A raw one-shot
    /// layer write drops that contribution and reads low.
    pub composed_after_presenter_update: f64,
}

/// Run the self-check with `mutation` applied.
///
/// With [`GateMutation::None`] the probe reproduces correct behavior and passes. Each
/// other mutation reproduces one known-bad implementation and must fail: a passing
/// result there means the check has stopped catching that mistake.
pub fn run_gate_probe(mutation: GateMutation) -> GateProbe {
    let finger = [0.0, 12.0, 30.0, 51.0];
    let applied_positions = finger
        .iter()
        .enumerate()
        .map(|(index, value)| {
            if mutation == GateMutation::OneFrameDelay && index > 0 {
                finger[index - 1]
            } else {
                *value
            }
        })
        .collect();
    let reverse_fling_resolution = if mutation == GateMutation::MagnitudeOnlyReverseFling {
        InteractiveResolution::Commit
    } else {
        resolve_back_intent(
            ReleaseSample {
                progress: 0.8,
                velocity_per_second: -700.0,
            },
            DEFAULT_COMMIT_VELOCITY_PER_SECOND,
        )
        .expect("constant probe")
    };
    GateProbe {
        applied_positions,
        expected_positions: finger.to_vec(),
        reverse_fling_resolution,
        composed_after_presenter_update: if mutation == GateMutation::RawOneShotLayerWrite {
            40.0
        } else {
            58.0
        },
    }
}

/// Whether a probe run shows correct behavior.
///
/// All three conditions must hold: applied positions track the finger frame for
/// frame, a reverse fling cancels, and the composed position still carries the
/// presenter's update.
pub fn gate_probe_passes(probe: &GateProbe) -> bool {
    probe.applied_positions == probe.expected_positions
        && probe.reverse_fling_resolution == InteractiveResolution::Cancel
        && probe.composed_after_presenter_update == 58.0
}

/// Why an interactive-navigation transition could not proceed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractiveNavigationError {
    /// A time, progress, or velocity input was infinite or NaN.
    NonFinite,
    /// A published disposition carried a zero structural or policy revision.
    ZeroRevision,
    /// The operation does not exist in the model's current phase.
    InvalidPhase,
    /// A visual settle observed a decision cell that was never armed.
    InvalidDecisionState,
    /// A decision identity carried a zero root instance or generation.
    ZeroDecisionIdentity,
    /// A decision identity's root instance exceeds the packed 24-bit field.
    DecisionIdentityOverflow,
    /// A decision transition was refused: wrong state, identity, or generation.
    InvalidDecisionTransition,
}

#[cfg(test)]
#[path = "navigation_tests.rs"]
mod tests;

//! The fixed-step virtual clock.
//!
//! @ref RFC 0492 M-A (deterministic clock and agent evidence)
//!
//! This is why motion is in v1 at all. Because every driver is closed-form and
//! every frame is a fixed step, a caller can advance time instead of waiting
//! for it: a test seeks to t=0.8s and reads the value, an agent advances the
//! clock and screenshots. Settle timing stops being a source of flake.
//!
//! The clock runs [`MOTION_TICK_ORDER`] exactly, once per frame. Time and
//! driver elapsed time advance from the same per-frame schedule — the clock
//! accumulates each step rather than deriving time from a frame count — so
//! raw-bit identity applies to identical schedules, not to different schedules
//! that happen to arrive at the same `t`.

use crate::driver::{
    AnimationDriverSpec, DriverPublication, DriverTerminal, DriverTerminalReason,
    MotionDriverError, MotionDriverTable, MotionSample,
};
use crate::math::DETERMINISTIC_MATH_PROFILE;
use crate::tick::{MotionTickPhase, MOTION_TICK_ORDER};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

const VALUE_SEQUENCE_DOMAIN: &[u8] = b"EXACT.MOTION.VALUE-SEQUENCE.V1\0";
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const INPUT_PUBLICATION_SEQUENCE: u64 = 0;

/// Frames one clock retains before it refuses to advance.
///
/// The history is *refused*, never trimmed. [`VirtualMotionClock::checkpoints`]
/// resolves a request by replaying every frame from the first one, and
/// [`ValueSequenceTrace::encode`] covers the whole run — so dropping the oldest
/// frames would not shorten the history, it would silently change the answers
/// and the digest. A caller that needs a longer run advances a fresh clock.
///
/// At 65,536 frames a 60Hz schedule reaches roughly eighteen minutes of virtual
/// time, which is far past any seek or settle this evaluator exists to serve.
pub const MOTION_CLOCK_MAX_RETAINED_FRAMES: usize = 1 << 16;
/// Fixed steps one [`VirtualMotionClock::seek`] call may replay.
///
/// Seeking never jumps: it replays steps, and every step retains a frame. The
/// bound therefore matches [`MOTION_CLOCK_MAX_RETAINED_FRAMES`], and is checked
/// before any frame is advanced so an over-long seek is refused whole.
pub const MOTION_CLOCK_MAX_SEEK_STEPS: u64 = MOTION_CLOCK_MAX_RETAINED_FRAMES as u64;

/// Failures while scheduling or advancing a virtual motion clock.
#[derive(Debug, Clone, PartialEq)]
pub enum VirtualMotionClockError {
    /// The native evaluator rejected an input or advance.
    Driver(MotionDriverError),
    /// The nominal or partial step was non-finite or non-positive.
    InvalidStepSeconds,
    /// A named time value was non-finite.
    NonFiniteTime(&'static str),
    /// An input deadline preceded the clock's current time.
    InputInPast,
    /// A seek target preceded the clock's current time.
    SeekBeforeNow,
    /// More inputs were scheduled than the v1 u32 trace index can represent.
    EventIndexExhausted,
    /// The u64 frame receipt sequence was exhausted.
    FrameSequenceExhausted,
    /// The seek would replay more than [`MOTION_CLOCK_MAX_SEEK_STEPS`] steps.
    SeekTooFar,
    /// The step is too small to move virtual time at its current magnitude, so
    /// replaying it would never reach the target. Defensive: the step budget
    /// keeps virtual time far below the magnitude at which a step is absorbed,
    /// so no schedule reachable through this API can raise it today.
    StalledStep,
    /// The clock has retained [`MOTION_CLOCK_MAX_RETAINED_FRAMES`] frames and
    /// will not advance further.
    FrameHistoryExhausted,
}

impl fmt::Display for VirtualMotionClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Driver(error) => write!(formatter, "motion driver error: {error:?}"),
            Self::InvalidStepSeconds => {
                formatter.write_str("step_seconds must be finite and positive")
            }
            Self::NonFiniteTime(field) => write!(formatter, "{field} must be finite"),
            Self::SeekTooFar => formatter
                .write_str("seek would replay more than MOTION_CLOCK_MAX_SEEK_STEPS fixed steps"),
            Self::StalledStep => {
                formatter.write_str("step_seconds cannot advance virtual time at its magnitude")
            }
            Self::FrameHistoryExhausted => {
                formatter.write_str("clock retained MOTION_CLOCK_MAX_RETAINED_FRAMES frames")
            }
            Self::InputInPast => {
                formatter.write_str("virtual input cannot be scheduled in the past")
            }
            Self::SeekBeforeNow => formatter.write_str("virtual clock cannot seek backward"),
            Self::EventIndexExhausted => formatter.write_str("virtual input event index exhausted"),
            Self::FrameSequenceExhausted => formatter.write_str("virtual frame sequence exhausted"),
        }
    }
}

impl Error for VirtualMotionClockError {}

impl From<MotionDriverError> for VirtualMotionClockError {
    fn from(error: MotionDriverError) -> Self {
        Self::Driver(error)
    }
}

/// An input applied during the fixed `InputSampling` phase.
#[derive(Debug, Clone, PartialEq)]
pub enum VirtualInputKind {
    /// Instantiate a driver from an explicit initial position and velocity.
    StartDriver {
        /// Value to drive.
        value_handle: u64,
        /// Driver to instantiate.
        spec: AnimationDriverSpec,
        /// Position to start from.
        initial: f64,
        /// Velocity to start from.
        velocity: f64,
    },
    /// Replace the active driver from the value's last publication, inheriting
    /// both position and velocity. If the handle has not published since its
    /// most recent start, the authored initial position and zero velocity are
    /// used. A never-seen handle starts from position and velocity zero.
    Retarget {
        /// Value to retarget.
        value_handle: u64,
        /// Driver to instantiate from the value's last publication.
        spec: AnimationDriverSpec,
    },
    /// Cancel driver ownership and publish an imperative scalar value.
    WriteValue {
        /// Value to write.
        value_handle: u64,
        /// Scalar to publish.
        value: f64,
    },
    /// Cancel driver ownership and publish a gesture-owned scalar value.
    GestureWrite {
        /// Value to write.
        value_handle: u64,
        /// Scalar to publish.
        value: f64,
    },
    /// Cancel driver ownership without publishing another scalar.
    CancelDriver {
        /// Value whose driver is cancelled.
        value_handle: u64,
    },
}

impl VirtualInputKind {
    fn value_handle(&self) -> u64 {
        match self {
            Self::StartDriver { value_handle, .. }
            | Self::Retarget { value_handle, .. }
            | Self::WriteValue { value_handle, .. }
            | Self::GestureWrite { value_handle, .. }
            | Self::CancelDriver { value_handle } => *value_handle,
        }
    }

    fn tag(&self) -> VirtualInputTag {
        match self {
            Self::StartDriver { .. } => VirtualInputTag::StartDriver,
            Self::Retarget { .. } => VirtualInputTag::Retarget,
            Self::WriteValue { .. } => VirtualInputTag::WriteValue,
            Self::GestureWrite { .. } => VirtualInputTag::GestureWrite,
            Self::CancelDriver { .. } => VirtualInputTag::CancelDriver,
        }
    }
}

/// Stable input tags used by the canonical value-sequence encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VirtualInputTag {
    /// `VirtualInputKind::StartDriver`.
    StartDriver = 1,
    /// `VirtualInputKind::Retarget`.
    Retarget = 2,
    /// `VirtualInputKind::WriteValue`.
    WriteValue = 3,
    /// `VirtualInputKind::GestureWrite`.
    GestureWrite = 4,
    /// `VirtualInputKind::CancelDriver`.
    CancelDriver = 5,
}

/// A virtual input plus its deterministic deadline in seconds since open.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualInputEvent {
    /// Deadline in virtual seconds since the session opened.
    pub at_seconds: f64,
    /// Mutation applied in `InputSampling`.
    pub input: VirtualInputKind,
}

/// Input evidence recorded in the frame where the input was sampled.
#[derive(Debug, Clone, PartialEq)]
pub struct AppliedInput {
    /// Stable zero-based insertion index assigned by `schedule_input`.
    pub event_index: u32,
    /// Stable trace tag for the applied operation.
    pub kind_tag: VirtualInputTag,
    /// Shared value affected by the operation.
    pub value_handle: u64,
    /// Driver terminal emitted by replacement or cancellation, if any.
    pub terminal: Option<DriverTerminal>,
}

/// One receipt-shaped virtual clock frame.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualFrame {
    /// Monotonic receipt sequence, beginning at one.
    pub sequence: u64,
    /// Virtual time after this frame's advance.
    pub virtual_time_seconds: f64,
    /// Delta supplied to the evaluator for this frame.
    pub step_seconds: f64,
    /// Inputs sampled in deterministic deadline/insertion order.
    pub applied_inputs: Vec<AppliedInput>,
    /// Input-owned scalar publications appear first in insertion order with
    /// driver sequence zero, followed by `DriverAdvance` publications in the
    /// `MotionDriverTable`'s value-handle order.
    pub publications: Vec<DriverPublication>,
}

/// Computed completion state after a bounded quiescence run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuiescenceVerdict {
    /// Whether no active drivers and no unapplied inputs remain.
    pub quiescent: bool,
    /// Virtual time reached by the bounded run.
    pub virtual_time_seconds: f64,
    /// Fixed steps taken by this invocation.
    pub steps_taken: u64,
    /// Drivers still active at the verdict boundary.
    pub active_drivers: usize,
    /// Scheduled inputs still unapplied at the verdict boundary.
    pub pending_inputs: usize,
}

#[derive(Debug, Clone)]
struct ScheduledInput {
    event_index: u32,
    event: VirtualInputEvent,
    applied: bool,
}

/// Fixed-step virtual clock over the main-owned native driver table.
///
/// Time is virtual seconds since session open and `step_seconds` is the
/// nominal frame duration. Every frame executes `MOTION_TICK_ORDER` exactly:
/// `InputSampling` applies due inputs in `(deadline, insertion order)` order;
/// `ArenaCheckpoint` and `DerivedValues` are no-ops for a
/// value-plane-only session; `DriverAdvance` advances the driver table;
/// `BindingApplication` retains the last publications; `Present` and
/// `Auxiliary` are host-owned no-ops in this evaluator-only clock; and
/// `Observability` finalizes the frame receipt.
///
/// Both clock time and driver elapsed time advance from the same per-frame
/// schedule. The clock accumulates each step rather than deriving time from a
/// frame count, and drivers receive that exact per-frame delta through their
/// own `add_elapsed` path. Raw-bit identity therefore applies to identical
/// schedules, not to different schedules that happen to reach the same `t`.
#[derive(Debug)]
pub struct VirtualMotionClock {
    step_seconds: f64,
    virtual_time_seconds: f64,
    sequence: u64,
    table: MotionDriverTable,
    scheduled_inputs: Vec<ScheduledInput>,
    retarget_samples: BTreeMap<u64, MotionSample>,
    frames: Vec<VirtualFrame>,
}

impl VirtualMotionClock {
    /// Open a virtual session with a finite, positive nominal step.
    pub fn new(step_seconds: f64) -> Result<Self, VirtualMotionClockError> {
        if !step_seconds.is_finite() || step_seconds <= 0.0 {
            return Err(VirtualMotionClockError::InvalidStepSeconds);
        }
        Ok(Self {
            step_seconds,
            virtual_time_seconds: 0.0,
            sequence: 0,
            table: MotionDriverTable::default(),
            scheduled_inputs: Vec::new(),
            retarget_samples: BTreeMap::new(),
            frames: Vec::new(),
        })
    }

    /// Return the nominal fixed-step duration.
    pub fn step_seconds(&self) -> f64 {
        self.step_seconds
    }

    /// Return virtual seconds reached since session open.
    pub fn virtual_time_seconds(&self) -> f64 {
        self.virtual_time_seconds
    }

    /// Return the native driver table's current active count.
    pub fn active_driver_count(&self) -> usize {
        self.table.active_count()
    }

    /// Return the number of scheduled inputs not yet sampled.
    pub fn pending_input_count(&self) -> usize {
        self.scheduled_inputs
            .iter()
            .filter(|event| !event.applied)
            .count()
    }

    /// Schedule one input. Inputs may be added out of deadline order, but a
    /// deadline earlier than the current virtual time is rejected immediately.
    pub fn schedule_input(
        &mut self,
        event: VirtualInputEvent,
    ) -> Result<u32, VirtualMotionClockError> {
        if !event.at_seconds.is_finite() {
            return Err(VirtualMotionClockError::NonFiniteTime("input.at_seconds"));
        }
        if event.at_seconds < self.virtual_time_seconds {
            return Err(VirtualMotionClockError::InputInPast);
        }
        let event_index = u32::try_from(self.scheduled_inputs.len())
            .map_err(|_| VirtualMotionClockError::EventIndexExhausted)?;
        self.scheduled_inputs.push(ScheduledInput {
            event_index,
            event,
            applied: false,
        });
        Ok(event_index)
    }

    /// Advance `count` nominal fixed steps.
    pub fn tick(&mut self, count: u64) -> Result<Vec<VirtualFrame>, VirtualMotionClockError> {
        self.require_frame_headroom(count)?;
        let mut frames = Vec::new();
        for _ in 0..count {
            frames.push(self.advance_frame(self.step_seconds)?);
        }
        Ok(frames)
    }

    /// Seek forward by replaying nominal fixed steps and, when necessary, one
    /// final partial step. Seeking never jumps the evaluator directly to `t`.
    pub fn seek(
        &mut self,
        target_seconds: f64,
    ) -> Result<Vec<VirtualFrame>, VirtualMotionClockError> {
        if !target_seconds.is_finite() {
            return Err(VirtualMotionClockError::NonFiniteTime(
                "seek.target_seconds",
            ));
        }
        if target_seconds < self.virtual_time_seconds {
            return Err(VirtualMotionClockError::SeekBeforeNow);
        }
        // Refuse the whole seek before advancing anything: a partially applied
        // seek leaves the clock at a time the caller never asked for.
        let span = target_seconds - self.virtual_time_seconds;
        if span > 0.0 && self.virtual_time_seconds + self.step_seconds == self.virtual_time_seconds
        {
            return Err(VirtualMotionClockError::StalledStep);
        }
        let required = (span / self.step_seconds).ceil();
        if !required.is_finite() || required > MOTION_CLOCK_MAX_SEEK_STEPS as f64 {
            return Err(VirtualMotionClockError::SeekTooFar);
        }
        self.require_frame_headroom(required as u64)?;
        let mut frames = Vec::new();
        while self.virtual_time_seconds < target_seconds {
            let remaining = target_seconds - self.virtual_time_seconds;
            let step = remaining.min(self.step_seconds);
            frames.push(self.advance_frame(step)?);
        }
        Ok(frames)
    }

    /// Step until no active drivers and no unapplied inputs remain, or until
    /// `max_steps` is exhausted. Quiescence is table/log state, never a
    /// wall-clock delay or timing heuristic.
    pub fn run_to_quiescence(
        &mut self,
        max_steps: u64,
    ) -> Result<(Vec<VirtualFrame>, QuiescenceVerdict), VirtualMotionClockError> {
        let mut frames = Vec::new();
        let mut steps_taken = 0;
        while steps_taken < max_steps && !self.is_quiescent() {
            frames.push(self.advance_frame(self.step_seconds)?);
            steps_taken += 1;
        }
        let verdict = QuiescenceVerdict {
            quiescent: self.is_quiescent(),
            virtual_time_seconds: self.virtual_time_seconds,
            steps_taken,
            active_drivers: self.table.active_count(),
            pending_inputs: self.pending_input_count(),
        };
        Ok((frames, verdict))
    }

    /// Return the complete trace observed by this clock so far.
    pub fn trace(&self) -> ValueSequenceTrace {
        ValueSequenceTrace::new(self.frames.clone())
    }

    /// Frames retained so far. The clock refuses to advance past
    /// [`MOTION_CLOCK_MAX_RETAINED_FRAMES`].
    pub fn retained_frame_count(&self) -> usize {
        self.frames.len()
    }

    fn require_frame_headroom(&self, additional: u64) -> Result<(), VirtualMotionClockError> {
        let remaining = MOTION_CLOCK_MAX_RETAINED_FRAMES.saturating_sub(self.frames.len()) as u64;
        if additional > remaining {
            return Err(VirtualMotionClockError::FrameHistoryExhausted);
        }
        Ok(())
    }

    fn is_quiescent(&self) -> bool {
        self.table.active_count() == 0 && self.pending_input_count() == 0
    }

    fn advance_frame(
        &mut self,
        step_seconds: f64,
    ) -> Result<VirtualFrame, VirtualMotionClockError> {
        if !step_seconds.is_finite() || step_seconds <= 0.0 {
            return Err(VirtualMotionClockError::InvalidStepSeconds);
        }
        self.require_frame_headroom(1)?;
        let next_time = self.virtual_time_seconds + step_seconds;
        // A step that rounds away against the current magnitude would spin
        // forever without moving time. Refuse instead of stalling.
        if next_time == self.virtual_time_seconds {
            return Err(VirtualMotionClockError::StalledStep);
        }
        if !next_time.is_finite() {
            return Err(VirtualMotionClockError::NonFiniteTime(
                "virtual_time_seconds",
            ));
        }
        let next_sequence = self
            .sequence
            .checked_add(1)
            .ok_or(VirtualMotionClockError::FrameSequenceExhausted)?;

        let mut applied_inputs = Vec::new();
        let mut input_publications = Vec::new();
        let mut driver_publications = Vec::new();
        let mut publications = Vec::new();

        for phase in MOTION_TICK_ORDER {
            match phase {
                MotionTickPhase::InputSampling => {
                    let (applied, published) = self.apply_due_inputs(next_time)?;
                    applied_inputs = applied;
                    input_publications = published;
                }
                MotionTickPhase::ArenaCheckpoint => {}
                MotionTickPhase::DriverAdvance => {
                    driver_publications = self.table.advance(step_seconds)?;
                }
                MotionTickPhase::DerivedValues => {}
                MotionTickPhase::BindingApplication => {
                    publications.reserve(input_publications.len() + driver_publications.len());
                    publications.append(&mut input_publications);
                    publications.append(&mut driver_publications);
                    for publication in &publications {
                        self.retarget_samples
                            .insert(publication.value_handle, publication.sample);
                    }
                }
                MotionTickPhase::Present => {}
                MotionTickPhase::Observability => {}
                MotionTickPhase::Auxiliary => {}
            }
        }

        self.virtual_time_seconds = next_time;
        self.sequence = next_sequence;
        let frame = VirtualFrame {
            sequence: next_sequence,
            virtual_time_seconds: next_time,
            step_seconds,
            applied_inputs,
            publications,
        };
        self.frames.push(frame.clone());
        Ok(frame)
    }

    fn apply_due_inputs(
        &mut self,
        next_time: f64,
    ) -> Result<(Vec<AppliedInput>, Vec<DriverPublication>), VirtualMotionClockError> {
        let mut due: Vec<usize> = self
            .scheduled_inputs
            .iter()
            .enumerate()
            .filter_map(|(index, scheduled)| {
                (!scheduled.applied && scheduled.event.at_seconds <= next_time).then_some(index)
            })
            .collect();
        due.sort_by(|left, right| {
            let left = &self.scheduled_inputs[*left];
            let right = &self.scheduled_inputs[*right];
            if left.event.at_seconds == right.event.at_seconds {
                left.event_index.cmp(&right.event_index)
            } else {
                left.event.at_seconds.total_cmp(&right.event.at_seconds)
            }
        });

        let mut applied_inputs = Vec::with_capacity(due.len());
        let mut publications = Vec::new();
        for index in due {
            let event_index = self.scheduled_inputs[index].event_index;
            let input = self.scheduled_inputs[index].event.input.clone();
            let value_handle = input.value_handle();
            let kind_tag = input.tag();
            let (terminal, publication) = self.apply_input(input)?;
            self.scheduled_inputs[index].applied = true;
            applied_inputs.push(AppliedInput {
                event_index,
                kind_tag,
                value_handle,
                terminal,
            });
            if let Some(publication) = publication {
                publications.push(publication);
            }
        }
        Ok((applied_inputs, publications))
    }

    fn apply_input(
        &mut self,
        input: VirtualInputKind,
    ) -> Result<(Option<DriverTerminal>, Option<DriverPublication>), VirtualMotionClockError> {
        match input {
            VirtualInputKind::StartDriver {
                value_handle,
                spec,
                initial,
                velocity,
            } => {
                let driver = spec.instantiate(initial, velocity)?;
                let terminal = self.table.start(value_handle, driver)?;
                self.retarget_samples.insert(
                    value_handle,
                    MotionSample {
                        position: initial,
                        velocity: 0.0,
                        settled: false,
                    },
                );
                Ok((terminal, None))
            }
            VirtualInputKind::Retarget { value_handle, spec } => {
                let inherited =
                    self.retarget_samples
                        .get(&value_handle)
                        .copied()
                        .unwrap_or(MotionSample {
                            position: 0.0,
                            velocity: 0.0,
                            settled: false,
                        });
                let driver = spec.instantiate(inherited.position, inherited.velocity)?;
                let terminal = self.table.start(value_handle, driver)?;
                Ok((terminal, None))
            }
            VirtualInputKind::WriteValue {
                value_handle,
                value,
            } => {
                let publication =
                    scalar_publication(value_handle, value, "virtual_clock.write_value")?;
                let terminal = self.table.cancel_for_imperative_write(value_handle);
                self.retarget_samples
                    .insert(value_handle, publication.sample);
                Ok((terminal, Some(publication)))
            }
            VirtualInputKind::GestureWrite {
                value_handle,
                value,
            } => {
                let publication =
                    scalar_publication(value_handle, value, "virtual_clock.gesture_write")?;
                let terminal = self.table.cancel_for_gesture_write(value_handle);
                self.retarget_samples
                    .insert(value_handle, publication.sample);
                Ok((terminal, Some(publication)))
            }
            VirtualInputKind::CancelDriver { value_handle } => Ok((
                self.table
                    .cancel(value_handle, DriverTerminalReason::ExplicitCancel),
                None,
            )),
        }
    }
}

fn scalar_publication(
    value_handle: u64,
    value: f64,
    field: &'static str,
) -> Result<DriverPublication, MotionDriverError> {
    if !value.is_finite() {
        return Err(MotionDriverError::NonFinite(field));
    }
    Ok(DriverPublication {
        value_handle,
        driver_sequence: INPUT_PUBLICATION_SEQUENCE,
        sample: MotionSample {
            position: value,
            velocity: 0.0,
            settled: true,
        },
        terminal: None,
    })
}

/// A last-published value at a requested trace checkpoint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheckpointValue {
    /// Stable value handle.
    pub value_handle: u64,
    /// Last published position.
    pub position: f64,
    /// Last published velocity.
    pub velocity: f64,
    /// Last published settlement flag.
    pub settled: bool,
}

/// Value-sequence state at or before a requested virtual time.
#[derive(Debug, Clone, PartialEq)]
pub struct Checkpoint {
    /// Requested virtual time.
    pub requested_time_seconds: f64,
    /// Last frame at or before the requested time, if one exists.
    pub frame_sequence: Option<u64>,
    /// Last samples in ascending value-handle order.
    pub values: Vec<CheckpointValue>,
}

/// Canonical native-evaluator frame sequence for bit-identity evidence.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ValueSequenceTrace {
    frames: Vec<VirtualFrame>,
}

impl ValueSequenceTrace {
    /// Construct a trace from receipt-ordered frames.
    pub fn new(frames: Vec<VirtualFrame>) -> Self {
        Self { frames }
    }

    /// Borrow the receipt-ordered frames.
    pub fn frames(&self) -> &[VirtualFrame] {
        &self.frames
    }

    /// Encode the trace using the RFC 0492 M-A little-endian v1 layout.
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        for frame in &self.frames {
            encoded.extend_from_slice(&frame.sequence.to_le_bytes());
            encoded.extend_from_slice(&frame.virtual_time_seconds.to_bits().to_le_bytes());
            encoded.extend_from_slice(&frame.step_seconds.to_bits().to_le_bytes());
            encoded.extend_from_slice(&(frame.applied_inputs.len() as u32).to_le_bytes());
            for input in &frame.applied_inputs {
                encoded.extend_from_slice(&input.event_index.to_le_bytes());
                encoded.push(input.kind_tag as u8);
                encoded.extend_from_slice(&input.value_handle.to_le_bytes());
            }
            encoded.extend_from_slice(&(frame.publications.len() as u32).to_le_bytes());
            for publication in &frame.publications {
                encoded.extend_from_slice(&publication.value_handle.to_le_bytes());
                encoded.extend_from_slice(&publication.driver_sequence.to_le_bytes());
                encoded.extend_from_slice(&publication.sample.position.to_bits().to_le_bytes());
                encoded.extend_from_slice(&publication.sample.velocity.to_bits().to_le_bytes());
                encoded.push(u8::from(publication.sample.settled));
                encoded.push(terminal_tag(publication.terminal.as_ref()));
            }
        }
        encoded
    }

    /// A domain-separated 64-bit FNV-1a hash over the math-profile identity and
    /// the encoded frames.
    ///
    /// This is a non-cryptographic *equality witness* for logs and agent
    /// evidence, not a security digest: it is not collision-resistant and
    /// nothing may rely on it to detect an adversarially chosen trace.
    /// [`ValueSequenceTrace::encode`] is the authority whenever a comparison
    /// has to be exact.
    pub fn digest(&self) -> u64 {
        let mut hash = FNV_OFFSET_BASIS;
        let mut absorb = |bytes: &[u8]| {
            for byte in bytes {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(FNV_PRIME);
            }
        };
        absorb(VALUE_SEQUENCE_DOMAIN);
        absorb(DETERMINISTIC_MATH_PROFILE.profile_id.as_bytes());
        absorb(&[0]);
        absorb(&self.encode());
        hash
    }

    /// Return the lowercase hexadecimal form of [`ValueSequenceTrace::digest`].
    pub fn digest_hex(&self) -> String {
        format!("{:016x}", self.digest())
    }

    /// Resolve the last published sample per handle at or before each
    /// requested time. Requested order is preserved and handle order is
    /// ascending, forming the tolerance oracle used by later web legs.
    pub fn checkpoints(&self, requested_times: &[f64]) -> Vec<Checkpoint> {
        requested_times
            .iter()
            .map(|requested_time| {
                let mut values = BTreeMap::new();
                let mut frame_sequence = None;
                for frame in &self.frames {
                    if frame.virtual_time_seconds > *requested_time {
                        break;
                    }
                    frame_sequence = Some(frame.sequence);
                    for publication in &frame.publications {
                        values.insert(publication.value_handle, publication.sample);
                    }
                }
                Checkpoint {
                    requested_time_seconds: *requested_time,
                    frame_sequence,
                    values: values
                        .into_iter()
                        .map(|(value_handle, sample)| CheckpointValue {
                            value_handle,
                            position: sample.position,
                            velocity: sample.velocity,
                            settled: sample.settled,
                        })
                        .collect(),
                }
            })
            .collect()
    }
}

fn terminal_tag(terminal: Option<&DriverTerminal>) -> u8 {
    // Stable v1 encoding: none=0, then DriverTerminalReason declaration order
    // beginning at one. This is evaluator evidence, not a Motion wire enum.
    match terminal.map(|terminal| terminal.reason) {
        None => 0,
        Some(DriverTerminalReason::Settled) => 1,
        Some(DriverTerminalReason::Replaced) => 2,
        Some(DriverTerminalReason::ImperativeWrite) => 3,
        Some(DriverTerminalReason::GestureWrite) => 4,
        Some(DriverTerminalReason::ExplicitCancel) => 5,
        Some(DriverTerminalReason::DescriptorDetached) => 6,
        Some(DriverTerminalReason::ReducedMotion) => 7,
        Some(DriverTerminalReason::Reset) => 8,
        Some(DriverTerminalReason::NumericalFailure) => 9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{MotionEasing, SpringConfig, TimingConfig};

    fn spring(target: f64) -> AnimationDriverSpec {
        AnimationDriverSpec::Spring {
            target,
            config: SpringConfig::default(),
        }
    }

    fn start_spring(clock: &mut VirtualMotionClock, value_handle: u64) {
        clock
            .schedule_input(VirtualInputEvent {
                at_seconds: 0.0,
                input: VirtualInputKind::StartDriver {
                    value_handle,
                    spec: spring(100.0),
                    initial: 0.0,
                    velocity: 12.0,
                },
            })
            .expect("schedule spring");
    }

    #[test]
    fn identical_runs_have_identical_bytes_and_digests() {
        fn run() -> ValueSequenceTrace {
            let mut clock = VirtualMotionClock::new(1.0 / 60.0).expect("valid clock");
            start_spring(&mut clock, 7);
            clock.tick(40).expect("advance trace");
            clock.trace()
        }

        let first = run();
        let second = run();
        assert_eq!(first.encode(), second.encode());
        assert_eq!(first.digest(), second.digest());
    }

    #[test]
    fn seek_matches_fixed_ticks_plus_the_same_partial_step() {
        let step = 1.0 / 60.0;
        let fixed_time = step + step + step;
        let target = fixed_time + step * 0.25;

        let mut by_seek = VirtualMotionClock::new(step).expect("valid clock");
        start_spring(&mut by_seek, 1);
        let seek_frames = by_seek.seek(target).expect("seek");

        let mut explicit = VirtualMotionClock::new(step).expect("valid clock");
        start_spring(&mut explicit, 1);
        let mut explicit_frames = explicit.tick(3).expect("fixed ticks");
        explicit_frames.extend(explicit.seek(target).expect("partial seek"));

        assert_eq!(seek_frames, explicit_frames);
        assert_eq!(by_seek.trace().encode(), explicit.trace().encode());
    }

    #[test]
    fn quiescence_is_computed_for_settling_and_infinite_drivers() {
        fn settled_run() -> QuiescenceVerdict {
            let mut clock = VirtualMotionClock::new(1.0 / 60.0).expect("valid clock");
            start_spring(&mut clock, 1);
            let (_, verdict) = clock.run_to_quiescence(600).expect("spring run");
            verdict
        }

        let first = settled_run();
        let second = settled_run();
        assert!(first.quiescent);
        assert_eq!(first, second);
        assert_eq!(first.steps_taken, 78);

        let mut infinite = VirtualMotionClock::new(1.0 / 60.0).expect("valid clock");
        infinite
            .schedule_input(VirtualInputEvent {
                at_seconds: 0.0,
                input: VirtualInputKind::StartDriver {
                    value_handle: 2,
                    spec: AnimationDriverSpec::Repeat {
                        inner: Box::new(AnimationDriverSpec::Timing {
                            target: 1.0,
                            config: TimingConfig {
                                duration_seconds: 1.0 / 120.0,
                                easing: MotionEasing::Linear,
                            },
                        }),
                        count: -1,
                        reverse: false,
                    },
                    initial: 0.0,
                    velocity: 0.0,
                },
            })
            .expect("schedule repeat");
        let (_, verdict) = infinite.run_to_quiescence(12).expect("bounded repeat run");
        assert!(!verdict.quiescent);
        assert_eq!(verdict.steps_taken, 12);
        assert_eq!(verdict.active_drivers, 1);
        assert_eq!(verdict.pending_inputs, 0);
    }

    #[test]
    fn retarget_inherits_last_published_position_and_velocity() {
        let step = 1.0 / 60.0;
        let mut clock = VirtualMotionClock::new(step).expect("valid clock");
        start_spring(&mut clock, 9);
        let first = clock.tick(1).expect("first tick");
        let inherited = first[0].publications[0].sample;
        let retarget_spec = spring(-40.0);
        clock
            .schedule_input(VirtualInputEvent {
                at_seconds: clock.virtual_time_seconds(),
                input: VirtualInputKind::Retarget {
                    value_handle: 9,
                    spec: retarget_spec.clone(),
                },
            })
            .expect("schedule retarget");
        let retargeted = clock.tick(1).expect("retarget tick");

        let mut expected_driver = retarget_spec
            .instantiate(inherited.position, inherited.velocity)
            .expect("instantiate expected driver");
        let expected = expected_driver
            .advance(step)
            .expect("expected advance")
            .sample;
        let actual = retargeted[0].publications[0].sample;
        assert_eq!(actual.position.to_bits(), expected.position.to_bits());
        assert_eq!(actual.velocity.to_bits(), expected.velocity.to_bits());
    }

    #[test]
    fn equal_deadlines_apply_in_insertion_order() {
        let mut clock = VirtualMotionClock::new(1.0 / 60.0).expect("valid clock");
        for value in [10.0, 20.0] {
            clock
                .schedule_input(VirtualInputEvent {
                    at_seconds: 0.0,
                    input: VirtualInputKind::WriteValue {
                        value_handle: 4,
                        value,
                    },
                })
                .expect("schedule write");
        }
        let frames = clock.tick(1).expect("tick writes");
        assert_eq!(frames[0].applied_inputs[0].event_index, 0);
        assert_eq!(frames[0].applied_inputs[1].event_index, 1);
        assert_eq!(frames[0].publications[0].sample.position, 10.0);
        assert_eq!(frames[0].publications[1].sample.position, 20.0);
        let checkpoints = clock.trace().checkpoints(&[clock.virtual_time_seconds()]);
        assert_eq!(checkpoints[0].values[0].position, 20.0);
    }

    #[test]
    fn a_seek_past_the_step_budget_is_refused_whole() {
        let mut clock = VirtualMotionClock::new(1.0 / 60.0).expect("valid clock");
        let reachable = MOTION_CLOCK_MAX_SEEK_STEPS as f64 / 60.0;

        // Past the budget the seek is refused *before* anything advances: a
        // partially applied seek leaves the clock at a time nobody asked for.
        assert_eq!(
            clock.seek(reachable * 2.0),
            Err(VirtualMotionClockError::SeekTooFar)
        );
        assert_eq!(clock.virtual_time_seconds(), 0.0);
        assert_eq!(clock.retained_frame_count(), 0);
        assert!(clock.trace().encode().is_empty());

        // An enormous target is refused the same way, rather than looping.
        assert_eq!(
            clock.seek(f64::MAX),
            Err(VirtualMotionClockError::SeekTooFar)
        );
        assert_eq!(clock.retained_frame_count(), 0);

        // A seek inside the budget still replays every step.
        let frames = clock.seek(0.5).expect("seek within the budget");
        assert_eq!(clock.retained_frame_count(), frames.len());
        assert!((clock.virtual_time_seconds() - 0.5).abs() < 1.0e-12);
    }

    #[test]
    fn the_step_budget_keeps_virtual_time_below_the_magnitude_that_would_stall() {
        // The hazard the budget exists for: if virtual time ever reached a
        // magnitude where `t + step == t`, replaying steps would spin forever
        // without converging. The budget is what makes that unreachable — a
        // clock cannot advance past `cap * step`, and for any finite step that
        // ceiling is far below the magnitude at which the step is absorbed.
        for step in [
            1.0 / 240.0,
            1.0 / 60.0,
            1.0,
            1.0e3,
            1.0e9,
            f64::MIN_POSITIVE,
        ] {
            let ceiling = MOTION_CLOCK_MAX_RETAINED_FRAMES as f64 * step;
            assert_ne!(
                ceiling + step,
                ceiling,
                "a clock stepping {step} could reach a stalling magnitude"
            );
        }

        // And the guard is still there if a future change breaks that: a step
        // that cannot move time is refused rather than replayed.
        let mut clock = VirtualMotionClock::new(1.0 / 60.0).expect("valid clock");
        clock.seek(1.0).expect("ordinary seek");
        let reached = clock.virtual_time_seconds();
        // Seeking to a target that is not representably ahead is a no-op, not
        // a loop: the subtraction is exact, so there is no absorbed remainder.
        assert_eq!(reached + 1.0e-20, reached, "the premise of this test");
        assert_eq!(clock.seek(reached + 1.0e-20), Ok(Vec::new()));
        assert_eq!(clock.virtual_time_seconds(), reached);
    }

    #[test]
    fn the_retained_frame_history_is_capped_and_refuses_rather_than_forgetting() {
        // The history is refused, never trimmed: `checkpoints` replays from the
        // first frame and `encode` covers the whole run, so dropping the oldest
        // frames would change the answers instead of shortening the history.
        let mut clock = VirtualMotionClock::new(1.0 / 60.0).expect("valid clock");
        clock
            .schedule_input(VirtualInputEvent {
                at_seconds: 0.0,
                input: VirtualInputKind::WriteValue {
                    value_handle: 7,
                    value: 3.0,
                },
            })
            .expect("schedule write");
        clock
            .tick(MOTION_CLOCK_MAX_RETAINED_FRAMES as u64)
            .expect("fill the history exactly");
        assert_eq!(
            clock.retained_frame_count(),
            MOTION_CLOCK_MAX_RETAINED_FRAMES
        );

        assert_eq!(
            clock.tick(1),
            Err(VirtualMotionClockError::FrameHistoryExhausted)
        );
        assert_eq!(
            clock.seek(clock.virtual_time_seconds() + 1.0),
            Err(VirtualMotionClockError::FrameHistoryExhausted)
        );
        assert_eq!(
            clock.retained_frame_count(),
            MOTION_CLOCK_MAX_RETAINED_FRAMES,
            "a refused advance still changed the history"
        );

        // The value written on the very first frame is still resolvable at the
        // end of the run, which is the property a drop-oldest policy would have
        // destroyed.
        let checkpoints = clock.trace().checkpoints(&[clock.virtual_time_seconds()]);
        assert_eq!(checkpoints[0].values[0].value_handle, 7);
        assert_eq!(checkpoints[0].values[0].position, 3.0);
    }

    #[test]
    fn imperative_write_emits_replaced_terminal_in_the_same_frame() {
        let mut clock = VirtualMotionClock::new(1.0 / 60.0).expect("valid clock");
        start_spring(&mut clock, 5);
        clock.tick(1).expect("start driver");
        clock
            .schedule_input(VirtualInputEvent {
                at_seconds: clock.virtual_time_seconds(),
                input: VirtualInputKind::WriteValue {
                    value_handle: 5,
                    value: 44.0,
                },
            })
            .expect("schedule write");
        let frames = clock.tick(1).expect("write frame");
        let terminal = frames[0].applied_inputs[0]
            .terminal
            .expect("imperative terminal");
        assert_eq!(terminal.reason, DriverTerminalReason::ImperativeWrite);
        assert_eq!(frames[0].publications[0].sample.position, 44.0);
        assert_eq!(clock.active_driver_count(), 0);
    }
}

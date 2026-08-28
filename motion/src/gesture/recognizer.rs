//! The deterministic recognizer state machine.
//!
//! @ref LLP 0099#gesture-recognizers
//!
//! One machine per authored descriptor. The descriptor variant selects the
//! transition table; every variant shares stream fencing, pointer-set
//! identity, velocity provenance, and terminal classification.

use super::*;

/// Lifecycle of one recognizer machine. A machine walks from Idle to Began,
/// then optionally to Active, and finally into one terminal state that it
/// stays in until `reset` rearms it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecognizerState {
    /// No stream attached; the next pointer-down frame starts one.
    Idle,
    /// Attached to a stream and still possible, but the activation threshold is unmet.
    Began,
    /// The threshold was met; the recognizer owns the gesture and reports every update.
    Active,
    /// Recognized and completed normally.
    Ended,
    /// Was active, then cut off; the event's terminal reason says by what.
    Cancelled,
    /// Ruled out before it ever activated.
    Failed,
}

impl RecognizerState {
    pub(crate) fn is_terminal(self) -> bool {
        matches!(self, Self::Ended | Self::Cancelled | Self::Failed)
    }
}

/// Phase carried on a published event. Began and Changed leave the recognizer
/// open; Ended, Cancelled, and Failed each close it for this stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureEventPhase {
    /// Opening event for the gesture, published when the recognizer attaches or activates.
    Began,
    /// A continuing update, including the intermediate taps of a multi-tap sequence.
    Changed,
    /// The gesture completed; this event carries the final payload.
    Ended,
    /// An active gesture was cut off before it could complete.
    Cancelled,
    /// The recognizer was ruled out without ever activating.
    Failed,
}

/// Why a recognizer stopped. Published on the closing event so a caller can
/// tell a real completion from each of the ways a gesture can be lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureTerminalReason {
    /// The authored conditions were met and the gesture finished.
    Completed,
    /// The contacts lifted before the activation threshold was reached.
    BelowThreshold,
    /// Released before the authored minimum duration, as for a long press.
    TooShort,
    /// Held past the authored maximum duration, or the next tap of a sequence
    /// arrived after the inter-tap window closed.
    TooLong,
    /// The centroid drifted past the maximum distance a tap or long press allows.
    MovedTooFar,
    /// Cross-axis travel dominated an axis-locked pan that authored
    /// `fail_cross_axis`.
    CrossAxis,
    /// The contact count left the descriptor's range, or a two-pointer gesture
    /// started from degenerate geometry.
    PointerCount,
    /// The pointer ids behind the stream changed; pointer-set identity is never
    /// reassigned under an in-flight recognizer.
    PointerSetChanged,
    /// The host cancelled the contacts themselves rather than lifting them.
    PointerCancelled,
    /// Something outside this recognizer took the stream, such as a system gesture.
    ExternalOwner,
    /// The presenter topology under the gesture changed while it was in flight.
    StructuralInvalidation,
    /// Cancelled by the caller at a reset boundary.
    Reset,
    /// Cancelled explicitly by author code.
    AuthorCancelled,
    /// Lost arbitration: another claim owns the stream, or a required failure never came.
    Blocked,
}

/// The typed measurement an event carries, one variant per recognizer kind.
#[derive(Default, Debug, Clone, PartialEq)]
pub enum GesturePayload {
    /// No measurement: attach and terminal events that report only a phase.
    #[default]
    None,
    /// Result of a tap, published as each tap in the sequence completes.
    Tap {
        /// Taps completed so far in this sequence, counting this one.
        tap_count: u8,
        /// Contact centroid at the tap, in the claimed node's local coordinate space.
        position: MotionPoint,
        /// How long this tap was held, in milliseconds.
        duration_ms: f64,
    },
    /// Result of a long press, published when it activates and again when it ends.
    LongPress {
        /// Contact centroid, in the claimed node's local coordinate space.
        position: MotionPoint,
        /// Time held since the stream began, in milliseconds.
        duration_ms: f64,
    },
    /// Result of a pan, published on every frame while it is active.
    Pan {
        /// Centroid movement since the stream began, in points.
        translation: MotionPoint,
        /// Centroid velocity in points per second, with the provenance of the estimate.
        velocity: VelocityEstimate,
    },
    /// Result of a fling, classified once at release.
    Fling {
        /// Centroid movement over the whole stream, in points.
        translation: MotionPoint,
        /// Release velocity in points per second, with the provenance of the estimate.
        velocity: VelocityEstimate,
    },
    /// Result of a pinch, published on every frame while it is active.
    Pinch {
        /// Current contact distance over the distance at stream start; 1.0 is unchanged.
        scale: f64,
        /// Midpoint of the two contacts, in the claimed node's local coordinate space.
        focal_point: MotionPoint,
        /// Rate of scale change per second, normalized by the initial contact distance.
        velocity_per_second: f64,
        /// Which estimator produced the scale velocity, and from how many samples.
        velocity_provenance: VelocityProvenance,
    },
    /// Result of a rotation, published on every frame while it is active.
    Rotation {
        /// Rotation since the stream began, in radians. Accumulated from per-frame
        /// shortest-angle deltas, so it is not wrapped to a single turn.
        radians: f64,
        /// Rate of rotation, in radians per second.
        velocity_per_second: f64,
        /// Which estimator produced the rotation velocity, and from how many samples.
        velocity_provenance: VelocityProvenance,
    },
}

/// One published gesture event: who produced it, the sample it was derived
/// from, and the typed payload for that recognizer kind.
#[derive(Debug, Clone, PartialEq)]
pub struct RecognizerEvent {
    /// Identity of the descriptor that produced this event.
    pub recognizer_id: GestureId,
    /// Generation the descriptor held when this event was produced, so events
    /// from before a reset boundary can be told apart from events after it.
    pub descriptor_generation: u32,
    /// Stream this event belongs to; a recognizer never spans streams.
    pub stream_id: GestureStreamId,
    /// Pointer ids backing the stream when the event was produced.
    pub pointer_set: PointerSetIdentity,
    /// Contact centroid in the claimed presenter node's local coordinate space.
    pub position: MotionPoint,
    /// Contact centroid in the platform window's coordinate space.
    pub absolute_position: MotionPoint,
    /// Mean platform-reported delta of the contacts since their previous sample.
    pub delta: MotionPoint,
    /// Mean normalized contact pressure, in the closed [0, 1] range.
    pub pressure: f64,
    /// Timestamp of the frame that produced this event, in milliseconds. Never
    /// decreases within a stream.
    pub timestamp_ms: f64,
    /// Where this event sits in the recognizer's lifecycle.
    pub phase: GestureEventPhase,
    /// Recognizer state after the transition this event reports.
    pub state: RecognizerState,
    /// Typed measurement for this recognizer kind, or `GesturePayload::None`.
    pub payload: GesturePayload,
    /// Set only on terminal events; says why the recognizer stopped.
    pub terminal_reason: Option<GestureTerminalReason>,
}

/// Everything one call into a recognizer published. An empty step is normal:
/// most frames advance internal state without producing an event.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RecognizerStep {
    /// Events to publish, in emission order.
    pub events: Vec<RecognizerEvent>,
}

impl RecognizerStep {
    fn one(event: RecognizerEvent) -> Self {
        Self {
            events: vec![event],
        }
    }

    fn none() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone)]
struct RecognizerStream {
    stream_id: GestureStreamId,
    pointer_set: PointerSetIdentity,
    start_timestamp_ms: f64,
    last_timestamp_ms: f64,
    start_centroid: MotionPoint,
    current_centroid: MotionPoint,
    current_absolute_centroid: MotionPoint,
    current_delta_centroid: MotionPoint,
    current_pressure: f64,
    velocity: VelocityTracker,
    initial_distance: Option<f64>,
    previous_angle: Option<f64>,
    accumulated_rotation: f64,
    distance_velocity: VelocityTracker,
    rotation_velocity: VelocityTracker,
}

impl RecognizerStream {
    fn new(frame: &PointerFrame) -> Result<Self, GestureInputError> {
        let centroid = frame.centroid();
        let mut velocity = VelocityTracker::default();
        velocity.add_sample(frame.pointer_set(), centroid, frame.timestamp_ms)?;
        let geometry = frame.two_pointer_geometry();
        let mut distance_velocity = VelocityTracker::default();
        let initial_scalar = geometry.map(|value| value.0).unwrap_or(0.0);
        distance_velocity.add_sample(
            frame.pointer_set(),
            MotionPoint {
                x: initial_scalar,
                y: 0.0,
            },
            frame.timestamp_ms,
        )?;
        let mut rotation_velocity = VelocityTracker::default();
        rotation_velocity.add_sample(frame.pointer_set(), MotionPoint::ZERO, frame.timestamp_ms)?;
        Ok(Self {
            stream_id: frame.stream_id,
            pointer_set: frame.pointer_set().clone(),
            start_timestamp_ms: frame.timestamp_ms,
            last_timestamp_ms: frame.timestamp_ms,
            start_centroid: centroid,
            current_centroid: centroid,
            current_absolute_centroid: frame.absolute_centroid(),
            current_delta_centroid: frame.delta_centroid(),
            current_pressure: frame.mean_pressure(),
            velocity,
            initial_distance: geometry.map(|value| value.0),
            previous_angle: geometry.map(|value| value.1),
            accumulated_rotation: 0.0,
            distance_velocity,
            rotation_velocity,
        })
    }

    fn validate_followup(&self, frame: &PointerFrame) -> Result<(), GestureInputError> {
        if frame.stream_id != self.stream_id {
            return Err(GestureInputError::StreamChanged);
        }
        if frame.timestamp_ms < self.last_timestamp_ms {
            return Err(GestureInputError::TimestampMovedBackward);
        }
        Ok(())
    }

    fn update(&mut self, frame: &PointerFrame) -> Result<(), GestureInputError> {
        let centroid = frame.centroid();
        self.velocity
            .add_sample(frame.pointer_set(), centroid, frame.timestamp_ms)?;
        if let Some((distance, angle, _)) = frame.two_pointer_geometry() {
            if let Some(previous) = self.previous_angle {
                self.accumulated_rotation += shortest_angle_delta(previous, angle);
            }
            self.previous_angle = Some(angle);
            self.distance_velocity.add_sample(
                frame.pointer_set(),
                MotionPoint {
                    x: distance,
                    y: 0.0,
                },
                frame.timestamp_ms,
            )?;
            self.rotation_velocity.add_sample(
                frame.pointer_set(),
                MotionPoint {
                    x: self.accumulated_rotation,
                    y: 0.0,
                },
                frame.timestamp_ms,
            )?;
        }
        self.current_centroid = centroid;
        self.current_absolute_centroid = frame.absolute_centroid();
        self.current_delta_centroid = frame.delta_centroid();
        self.current_pressure = frame.mean_pressure();
        self.last_timestamp_ms = frame.timestamp_ms;
        Ok(())
    }

    fn translation(&self) -> MotionPoint {
        self.current_centroid.subtract(self.start_centroid)
    }

    fn duration_ms(&self) -> f64 {
        self.last_timestamp_ms - self.start_timestamp_ms
    }
}

fn shortest_angle_delta(previous: f64, next: f64) -> f64 {
    let mut delta = next - previous;
    while delta > std::f64::consts::PI {
        delta -= std::f64::consts::TAU;
    }
    while delta < -std::f64::consts::PI {
        delta += std::f64::consts::TAU;
    }
    delta
}

/// One deterministic recognizer machine. The descriptor variant selects the
/// transition table; all variants share stream and pointer-set fencing.
#[derive(Debug, Clone)]
pub struct GestureRecognizer {
    descriptor: RecognizerDescriptor,
    state: RecognizerState,
    stream: Option<RecognizerStream>,
    completed_taps: u8,
    last_tap_up_ms: Option<f64>,
}

impl GestureRecognizer {
    /// Builds an idle recognizer from an authored descriptor.
    ///
    /// The descriptor is validated first and stored only if it passes, so a
    /// recognizer that exists always holds a valid plan.
    pub fn new(descriptor: RecognizerDescriptor) -> Result<Self, GestureDescriptorError> {
        Ok(Self {
            descriptor: descriptor.validate()?,
            state: RecognizerState::Idle,
            stream: None,
            completed_taps: 0,
            last_tap_up_ms: None,
        })
    }

    /// The validated descriptor this machine runs, including its current generation.
    pub fn descriptor(&self) -> RecognizerDescriptor {
        self.descriptor
    }

    /// The current lifecycle state. Terminal states stay terminal until `reset`.
    pub fn state(&self) -> RecognizerState {
        self.state
    }

    /// Advances the machine by one pointer frame and returns whatever it
    /// published, which is often nothing.
    ///
    /// Frames must stay on one stream and carry non-decreasing timestamps, and
    /// a pointer-down frame may only add pointers to the existing set, never
    /// replace them. Feeding a frame to a recognizer that already reached a
    /// terminal state is an error; call `reset` to rearm it first.
    pub fn process(&mut self, frame: &PointerFrame) -> Result<RecognizerStep, GestureInputError> {
        if self.state.is_terminal() {
            return Err(GestureInputError::TerminalState);
        }
        if self.state == RecognizerState::Idle {
            return self.begin(frame);
        }
        if frame.phase == PointerPhase::Down
            && matches!(self.descriptor, RecognizerDescriptor::Tap(_))
            && self.completed_taps > 0
        {
            return self.begin_next_tap(frame);
        }
        if frame.phase == PointerPhase::Down {
            return self.join_pointer_set(frame);
        }

        let stream = self
            .stream
            .as_ref()
            .expect("non-idle recognizer has stream");
        stream.validate_followup(frame)?;
        if frame.pointer_set() != &stream.pointer_set {
            let reason = GestureTerminalReason::PointerSetChanged;
            return Ok(self.terminal_for_current(frame.timestamp_ms, reason));
        }
        if frame.phase == PointerPhase::Cancel {
            return Ok(self.terminal_for_current(
                frame.timestamp_ms,
                GestureTerminalReason::PointerCancelled,
            ));
        }

        if frame.pointer_set().len() < self.descriptor.common().min_pointers as usize {
            if frame.phase == PointerPhase::Up {
                return Ok(self.terminal_for_current(
                    frame.timestamp_ms,
                    GestureTerminalReason::PointerCount,
                ));
            }
            self.stream
                .as_mut()
                .expect("waiting recognizer has stream")
                .update(frame)?;
            return Ok(RecognizerStep::none());
        }

        self.stream
            .as_mut()
            .expect("validated stream")
            .update(frame)?;
        match self.descriptor {
            RecognizerDescriptor::Pan(descriptor) => self.process_pan(frame, descriptor),
            RecognizerDescriptor::Tap(descriptor) => self.process_tap(frame, descriptor),
            RecognizerDescriptor::LongPress(descriptor) => {
                self.process_long_press(frame, descriptor)
            }
            RecognizerDescriptor::Fling(descriptor) => self.process_fling(frame, descriptor),
            RecognizerDescriptor::Pinch(descriptor) => self.process_pinch(frame, descriptor),
            RecognizerDescriptor::Rotation(descriptor) => self.process_rotation(frame, descriptor),
        }
    }

    /// Reset is a fence: in-flight state is discarded without publishing an
    /// event after the caller's reset boundary, and the descriptor is rearmed
    /// under the new non-zero generation.
    pub fn reset(&mut self, new_generation: u32) -> Result<(), GestureDescriptorError> {
        if new_generation == 0 {
            return Err(GestureDescriptorError::ZeroIdentity(
                "recognizer.generation",
            ));
        }
        self.descriptor.set_generation(new_generation);
        self.state = RecognizerState::Idle;
        self.stream = None;
        self.completed_taps = 0;
        self.last_tap_up_ms = None;
        Ok(())
    }

    /// Terminates an in-flight recognizer with an externally supplied reason.
    ///
    /// An active recognizer publishes Cancelled and any other in-flight one
    /// publishes Failed; idle and already-terminal recognizers publish nothing.
    /// The timestamp must be finite and no earlier than the last frame seen.
    pub fn cancel(
        &mut self,
        reason: GestureTerminalReason,
        timestamp_ms: f64,
    ) -> Result<RecognizerStep, GestureInputError> {
        if !timestamp_ms.is_finite() {
            return Err(GestureInputError::NonFinite("recognizer.cancel.timestamp"));
        }
        if self.state == RecognizerState::Idle || self.state.is_terminal() {
            return Ok(RecognizerStep::none());
        }
        let last = self
            .stream
            .as_ref()
            .expect("in-flight recognizer has stream")
            .last_timestamp_ms;
        if timestamp_ms < last {
            return Err(GestureInputError::TimestampMovedBackward);
        }
        Ok(self.terminal_for_current(timestamp_ms, reason))
    }

    fn begin(&mut self, frame: &PointerFrame) -> Result<RecognizerStep, GestureInputError> {
        if frame.phase != PointerPhase::Down {
            return Err(GestureInputError::InvalidTransition);
        }
        self.completed_taps = 0;
        self.last_tap_up_ms = None;
        let common = self.descriptor.common();
        if frame.pointer_set().len() > common.max_pointers as usize {
            self.state = RecognizerState::Failed;
            let stream = RecognizerStream::new(frame)?;
            self.stream = Some(stream);
            return Ok(self.event_step(
                frame.timestamp_ms,
                GestureEventPhase::Failed,
                GesturePayload::None,
                Some(GestureTerminalReason::PointerCount),
            ));
        }
        if frame.pointer_set().len() < common.min_pointers as usize {
            // UIKit delivers members independently. The arena stream and its
            // hit-path topology already exist, but a two-pointer recognizer
            // remains possible until enough stable pointer ids join it.
            self.stream = Some(RecognizerStream::new(frame)?);
            self.state = RecognizerState::Began;
            return Ok(RecognizerStep::none());
        }
        if matches!(
            self.descriptor,
            RecognizerDescriptor::Pinch(_) | RecognizerDescriptor::Rotation(_)
        ) && match frame.two_pointer_geometry() {
            Some(geometry) => geometry.0 <= f64::EPSILON,
            None => true,
        } {
            self.state = RecognizerState::Failed;
            self.stream = Some(RecognizerStream::new(frame)?);
            return Ok(self.event_step(
                frame.timestamp_ms,
                GestureEventPhase::Failed,
                GesturePayload::None,
                Some(GestureTerminalReason::PointerCount),
            ));
        }
        self.stream = Some(RecognizerStream::new(frame)?);
        self.state = RecognizerState::Began;
        Ok(self.event_step(
            frame.timestamp_ms,
            GestureEventPhase::Began,
            GesturePayload::None,
            None,
        ))
    }

    fn join_pointer_set(
        &mut self,
        frame: &PointerFrame,
    ) -> Result<RecognizerStep, GestureInputError> {
        let existing = self
            .stream
            .as_ref()
            .expect("non-idle recognizer has stream");
        existing.validate_followup(frame)?;
        let old_members = existing.pointer_set.members().to_vec();
        let new_members = frame.pointer_set().members();
        if new_members.len() <= old_members.len()
            || !old_members
                .iter()
                .all(|member| new_members.contains(member))
        {
            return Err(GestureInputError::InvalidTransition);
        }

        let common = self.descriptor.common();
        self.stream = Some(RecognizerStream::new(frame)?);
        if new_members.len() > common.max_pointers as usize {
            return Ok(
                self.terminal_for_current(frame.timestamp_ms, GestureTerminalReason::PointerCount)
            );
        }
        if self.state == RecognizerState::Active {
            return Ok(self.terminal_for_current(
                frame.timestamp_ms,
                GestureTerminalReason::PointerSetChanged,
            ));
        }
        if new_members.len() < common.min_pointers as usize {
            return Ok(RecognizerStep::none());
        }
        if matches!(
            self.descriptor,
            RecognizerDescriptor::Pinch(_) | RecognizerDescriptor::Rotation(_)
        ) && match frame.two_pointer_geometry() {
            Some(geometry) => geometry.0 <= f64::EPSILON,
            None => true,
        } {
            return Ok(
                self.terminal_for_current(frame.timestamp_ms, GestureTerminalReason::PointerCount)
            );
        }

        // Rebase velocity/geometry at the membership boundary. For a
        // recognizer that was waiting below its minimum, this is its one
        // visible Began event. A one-or-more-pointer recognizer already
        // emitted Began and simply remains possible under the expanded set.
        if old_members.len() < common.min_pointers as usize {
            self.state = RecognizerState::Began;
            return Ok(self.event_step(
                frame.timestamp_ms,
                GestureEventPhase::Began,
                GesturePayload::None,
                None,
            ));
        }
        Ok(RecognizerStep::none())
    }

    fn begin_next_tap(
        &mut self,
        frame: &PointerFrame,
    ) -> Result<RecognizerStep, GestureInputError> {
        let RecognizerDescriptor::Tap(descriptor) = self.descriptor else {
            return Err(GestureInputError::InvalidTransition);
        };
        let Some(previous_up) = self.last_tap_up_ms else {
            return Err(GestureInputError::InvalidTransition);
        };
        if frame.timestamp_ms < previous_up {
            return Err(GestureInputError::TimestampMovedBackward);
        }
        if frame.timestamp_ms - previous_up > descriptor.maximum_inter_tap_ms {
            self.state = RecognizerState::Failed;
            return Ok(self.event_step(
                frame.timestamp_ms,
                GestureEventPhase::Failed,
                GesturePayload::None,
                Some(GestureTerminalReason::TooLong),
            ));
        }
        let common = descriptor.common;
        if frame.pointer_set().len() < common.min_pointers as usize
            || frame.pointer_set().len() > common.max_pointers as usize
        {
            self.state = RecognizerState::Failed;
            return Ok(self.event_step(
                frame.timestamp_ms,
                GestureEventPhase::Failed,
                GesturePayload::None,
                Some(GestureTerminalReason::PointerCount),
            ));
        }
        self.stream = Some(RecognizerStream::new(frame)?);
        Ok(RecognizerStep::none())
    }

    fn process_pan(
        &mut self,
        frame: &PointerFrame,
        descriptor: PanDescriptor,
    ) -> Result<RecognizerStep, GestureInputError> {
        let stream = self.stream.as_ref().expect("pan stream");
        let translation = stream.translation();
        let (primary, cross) = axis_components(descriptor.axis, translation);
        if self.state == RecognizerState::Began {
            let activated =
                if descriptor.active_offset_x.is_some() || descriptor.active_offset_y.is_some() {
                    outside_offset(translation.x, descriptor.active_offset_x)
                        || outside_offset(translation.y, descriptor.active_offset_y)
                } else {
                    match descriptor.axis {
                        GestureAxis::Any => {
                            stream.start_centroid.distance(stream.current_centroid)
                                >= descriptor.minimum_distance
                        }
                        _ => {
                            primary.abs() >= descriptor.minimum_distance
                                && primary.abs() >= cross.abs() * descriptor.axis_lock_ratio
                        }
                    }
                };
            if activated {
                self.state = RecognizerState::Active;
            } else if descriptor.fail_cross_axis
                && descriptor.axis != GestureAxis::Any
                && cross.abs() >= descriptor.minimum_distance
                && cross.abs() > primary.abs() * descriptor.axis_lock_ratio
            {
                self.state = RecognizerState::Failed;
                return Ok(self.event_step(
                    frame.timestamp_ms,
                    GestureEventPhase::Failed,
                    GesturePayload::None,
                    Some(GestureTerminalReason::CrossAxis),
                ));
            }
        }
        if frame.phase == PointerPhase::Up {
            if self.state == RecognizerState::Active {
                self.state = RecognizerState::Ended;
                return Ok(self.pan_event(frame.timestamp_ms, GestureEventPhase::Ended));
            }
            self.state = RecognizerState::Failed;
            return Ok(self.event_step(
                frame.timestamp_ms,
                GestureEventPhase::Failed,
                GesturePayload::None,
                Some(GestureTerminalReason::BelowThreshold),
            ));
        }
        if self.state == RecognizerState::Active {
            let phase = if frame.phase == PointerPhase::Move {
                GestureEventPhase::Changed
            } else {
                GestureEventPhase::Began
            };
            Ok(self.pan_event(frame.timestamp_ms, phase))
        } else {
            Ok(RecognizerStep::none())
        }
    }

    fn process_tap(
        &mut self,
        frame: &PointerFrame,
        descriptor: TapDescriptor,
    ) -> Result<RecognizerStep, GestureInputError> {
        let stream = self.stream.as_ref().expect("tap stream");
        let distance = stream.start_centroid.distance(stream.current_centroid);
        if distance > descriptor.maximum_distance {
            self.state = RecognizerState::Failed;
            return Ok(self.event_step(
                frame.timestamp_ms,
                GestureEventPhase::Failed,
                GesturePayload::None,
                Some(GestureTerminalReason::MovedTooFar),
            ));
        }
        if stream.duration_ms() > descriptor.maximum_duration_ms {
            self.state = RecognizerState::Failed;
            return Ok(self.event_step(
                frame.timestamp_ms,
                GestureEventPhase::Failed,
                GesturePayload::None,
                Some(GestureTerminalReason::TooLong),
            ));
        }
        if frame.phase == PointerPhase::Up {
            let position = stream.current_centroid;
            let duration_ms = stream.duration_ms();
            self.completed_taps = self.completed_taps.saturating_add(1);
            if self.completed_taps < descriptor.tap_count {
                self.last_tap_up_ms = Some(frame.timestamp_ms);
                return Ok(self.event_step(
                    frame.timestamp_ms,
                    GestureEventPhase::Changed,
                    GesturePayload::Tap {
                        tap_count: self.completed_taps,
                        position,
                        duration_ms,
                    },
                    None,
                ));
            }
            self.state = RecognizerState::Ended;
            return Ok(self.event_step(
                frame.timestamp_ms,
                GestureEventPhase::Ended,
                GesturePayload::Tap {
                    tap_count: self.completed_taps,
                    position,
                    duration_ms,
                },
                Some(GestureTerminalReason::Completed),
            ));
        }
        Ok(RecognizerStep::none())
    }

    fn process_long_press(
        &mut self,
        frame: &PointerFrame,
        descriptor: LongPressDescriptor,
    ) -> Result<RecognizerStep, GestureInputError> {
        let stream = self.stream.as_ref().expect("long press stream");
        if stream.start_centroid.distance(stream.current_centroid) > descriptor.maximum_distance {
            self.state = RecognizerState::Failed;
            return Ok(self.event_step(
                frame.timestamp_ms,
                GestureEventPhase::Failed,
                GesturePayload::None,
                Some(GestureTerminalReason::MovedTooFar),
            ));
        }
        if self.state == RecognizerState::Began
            && stream.duration_ms() >= descriptor.minimum_duration_ms
        {
            self.state = RecognizerState::Active;
            if frame.phase != PointerPhase::Up {
                return Ok(self.long_press_event(frame.timestamp_ms, GestureEventPhase::Began));
            }
        }
        if frame.phase == PointerPhase::Up {
            if self.state == RecognizerState::Active {
                self.state = RecognizerState::Ended;
                return Ok(self.long_press_event(frame.timestamp_ms, GestureEventPhase::Ended));
            }
            self.state = RecognizerState::Failed;
            return Ok(self.event_step(
                frame.timestamp_ms,
                GestureEventPhase::Failed,
                GesturePayload::None,
                Some(GestureTerminalReason::TooShort),
            ));
        }
        Ok(RecognizerStep::none())
    }

    fn process_fling(
        &mut self,
        frame: &PointerFrame,
        descriptor: FlingDescriptor,
    ) -> Result<RecognizerStep, GestureInputError> {
        if frame.phase != PointerPhase::Up {
            return Ok(RecognizerStep::none());
        }
        let stream = self.stream.as_ref().expect("fling stream");
        let translation = stream.translation();
        let estimate = stream.velocity.estimate();
        let velocity = axis_value(descriptor.axis, estimate.velocity_per_second);
        let distance = axis_value(descriptor.axis, translation).abs();
        if velocity.abs() < descriptor.minimum_velocity_per_second
            || distance < descriptor.minimum_distance
        {
            self.state = RecognizerState::Failed;
            return Ok(self.event_step(
                frame.timestamp_ms,
                GestureEventPhase::Failed,
                GesturePayload::None,
                Some(GestureTerminalReason::BelowThreshold),
            ));
        }
        self.state = RecognizerState::Ended;
        Ok(self.event_step(
            frame.timestamp_ms,
            GestureEventPhase::Ended,
            GesturePayload::Fling {
                translation,
                velocity: estimate,
            },
            Some(GestureTerminalReason::Completed),
        ))
    }

    fn process_pinch(
        &mut self,
        frame: &PointerFrame,
        descriptor: PinchDescriptor,
    ) -> Result<RecognizerStep, GestureInputError> {
        let stream = self.stream.as_ref().expect("pinch stream");
        let (distance, _, focal_point) = frame
            .two_pointer_geometry()
            .expect("pointer-set fence preserves two contacts");
        let initial = stream.initial_distance.expect("pinch initial distance");
        let scale = distance / initial;
        if self.state == RecognizerState::Began
            && (scale - 1.0).abs() >= descriptor.minimum_scale_delta
        {
            self.state = RecognizerState::Active;
        }
        if frame.phase == PointerPhase::Up {
            if self.state != RecognizerState::Active {
                self.state = RecognizerState::Failed;
                return Ok(self.event_step(
                    frame.timestamp_ms,
                    GestureEventPhase::Failed,
                    GesturePayload::None,
                    Some(GestureTerminalReason::BelowThreshold),
                ));
            }
            self.state = RecognizerState::Ended;
            return Ok(self.pinch_event(
                frame.timestamp_ms,
                GestureEventPhase::Ended,
                scale,
                focal_point,
            ));
        }
        if self.state == RecognizerState::Active {
            Ok(self.pinch_event(
                frame.timestamp_ms,
                GestureEventPhase::Changed,
                scale,
                focal_point,
            ))
        } else {
            Ok(RecognizerStep::none())
        }
    }

    fn process_rotation(
        &mut self,
        frame: &PointerFrame,
        descriptor: RotationDescriptor,
    ) -> Result<RecognizerStep, GestureInputError> {
        let radians = self
            .stream
            .as_ref()
            .expect("rotation stream")
            .accumulated_rotation;
        if self.state == RecognizerState::Began
            && radians.abs() >= descriptor.minimum_rotation_radians
        {
            self.state = RecognizerState::Active;
        }
        if frame.phase == PointerPhase::Up {
            if self.state != RecognizerState::Active {
                self.state = RecognizerState::Failed;
                return Ok(self.event_step(
                    frame.timestamp_ms,
                    GestureEventPhase::Failed,
                    GesturePayload::None,
                    Some(GestureTerminalReason::BelowThreshold),
                ));
            }
            self.state = RecognizerState::Ended;
            return Ok(self.rotation_event(frame.timestamp_ms, GestureEventPhase::Ended));
        }
        if self.state == RecognizerState::Active {
            Ok(self.rotation_event(frame.timestamp_ms, GestureEventPhase::Changed))
        } else {
            Ok(RecognizerStep::none())
        }
    }

    fn terminal_for_current(
        &mut self,
        timestamp_ms: f64,
        reason: GestureTerminalReason,
    ) -> RecognizerStep {
        let phase = if self.state == RecognizerState::Active {
            self.state = RecognizerState::Cancelled;
            GestureEventPhase::Cancelled
        } else {
            self.state = RecognizerState::Failed;
            GestureEventPhase::Failed
        };
        self.event_step(timestamp_ms, phase, GesturePayload::None, Some(reason))
    }

    fn pan_event(&self, timestamp_ms: f64, phase: GestureEventPhase) -> RecognizerStep {
        let stream = self.stream.as_ref().expect("pan stream");
        self.event_step(
            timestamp_ms,
            phase,
            GesturePayload::Pan {
                translation: stream.translation(),
                velocity: stream.velocity.estimate(),
            },
            (phase == GestureEventPhase::Ended).then_some(GestureTerminalReason::Completed),
        )
    }

    fn long_press_event(&self, timestamp_ms: f64, phase: GestureEventPhase) -> RecognizerStep {
        let stream = self.stream.as_ref().expect("long press stream");
        self.event_step(
            timestamp_ms,
            phase,
            GesturePayload::LongPress {
                position: stream.current_centroid,
                duration_ms: stream.duration_ms(),
            },
            (phase == GestureEventPhase::Ended).then_some(GestureTerminalReason::Completed),
        )
    }

    fn pinch_event(
        &self,
        timestamp_ms: f64,
        phase: GestureEventPhase,
        scale: f64,
        focal_point: MotionPoint,
    ) -> RecognizerStep {
        let stream = self.stream.as_ref().expect("pinch stream");
        let estimate = stream.distance_velocity.estimate();
        let distance_velocity = estimate.velocity_per_second.x;
        let initial = stream.initial_distance.expect("pinch initial distance");
        self.event_step(
            timestamp_ms,
            phase,
            GesturePayload::Pinch {
                scale,
                focal_point,
                velocity_per_second: distance_velocity / initial,
                velocity_provenance: estimate.provenance,
            },
            (phase == GestureEventPhase::Ended).then_some(GestureTerminalReason::Completed),
        )
    }

    fn rotation_event(&self, timestamp_ms: f64, phase: GestureEventPhase) -> RecognizerStep {
        let stream = self.stream.as_ref().expect("rotation stream");
        let estimate = stream.rotation_velocity.estimate();
        self.event_step(
            timestamp_ms,
            phase,
            GesturePayload::Rotation {
                radians: stream.accumulated_rotation,
                velocity_per_second: estimate.velocity_per_second.x,
                velocity_provenance: estimate.provenance,
            },
            (phase == GestureEventPhase::Ended).then_some(GestureTerminalReason::Completed),
        )
    }

    fn event_step(
        &self,
        timestamp_ms: f64,
        phase: GestureEventPhase,
        payload: GesturePayload,
        terminal_reason: Option<GestureTerminalReason>,
    ) -> RecognizerStep {
        let stream = self.stream.as_ref().expect("event requires stream");
        RecognizerStep::one(RecognizerEvent {
            recognizer_id: self.descriptor.common().id,
            descriptor_generation: self.descriptor.common().generation,
            stream_id: stream.stream_id,
            pointer_set: stream.pointer_set.clone(),
            position: stream.current_centroid,
            absolute_position: stream.current_absolute_centroid,
            delta: stream.current_delta_centroid,
            pressure: stream.current_pressure,
            timestamp_ms,
            phase,
            state: self.state,
            payload,
            terminal_reason,
        })
    }
}

fn axis_components(axis: GestureAxis, value: MotionPoint) -> (f64, f64) {
    match axis {
        GestureAxis::Any | GestureAxis::Horizontal => (value.x, value.y),
        GestureAxis::Vertical => (value.y, value.x),
    }
}

fn outside_offset(value: f64, offset: Option<(f64, f64)>) -> bool {
    offset.is_some_and(|(minimum, maximum)| value < minimum || value > maximum)
}

fn axis_value(axis: GestureAxis, value: MotionPoint) -> f64 {
    match axis {
        GestureAxis::Any => {
            if value.x.abs() >= value.y.abs() {
                value.x
            } else {
                value.y
            }
        }
        GestureAxis::Horizontal => value.x,
        GestureAxis::Vertical => value.y,
    }
}

#[cfg(test)]
#[path = "recognizer_tests.rs"]
mod tests;

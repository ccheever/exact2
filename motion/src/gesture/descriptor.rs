//! Authored recognizer descriptors and their validation.
//!
//! @ref LLP 0099#gesture-recognizers
//!
//! These are plan data: a compiler emits them, the evaluator consumes them,
//! and every field is validated before a recognizer is instantiated.

use super::*;

/// Which recognizer a descriptor authors. The `u8` discriminants are the wire
/// encoding a compiler emits; `TryFrom<u8>` is the only way back from one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecognizerKind {
    /// Continuous drag of the contact centroid, activated by distance or an offset band.
    Pan = 1,
    /// One tap, or a multi-tap sequence bounded by duration, drift, and inter-tap time.
    Tap = 2,
    /// A contact held roughly in place until a minimum duration elapses.
    LongPress = 3,
    /// A flick, classified at pointer up from release velocity and travel.
    Fling = 4,
    /// Two-pointer scale, measured against the contact distance at stream start.
    Pinch = 5,
    /// Two-pointer rotation, accumulated from the angle between the contacts.
    Rotation = 6,
}

impl TryFrom<u8> for RecognizerKind {
    type Error = GestureDescriptorError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Pan),
            2 => Ok(Self::Tap),
            3 => Ok(Self::LongPress),
            4 => Ok(Self::Fling),
            5 => Ok(Self::Pinch),
            6 => Ok(Self::Rotation),
            _ => Err(GestureDescriptorError::InvalidEnum {
                field: "recognizer.kind",
                value,
            }),
        }
    }
}

/// The axis a pan or fling measures its thresholds along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GestureAxis {
    /// Both axes count; travel is the straight-line distance the centroid moved.
    Any = 0,
    /// Only x travel counts toward the threshold; y is the cross axis.
    Horizontal = 1,
    /// Only y travel counts toward the threshold; x is the cross axis.
    Vertical = 2,
}

impl TryFrom<u8> for GestureAxis {
    type Error = GestureDescriptorError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Any),
            1 => Ok(Self::Horizontal),
            2 => Ok(Self::Vertical),
            _ => Err(GestureDescriptorError::InvalidEnum {
                field: "recognizer.axis",
                value,
            }),
        }
    }
}

/// The identity and pointer-count fields every recognizer descriptor carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecognizerCommon {
    /// Stable identity of the authored recognizer. Must be non-zero.
    pub id: GestureId,
    /// Bumped on every reset so events published before a reset boundary stay
    /// distinguishable from events after it. Must be non-zero.
    pub generation: u32,
    /// Fewest simultaneous contacts before the recognizer can activate. Must be at least 1.
    pub min_pointers: u8,
    /// Most simultaneous contacts tolerated; more than this fails the recognizer. Must be at
    /// least `min_pointers` and at most `MAX_POINTERS_PER_STREAM`.
    pub max_pointers: u8,
}

/// Authored thresholds for a pan recognizer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanDescriptor {
    /// Identity and pointer-count bounds.
    pub common: RecognizerCommon,
    /// Axis the activation thresholds are measured along.
    pub axis: GestureAxis,
    /// Centroid travel required to activate, in points. Zero is the authored
    /// "activate on the first movement" idiom and is accepted.
    pub minimum_distance: f64,
    /// How far the primary axis must lead the cross axis to activate an axis-locked pan, as
    /// a ratio of the two magnitudes. Must be at least 1.0.
    pub axis_lock_ratio: f64,
    /// When set, cross-axis travel that dominates the primary axis fails the
    /// recognizer outright instead of leaving it waiting.
    pub fail_cross_axis: bool,
    /// Optional inclusive `(minimum, maximum)` band for x translation, in points; the pan
    /// activates once translation leaves it. Authoring either band replaces the
    /// `minimum_distance` test entirely.
    pub active_offset_x: Option<(f64, f64)>,
    /// Optional inclusive `(minimum, maximum)` band for y translation, in points, applied
    /// the same way as `active_offset_x`.
    pub active_offset_y: Option<(f64, f64)>,
}

/// Authored thresholds for a tap recognizer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TapDescriptor {
    /// Identity and pointer-count bounds.
    pub common: RecognizerCommon,
    /// Longest a single tap may stay down, in milliseconds. Past it the tap fails.
    pub maximum_duration_ms: f64,
    /// Longest gap allowed between one tap's release and the next tap's press, in
    /// milliseconds. Only meaningful when `tap_count` is greater than 1.
    pub maximum_inter_tap_ms: f64,
    /// Farthest the centroid may drift from where it started, in points.
    pub maximum_distance: f64,
    /// Taps that must complete before the gesture ends. Must be at least 1.
    pub tap_count: u8,
}

/// Authored thresholds for a long press recognizer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LongPressDescriptor {
    /// Identity and pointer-count bounds.
    pub common: RecognizerCommon,
    /// How long the contact must be held before the press activates, in milliseconds.
    pub minimum_duration_ms: f64,
    /// Farthest the centroid may drift while held, in points, before the press fails.
    pub maximum_distance: f64,
}

/// Authored thresholds for a fling recognizer, which is classified once at
/// pointer up rather than tracked continuously.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlingDescriptor {
    /// Identity and pointer-count bounds.
    pub common: RecognizerCommon,
    /// Axis the release velocity and travel are measured along.
    pub axis: GestureAxis,
    /// Release speed required along the axis, in points per second.
    pub minimum_velocity_per_second: f64,
    /// Travel required along the axis, in points, on top of the velocity floor.
    pub minimum_distance: f64,
}

/// Authored thresholds for a pinch recognizer, which requires exactly two pointers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PinchDescriptor {
    /// Identity and pointer-count bounds; both must be exactly 2.
    pub common: RecognizerCommon,
    /// How far the scale must move away from 1.0 before the pinch activates, as a
    /// unitless ratio of the current contact distance to the initial one.
    pub minimum_scale_delta: f64,
}

/// Authored thresholds for a rotation recognizer, which requires exactly two pointers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotationDescriptor {
    /// Identity and pointer-count bounds; both must be exactly 2.
    pub common: RecognizerCommon,
    /// Accumulated rotation required before the gesture activates, in radians.
    pub minimum_rotation_radians: f64,
}

/// One authored recognizer plan. The variant selects both the validation rules
/// and the state machine that will run it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecognizerDescriptor {
    /// A pan, with its axis and activation thresholds.
    Pan(PanDescriptor),
    /// A tap or multi-tap, with its timing and drift bounds.
    Tap(TapDescriptor),
    /// A long press, with its hold duration and drift bound.
    LongPress(LongPressDescriptor),
    /// A fling, with its release velocity and travel floors.
    Fling(FlingDescriptor),
    /// A two-pointer pinch, with its scale threshold.
    Pinch(PinchDescriptor),
    /// A two-pointer rotation, with its angle threshold.
    Rotation(RotationDescriptor),
}

impl RecognizerDescriptor {
    /// Checks every authored field and returns the descriptor unchanged when it
    /// passes. Identity, generation, and pointer counts are checked for every
    /// variant; each variant then checks its own thresholds for finiteness and
    /// range. `GestureRecognizer::new` runs this, so a live recognizer always
    /// holds a plan that passed.
    pub fn validate(self) -> Result<Self, GestureDescriptorError> {
        let common = self.common();
        validate_common(common)?;
        match self {
            Self::Pan(descriptor) => {
                // Zero is the authored "activate on first movement" idiom: the
                // web recognizer treats minDistance 0 exactly that way, and
                // shipped Contract components (FacetResizable, the collage
                // dividers) author it. Rejecting it here was a web/native
                // divergence that voided whole snapshots via InvalidGestureGraph.
                finite_nonnegative(descriptor.minimum_distance, "pan.minimum_distance")?;
                finite_positive(descriptor.axis_lock_ratio, "pan.axis_lock_ratio")?;
                if descriptor.axis_lock_ratio < 1.0 {
                    return Err(GestureDescriptorError::OutOfRange("pan.axis_lock_ratio"));
                }
                validate_optional_offset(descriptor.active_offset_x, "pan.active_offset_x")?;
                validate_optional_offset(descriptor.active_offset_y, "pan.active_offset_y")?;
            }
            Self::Tap(descriptor) => {
                finite_positive(descriptor.maximum_duration_ms, "tap.maximum_duration_ms")?;
                finite_positive(descriptor.maximum_inter_tap_ms, "tap.maximum_inter_tap_ms")?;
                finite_nonnegative(descriptor.maximum_distance, "tap.maximum_distance")?;
                if descriptor.tap_count == 0 {
                    return Err(GestureDescriptorError::OutOfRange("tap.tap_count"));
                }
            }
            Self::LongPress(descriptor) => {
                finite_positive(
                    descriptor.minimum_duration_ms,
                    "long_press.minimum_duration_ms",
                )?;
                finite_nonnegative(descriptor.maximum_distance, "long_press.maximum_distance")?;
            }
            Self::Fling(descriptor) => {
                finite_positive(
                    descriptor.minimum_velocity_per_second,
                    "fling.minimum_velocity_per_second",
                )?;
                finite_nonnegative(descriptor.minimum_distance, "fling.minimum_distance")?;
            }
            Self::Pinch(descriptor) => {
                require_exactly_two(descriptor.common, "pinch.pointer_count")?;
                finite_positive(descriptor.minimum_scale_delta, "pinch.minimum_scale_delta")?;
            }
            Self::Rotation(descriptor) => {
                require_exactly_two(descriptor.common, "rotation.pointer_count")?;
                finite_positive(
                    descriptor.minimum_rotation_radians,
                    "rotation.minimum_rotation_radians",
                )?;
            }
        }
        Ok(self)
    }

    /// The variant tag, for callers that dispatch on recognizer kind without
    /// destructuring the descriptor.
    pub fn kind(self) -> RecognizerKind {
        match self {
            Self::Pan(_) => RecognizerKind::Pan,
            Self::Tap(_) => RecognizerKind::Tap,
            Self::LongPress(_) => RecognizerKind::LongPress,
            Self::Fling(_) => RecognizerKind::Fling,
            Self::Pinch(_) => RecognizerKind::Pinch,
            Self::Rotation(_) => RecognizerKind::Rotation,
        }
    }

    /// The identity and pointer-count fields, whichever variant this is.
    pub fn common(self) -> RecognizerCommon {
        match self {
            Self::Pan(value) => value.common,
            Self::Tap(value) => value.common,
            Self::LongPress(value) => value.common,
            Self::Fling(value) => value.common,
            Self::Pinch(value) => value.common,
            Self::Rotation(value) => value.common,
        }
    }

    pub(crate) fn set_generation(&mut self, generation: u32) {
        match self {
            Self::Pan(value) => value.common.generation = generation,
            Self::Tap(value) => value.common.generation = generation,
            Self::LongPress(value) => value.common.generation = generation,
            Self::Fling(value) => value.common.generation = generation,
            Self::Pinch(value) => value.common.generation = generation,
            Self::Rotation(value) => value.common.generation = generation,
        }
    }
}

/// Why an authored descriptor was rejected. Every variant names the offending
/// field so the failure can be reported against the authored source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GestureDescriptorError {
    /// An encoded enum field held a value with no corresponding variant.
    InvalidEnum {
        /// Dotted name of the field that failed to decode, such as `"recognizer.kind"`.
        field: &'static str,
        /// The unrecognized byte that was decoded.
        value: u8,
    },
    /// A field required to be non-zero was zero, such as a recognizer id or generation.
    ZeroIdentity(&'static str),
    /// A floating-point threshold was NaN or infinite.
    NonFinite(&'static str),
    /// A field was finite but outside the range its recognizer requires.
    OutOfRange(&'static str),
}

fn validate_common(common: RecognizerCommon) -> Result<(), GestureDescriptorError> {
    if common.id.0 == 0 {
        return Err(GestureDescriptorError::ZeroIdentity("recognizer.id"));
    }
    if common.generation == 0 {
        return Err(GestureDescriptorError::ZeroIdentity(
            "recognizer.generation",
        ));
    }
    if common.min_pointers == 0
        || common.max_pointers < common.min_pointers
        || common.max_pointers as usize > MAX_POINTERS_PER_STREAM
    {
        return Err(GestureDescriptorError::OutOfRange(
            "recognizer.pointer_count",
        ));
    }
    Ok(())
}

fn require_exactly_two(
    common: RecognizerCommon,
    field: &'static str,
) -> Result<(), GestureDescriptorError> {
    if common.min_pointers != 2 || common.max_pointers != 2 {
        Err(GestureDescriptorError::OutOfRange(field))
    } else {
        Ok(())
    }
}

fn finite_positive(value: f64, field: &'static str) -> Result<(), GestureDescriptorError> {
    if !value.is_finite() {
        Err(GestureDescriptorError::NonFinite(field))
    } else if value <= 0.0 {
        Err(GestureDescriptorError::OutOfRange(field))
    } else {
        Ok(())
    }
}

fn finite_nonnegative(value: f64, field: &'static str) -> Result<(), GestureDescriptorError> {
    if !value.is_finite() {
        Err(GestureDescriptorError::NonFinite(field))
    } else if value < 0.0 {
        Err(GestureDescriptorError::OutOfRange(field))
    } else {
        Ok(())
    }
}

fn validate_optional_offset(
    offset: Option<(f64, f64)>,
    field: &'static str,
) -> Result<(), GestureDescriptorError> {
    let Some((minimum, maximum)) = offset else {
        return Ok(());
    };
    if !minimum.is_finite() || !maximum.is_finite() {
        Err(GestureDescriptorError::NonFinite(field))
    } else if minimum > maximum {
        Err(GestureDescriptorError::OutOfRange(field))
    } else {
        Ok(())
    }
}

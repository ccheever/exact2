//! Pointer frames: the raw continuous-input records the recognizers consume.
//!
//! @ref LLP 0099#gesture-recognizers
//!
//! A frame is one host input sample for one stream: an ordered contact set at
//! a timestamp. Frame validation owns pointer-set identity, alias rejection,
//! and monotonic time, so no recognizer has to re-derive them.

/// Contacts one pointer stream may carry at once.
pub const MAX_POINTERS_PER_STREAM: usize = 16;

/// Stable identity of one authored recognizer. The compiler assigns it, and
/// every event that recognizer publishes carries it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GestureId(pub u64);

/// Host identity of one contact, stable for as long as that contact exists.
/// Zero is reserved to mean "absent" and is rejected during validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointerId(pub u64);

/// Identity of one continuous input stream, from its first contact through its
/// terminal frame. Recognizers and the arena fence on it: a frame naming a
/// different stream is rejected rather than merged into the current one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GestureStreamId(pub u64);

/// A position or vector in a two-dimensional space, measured in points. The
/// same type carries positions, deltas, and velocities; the field holding it
/// says which.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct MotionPoint {
    /// Horizontal component, in points, increasing to the right.
    pub x: f64,
    /// Vertical component, in points, increasing downward.
    pub y: f64,
}

impl MotionPoint {
    /// The origin, and equally the zero vector. Reported as the velocity when a
    /// tracker holds too few samples to fit one.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub(crate) fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    pub(crate) fn distance(self, other: Self) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    pub(crate) fn subtract(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

/// The sorted, deduplicated contact set behind one frame. Two frames belong to
/// the same gesture only when their pointer sets are equal, so recognizers
/// compare this value instead of re-deriving contact membership per frame.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointerSetIdentity {
    members: Vec<PointerId>,
}

impl PointerSetIdentity {
    /// Sort and validate `members` into a pointer set. Rejects an empty set, a
    /// zero pointer id, a repeated id, and more than
    /// [`MAX_POINTERS_PER_STREAM`] contacts, so any constructed value is a
    /// usable identity.
    pub fn new<I>(members: I) -> Result<Self, GestureInputError>
    where
        I: IntoIterator<Item = PointerId>,
    {
        let mut members: Vec<_> = members.into_iter().collect();
        members.sort_unstable();
        if members.is_empty() {
            return Err(GestureInputError::EmptyPointerSet);
        }
        if members.iter().any(|pointer| pointer.0 == 0) {
            return Err(GestureInputError::ZeroIdentity("pointer.id"));
        }
        if members.len() > MAX_POINTERS_PER_STREAM {
            return Err(GestureInputError::TooManyPointers {
                count: members.len(),
                maximum: MAX_POINTERS_PER_STREAM,
            });
        }
        if members.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(GestureInputError::DuplicatePointerId);
        }
        Ok(Self { members })
    }

    /// The member contacts in ascending id order. The order is canonical, so
    /// equal sets always compare equal.
    pub fn members(&self) -> &[PointerId] {
        &self.members
    }

    /// Number of contacts in the set. A validated set holds at least one.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Always false for a validated set; present so the type reads naturally
    /// beside `len`.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

/// What one frame reports about the stream's lifetime. The discriminants are
/// the wire encoding the host adapter sends, decoded through `TryFrom<u8>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PointerPhase {
    /// A contact went down, starting the stream or joining an existing one.
    Down = 1,
    /// The existing contacts moved; the pointer set is unchanged.
    Move = 2,
    /// A contact lifted. The stream ends normally once the last one has.
    Up = 3,
    /// The host revoked the stream. Recognizers terminate without publishing a
    /// completed outcome.
    Cancel = 4,
    /// A synthesized wake that re-delivers the current contacts at a later
    /// timestamp, so time-dependent recognizers such as long press can advance
    /// without any host movement.
    Timer = 5,
}

impl TryFrom<u8> for PointerPhase {
    type Error = GestureInputError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Down),
            2 => Ok(Self::Move),
            3 => Ok(Self::Up),
            4 => Ok(Self::Cancel),
            5 => Ok(Self::Timer),
            _ => Err(GestureInputError::InvalidEnum {
                field: "pointer.phase",
                value,
            }),
        }
    }
}

/// One contact within a frame, already validated: every coordinate is finite
/// and the pressure lies in range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerContact {
    /// Host identity of this contact, stable across the frames of one stream.
    pub id: PointerId,
    /// Position in the claimed presenter node's local coordinate space.
    pub position: MotionPoint,
    /// Position in the platform window's coordinate space.
    pub absolute_position: MotionPoint,
    /// Platform-provided delta since this pointer's previous sample.
    pub delta: MotionPoint,
    /// Normalized pressure in the closed [0, 1] range.
    pub pressure: f64,
}

/// One validated host input sample: an ordered contact set for one stream at
/// one timestamp. Construction is what establishes pointer-set identity, so no
/// recognizer has to derive it again.
#[derive(Debug, Clone, PartialEq)]
pub struct PointerFrame {
    /// The stream this sample belongs to. A recognizer rejects a frame whose
    /// stream differs from the one it started on.
    pub stream_id: GestureStreamId,
    /// What this sample reports about the stream's lifetime.
    pub phase: PointerPhase,
    /// Host sample time in milliseconds. Must be finite, and may not move
    /// backward within one stream.
    pub timestamp_ms: f64,
    contacts: Vec<PointerContact>,
    pointer_set: PointerSetIdentity,
}

impl PointerFrame {
    /// Validate one host sample into a frame. Contacts are sorted by id and
    /// their pointer-set identity derived here. Rejects a zero stream id, a
    /// non-finite timestamp or contact value, a pressure outside [0, 1], and
    /// any contact set [`PointerSetIdentity::new`] refuses.
    pub fn new(
        stream_id: GestureStreamId,
        phase: PointerPhase,
        timestamp_ms: f64,
        contacts: impl IntoIterator<Item = PointerContact>,
    ) -> Result<Self, GestureInputError> {
        if stream_id.0 == 0 {
            return Err(GestureInputError::ZeroIdentity("pointer.stream_id"));
        }
        if !timestamp_ms.is_finite() {
            return Err(GestureInputError::NonFinite("pointer.timestamp_ms"));
        }
        let mut contacts: Vec<_> = contacts.into_iter().collect();
        if contacts.iter().any(|contact| {
            !contact.position.is_finite()
                || !contact.absolute_position.is_finite()
                || !contact.delta.is_finite()
                || !contact.pressure.is_finite()
        }) {
            return Err(GestureInputError::NonFinite("pointer.contact"));
        }
        if contacts
            .iter()
            .any(|contact| !(0.0..=1.0).contains(&contact.pressure))
        {
            return Err(GestureInputError::OutOfRange("pointer.pressure"));
        }
        contacts.sort_unstable_by_key(|contact| contact.id);
        let pointer_set = PointerSetIdentity::new(contacts.iter().map(|contact| contact.id))?;
        Ok(Self {
            stream_id,
            phase,
            timestamp_ms,
            contacts,
            pointer_set,
        })
    }

    /// The frame's contacts in ascending pointer-id order.
    pub fn contacts(&self) -> &[PointerContact] {
        &self.contacts
    }

    /// Identity of this frame's contact set. Comparing it against the set a
    /// recognizer started on is how a mid-stream contact change is detected.
    pub fn pointer_set(&self) -> &PointerSetIdentity {
        &self.pointer_set
    }

    pub(crate) fn centroid(&self) -> MotionPoint {
        let count = self.contacts.len() as f64;
        MotionPoint {
            x: self
                .contacts
                .iter()
                .map(|contact| contact.position.x)
                .sum::<f64>()
                / count,
            y: self
                .contacts
                .iter()
                .map(|contact| contact.position.y)
                .sum::<f64>()
                / count,
        }
    }

    pub(crate) fn absolute_centroid(&self) -> MotionPoint {
        let count = self.contacts.len() as f64;
        MotionPoint {
            x: self
                .contacts
                .iter()
                .map(|contact| contact.absolute_position.x)
                .sum::<f64>()
                / count,
            y: self
                .contacts
                .iter()
                .map(|contact| contact.absolute_position.y)
                .sum::<f64>()
                / count,
        }
    }

    pub(crate) fn delta_centroid(&self) -> MotionPoint {
        let count = self.contacts.len() as f64;
        MotionPoint {
            x: self
                .contacts
                .iter()
                .map(|contact| contact.delta.x)
                .sum::<f64>()
                / count,
            y: self
                .contacts
                .iter()
                .map(|contact| contact.delta.y)
                .sum::<f64>()
                / count,
        }
    }

    pub(crate) fn mean_pressure(&self) -> f64 {
        self.contacts
            .iter()
            .map(|contact| contact.pressure)
            .sum::<f64>()
            / self.contacts.len() as f64
    }

    pub(crate) fn two_pointer_geometry(&self) -> Option<(f64, f64, MotionPoint)> {
        let [first, second] = self.contacts.as_slice() else {
            return None;
        };
        let dx = second.position.x - first.position.x;
        let dy = second.position.y - first.position.y;
        Some(((dx * dx + dy * dy).sqrt(), dy.atan2(dx), self.centroid()))
    }
}
/// Why a host sample, tracker configuration, or recognizer transition was
/// refused. Every variant is a rejection at the boundary: the offending input
/// never reaches recognizer state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GestureInputError {
    /// A wire enum byte carried no defined meaning.
    InvalidEnum {
        /// Dotted name of the field that carried the byte.
        field: &'static str,
        /// The undecodable byte, as received.
        value: u8,
    },
    /// An identity field was zero, which is reserved to mean "absent". Carries
    /// the name of the field.
    ZeroIdentity(&'static str),
    /// A coordinate, timestamp, or pressure was NaN or infinite. Carries the
    /// name of the field.
    NonFinite(&'static str),
    /// A value was finite but outside its defined range, such as a pressure
    /// beyond [0, 1]. Carries the name of the field.
    OutOfRange(&'static str),
    /// A pointer set was built with no members; every frame carries at least
    /// one contact.
    EmptyPointerSet,
    /// More simultaneous contacts than one stream may carry.
    TooManyPointers {
        /// Contacts the caller offered.
        count: usize,
        /// The cap that was exceeded, [`MAX_POINTERS_PER_STREAM`].
        maximum: usize,
    },
    /// The same pointer id appeared twice in one contact set.
    DuplicatePointerId,
    /// A sample's timestamp preceded the previous sample's on the same stream.
    /// Time is monotonic per stream, so the frame is refused, never reordered.
    TimestampMovedBackward,
    /// A frame arrived for a stream other than the one this recognizer or
    /// tracker was started on. Streams are fenced, never merged.
    StreamChanged,
    /// The frame's phase is not one the recognizer's current state accepts.
    InvalidTransition,
    /// The recognizer already reached a terminal state and must be reset before
    /// it will process another frame.
    TerminalState,
    /// A velocity tracker was asked for a sample cap outside the supported
    /// range of 2 through `VELOCITY_SAMPLE_CAP`.
    InvalidCapacity,
    /// A velocity tracker was asked for a horizon that was not positive or that
    /// exceeded `VELOCITY_HORIZON_MS`. The constant is a ceiling, not a default.
    InvalidHorizon,
    /// A monotonic sequence counter ran out of values. The stream is abandoned
    /// rather than wrapped, so ordering stays total.
    SequenceExhausted,
}

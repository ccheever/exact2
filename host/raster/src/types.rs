//! Fixed native-image limits and integer metadata; no source strings or bytes.

/// Unique decoded allocations plus in-flight output/scratch/copy reservations,
/// for a session made without a budget of its own; one decode's peak, always.
pub const SESSION_BYTES: u64 = 32 * 1024 * 1024;
pub const RUNNING_DECODES: usize = 2;
pub const DELIVERY_CELLS: usize = 2;
/// Includes queued, running, ready-but-undelivered, and failed requests.
pub const PENDING_JOBS: usize = 64;
/// Live subscriptions plus detached cache entries pinned by external leases.
pub const SUBSCRIPTIONS: usize = 1024;
pub const COLD_ENTRIES: usize = 256;
pub const MAX_ENCODED_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_HEADER_BYTES: u64 = 256 * 1024;
pub const MAX_SOURCE_PIXELS: u64 = 64 * 1024 * 1024;
/// A `data:` image source's bound, in bytes of URL text, on every host (LLP
/// 1011 §2): small generated pictures, not photos (those are `app:/` files).
/// Apple's `RasterInput.dataLimit` and the web hosts' `DATA_LIMIT` are this.
pub const MAX_DATA_URL_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}

/// Source IDs belong to the adapter's bounded live/job/cache resolver mapping.
/// Never intern every visited source. `generation` identifies immutable bytes.
/// `variant` identifies the adapter's fixed orientation/color decoding policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RasterKey {
    pub source: u64,
    pub generation: u64,
    pub pixels: PixelSize,
    pub variant: u32,
}

/// A native view identity including its incarnation; cancellation is exact.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ViewKey {
    pub view: u64,
    pub generation: u64,
}

/// Natural source pixels remain layout units independently of decode size/DPR.
/// The adapter enforces encoded/header limits BEFORE loading/parsing; these
/// fields let admission reject unsupported work again before decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Metadata {
    pub natural: PixelSize,
    pub encoded_bytes: u64,
    pub header_bytes: u64,
}

/// Peak concurrent storage. `copy_bytes` is a separate physical allocation,
/// not another reference to the output. The native decoder must honor these
/// bounds during allocation, not discover an excess only at completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeCost {
    pub stride: u64,
    pub height: u32,
    pub output_bytes: u64,
    pub scratch_bytes: u64,
    pub copy_bytes: u64,
}

impl DecodeCost {
    /// Both native adapters normalize to premultiplied RGBA8. Admission checks
    /// stride >= requested width * 4 and output bytes == stride * height.
    pub fn checked(stride: u64, height: u32, scratch: u64, copy: u64) -> Result<Self, Refusal> {
        let output_bytes = stride
            .checked_mul(u64::from(height))
            .ok_or(Refusal::Overflow)?;
        let cost = Self {
            stride,
            height,
            output_bytes,
            scratch_bytes: scratch,
            copy_bytes: copy,
        };
        cost.peak()?;
        if stride == 0 || height == 0 {
            return Err(Refusal::InvalidDimensions);
        }
        Ok(cost)
    }

    pub fn peak(self) -> Result<u64, Refusal> {
        self.output_bytes
            .checked_add(self.scratch_bytes)
            .and_then(|n| n.checked_add(self.copy_bytes))
            .ok_or(Refusal::Overflow)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Priority {
    Visible,
    Overscan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Demand {
    pub view: ViewKey,
    pub key: RasterKey,
    pub metadata: Metadata,
    pub cost: DecodeCost,
    pub priority: Priority,
}

/// Opaque, process-unique ID; native FFI preserves all 64 bits.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RequestId(pub(crate) u64);
impl RequestId {
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    Overflow,
    InvalidDimensions,
    EncodedLimit,
    HeaderLimit,
    SourcePixels,
    TooLarge,
    Budget,
    QueueFull,
    SubscriberLimit,
    ConflictingMetadata,
    Paused,
    Shutdown,
    Stale,
    ActualExceedsReservation,
    DecodeFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestStatus {
    Queued,
    WaitingBudget,
    Decoding,
    Ready,
    Failed(Refusal),
}

/// Actual retained allocations at completion; scratch is no longer alive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidentBytes {
    pub output: u64,
    pub copy: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Stats {
    pub resident_bytes: u64,
    pub reserved_bytes: u64,
    pub pinned_bytes: u64,
    /// Unpinned cache candidates, not a promise that opaque native owners drop.
    pub cold_bytes: u64,
    /// Allocations still retained after the core cache/leases have retired.
    pub retiring_bytes: u64,
    pub peak_bytes: u64,
    pub queued: usize,
    pub running: usize,
    pub ready: usize,
    pub delivery_cells: usize,
    pub pending_jobs: usize,
    pub subscribers: usize,
    pub cold_entries: usize,
    pub dedup_hits: u64,
    pub cancelled: u64,
    pub evicted: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GateStats {
    pub running: usize,
    pub ready: usize,
    pub sessions: usize,
}

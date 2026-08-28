//! The motion tick-phase order.
//!
//! @ref RFC 0492 (one clock)
//!
//! Every tick source — a display link, a test's virtual clock, an agent
//! advancing time — runs these phases in this order, exactly once each. The
//! order is the contract: input is sampled before the arena checkpoints,
//! arbitration settles before drivers advance, and bindings apply only after
//! every value for the frame is final.

/// One phase of a motion tick, in execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionTickPhase {
    /// Host input samples become pointer frames and value commands apply.
    InputSampling,
    /// The gesture arena re-reads eligibility and settles claims.
    ArenaCheckpoint,
    /// Active drivers advance by the frame's delta and publish samples.
    DriverAdvance,
    /// Derived values recompute from the values published this frame.
    DerivedValues,
    /// Published values reach their `transform` and `opacity` sinks.
    BindingApplication,
    /// The frame is handed to the presenter.
    Present,
    /// Frame receipts and evidence are finalized.
    Observability,
    /// Host-owned work that must not observe a half-applied frame.
    Auxiliary,
}

/// The tick phases in the order every tick source must run them.
pub const MOTION_TICK_ORDER: [MotionTickPhase; 8] = [
    MotionTickPhase::InputSampling,
    MotionTickPhase::ArenaCheckpoint,
    MotionTickPhase::DriverAdvance,
    MotionTickPhase::DerivedValues,
    MotionTickPhase::BindingApplication,
    MotionTickPhase::Present,
    MotionTickPhase::Observability,
    MotionTickPhase::Auxiliary,
];

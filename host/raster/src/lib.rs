//! Native raster ownership (LLP 1010 §6.3), shared by Apple and Linux.
//!
//! Keep one [`RasterSession`] across active/candidate/retiring app generations.
//! Native codecs and the two process workers remain in the adapters. Requests
//! contain metadata only. [`Gate`] admits at most two RUNNING decodes globally
//! and reserves one of two delivery cells in the requesting session. Completion
//! returns the running slot; its charged result keeps the session cell until
//! taken or cancelled. Another session never waits for this session's UI.
//!
//! A backing/provider must retain [`AllocationCharge`] before allocating. That
//! charge owns only an independent budget account, never a payload or runtime.
//! [`RasterLease`] pairs an opaque native payload with the charge; native views,
//! providers and GPU copies may outlive it. A distinct copy needs a distinct
//! reservation. Native scratch MUST be destroyed before completing/dropping a
//! permit. Opaque payload destruction must be safe on a worker thread.

mod account;
mod policy;
mod types;

pub use account::{AllocationCharge, AllocationReservation};
pub use policy::{DecodePermit, Gate, RasterLease, RasterSession};
pub use types::*;

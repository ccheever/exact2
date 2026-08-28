//! The continuous-input path: pointer frames in, arbitrated gesture events out.
//!
//! @ref LLP 0099#gesture-recognizers
//! @ref LLP 0099#gesture-composition
//! @ref LLP 0099#velocity-tracking
//!
//! Recognition is deterministic and owned here, not by the host. Platform code
//! is a sample adapter: it reports contacts and geometry. This module owns the
//! state transitions, pointer-set identity, velocity provenance, frozen claim
//! topology, checkpointed eligibility, and terminal outcome classification.
//!
//! The pieces compose in one direction:
//!
//! - [`pointer`] validates host samples into [`PointerFrame`]s.
//! - [`velocity`] estimates pointer velocity with typed provenance.
//! - [`descriptor`] is the authored plan data for one recognizer.
//! - [`recognizer`] runs the per-descriptor state machine.
//! - [`claim`] declares who may win a stream, and [`arena`] decides.
//! - [`state`] joins the separately published facts arbitration reads.
//! - [`composition`] relates several recognizers on one node.
//! - [`graph`] builds a plan into a runnable topology, and [`controller`]
//!   runs it.

pub mod arena;
pub mod claim;
pub mod composition;
pub mod controller;
pub mod descriptor;
pub mod graph;
pub mod pointer;
pub mod publication;
pub mod recognizer;
pub mod state;
pub mod velocity;

pub use arena::*;
pub use claim::*;
pub use composition::*;
pub use controller::*;
pub use descriptor::*;
pub use graph::*;
pub use pointer::*;
pub use recognizer::*;
pub use state::*;
pub use velocity::*;

//! The motion evaluator.
//!
//! @ref RFC 0492 (motion refresh program)
//! @ref LLP 0099 (motion)
//! @ref RFC 0100 (interactive navigation transitions)
//!
//! One crate evaluates motion on every surface. Animation is plan data — typed
//! Rust records a compiler emits ([`plan`]) — and this crate advances it:
//!
//! - [`shared_value`] is the value plane: single-word values with typed handles.
//! - [`driver`] is the closed-form evaluator: springs, timing curves, decays.
//! - [`clock`] advances the whole thing in fixed steps, so time is seekable.
//! - [`gesture`] is the continuous-input path: frames, recognizers, the arena.
//! - [`navigation`] is the interactive-navigation model drag-back depends on.
//! - [`math`] pins the transcendentals, so the same plan yields the same bits.
//! - [`tick`] fixes the phase order every tick source runs.
//!
//! No JavaScript runs on the frame path, and nothing here delegates to a second
//! executor: there is one evaluator, and its sinks are `transform` and
//! `opacity`.

#![deny(missing_docs)]

pub mod clock;
pub mod driver;
pub mod gesture;
pub mod math;
pub mod navigation;
pub mod plan;
pub mod shared_value;
pub mod tick;

//! The motion evaluator.
//!
//! One representation, two executors.
//!
//! Motion is CSS's `transition` model. A property's **target** is its style
//! value in the kernel; a `transition` row on the node says how the property
//! gets there; the **presentation** value is what the host paints this frame.
//! There is no second value graph, no shared-value plane, and nothing to bind:
//! the style row *is* the binding.
//!
//! Two executors run the one representation. On the web the browser is the
//! executor — the host emits the row as CSS and the compositor interpolates.
//! Everywhere the platform has no such engine, this crate is the executor:
//!
//! - [`property`] — compositor properties and the explicit numeric-height trial.
//! - [`animation`] — CSS keyframe animations, their timing and their values.
//! - [`easing`] — CSS easing functions, held to the browser's outputs.
//! - [`spring`] — the one timing function CSS lacks, and its lowering to
//!   keyframes so the web plays the same curve.
//! - [`transition`] — the declaration, and CSS's rules for starting,
//!   interrupting, and reversing a transition.
//! - [`path`] — a path's `d` as a value, and when two interpolate.
//! - [`color`] — CSS colours as premultiplied values.
//! - [`animation`] — CSS `@keyframes` and `animation`, sampled in closed form.
//! - [`engine`] — per-node presentation state under a seekable clock.
//! - [`uikit`] — UIKit's spring parameterisations, resolved to the springs
//!   UIKit builds.
//! - [`velocity`] — a pointer-velocity estimate for hosts without one.
//! - [`math`] — pinned transcendentals, so the same input yields the same bits.
//!
//! Time is a number the host supplies. Every value is a closed-form function
//! of that number, so advancing the clock to `t` in one step or in sixty gives
//! the same bits — a test seeks, an agent seeks, nobody waits for a settle.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod animation;
pub mod color;
pub mod easing;
pub mod engine;
pub mod gesture;
pub mod math;
pub mod parse;
pub mod path;
pub mod property;
pub mod spring;
pub mod transition;
pub mod uikit;
pub mod velocity;

pub use animation::link as link_animations;
pub use animation::{
    Animation, AnimationError, Animations, Direction, FillMode, Keyframes, Phase, MAX_ANIMATIONS,
    MAX_KEYFRAMES, MAX_VALUES,
};
pub use easing::{Easing, EasingError, LinearStop, StepPosition};
pub use engine::{
    AnimationPlay, Change, Engine, EngineError, EngineLinks, HoldEnd, HoldStart, HoldToken,
    NamedTimeline, PlayedCurve, PlayedTransition, Presentation, SpringDescriptor, SpringFrames,
    TransformHold, TransitionEnds,
};
pub use parse::ParseError;
pub use path::{PathCommand, PathValue};
pub use property::{Property, Value};
pub use spring::{Keyframe, SpringConfig, SpringError, SpringSample};
pub use transition::{
    TimingFunction, Transition, TransitionError, TransitionProperty, Transitions, MAX_TRANSITIONS,
};
pub use velocity::VelocityTracker;

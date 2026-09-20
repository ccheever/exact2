//! Saved scalar motion. Callers declare animation work through World::work.
#![forbid(unsafe_code)]
mod spring;
mod tween;
pub use spring::{Spring, SpringConfig};
pub use tween::Tween;

/// Cubic interpolation between distinct increasing edges.
pub fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

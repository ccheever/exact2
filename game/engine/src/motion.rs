//! Named kinematics in metres and seconds. `Move` approaches the XZ stick's
//! target velocity (speed in m/s) at `accel` m/s², or `brake` m/s² when released,
//! clamping stick length to one and arriving exactly without overshoot. Y is
//! untouched. `Jump` sets upward velocity to sqrt(2 × gravity × height), with
//! height in metres and positive downward gravity in m/s²; `Gravity` subtracts
//! gravity × dt from Y. Steps take seconds. The game owns grounding and position
//! integration (use v × dt − gravity × dt² / 2 for a ballistic hop).
use crate::{math, Vec3};

/// Accelerate toward a planar wish velocity; brake to an exact stop on release.
#[derive(Clone, Copy, Debug)]
pub struct Move {
    /// Maximum planar speed in metres per second.
    pub speed: f32,
    /// Acceleration toward held input in metres per second squared.
    pub accel: f32,
    /// Deceleration with released input in metres per second squared.
    pub brake: f32,
}
impl Move {
    /// Step XZ velocity, preserving Y and the magnitude of an analog stick.
    pub fn step(self, velocity: &mut Vec3, wish: Vec3, dt: f32) {
        for value in [self.speed, self.accel, self.brake, dt] {
            assert!(value.is_finite() && value >= 0.0);
        }
        assert!(wish.x.is_finite() && wish.z.is_finite());
        let wish = Vec3::new(wish.x, 0.0, wish.z).clamp_length_max(1.0);
        let target = wish * self.speed;
        let current = Vec3::new(velocity.x, 0.0, velocity.z);
        let delta = target - current;
        let distance = delta.length();
        let step = if wish == Vec3::ZERO {
            self.brake
        } else {
            self.accel
        } * dt;
        let next = if distance <= step {
            target
        } else {
            current + delta * (step / distance)
        };
        velocity.x = next.x;
        velocity.z = next.z;
    }
}

/// A ballistic jump specified by its height above the starting position.
#[derive(Clone, Copy, Debug)]
pub struct Jump {
    /// Rise in metres.
    pub height: f32,
    /// Positive downward acceleration in metres per second squared.
    pub gravity: f32,
}
impl Jump {
    /// Start a hop, preserving XZ velocity. The caller decides whether grounded.
    pub fn start(self, velocity: &mut Vec3) {
        assert!(self.height.is_finite() && self.height >= 0.0);
        assert!(self.gravity.is_finite() && self.gravity > 0.0);
        velocity.y = math::sqrt(2.0 * self.gravity * self.height);
    }
}

/// Positive downward acceleration in metres per second squared.
#[derive(Clone, Copy, Debug)]
pub struct Gravity(pub f32);
impl Gravity {
    /// Step vertical velocity for `dt` seconds, preserving XZ.
    pub fn step(self, velocity: &mut Vec3, dt: f32) {
        assert!(self.0.is_finite() && self.0 >= 0.0 && dt.is_finite() && dt >= 0.0);
        velocity.y -= self.0 * dt;
    }
}

//! A saved, kinematic character without colliders or a physics dependency.
use crate::motion::{Gravity, Jump, Move};
use crate::{Data, Transform, Vec3};
use std::ops::RangeInclusive;

/// Movement configuration and velocity, embedded in a game's player component.
#[derive(Clone, Debug, Data)]
pub struct Character {
    /// Current velocity in metres per second; saved and hashed with the component.
    pub velocity: Vec3,
    speed: f32,
    accel: f32,
    brake: f32,
    jump: f32,
    gravity: f32,
    ground: f32,
    bounds: Option<[f32; 2]>,
}
/// Events from one step. Grounded describes the end of the step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Contact {
    /// At the ground height with no vertical velocity.
    pub grounded: bool,
    /// Accepted a jump from the ground this step.
    pub jumped: bool,
    /// Reached the ground from the air this step.
    pub landed: bool,
}
impl Default for Character {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            speed: 4.0,
            accel: 12.0,
            brake: 20.0,
            jump: 1.2,
            gravity: 9.81,
            ground: 0.0,
            bounds: None,
        }
    }
}
impl Character {
    /// Four metres/second, acceleration 12, braking 20, jump 1.2m, gravity 9.81.
    /// Ground is zero; XZ is unbounded until bounds_xz is supplied.
    pub fn new() -> Self {
        Self::default()
    }
    /// Planar speed in metres per second.
    pub fn speed(mut self, value: f32) -> Self {
        assert!(value.is_finite() && value >= 0.0);
        self.speed = value;
        self
    }
    /// Acceleration in metres per second squared.
    pub fn accel(mut self, value: f32) -> Self {
        assert!(value.is_finite() && value >= 0.0);
        self.accel = value;
        self
    }
    /// Braking in metres per second squared, arriving at zero exactly.
    pub fn brake(mut self, value: f32) -> Self {
        assert!(value.is_finite() && value >= 0.0);
        self.brake = value;
        self
    }
    /// Ballistic height above the ground in metres.
    pub fn jump(mut self, value: f32) -> Self {
        assert!(value.is_finite() && value >= 0.0);
        self.jump = value;
        self
    }
    /// Positive downward acceleration in metres per second squared.
    pub fn gravity(mut self, value: f32) -> Self {
        assert!(value.is_finite() && value > 0.0);
        self.gravity = value;
        self
    }
    /// Ground contact height of the pose's origin, in metres.
    pub fn ground(mut self, value: f32) -> Self {
        assert!(value.is_finite());
        self.ground = value;
        self
    }
    /// Inclusive bounds for both X and Z; outward velocity stops at the edge.
    pub fn bounds_xz(mut self, range: RangeInclusive<f32>) -> Self {
        let (min, max) = range.into_inner();
        assert!(min.is_finite() && max.is_finite() && min <= max);
        self.bounds = Some([min, max]);
        self
    }
    /// Move the pose by dt seconds. Jump is a press edge; air presses are ignored.
    /// Uses Move, Jump and Gravity, with ballistic vertical integration and exact
    /// ground/bounds arrival. A zero-duration step has no effect or events.
    pub fn step(&mut self, pose: &mut Transform, wish: Vec3, jump: bool, dt: f32) -> Contact {
        assert!(dt.is_finite() && dt >= 0.0);
        let grounded = pose.position.y <= self.ground && self.velocity.y <= 0.0;
        if dt == 0.0 {
            return Contact {
                grounded,
                ..Contact::default()
            };
        }
        Move {
            speed: self.speed,
            accel: self.accel,
            brake: self.brake,
        }
        .step(&mut self.velocity, wish, dt);
        if grounded {
            pose.position.y = self.ground;
            self.velocity.y = 0.0;
        }
        let jumped = grounded && jump && self.jump > 0.0;
        if jumped {
            Jump {
                height: self.jump,
                gravity: self.gravity,
            }
            .start(&mut self.velocity);
        }
        pose.position.x += self.velocity.x * dt;
        pose.position.z += self.velocity.z * dt;
        let airborne = !grounded || jumped;
        let mut landed = false;
        if airborne {
            pose.position.y += self.velocity.y * dt - 0.5 * self.gravity * dt * dt;
            Gravity(self.gravity).step(&mut self.velocity, dt);
            if pose.position.y <= self.ground {
                pose.position.y = self.ground;
                self.velocity.y = 0.0;
                landed = true;
            }
        }
        if let Some([min, max]) = self.bounds {
            for axis in [0, 2] {
                pose.position[axis] = pose.position[axis].clamp(min, max);
                if (pose.position[axis] == min && self.velocity[axis] < 0.0)
                    || (pose.position[axis] == max && self.velocity[axis] > 0.0)
                {
                    self.velocity[axis] = 0.0;
                }
            }
        }
        Contact {
            grounded: pose.position.y == self.ground && self.velocity.y == 0.0,
            jumped,
            landed,
        }
    }
}

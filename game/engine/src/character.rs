//! A saved, kinematic character without colliders or a physics dependency.
use crate::motion::{Gravity, Jump, Move};
use crate::{Component, Target, Transform, Vec3, World};
use std::ops::RangeInclusive;

/// Movement configuration and velocity, attached directly to a named entity.
#[derive(Clone, Debug, Component)]
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
    airborne: bool,
}
/// Events from one step. Grounded describes the end of the step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    /// Actual displacement, including bounds and ground corrections.
    pub displacement: Vec3,
    /// At the ground height with no vertical velocity.
    pub grounded: bool,
    /// Accepted a jump from the ground this step.
    pub jumped: bool,
    /// Entered ground contact this step, including external pose or ground edits.
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
            airborne: false,
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
    /// ground/bounds arrival. A zero-duration step only queries actual contact;
    /// it neither corrects penetration nor changes state or emits events.
    pub fn step(&mut self, pose: &mut Transform, wish: Vec3, jump: bool, dt: f32) -> Contact {
        assert!(dt.is_finite() && dt >= 0.0);
        let before = pose.position;
        let grounded = pose.position.y <= self.ground && self.velocity.y <= 0.0;
        if dt == 0.0 {
            return Contact {
                grounded: pose.position.y == self.ground && self.velocity.y == 0.0,
                ..Contact::default()
            };
        }
        Move {
            speed: self.speed,
            accel: self.accel,
            brake: self.brake,
        }
        .step(&mut self.velocity, wish, dt);
        let mut landed =
            grounded && (self.airborne || pose.position.y < self.ground || self.velocity.y < 0.0);
        // Correct penetration without cancelling an upward velocity.
        pose.position.y = pose.position.y.max(self.ground);
        if grounded {
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
                pose.position[axis] = crate::math::clamp(pose.position[axis], min, max);
                if (pose.position[axis] == min && self.velocity[axis] < 0.0)
                    || (pose.position[axis] == max && self.velocity[axis] > 0.0)
                {
                    self.velocity[axis] = 0.0;
                }
            }
        }
        self.airborne = pose.position.y != self.ground || self.velocity.y != 0.0;
        Contact {
            displacement: pose.position - before,
            grounded: !self.airborne,
            jumped,
            landed,
        }
    }
}

/// A named character and its world timestep; no component borrow escapes a step.
pub struct CharacterHandle<'a> {
    world: &'a mut World,
    entity: crate::Entity,
}
impl World {
    /// Address a required Character + Transform. Missing components are setup errors.
    ///
    /// A component lease must end before addressing a character.
    /// ```compile_fail
    /// use exact_game::{World, Transform, character::Character, Vec3};
    /// let mut w = World::new(60, 7);
    /// w.spawn_named("player", (Transform::default(), Character::new()));
    /// let pose = w.get::<Transform>("player").unwrap();
    /// w.character("player").step(Vec3::ZERO, false);
    /// println!("{:?}", pose.position);
    /// ```
    pub fn character(&mut self, target: impl Target) -> CharacterHandle<'_> {
        let label = target.label();
        let entity = target.entity(self).unwrap_or_else(|| panic!("character target `{label}` does not exist; `tree world` lists names; add `w.spawn_named` in setup"));
        assert!(!self.has_component_named(entity, "CapsuleController"),
            "character target `{label}` has both Character and CapsuleController; pick one movement controller");
        assert!(
            self.has::<Character>(entity),
            "character target has no Character; add `Character::new()` to its `w.spawn_named` tuple in setup; inspect `state world:{label}`"
        );
        assert!(
            self.has::<Transform>(entity),
            "character target has no Transform; add `Transform::default()` to its `w.spawn_named` tuple in setup; inspect `state world:{label}`"
        );
        CharacterHandle {
            world: self,
            entity,
        }
    }
}
impl CharacterHandle<'_> {
    /// Step with the world's fixed dt, returning contact events for this tick.
    pub fn step(self, wish: Vec3, jump: bool) -> Contact {
        self.world.get_mut::<Character>(self.entity).unwrap().step(
            &mut self.world.get_mut::<Transform>(self.entity).unwrap(),
            wish,
            jump,
            self.world.dt(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default, crate::Component)]
    struct Beacon {
        lit: bool,
    }
    #[test]
    #[should_panic(expected = "world:ranger")]
    fn missing_component_names_requested_target() {
        let mut w = World::new(60, 7);
        w.spawn_named("ranger", Transform::default());
        let _ = w.character("ranger");
    }
    #[test]
    fn named_step_matches_explicit_step_and_releases_borrows() {
        let mut w = World::new(60, 7);
        w.spawn_named(
            "player",
            (Transform::at(0.0, 0.9, 0.0), Character::new().ground(0.9)),
        );
        let mut expected = Character::new().ground(0.9);
        let mut pose = Transform::at(0.0, 0.9, 0.0);
        for tick in 0..120 {
            let wish = if tick < 90 { -Vec3::Z } else { Vec3::ZERO };
            let jump = tick == 30;
            assert_eq!(
                w.character("player").step(wish, jump),
                expected.step(&mut pose, wish, jump, w.dt())
            );
            assert_eq!(w.global_position("player"), Some(pose.position));
        }
        assert_eq!(
            w.get::<Character>("player").unwrap().velocity,
            expected.velocity
        );
    }
    #[test]
    #[should_panic(expected = "world:player")]
    fn missing_character_is_a_setup_error() {
        let mut w = World::new(60, 7);
        w.spawn_named("player", Transform::default());
        w.character("player");
    }
    #[test]
    fn nearest_uses_current_global_pose_and_stable_ties() {
        let mut w = World::new(60, 7);
        let parent = w.spawn(Transform::at(10.0, 0.0, 0.0));
        w.spawn_named(
            "player",
            (crate::Parent(parent), Transform::at(1.0, 0.0, 0.0)),
        );
        let a = w.spawn((Transform::at(10.0, 90.0, 0.0), Beacon::default()));
        let b = w.spawn((Transform::at(12.0, 0.0, 0.0), Beacon::default()));
        assert_eq!(w.global_position("player"), Some(Vec3::new(11.0, 0.0, 0.0)));
        assert_eq!(w.local_position("player"), Some(Vec3::X));
        assert_eq!(
            w.nearest_xz_where::<Beacon>("player", 1.0, |_| true),
            Some(a)
        );
        assert!(w
            .nearest_xz_where::<Beacon>("missing", 1.0, |_| true)
            .is_none());
        assert!(w
            .nearest_xz_where::<Beacon>("player", 0.9, |_| true)
            .is_none());
        w.get_mut::<Beacon>(a).unwrap().lit = true;
        w.get_mut::<Transform>(b).unwrap().position.x = 12.4;
        assert_eq!(
            w.nearest_xz_where::<Beacon>("player", 1.5, |b| !b.lit),
            Some(b)
        );
        w.get_mut::<Transform>(parent).unwrap().position.x = 20.0;
        assert!(w
            .nearest_xz_where::<Beacon>("player", 1.0, |_| true)
            .is_none());
    }
}

#[cfg(test)]
mod restart_edge_regression {
    use crate::{Game, Input, Sim, Value, World};
    #[derive(Default, crate::Args)]
    struct Options {
        seed: u64,
        #[restart]
        again: bool,
    }
    struct Example;
    impl Game for Example {
        type Args = Options;
        const ID: &'static str = "restart-edges";
        fn setup(w: &mut World, args: &Options) {
            w.reseed(args.seed);
        }
        fn tick(_: &mut World, _: &Input, _: &Options) {}
    }
    #[test]
    fn setup_changes_are_not_restart_edges() {
        let mut sim = Sim::<Example>::new(Options::default()).unwrap();
        sim.bind(&[Value::Number(1.), Value::Bool(false)], None)
            .unwrap();
        assert_eq!(sim.restarted, 0);
        for (edge, count) in [(true, 1), (false, 2)] {
            sim.bind(&[Value::Number(1.), Value::Bool(edge)], None)
                .unwrap();
            assert_eq!(sim.restarted, count);
        }
    }
    #[test]
    fn restart_counter_survives_carry_and_paranoid_reconstruction() {
        for mode in [
            crate::Paranoid::Off,
            crate::Paranoid::Save,
            crate::Paranoid::FreshGame,
        ] {
            let mut sim = Sim::<Example>::new(Options::default())
                .unwrap()
                .paranoid(mode);
            sim.bind(&[Value::Number(0.), Value::Bool(true)], None)
                .unwrap();
            let save = sim.save().unwrap();
            sim.restore_bound(&save).unwrap();
            assert_eq!(sim.restarted, 1, "carry");
            sim.advance(0., crate::Clock::Seekable);
            sim.advance(100., crate::Clock::Seekable);
            assert_eq!(sim.restarted, 1, "paranoid {mode:?}");
        }
    }
}

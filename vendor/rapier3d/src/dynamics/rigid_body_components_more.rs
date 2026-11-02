#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Copy, PartialEq)]
/// The user-defined external forces applied to this rigid-body.
pub struct RigidBodyForces {
    /// Accumulation of external forces (only for dynamic bodies).
    pub force: Vector,
    /// Accumulation of external torques (only for dynamic bodies).
    pub torque: AngVector,
    /// Gravity is multiplied by this scaling factor before it's
    /// applied to this rigid-body.
    pub gravity_scale: Real,
    /// Forces applied by the user.
    pub user_force: Vector,
    /// Torque applied by the user.
    pub user_torque: AngVector,
    /// Are gyroscopic forces enabled for this rigid-body?
    #[cfg(feature = "dim3")]
    pub gyroscopic_forces_enabled: bool,
}

impl Default for RigidBodyForces {
    fn default() -> Self {
        #[cfg(feature = "dim2")]
        return Self {
            force: Vector::ZERO,
            torque: 0.0,
            gravity_scale: 1.0,
            user_force: Vector::ZERO,
            user_torque: 0.0,
        };

        #[cfg(feature = "dim3")]
        return Self {
            force: Vector::ZERO,
            torque: AngVector::ZERO,
            gravity_scale: 1.0,
            user_force: Vector::ZERO,
            user_torque: AngVector::ZERO,
            gyroscopic_forces_enabled: true,
        };
    }
}

impl RigidBodyForces {
    /// Integrate these forces to compute new velocities.
    #[must_use]
    pub fn integrate(
        &self,
        dt: Real,
        init_vels: &RigidBodyVelocity<Real>,
        mprops: &RigidBodyMassProps,
    ) -> RigidBodyVelocity<Real> {
        let linear_acc = self.force * mprops.effective_inv_mass;
        let angular_acc = mprops.effective_world_inv_inertia * self.torque;

        RigidBodyVelocity {
            linvel: init_vels.linvel + linear_acc * dt,
            angvel: init_vels.angvel + angular_acc * dt,
        }
    }

    /// Adds to `self` the gravitational force that would result in a gravitational acceleration
    /// equal to `gravity`.
    pub fn compute_effective_force_and_torque(&mut self, gravity: Vector, mass: Vector) {
        self.force = self.user_force + gravity * mass * self.gravity_scale;
        self.torque = self.user_torque;
    }

    /// Applies a force at the given world-space point of the rigid-body with the given mass properties.
    pub fn apply_force_at_point(
        &mut self,
        rb_mprops: &RigidBodyMassProps,
        force: Vector,
        point: Vector,
    ) {
        self.user_force += force;
        self.user_torque += (point - rb_mprops.world_com).gcross(force);
    }
}

#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Copy, PartialEq)]
/// Information used for Continuous-Collision-Detection.
pub struct RigidBodyCcd {
    /// The distance used by the CCD solver to decide if a movement would
    /// result in a tunnelling problem.
    pub ccd_thickness: Real,
    /// Is CCD active for this rigid-body?
    ///
    /// Set automatically for any **dynamic** body moving fast enough to tunnel (regardless of
    /// `self.ccd_enabled`): it then sweeps fixed colliders, or all bodies if `ccd_enabled` is set too.
    pub ccd_active: bool,
    /// Is full ("bullet") CCD enabled for this rigid-body?
    ///
    /// Fast dynamic bodies always sweep *fixed* colliders; `true` upgrades this body to also
    /// sweep kinematic and dynamic bodies.
    pub ccd_enabled: bool,
    /// The soft-CCD prediction distance for this rigid-body.
    pub soft_ccd_prediction: Real,
    /// Allow this body to exceed the angular speed cap.
    ///
    /// By default angular velocity is clamped each substep to ~45°/step to keep CCD reliable;
    /// set `true` for bodies that must spin fast (e.g. wheels).
    pub allow_fast_rotation: bool,
}

impl Default for RigidBodyCcd {
    fn default() -> Self {
        Self {
            ccd_thickness: Real::MAX,
            ccd_active: false,
            ccd_enabled: false,
            soft_ccd_prediction: 0.0,
            allow_fast_rotation: false,
        }
    }
}

impl RigidBodyCcd {
    /// The maximum velocity any point of any collider attached to this rigid-body
    /// moving with the given velocity can have.
    ///
    /// `max_extent` is the body's farthest collider point distance from its center of
    /// mass ([`RigidBodyMassProps::max_extent`]).
    pub fn max_point_velocity(&self, vels: &RigidBodyVelocity<Real>, max_extent: Real) -> Real {
        #[cfg(feature = "dim2")]
        return vels.linvel.length() + vels.angvel.abs() * max_extent;
        #[cfg(feature = "dim3")]
        return vels.linvel.length() + vels.angvel.length() * max_extent;
    }

    /// Is this rigid-body moving fast enough so that it may cause a tunneling problem?
    ///
    /// The fast-body criterion: fast when the farthest point of its colliders can move more
    /// than half the body’s thinnest extent (`ccd_thickness`) within one timestep.
    pub fn is_moving_fast(
        &self,
        dt: Real,
        vels: &RigidBodyVelocity<Real>,
        forces: Option<&RigidBodyForces>,
        max_extent: Real,
    ) -> bool {
        let max_point_velocity = if let Some(forces) = forces {
            let linear_part = (vels.linvel + forces.force * dt).length();
            #[cfg(feature = "dim2")]
            let angular_part = (vels.angvel + forces.torque * dt).abs() * max_extent;
            #[cfg(feature = "dim3")]
            let angular_part = (vels.angvel + forces.torque * dt).length() * max_extent;
            linear_part + angular_part
        } else {
            self.max_point_velocity(vels, max_extent)
        };

        max_point_velocity * dt > Self::FAST_BODY_SAFETY_FACTOR * self.ccd_thickness
    }

    /// The fast-body safety factor: a body is fast when it can move more than half its
    /// thinnest extent in one step.
    pub const FAST_BODY_SAFETY_FACTOR: Real = 0.5;

    /// The fast-body criterion evaluated on the actual solved motion of this step.
    ///
    /// `pos` must hold the solved `next_position`; the test uses the larger of the actual pose
    /// delta and the velocity-based estimate.
    pub fn is_moving_fast_with_next_position(
        &self,
        dt: Real,
        vels: &RigidBodyVelocity<Real>,
        pos: &RigidBodyPosition,
        local_com: Vector,
        max_extent: Real,
    ) -> bool {
        let com1 = pos.position * local_com;
        let com2 = pos.next_position * local_com;

        // Rotation contribution to the moved distance of the farthest point:
        // 2D: |sin(Δθ)| · maxExtent; 3D: 2·|Δq.v| · maxExtent ≈ Δθ · maxExtent.
        let delta_rot = pos.next_position.rotation * pos.position.rotation.inverse();
        #[cfg(feature = "dim2")]
        let angular_delta = delta_rot.sin().abs() * max_extent;
        #[cfg(feature = "dim3")]
        let angular_delta =
            2.0 * Vector::new(delta_rot.x, delta_rot.y, delta_rot.z).length() * max_extent;

        let max_delta_position = (com2 - com1).length() + angular_delta;
        let max_velocity = self.max_point_velocity(vels, max_extent);
        let max_motion = max_delta_position.max(max_velocity * dt);

        max_motion > Self::FAST_BODY_SAFETY_FACTOR * self.ccd_thickness
    }
}

#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash)]
/// Internal identifiers used by the physics engine.
pub struct RigidBodyIds {
    pub(crate) active_island_id: u32,
    pub(crate) active_set_id: u32,
    /// The persistent island this body belongs to ([`crate::dynamics::INVALID_ISLAND`] for fixed
    /// or disabled bodies).
    pub(crate) island_id: u32,
    /// This body's index in its persistent island's `bodies` array (also its
    /// union-find node id during an island split).
    pub(crate) island_index: u32,
}

impl Default for RigidBodyIds {
    fn default() -> Self {
        Self {
            active_island_id: u32::MAX,
            active_set_id: u32::MAX,
            island_id: crate::dynamics::INVALID_ISLAND,
            island_index: u32::MAX,
        }
    }
}

#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
#[derive(Default, Clone, Debug, PartialEq, Eq)]
/// The set of colliders attached to this rigid-bodies.
///
/// This should not be modified manually unless you really know what
/// you are doing (for example if you are trying to integrate Rapier
/// to a game engine using its component-based interface).
pub struct RigidBodyColliders(pub Vec<ColliderHandle>);

impl RigidBodyColliders {
    /// Detach a collider from this rigid-body.
    pub fn detach_collider(
        &mut self,
        rb_changes: &mut RigidBodyChanges,
        co_handle: ColliderHandle,
    ) {
        if let Some(i) = self.0.iter().position(|e| *e == co_handle) {
            rb_changes.set(RigidBodyChanges::COLLIDERS, true);
            self.0.swap_remove(i);
        }
    }

    /// Attach a collider to this rigid-body.
    pub fn attach_collider(
        &mut self,
        rb_type: RigidBodyType,
        rb_changes: &mut RigidBodyChanges,
        rb_ccd: &mut RigidBodyCcd,
        rb_mprops: &mut RigidBodyMassProps,
        rb_pos: &RigidBodyPosition,
        co_handle: ColliderHandle,
        co_pos: &mut ColliderPosition,
        co_parent: &ColliderParent,
        co_shape: &ColliderShape,
        co_mprops: &ColliderMassProps,
    ) {
        rb_changes.set(RigidBodyChanges::COLLIDERS, true);

        co_pos.0 = rb_pos.position * co_parent.pos_wrt_parent;
        // Shapes the continuous phase never sweeps (meshes, heightfields, polylines, voxels)
        // don't count toward CCD thickness: a trimesh's zero `ccd_thickness` would flag the body
        // as fast every step for a sweep that never happens.
        if !crate::dynamics::ccd::shape_never_ccd_swept(&**co_shape) {
            rb_ccd.ccd_thickness = rb_ccd.ccd_thickness.min(co_shape.ccd_thickness());
        }

        let mass_properties = co_mprops
            .mass_properties(&**co_shape)
            .transform_by(&co_parent.pos_wrt_parent);
        self.0.push(co_handle);
        rb_mprops.local_mprops += mass_properties;
        rb_mprops.update_world_mass_properties(rb_type, &rb_pos.position);
    }

    /// Update the positions of all the colliders attached to this rigid-body.
    pub(crate) fn update_positions(
        &self,
        colliders: &mut ColliderSet,
        modified_colliders: &mut ModifiedColliders,
        parent_pos: &Pose,
    ) {
        for handle in &self.0 {
            // NOTE: the ColliderParent component must exist if we enter this method.
            // NOTE: currently, we are propagating the position even if the collider is disabled.
            //       Is that the best behavior?
            let co = colliders.index_mut_internal(*handle);
            let new_pos = parent_pos * co.parent.as_ref().unwrap().pos_wrt_parent;

            // Set the modification flag so we can benefit from the modification-tracking
            // when updating the narrow-phase/broad-phase afterwards.
            modified_colliders.push_once(*handle, co);

            co.changes |= ColliderChanges::POSITION;
            co.pos = ColliderPosition(new_pos);
        }
    }
}

#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
#[derive(Default, Clone, Debug, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// The dominance groups of a rigid-body.
pub struct RigidBodyDominance(pub i8);

impl RigidBodyDominance {
    /// The actual dominance group of this rigid-body, after taking into account its type.
    pub fn effective_group(&self, status: &RigidBodyType) -> i16 {
        if status.is_dynamic_or_kinematic() {
            self.0 as i16
        } else {
            i8::MAX as i16 + 1
        }
    }
}

/// Controls when a body goes to sleep (becomes inactive to save CPU).
///
/// ## Sleeping System
///
/// Bodies automatically sleep when they're at rest, dramatically improving performance
/// in scenes with many inactive objects. Sleeping bodies are:
/// - Excluded from simulation (no collision detection, no velocity integration)
/// - Automatically woken when disturbed (hit by moving object, connected via joint)
/// - Woken manually with `body.wake_up()` or `islands.wake_up()`
///
/// ## When to disable sleeping
///
/// Most bodies should sleep! Only disable if the body needs to stay active despite being still:
/// - Bodies you frequently query for raycasts/contacts
/// - Bodies with time-based behaviors while stationary
///
/// Use `RigidBodyBuilder::can_sleep(false)` or `RigidBodyActivation::cannot_sleep()`.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
pub struct RigidBodyActivation {
    /// Velocity threshold for sleeping (scaled by `length_unit`).
    ///
    /// Compared against the body's farthest-point speed (`|linvel| + |angvel| * max_extent`)
    /// and against its farthest-point displacement rate (so solver position
    /// corrections count as motion too). If negative, body never sleeps. Default: 0.05 units/second.
    pub normalized_linear_threshold: Real,

    /// Angular velocity threshold for sleeping (radians/second).
    ///
    /// For bodies with colliders, angular motion is folded into the point-velocity check of
    /// `normalized_linear_threshold`; this raw threshold only applies to collider-less bodies.
    /// If negative, body never sleeps. Default: 0.5 rad/s.
    pub angular_threshold: Real,

    /// How long the body must stay below the velocity threshold before sleeping (seconds).
    ///
    /// Default: 0.5 seconds.
    pub time_until_sleep: Real,

    /// Internal timer tracking how long body has been still.
    pub time_since_can_sleep: Real,

    /// Is this body currently sleeping?
    pub sleeping: bool,

    /// Pose at the previous step: the sleep check measures actual per-step displacement,
    /// solver position corrections included (those never show up in the velocities).
    pub(crate) sleep_prev_pose: Pose,
}

impl Default for RigidBodyActivation {
    fn default() -> Self {
        Self::active()
    }
}

impl RigidBodyActivation {
    /// The default linear velocity below which a body can be put to sleep.
    ///
    /// Default: `0.05` length units per second.
    pub fn default_normalized_linear_threshold() -> Real {
        0.05
    }

    /// The default angular velocity below which a body can be put to sleep.
    pub fn default_angular_threshold() -> Real {
        0.5
    }

    /// The amount of time the rigid-body must remain below it’s linear and angular velocity
    /// threshold before falling to sleep.
    ///
    /// Default: half a second.
    pub fn default_time_until_sleep() -> Real {
        0.5
    }

    /// Create a new rb_activation status initialised with the default rb_activation threshold and is active.
    pub fn active() -> Self {
        RigidBodyActivation {
            normalized_linear_threshold: Self::default_normalized_linear_threshold(),
            angular_threshold: Self::default_angular_threshold(),
            time_until_sleep: Self::default_time_until_sleep(),
            time_since_can_sleep: 0.0,
            sleeping: false,
            sleep_prev_pose: Pose::IDENTITY,
        }
    }

    /// Create a new rb_activation status initialised with the default rb_activation threshold and is inactive.
    pub fn inactive() -> Self {
        RigidBodyActivation {
            normalized_linear_threshold: Self::default_normalized_linear_threshold(),
            angular_threshold: Self::default_angular_threshold(),
            time_until_sleep: Self::default_time_until_sleep(),
            time_since_can_sleep: Self::default_time_until_sleep(),
            sleeping: true,
            sleep_prev_pose: Pose::IDENTITY,
        }
    }

    /// Create a new activation status that prevents the rigid-body from sleeping.
    pub fn cannot_sleep() -> Self {
        RigidBodyActivation {
            normalized_linear_threshold: -1.0,
            angular_threshold: -1.0,
            ..Self::active()
        }
    }

    /// Returns `true` if the body is not asleep.
    #[inline]
    pub fn is_active(&self) -> bool {
        !self.sleeping
    }

    /// Wakes up this rigid-body.
    #[inline]
    pub fn wake_up(&mut self, strong: bool) {
        self.sleeping = false;

        if strong {
            self.time_since_can_sleep = 0.0;
        }
    }

    /// Put this rigid-body to sleep.
    #[inline]
    pub fn sleep(&mut self) {
        self.sleeping = true;
        self.time_since_can_sleep = self.time_until_sleep;
    }

    /// Does this body have a sufficiently low kinetic energy for a long enough
    /// duration to be eligible for sleeping?
    pub fn is_eligible_for_sleep(&self) -> bool {
        self.time_since_can_sleep >= self.time_until_sleep
    }

    pub(crate) fn update_energy(
        &mut self,
        body_type: RigidBodyType,
        length_unit: Real,
        sq_linvel: Real,
        sq_angvel: Real,
        max_extent: Real,
        pose: &Pose,
        dt: Real,
    ) {
        // A manual `RigidBody::sleep()` pins sleep eligibility until something wakes the
        // body: the velocity/drift gates must not cancel it (a teleport right before
        // sleeping trips the drift gate, keeping the body simulated while flagged asleep).
        if self.sleeping {
            self.time_since_can_sleep = self.time_until_sleep;
            return;
        }

        let can_sleep = match body_type {
            RigidBodyType::Dynamic => {
                let linear_threshold = self.normalized_linear_threshold * length_unit;
                let prev_pose = core::mem::replace(&mut self.sleep_prev_pose, *pose);
                let angular_ok = if max_extent > 0.0 {
                    use crate::num::FloatConst;
                    // Use a fixed angular threshold that unambiguously imply movement.
                    // The position-based criteria will be more restrictive, but we keep
                    // this for the rare case where the orientation’s periodicity would
                    // make the pose drift estimate too approximate.
                    self.angular_threshold >= 0.0
                        && sq_angvel < Real::FRAC_PI_2() * Real::FRAC_PI_2()
                } else {
                    // Collider-less bodies have `max_extent == 0` so we need to take its
                    // angular velocity into account since the pose delta cannot take its
                    // rotation into account.
                    sq_angvel < self.angular_threshold * self.angular_threshold.abs()
                };

                let drift = crate::geometry::relative_pose_drift(&prev_pose, pose, max_extent);
                angular_ok && drift * 0.5 < linear_threshold * dt
            }
            RigidBodyType::KinematicPositionBased | RigidBodyType::KinematicVelocityBased => {
                // Platforms only sleep if both velocities are exactly zero. If it’s not exactly
                // zero, then the user really wants them to move.
                sq_linvel == 0.0 && sq_angvel == 0.0
            }
            RigidBodyType::Fixed => true,
        };

        if can_sleep {
            self.time_since_can_sleep += dt;
        } else {
            self.time_since_can_sleep = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Real;

    #[test]
    fn test_interpolate_velocity() {
        // Interpolate and then integrate the velocity to see if
        // the end positions match.
        #[cfg(feature = "f32")]
        let mut rng = oorandom::Rand32::new(0);
        #[cfg(feature = "f64")]
        let mut rng = oorandom::Rand64::new(0);

        for i in -10..=10 {
            let mult = i as Real;
            let (local_com, curr_pos, next_pos);
            #[cfg(feature = "dim2")]
            {
                local_com = Vector::new(rng.rand_float(), rng.rand_float());
                curr_pos = Pose::new(
                    Vector::new(rng.rand_float(), rng.rand_float()) * mult,
                    rng.rand_float(),
                );
                next_pos = Pose::new(
                    Vector::new(rng.rand_float(), rng.rand_float()) * mult,
                    rng.rand_float(),
                );
            }
            #[cfg(feature = "dim3")]
            {
                local_com = Vector::new(rng.rand_float(), rng.rand_float(), rng.rand_float());
                curr_pos = Pose::new(
                    Vector::new(rng.rand_float(), rng.rand_float(), rng.rand_float()) * mult,
                    Vector::new(rng.rand_float(), rng.rand_float(), rng.rand_float()),
                );
                next_pos = Pose::new(
                    Vector::new(rng.rand_float(), rng.rand_float(), rng.rand_float()) * mult,
                    Vector::new(rng.rand_float(), rng.rand_float(), rng.rand_float()),
                );
            }

            let dt = 0.016;
            let rb_pos = RigidBodyPosition {
                position: curr_pos,
                next_position: next_pos,
            };
            let vel = rb_pos.interpolate_velocity(1.0 / dt, local_com);
            let interp_pos = vel.integrate(dt, &curr_pos, &local_com);
            approx::assert_relative_eq!(interp_pos, next_pos, epsilon = 1.0e-5);
        }
    }

    /// Runs `steps` of `update_energy` on a body that only ever moves by `shift_per_step`
    /// (zero velocity: the motion stands for solver position corrections).
    fn creep(shift_per_step: Real, steps: usize, dt: Real) -> RigidBodyActivation {
        let mut activation = RigidBodyActivation::active();
        let mut shift = 0.0;

        for _ in 0..steps {
            shift += shift_per_step;
            let pose = Pose::from_translation(Vector::X * shift);
            activation.update_energy(RigidBodyType::Dynamic, 1.0, 0.0, 0.0, 1.0, &pose, dt);
        }

        activation
    }

    /// Runs `steps` of `update_energy` on a body pinned to one pose while reporting `linvel`,
    /// like a body in a loaded stack whose contacts cancel its velocity again every step.
    fn pinned_with_velocity(linvel: Real, steps: usize, dt: Real) -> RigidBodyActivation {
        let mut activation = RigidBodyActivation::active();
        let pose = Pose::from_translation(Vector::X * 3.0);

        for _ in 0..steps {
            activation.update_energy(
                RigidBodyType::Dynamic,
                1.0,
                linvel * linvel,
                0.0,
                1.0,
                &pose,
                dt,
            );
        }

        activation
    }

    #[test]
    fn test_sleep_allows_pinned_body_with_residual_velocity() {
        let dt = 1.0 / 60.0;
        let threshold = RigidBodyActivation::default_normalized_linear_threshold();
        let steps = (10.0 * RigidBodyActivation::default_time_until_sleep() / dt) as usize;

        // A still body sleeps even while its velocity reads several times the threshold:
        // that residual is the solver cancelling itself, not motion.
        assert!(pinned_with_velocity(threshold * 5.0, steps, dt).is_eligible_for_sleep());
    }

    #[test]
    fn test_sleep_gates_position_corrections() {
        let dt = 1.0 / 60.0;
        // The per-step displacement the sleep metric tolerates: the threshold, halved.
        let budget = 2.0 * RigidBodyActivation::default_normalized_linear_threshold() * dt;
        let steps = (10.0 * RigidBodyActivation::default_time_until_sleep() / dt) as usize;

        // Creeping faster than the budget blocks sleep, however small the velocities are.
        assert!(!creep(budget * 1.5, steps, dt).is_eligible_for_sleep());

        // Creeping below it doesn't, no matter how long the body has been still.
        assert!(creep(budget * 0.5, steps, dt).is_eligible_for_sleep());
        assert!(creep(0.0, steps, dt).is_eligible_for_sleep());
    }
}

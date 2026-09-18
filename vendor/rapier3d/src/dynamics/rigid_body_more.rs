/// ## Applying forces and torques
impl RigidBody {
    /// Clears all forces that were added with `add_force()`.
    ///
    /// User-defined forces are **not** cleared automatically: once added, a force keeps being
    /// applied at every physics step until you change it or clear it with this method. Call this
    /// when you want a previously-added force to stop being applied.
    pub fn reset_forces(&mut self, wake_up: bool) {
        if self.forces.user_force != Vector::ZERO {
            self.forces.user_force = Vector::ZERO;

            if wake_up {
                self.wake_up(true);
            }
        }
    }

    /// Clears all torques that were added with `add_torque()`.
    ///
    /// User-defined torques are **not** cleared automatically: once added, a torque keeps being
    /// applied at every physics step until you change it or clear it with this method.
    #[cfg(feature = "dim2")]
    pub fn reset_torques(&mut self, wake_up: bool) {
        if self.forces.user_torque != 0.0 {
            self.forces.user_torque = 0.0;

            if wake_up {
                self.wake_up(true);
            }
        }
    }

    /// Clears all torques that were added with `add_torque()`.
    ///
    /// User-defined torques are **not** cleared automatically: once added, a torque keeps being
    /// applied at every physics step until you change it or clear it with this method.
    #[cfg(feature = "dim3")]
    pub fn reset_torques(&mut self, wake_up: bool) {
        if self.forces.user_torque != AngVector::ZERO {
            self.forces.user_torque = AngVector::ZERO;

            if wake_up {
                self.wake_up(true);
            }
        }
    }

    /// Applies a continuous force to this body (like thrust, wind, or magnets).
    ///
    /// Unlike [`apply_impulse()`](Self::apply_impulse) which is instant, a force is applied
    /// continuously over time. Successive calls to `add_force()` accumulate. Use for:
    /// - Rocket/jet thrust
    /// - Wind or water currents
    /// - Magnetic/gravity fields
    /// - Continuous pushing/pulling
    ///
    /// User-defined forces are **not** cleared automatically: once added, a force keeps being
    /// applied at every physics step until you change it or clear it with
    /// [`reset_forces()`](Self::reset_forces). To apply a force for a single step only, call
    /// `reset_forces()` after stepping the simulation.
    ///
    /// # Example
    /// ```
    /// # use rapier3d::prelude::*;
    /// # let mut bodies = RigidBodySet::new();
    /// # let body = bodies.insert(RigidBodyBuilder::dynamic());
    /// // Apply thrust every frame
    /// bodies[body].add_force(Vector::new(0.0, 100.0, 0.0), true);
    /// ```
    ///
    /// Only affects dynamic bodies (does nothing for kinematic/fixed bodies).
    pub fn add_force(&mut self, force: Vector, wake_up: bool) {
        if force != Vector::ZERO && self.body_type == RigidBodyType::Dynamic {
            self.forces.user_force += force;

            if wake_up {
                self.wake_up(true);
            }
        }
    }

    /// Applies a continuous rotational force (torque) to spin this body.
    ///
    /// Like `add_force()` but for rotation. Successive calls accumulate, and the torque is **not**
    /// cleared automatically (see [`reset_torques()`](Self::reset_torques)).
    /// In 2D: positive = counter-clockwise, negative = clockwise.
    ///
    /// Only affects dynamic bodies.
    #[cfg(feature = "dim2")]
    pub fn add_torque(&mut self, torque: Real, wake_up: bool) {
        if !torque.is_zero() && self.body_type == RigidBodyType::Dynamic {
            self.forces.user_torque += torque;

            if wake_up {
                self.wake_up(true);
            }
        }
    }

    /// Applies a continuous rotational force (torque) to spin this body.
    ///
    /// Like `add_force()` but for rotation. In 3D, the torque vector direction
    /// determines the rotation axis (right-hand rule).
    ///
    /// Only affects dynamic bodies.
    #[cfg(feature = "dim3")]
    pub fn add_torque(&mut self, torque: Vector, wake_up: bool) {
        if torque != Vector::ZERO && self.body_type == RigidBodyType::Dynamic {
            self.forces.user_torque += torque;

            if wake_up {
                self.wake_up(true);
            }
        }
    }

    /// Applies force at a specific point on the body (creates both force and torque).
    ///
    /// When you push an object off-center, it both moves AND spins. This method handles both effects.
    /// The force creates linear acceleration, and the offset from center-of-mass creates torque.
    ///
    /// Use for: Forces applied at contact points, explosions at specific locations, pushing objects.
    ///
    /// # Parameters
    /// * `force` - The force vector to apply
    /// * `point` - Where to apply the force (world coordinates)
    ///
    /// Only affects dynamic bodies.
    pub fn add_force_at_point(&mut self, force: Vector, point: Vector, wake_up: bool) {
        if force != Vector::ZERO && self.body_type == RigidBodyType::Dynamic {
            self.forces.user_force += force;
            self.forces.user_torque += (point - self.mprops.world_com).gcross(force);

            if wake_up {
                self.wake_up(true);
            }
        }
    }
}

/// ## Applying impulses and angular impulses
impl RigidBody {
    /// Instantly changes the velocity by applying an impulse (like a kick or explosion).
    ///
    /// An impulse is an instant change in momentum. Think of it as a "one-time push" that
    /// immediately affects velocity. Use for:
    /// - Jumping (apply upward impulse)
    /// - Explosions pushing objects away
    /// - Getting hit by something
    /// - Launching projectiles
    ///
    /// The effect depends on the body's mass - heavier objects will be affected less by the same impulse.
    ///
    /// **For continuous forces** (like rocket thrust or wind), use [`add_force()`](Self::add_force) instead.
    ///
    /// # Example
    /// ```
    /// # use rapier3d::prelude::*;
    /// # let mut bodies = RigidBodySet::new();
    /// # let body = bodies.insert(RigidBodyBuilder::dynamic());
    /// // Make a character jump
    /// bodies[body].apply_impulse(Vector::new(0.0, 300.0, 0.0), true);
    /// ```
    ///
    /// Only affects dynamic bodies (does nothing for kinematic/fixed bodies).
    #[profiling::function]
    pub fn apply_impulse(&mut self, impulse: Vector, wake_up: bool) {
        if impulse != Vector::ZERO && self.body_type == RigidBodyType::Dynamic {
            self.vels.linvel += impulse * self.mprops.effective_inv_mass;

            if wake_up {
                self.wake_up(true);
            }
        }
    }

    /// Applies an angular impulse at the center-of-mass of this rigid-body.
    /// The impulse is applied right away, changing the angular velocity.
    /// This does nothing on non-dynamic bodies.
    #[cfg(feature = "dim2")]
    #[profiling::function]
    pub fn apply_torque_impulse(&mut self, torque_impulse: Real, wake_up: bool) {
        if !torque_impulse.is_zero() && self.body_type == RigidBodyType::Dynamic {
            self.vels.angvel += self.mprops.effective_world_inv_inertia * torque_impulse;

            if wake_up {
                self.wake_up(true);
            }
        }
    }

    /// Instantly changes rotation speed by applying angular impulse (like a sudden spin).
    ///
    /// In 3D, the impulse vector direction determines the spin axis (right-hand rule).
    /// Like `apply_impulse()` but for rotation. Only affects dynamic bodies.
    #[cfg(feature = "dim3")]
    #[profiling::function]
    pub fn apply_torque_impulse(&mut self, torque_impulse: Vector, wake_up: bool) {
        if torque_impulse != Vector::ZERO && self.body_type == RigidBodyType::Dynamic {
            self.vels.angvel += self.mprops.effective_world_inv_inertia * torque_impulse;

            if wake_up {
                self.wake_up(true);
            }
        }
    }

    /// Applies impulse at a specific point on the body (creates both linear and angular effects).
    ///
    /// Like `add_force_at_point()` but instant instead of continuous. When you hit an object
    /// off-center, it both flies away AND spins - this method handles both.
    ///
    /// # Example
    /// ```
    /// # use rapier3d::prelude::*;
    /// # let mut bodies = RigidBodySet::new();
    /// # let body = bodies.insert(RigidBodyBuilder::dynamic());
    /// // Hit the top-left corner of a box
    /// bodies[body].apply_impulse_at_point(
    ///     Vector::new(100.0, 0.0, 0.0),
    ///     Vector::new(-0.5, 0.5, 0.0),  // Top-left of a 1x1 box
    ///     true
    /// );
    /// // Box will move right AND spin
    /// ```
    ///
    /// Only affects dynamic bodies.
    pub fn apply_impulse_at_point(&mut self, impulse: Vector, point: Vector, wake_up: bool) {
        let torque_impulse = (point - self.mprops.world_com).gcross(impulse);
        self.apply_impulse(impulse, wake_up);
        self.apply_torque_impulse(torque_impulse, wake_up);
    }

    /// Returns the total force currently queued to be applied this frame.
    ///
    /// This is the sum of all `add_force()` calls since the last physics step.
    /// Returns zero for non-dynamic bodies.
    pub fn user_force(&self) -> Vector {
        if self.body_type == RigidBodyType::Dynamic {
            self.forces.user_force
        } else {
            Vector::ZERO
        }
    }

    /// Returns the total torque currently queued to be applied this frame.
    ///
    /// This is the sum of all `add_torque()` calls since the last physics step.
    /// Returns zero for non-dynamic bodies.
    pub fn user_torque(&self) -> AngVector {
        if self.body_type == RigidBodyType::Dynamic {
            self.forces.user_torque
        } else {
            #[cfg(feature = "dim2")]
            {
                0.0
            }
            #[cfg(feature = "dim3")]
            {
                AngVector::ZERO
            }
        }
    }

    /// Checks if gyroscopic forces are enabled (3D only).
    ///
    /// Gyroscopic forces cause spinning objects to resist changes in rotation axis
    /// (like how spinning tops stay upright). Adds slight CPU cost.
    #[cfg(feature = "dim3")]
    pub fn gyroscopic_forces_enabled(&self) -> bool {
        self.forces.gyroscopic_forces_enabled
    }

    /// Enables/disables gyroscopic forces for more realistic spinning behavior.
    ///
    /// When enabled, rapidly spinning objects resist rotation axis changes (like gyroscopes).
    /// Examples: spinning tops, flywheels, rotating spacecraft.
    ///
    /// **Default**: Disabled (costs performance, rarely needed in games).
    #[cfg(feature = "dim3")]
    pub fn enable_gyroscopic_forces(&mut self, enabled: bool) {
        self.forces.gyroscopic_forces_enabled = enabled;
    }
}

impl RigidBody {
    /// Calculates the velocity at a specific point on this body.
    ///
    /// Due to rotation, different points on a rigid body move at different speeds.
    /// This computes the linear velocity at any world-space point.
    ///
    /// Useful for: impact calculations, particle effects, sound volume based on impact speed.
    pub fn velocity_at_point(&self, point: Vector) -> Vector {
        self.vels.velocity_at_point(point, self.mprops.world_com)
    }

    /// Calculates the kinetic energy of this body (energy from motion).
    ///
    /// Returns `0.5 * mass * velocity² + 0.5 * inertia * angular_velocity²`
    /// Useful for physics-based gameplay (energy tracking, damage based on impact energy).
    pub fn kinetic_energy(&self) -> Real {
        self.vels.kinetic_energy(&self.mprops)
    }

    /// Calculates the gravitational potential energy of this body.
    ///
    /// Returns `mass * gravity * height`. Useful for energy conservation checks.
    pub fn gravitational_potential_energy(&self, dt: Real, gravity: Vector) -> Real {
        let world_com = self.mprops.local_mprops.world_com(&self.pos.position);

        // Project position back along velocity vector one half-step (leap-frog)
        // to sync up the potential energy with the kinetic energy:
        let world_com = world_com - self.vels.linvel * (dt / 2.0);

        -self.mass() * self.forces.gravity_scale * gravity.dot(world_com)
    }

    /// Computes the angular velocity of this rigid-body after application of gyroscopic forces.
    #[cfg(feature = "dim3")]
    pub fn angvel_with_gyroscopic_forces(&self, dt: Real) -> AngVector {
        let mprops = &self.mprops.local_mprops;
        // World-space principal axes = body rotation ∘ principal frame.
        let principal_axes = self.pos.position.rotation * mprops.principal_inertia_local_frame;
        gyroscopic_corrected_angvel(
            self.angvel(),
            principal_axes,
            mprops.principal_inertia(),
            mprops.inv_principal_inertia,
            dt,
        )
    }
}

/// A builder for creating rigid bodies with custom properties.
///
/// This builder lets you configure all properties of a rigid body before adding it to your world.
/// Start with one of the type constructors ([`dynamic()`](Self::dynamic), [`fixed()`](Self::fixed),
///  [`kinematic_position_based()`](Self::kinematic_position_based), or
/// [`kinematic_velocity_based()`](Self::kinematic_velocity_based)), then chain property setters,
/// and finally call [`build()`](Self::build).
///
/// # Example
///
/// ```
/// # use rapier3d::prelude::*;
/// let body = RigidBodyBuilder::dynamic()
///     .translation(Vector::new(0.0, 5.0, 0.0))  // Start 5 units above ground
///     .linvel(Vector::new(1.0, 0.0, 0.0))       // Initial velocity to the right
///     .can_sleep(false)                          // Keep always active
///     .build();
/// ```
#[derive(Clone, Debug, PartialEq)]
#[must_use = "Builder functions return the updated builder"]
pub struct RigidBodyBuilder {
    /// The initial position of the rigid-body to be built.
    pub position: Pose,
    /// The linear velocity of the rigid-body to be built.
    pub linvel: Vector,
    /// The angular velocity of the rigid-body to be built.
    pub angvel: AngVector,
    /// The scale factor applied to the gravity affecting the rigid-body to be built, `1.0` by default.
    pub gravity_scale: Real,
    /// Damping factor for gradually slowing down the translational motion of the rigid-body, `0.0` by default.
    pub linear_damping: Real,
    /// Damping factor for gradually slowing down the angular motion of the rigid-body, `0.0` by default.
    pub angular_damping: Real,
    /// The type of rigid-body being constructed.
    pub body_type: RigidBodyType,
    mprops_flags: LockedAxes,
    /// The additional mass-properties of the rigid-body being built. See [`RigidBodyBuilder::additional_mass_properties`] for more information.
    additional_mass_properties: RigidBodyAdditionalMassProps,
    /// Whether the rigid-body to be created can sleep if it reaches a dynamic equilibrium.
    pub can_sleep: bool,
    /// Whether the rigid-body is to be created asleep.
    pub sleeping: bool,
    /// Whether full ("bullet") Continuous Collision-Detection is enabled for the rigid-body to be
    /// built. Fast dynamic bodies always sweep fixed colliders; this also sweeps kinematic and
    /// dynamic bodies. CCD prevents tunneling but may allow limited interpenetration of colliders.
    pub ccd_enabled: bool,
    /// The maximum prediction distance Soft Continuous Collision-Detection.
    ///
    /// When set to 0, soft CCD is disabled. Soft-CCD helps prevent tunneling especially of
    /// slow-but-thin to moderately fast objects. The soft CCD prediction distance indicates how
    /// far in the object’s path the CCD algorithm is allowed to inspect. Large values can impact
    /// performance badly by increasing the work needed from the broad-phase.
    ///
    /// It is a generally cheaper variant of regular CCD (that can be enabled with
    /// [`RigidBodyBuilder::ccd_enabled`] since it relies on predictive constraints instead of
    /// shape-cast and substeps.
    pub soft_ccd_prediction: Real,
    /// Allow the rigid-body being built to exceed the angular speed cap.
    /// See [`RigidBody::set_allow_fast_rotation`].
    pub allow_fast_rotation: bool,
    /// The dominance group of the rigid-body to be built.
    pub dominance_group: i8,
    /// Will the rigid-body being built be enabled?
    pub enabled: bool,
    /// An arbitrary user-defined 128-bit integer associated to the rigid-bodies built by this builder.
    pub user_data: u128,
    /// The additional number of solver iterations run for the constraints directly
    /// involving this rigid-body.
    ///
    /// See [`RigidBody::set_additional_solver_iterations`] for additional information.
    pub additional_solver_iterations: usize,
    /// Are gyroscopic forces enabled for this rigid-body?
    pub gyroscopic_forces_enabled: bool,
}

impl Default for RigidBodyBuilder {
    fn default() -> Self {
        Self::dynamic()
    }
}

impl RigidBodyBuilder {
    /// Initialize a new builder for a rigid body which is either fixed, dynamic, or kinematic.
    pub fn new(body_type: RigidBodyType) -> Self {
        #[cfg(feature = "dim2")]
        let angvel = 0.0;
        #[cfg(feature = "dim3")]
        let angvel = AngVector::ZERO;

        Self {
            position: Pose::IDENTITY,
            linvel: Vector::ZERO,
            angvel,
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            body_type,
            mprops_flags: LockedAxes::empty(),
            additional_mass_properties: RigidBodyAdditionalMassProps::default(),
            can_sleep: true,
            sleeping: false,
            ccd_enabled: false,
            soft_ccd_prediction: 0.0,
            allow_fast_rotation: false,
            dominance_group: 0,
            enabled: true,
            user_data: 0,
            additional_solver_iterations: 0,
            gyroscopic_forces_enabled: true,
        }
    }

    /// Initializes the builder of a new fixed rigid body.
    #[deprecated(note = "use `RigidBodyBuilder::fixed()` instead")]
    pub fn new_static() -> Self {
        Self::fixed()
    }
    /// Initializes the builder of a new velocity-based kinematic rigid body.
    #[deprecated(note = "use `RigidBodyBuilder::kinematic_velocity_based()` instead")]
    pub fn new_kinematic_velocity_based() -> Self {
        Self::kinematic_velocity_based()
    }
    /// Initializes the builder of a new position-based kinematic rigid body.
    #[deprecated(note = "use `RigidBodyBuilder::kinematic_position_based()` instead")]
    pub fn new_kinematic_position_based() -> Self {
        Self::kinematic_position_based()
    }

    /// Creates a builder for a **fixed** (static) rigid body.
    ///
    /// Fixed bodies never move and are not affected by any forces. Use them for:
    /// - Walls, floors, and ceilings
    /// - Static terrain and level geometry
    /// - Any object that should never move in your simulation
    ///
    /// Fixed bodies have infinite mass and never sleep.
    pub fn fixed() -> Self {
        Self::new(RigidBodyType::Fixed)
    }

    /// Creates a builder for a **velocity-based kinematic** rigid body.
    ///
    /// Kinematic bodies are moved by directly setting their velocity (not by applying forces).
    /// They can push dynamic bodies but are not affected by them. Use for:
    /// - Moving platforms and elevators
    /// - Doors and sliding panels
    /// - Any object you want to control directly while still affecting other physics objects
    ///
    /// Set velocity with [`RigidBody::set_linvel`] and [`RigidBody::set_angvel`].
    pub fn kinematic_velocity_based() -> Self {
        Self::new(RigidBodyType::KinematicVelocityBased)
    }

    /// Creates a builder for a **position-based kinematic** rigid body.
    ///
    /// Similar to velocity-based kinematic, but you control it by setting its next position
    /// directly rather than setting velocity. Rapier will automatically compute the velocity
    /// needed to reach that position. Use for objects animated by external systems.
    pub fn kinematic_position_based() -> Self {
        Self::new(RigidBodyType::KinematicPositionBased)
    }

    /// Creates a builder for a **dynamic** rigid body.
    ///
    /// Dynamic bodies are fully simulated - they respond to gravity, forces, collisions, and
    /// constraints. This is the most common type for interactive objects. Use for:
    /// - Physics objects that should fall and bounce (boxes, spheres, ragdolls)
    /// - Projectiles and debris
    /// - Vehicles and moving characters (when not using kinematic control)
    /// - Any object that should behave realistically under physics
    ///
    /// Dynamic bodies can sleep (become inactive) when at rest to save performance.
    pub fn dynamic() -> Self {
        Self::new(RigidBodyType::Dynamic)
    }

    /// Sets the additional number of solver iterations run for the constraints directly
    /// involving this rigid-body.
    ///
    /// See [`RigidBody::set_additional_solver_iterations`] for additional information.
    pub fn additional_solver_iterations(mut self, additional_iterations: usize) -> Self {
        self.additional_solver_iterations = additional_iterations;
        self
    }

    /// Sets the scale applied to the gravity force affecting the rigid-body to be created.
    pub fn gravity_scale(mut self, scale_factor: Real) -> Self {
        self.gravity_scale = scale_factor;
        self
    }

    /// Sets the dominance group (advanced collision priority system).
    ///
    /// Higher dominance groups can push lower ones but not vice versa.
    /// Rarely needed - most games don't use this. Default is 0 (all equal priority).
    ///
    /// Use case: Heavy objects that should always push lighter ones in contacts.
    pub fn dominance_group(mut self, group: i8) -> Self {
        self.dominance_group = group;
        self
    }

    /// Sets the initial position (XYZ coordinates) where this body will be created.
    ///
    /// # Example
    /// ```
    /// # use rapier3d::prelude::*;
    /// let body = RigidBodyBuilder::dynamic()
    ///     .translation(Vector::new(10.0, 5.0, -3.0))
    ///     .build();
    /// ```
    pub fn translation(mut self, translation: Vector) -> Self {
        self.position.translation = translation;
        self
    }

    /// Sets the initial rotation/orientation of the body to be created.
    ///
    /// # Example
    /// ```
    /// # use rapier3d::prelude::*;
    /// // Rotate 45 degrees around Y axis (in 3D)
    /// let body = RigidBodyBuilder::dynamic()
    ///     .rotation(Vector::new(0.0, std::f32::consts::PI / 4.0, 0.0))
    ///     .build();
    /// ```
    pub fn rotation(mut self, angle: AngVector) -> Self {
        self.position.rotation = rotation_from_angle(angle);
        self
    }

    /// Sets the initial position (translation and orientation) of the rigid-body to be created.
    #[deprecated = "renamed to `RigidBodyBuilder::pose`"]
    pub fn position(mut self, pos: Pose) -> Self {
        self.position = pos;
        self
    }

    /// Sets the initial pose (translation and orientation) of the rigid-body to be created.
    pub fn pose(mut self, pos: Pose) -> Self {
        self.position = pos;
        self
    }

    /// An arbitrary user-defined 128-bit integer associated to the rigid-bodies built by this builder.
    pub fn user_data(mut self, data: u128) -> Self {
        self.user_data = data;
        self
    }

    /// Sets the additional mass-properties of the rigid-body being built.
    ///
    /// This will be overridden by a call to [`Self::additional_mass`] so it only makes sense to call
    /// either [`Self::additional_mass`] or [`Self::additional_mass_properties`].    
    ///
    /// Note that "additional" means that the final mass-properties of the rigid-bodies depends
    /// on the initial mass-properties of the rigid-body (set by this method)
    /// to which is added the contributions of all the colliders with non-zero density
    /// attached to this rigid-body.
    ///
    /// Therefore, if you want your provided mass-properties to be the final
    /// mass-properties of your rigid-body, don't attach colliders to it, or
    /// only attach colliders with densities equal to zero.
    pub fn additional_mass_properties(mut self, mprops: MassProperties) -> Self {
        self.additional_mass_properties = RigidBodyAdditionalMassProps::MassProps(mprops);
        self
    }

    /// Sets the additional mass of the rigid-body being built.
    ///
    /// This will be overridden by a call to [`Self::additional_mass_properties`] so it only makes
    /// sense to call either [`Self::additional_mass`] or [`Self::additional_mass_properties`].    
    ///
    /// This is only the "additional" mass because the total mass of the  rigid-body is
    /// equal to the sum of this additional mass and the mass computed from the colliders
    /// (with non-zero densities) attached to this rigid-body.
    ///
    /// The total angular inertia of the rigid-body will be scaled automatically based on this
    /// additional mass. If this scaling effect isn’t desired, use [`Self::additional_mass_properties`]
    /// instead of this method.
    ///
    /// # Parameters
    /// * `mass`- The mass that will be added to the created rigid-body.
    pub fn additional_mass(mut self, mass: Real) -> Self {
        self.additional_mass_properties = RigidBodyAdditionalMassProps::Mass(mass);
        self
    }

    /// Sets which movement axes are locked (cannot move/rotate).
    ///
    /// See [`LockedAxes`] for examples of constraining movement to specific directions.
    pub fn locked_axes(mut self, locked_axes: LockedAxes) -> Self {
        self.mprops_flags = locked_axes;
        self
    }

    /// Prevents all translational movement (body can still rotate).
    ///
    /// Use for turrets, spinning objects fixed in place, etc.
    pub fn lock_translations(mut self) -> Self {
        self.mprops_flags.set(LockedAxes::TRANSLATION_LOCKED, true);
        self
    }

    /// Locks translation along specific axes.
    ///
    /// # Example
    /// ```
    /// # use rapier3d::prelude::*;
    /// // 2D game in 3D: lock Z movement
    /// let body = RigidBodyBuilder::dynamic()
    ///     .enabled_translations(true, true, false)  // X, Y free; Z locked
    ///     .build();
    /// ```
    pub fn enabled_translations(
        mut self,
        allow_translations_x: bool,
        allow_translations_y: bool,
        #[cfg(feature = "dim3")] allow_translations_z: bool,
    ) -> Self {
        self.mprops_flags
            .set(LockedAxes::TRANSLATION_LOCKED_X, !allow_translations_x);
        self.mprops_flags
            .set(LockedAxes::TRANSLATION_LOCKED_Y, !allow_translations_y);
        #[cfg(feature = "dim3")]
        self.mprops_flags
            .set(LockedAxes::TRANSLATION_LOCKED_Z, !allow_translations_z);
        self
    }

    #[deprecated(note = "Use `enabled_translations` instead")]
    /// Only allow translations of this rigid-body around specific coordinate axes.
    pub fn restrict_translations(
        self,
        allow_translations_x: bool,
        allow_translations_y: bool,
        #[cfg(feature = "dim3")] allow_translations_z: bool,
    ) -> Self {
        self.enabled_translations(
            allow_translations_x,
            allow_translations_y,
            #[cfg(feature = "dim3")]
            allow_translations_z,
        )
    }

    /// Prevents all rotational movement (body can still translate).
    ///
    /// Use for characters that shouldn't tip over, objects that should only slide, etc.
    pub fn lock_rotations(mut self) -> Self {
        self.mprops_flags.set(LockedAxes::ROTATION_LOCKED_X, true);
        self.mprops_flags.set(LockedAxes::ROTATION_LOCKED_Y, true);
        self.mprops_flags.set(LockedAxes::ROTATION_LOCKED_Z, true);
        self
    }

    /// Only allow rotations of this rigid-body around specific coordinate axes.
    #[cfg(feature = "dim3")]
    pub fn enabled_rotations(
        mut self,
        allow_rotations_x: bool,
        allow_rotations_y: bool,
        allow_rotations_z: bool,
    ) -> Self {
        self.mprops_flags
            .set(LockedAxes::ROTATION_LOCKED_X, !allow_rotations_x);
        self.mprops_flags
            .set(LockedAxes::ROTATION_LOCKED_Y, !allow_rotations_y);
        self.mprops_flags
            .set(LockedAxes::ROTATION_LOCKED_Z, !allow_rotations_z);
        self
    }

    /// Locks or unlocks rotations of this rigid-body along each cartesian axes.
    #[deprecated(note = "Use `enabled_rotations` instead")]
    #[cfg(feature = "dim3")]
    pub fn restrict_rotations(
        self,
        allow_rotations_x: bool,
        allow_rotations_y: bool,
        allow_rotations_z: bool,
    ) -> Self {
        self.enabled_rotations(allow_rotations_x, allow_rotations_y, allow_rotations_z)
    }

    /// Sets linear damping (how quickly linear velocity decreases over time).
    ///
    /// Models air resistance, drag, etc. Higher values = faster slowdown.
    /// - `0.0` = no drag (space)
    /// - `0.1` = light drag (air)
    /// - `1.0+` = heavy drag (underwater)
    pub fn linear_damping(mut self, factor: Real) -> Self {
        self.linear_damping = factor;
        self
    }

    /// Sets angular damping (how quickly rotation speed decreases over time).
    ///
    /// Models rotational drag. Higher values = spinning stops faster.
    pub fn angular_damping(mut self, factor: Real) -> Self {
        self.angular_damping = factor;
        self
    }

    /// Sets the initial linear velocity (movement speed and direction).
    ///
    /// The body will start moving at this velocity when created.
    pub fn linvel(mut self, linvel: Vector) -> Self {
        self.linvel = linvel;
        self
    }

    /// Sets the initial angular velocity (rotation speed).
    ///
    /// The body will start rotating at this speed when created.
    pub fn angvel(mut self, angvel: AngVector) -> Self {
        self.angvel = angvel;
        self
    }

    /// Sets whether this body can go to sleep when at rest (default: `true`).
    ///
    /// Sleeping bodies are excluded from simulation until disturbed, saving CPU.
    /// Set to `false` if you need the body always active (e.g., for continuous queries).
    pub fn can_sleep(mut self, can_sleep: bool) -> Self {
        self.can_sleep = can_sleep;
        self
    }

    /// Enables full ("bullet") Continuous Collision Detection: fast dynamic bodies already sweep
    /// **fixed** colliders automatically; this upgrades the body to also sweep **kinematic and
    /// dynamic** bodies at extra cost — for projectiles and fast small
    /// objects that must not tunnel through other moving bodies. Setting
    /// [`IntegrationParameters::max_ccd_substeps`] to `0` disables CCD world-wide.
    ///
    /// # Example
    /// ```
    /// # use rapier3d::prelude::*;
    /// // Bullet that should never tunnel through walls or other moving bodies
    /// let bullet = RigidBodyBuilder::dynamic()
    ///     .ccd_enabled(true)
    ///     .build();
    /// ```
    pub fn ccd_enabled(mut self, enabled: bool) -> Self {
        self.ccd_enabled = enabled;
        self
    }

    /// Sets the maximum prediction distance Soft Continuous Collision-Detection.
    ///
    /// When set to 0, soft-CCD is disabled. Soft-CCD helps prevent tunneling especially of
    /// slow-but-thin to moderately fast objects. The soft CCD prediction distance indicates how
    /// far in the object’s path the CCD algorithm is allowed to inspect. Large values can impact
    /// performance badly by increasing the work needed from the broad-phase.
    ///
    /// It is a generally cheaper variant of regular CCD (that can be enabled with
    /// [`RigidBodyBuilder::ccd_enabled`] since it relies on predictive constraints instead of
    /// shape-cast and substeps.
    pub fn soft_ccd_prediction(mut self, prediction_distance: Real) -> Self {
        self.soft_ccd_prediction = prediction_distance;
        self
    }

    /// Allow the rigid-body being built to exceed the angular speed cap.
    ///
    /// By default angular velocity is clamped each substep to ~45°/step to keep CCD reliable;
    /// pass `true` for bodies that must spin fast, e.g. wheels.
    pub fn allow_fast_rotation(mut self, allow: bool) -> Self {
        self.allow_fast_rotation = allow;
        self
    }

    /// Sets whether the rigid-body is to be created asleep.
    pub fn sleeping(mut self, sleeping: bool) -> Self {
        self.sleeping = sleeping;
        self
    }

    /// Are gyroscopic forces enabled for this rigid-body?
    ///
    /// Enabling gyroscopic forces allows more realistic behaviors like gyroscopic precession,
    /// but result in a slight performance overhead.
    ///
    /// Disabled by default.
    #[cfg(feature = "dim3")]
    pub fn gyroscopic_forces_enabled(mut self, enabled: bool) -> Self {
        self.gyroscopic_forces_enabled = enabled;
        self
    }

    /// Enable or disable the rigid-body after its creation.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Build a new rigid-body with the parameters configured with this builder.
    pub fn build(&self) -> RigidBody {
        let mut rb = RigidBody::new();
        rb.pos.next_position = self.position;
        rb.pos.position = self.position;
        rb.vels.linvel = self.linvel;
        rb.vels.angvel = self.angvel;
        rb.body_type = self.body_type;
        rb.user_data = self.user_data;
        rb.additional_solver_iterations = self.additional_solver_iterations;

        if self.additional_mass_properties
            != RigidBodyAdditionalMassProps::MassProps(MassProperties::default())
            && self.additional_mass_properties != RigidBodyAdditionalMassProps::Mass(0.0)
        {
            rb.mprops.additional_local_mprops = Some(Box::new(self.additional_mass_properties));
        }

        rb.mprops.flags = self.mprops_flags;
        rb.damping.linear_damping = self.linear_damping;
        rb.damping.angular_damping = self.angular_damping;
        rb.forces.gravity_scale = self.gravity_scale;
        #[cfg(feature = "dim3")]
        {
            rb.forces.gyroscopic_forces_enabled = self.gyroscopic_forces_enabled;
        }
        rb.dominance = RigidBodyDominance(self.dominance_group);
        rb.enabled = self.enabled;
        rb.enable_ccd(self.ccd_enabled);
        rb.set_soft_ccd_prediction(self.soft_ccd_prediction);
        rb.set_allow_fast_rotation(self.allow_fast_rotation);

        if self.can_sleep && self.sleeping {
            rb.sleep();
        }

        if !self.can_sleep {
            rb.activation.normalized_linear_threshold = -1.0;
            rb.activation.angular_threshold = -1.0;
        }

        rb
    }
}

impl From<RigidBodyBuilder> for RigidBody {
    fn from(val: RigidBodyBuilder) -> RigidBody {
        val.build()
    }
}

/// One explicit, angular-momentum-preserving gyroscopic correction of a world-space angular velocity,
/// computed in the world principal-inertia frame (`principal_axes`) so `w × I·w` is exact for tilted
/// axes. Shared by [`RigidBody::angvel_with_gyroscopic_forces`] and the solver's per-substep pass.
#[cfg(feature = "dim3")]
#[inline]
pub(crate) fn gyroscopic_corrected_angvel(
    angvel: AngVector,
    principal_axes: Rotation,
    principal_inertia: AngVector,
    inv_principal_inertia: AngVector,
    dt: Real,
) -> AngVector {
    // NOTE: integrating the gyroscopic forces implicitly are both slower and
    //       very dissipative. Instead, we only keep the explicit term and
    //       ensure angular momentum is preserved (similar to Jolt).
    let w = principal_axes.inverse() * angvel;
    let curr_momentum = principal_inertia * w;
    let explicit_gyro_momentum = -w.cross(curr_momentum) * dt;
    let total_momentum = curr_momentum + explicit_gyro_momentum;
    let total_momentum_sqnorm = total_momentum.length_squared();

    if total_momentum_sqnorm != 0.0 {
        let capped_momentum =
            total_momentum * (curr_momentum.length_squared() / total_momentum_sqnorm).sqrt();
        principal_axes * (inv_principal_inertia * capped_momentum)
    } else {
        angvel
    }
}

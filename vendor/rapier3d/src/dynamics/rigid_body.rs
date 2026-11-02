use crate::alloc_prelude::*;
#[cfg(all(not(feature = "std"), feature = "dim3"))]
use simba::scalar::ComplexField;

#[cfg(doc)]
use super::IntegrationParameters;
use crate::dynamics::{
    LockedAxes, MassProperties, RigidBodyActivation, RigidBodyAdditionalMassProps, RigidBodyCcd,
    RigidBodyChanges, RigidBodyColliders, RigidBodyDamping, RigidBodyDominance, RigidBodyForces,
    RigidBodyIds, RigidBodyMassProps, RigidBodyPosition, RigidBodyType, RigidBodyVelocity,
};
use crate::geometry::{
    ColliderHandle, ColliderMassProps, ColliderParent, ColliderPosition, ColliderSet, ColliderShape,
};
use crate::math::{AngVector, Pose, Real, Rotation, Vector, rotation_from_angle};
use crate::utils::CrossProduct;

#[cfg(feature = "dim2")]
use crate::num::Zero;

#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
/// A physical object that can move, rotate, and collide with other objects in your simulation.
///
/// Rigid bodies are the fundamental moving objects in physics simulations. Think of them as
/// the "physical representation" of your game objects - a character, a crate, a vehicle, etc.
///
/// ## Body types
///
/// - **Dynamic**: Affected by forces, gravity, and collisions. Use for objects that should move realistically (falling boxes, projectiles, etc.)
/// - **Fixed**: Never moves. Use for static geometry like walls, floors, and terrain
/// - **Kinematic**: Moved by setting velocity or position directly, not by forces. Use for moving platforms, doors, or player-controlled characters
///
/// ## Creating bodies
///
/// Always use [`RigidBodyBuilder`] to create new rigid bodies:
///
/// ```
/// # use rapier3d::prelude::*;
/// # let mut bodies = RigidBodySet::new();
/// let body = RigidBodyBuilder::dynamic()
///     .translation(Vector::new(0.0, 10.0, 0.0))
///     .build();
/// let handle = bodies.insert(body);
/// ```
#[derive(Debug, Clone)]
// #[repr(C)]
// #[repr(align(64))]
pub struct RigidBody {
    pub(crate) ids: RigidBodyIds,
    pub(crate) pos: RigidBodyPosition,
    pub(crate) damping: RigidBodyDamping<Real>,
    pub(crate) vels: RigidBodyVelocity<Real>,
    pub(crate) forces: RigidBodyForces,
    pub(crate) mprops: RigidBodyMassProps,

    pub(crate) ccd_vels: RigidBodyVelocity<Real>,
    pub(crate) ccd: RigidBodyCcd,
    pub(crate) colliders: RigidBodyColliders,
    /// Whether or not this rigid-body is sleeping.
    pub(crate) activation: RigidBodyActivation,
    pub(crate) changes: RigidBodyChanges,
    /// The status of the body, governing how it is affected by external forces.
    pub(crate) body_type: RigidBodyType,
    /// The dominance group this rigid-body is part of.
    pub(crate) dominance: RigidBodyDominance,
    pub(crate) enabled: bool,
    pub(crate) additional_solver_iterations: usize,
    /// User-defined data associated to this rigid-body.
    pub user_data: u128,
}

impl Default for RigidBody {
    fn default() -> Self {
        Self::new()
    }
}

impl RigidBody {
    fn new() -> Self {
        Self {
            pos: RigidBodyPosition::default(),
            mprops: RigidBodyMassProps::default(),
            ccd_vels: RigidBodyVelocity::default(),
            vels: RigidBodyVelocity::default(),
            damping: RigidBodyDamping::default(),
            forces: RigidBodyForces::default(),
            ccd: RigidBodyCcd::default(),
            ids: RigidBodyIds::default(),
            colliders: RigidBodyColliders::default(),
            activation: RigidBodyActivation::active(),
            changes: RigidBodyChanges::all(),
            body_type: RigidBodyType::Dynamic,
            dominance: RigidBodyDominance::default(),
            enabled: true,
            user_data: 0,
            additional_solver_iterations: 0,
        }
    }

    pub(crate) fn reset_internal_references(&mut self) {
        self.colliders.0 = Vec::new();
        self.ids = Default::default();
    }

    /// Copy all the characteristics from `other` to `self`.
    ///
    /// If you have a mutable reference to a rigid-body `rigid_body: &mut RigidBody`, attempting to
    /// assign it a whole new rigid-body instance, e.g., `*rigid_body = RigidBodyBuilder::dynamic().build()`,
    /// will crash due to some internal indices being overwritten. Instead, use
    /// `rigid_body.copy_from(&RigidBodyBuilder::dynamic().build())`.
    ///
    /// This method will allow you to set most characteristics of this rigid-body from another
    /// rigid-body instance without causing any breakage.
    ///
    /// This method **cannot** be used for editing the list of colliders attached to this rigid-body.
    /// Therefore, the list of colliders attached to `self` won’t be replaced by the one attached
    /// to `other`.
    ///
    /// The pose of `other` will only copied into `self` if `self` doesn’t have a parent (if it has
    /// a parent, its position is directly controlled by the parent rigid-body).
    pub fn copy_from(&mut self, other: &RigidBody) {
        // NOTE: we deconstruct the rigid-body struct to be sure we don’t forget to
        //       add some copies here if we add more field to RigidBody in the future.
        let RigidBody {
            pos,
            mprops,
            ccd_vels: integrated_vels,
            vels,
            damping,
            forces,
            ccd,
            ids: _ids,             // Internal ids must not be overwritten.
            colliders: _colliders, // This function cannot be used to edit collider sets.
            activation,
            changes: _changes, // Will be set to ALL.
            body_type,
            dominance,
            enabled,
            additional_solver_iterations,
            user_data,
        } = other;

        self.pos = *pos;
        self.mprops = mprops.clone();
        self.ccd_vels = *integrated_vels;
        self.vels = *vels;
        self.damping = *damping;
        self.forces = *forces;
        self.ccd = *ccd;
        self.activation = *activation;
        self.body_type = *body_type;
        self.dominance = *dominance;
        self.enabled = *enabled;
        self.additional_solver_iterations = *additional_solver_iterations;
        self.user_data = *user_data;

        self.changes = RigidBodyChanges::all();
    }

    /// The additional number of solver iterations run for the constraints directly
    /// involving this rigid-body.
    ///
    /// See [`Self::set_additional_solver_iterations`] for additional information.
    pub fn additional_solver_iterations(&self) -> usize {
        self.additional_solver_iterations
    }

    /// Set the additional number of solver substeps run for the simulation island containing this
    /// rigid-body (default: 0). Each extra substep re-derives the soft constraint bias at a smaller
    /// timestep, improving accuracy for stiff couplings (joint chains, high mass-ratio stacks). The
    /// whole connected component (contacts + joints) runs `num_solver_iterations +
    /// max(additional_solver_iterations)` substeps, so the cost scales with component size —
    /// attaching an elevated body to a large pile substeps the pile too.
    pub fn set_additional_solver_iterations(&mut self, additional_iterations: usize) {
        self.additional_solver_iterations = additional_iterations;
    }

    /// The activation status of this rigid-body.
    pub fn activation(&self) -> &RigidBodyActivation {
        &self.activation
    }

    /// Mutable reference to the activation status of this rigid-body.
    pub fn activation_mut(&mut self) -> &mut RigidBodyActivation {
        self.changes |= RigidBodyChanges::SLEEP;
        &mut self.activation
    }

    /// Is this rigid-body enabled?
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Sets whether this rigid-body is enabled or not.
    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled != self.enabled {
            if enabled {
                // NOTE: this is probably overkill, but it makes sure we don’t
                // forget anything that needs to be updated because the rigid-body
                // was basically interpreted as if it was removed while it was
                // disabled.
                self.changes = RigidBodyChanges::all();
            } else {
                self.changes |= RigidBodyChanges::ENABLED_OR_DISABLED;
            }

            self.enabled = enabled;
        }
    }

    /// The linear damping coefficient (velocity reduction over time).
    ///
    /// Damping gradually slows down moving objects. `0.0` = no damping (infinite momentum),
    /// higher values = faster slowdown. Use for air resistance, friction, etc.
    #[inline]
    pub fn linear_damping(&self) -> Real {
        self.damping.linear_damping
    }

    /// Sets how quickly linear velocity decreases over time.
    ///
    /// - `0.0` = no slowdown (space/frictionless)
    /// - `0.1` = gradual slowdown (air resistance)
    /// - `1.0+` = rapid slowdown (thick fluid)
    #[inline]
    pub fn set_linear_damping(&mut self, damping: Real) {
        self.damping.linear_damping = damping;
    }

    /// The angular damping coefficient (rotation slowdown over time).
    ///
    /// Like linear damping but for rotation. Higher values make spinning objects stop faster.
    #[inline]
    pub fn angular_damping(&self) -> Real {
        self.damping.angular_damping
    }

    /// Sets how quickly angular velocity decreases over time.
    ///
    /// Controls how fast spinning objects slow down.
    #[inline]
    pub fn set_angular_damping(&mut self, damping: Real) {
        self.damping.angular_damping = damping
    }

    /// The type of this rigid-body.
    pub fn body_type(&self) -> RigidBodyType {
        self.body_type
    }

    /// Sets the type of this rigid-body.
    pub fn set_body_type(&mut self, status: RigidBodyType, wake_up: bool) {
        if status != self.body_type {
            self.changes.insert(RigidBodyChanges::TYPE);
            self.body_type = status;

            if status == RigidBodyType::Fixed {
                self.vels = RigidBodyVelocity::zero();
            }

            // The effective mass-properties depend on the body type (kinematic and
            // fixed bodies have zero effective inverse masses).
            self.update_world_mass_properties();

            if self.is_dynamic_or_kinematic() && wake_up {
                self.wake_up(true);
            }
        }
    }

    /// The center of mass position in world coordinates.
    ///
    /// This is the "balance point" where the body's mass is centered. Forces applied here
    /// produce no rotation, only translation.
    #[inline]
    pub fn center_of_mass(&self) -> Vector {
        self.mprops.world_com
    }

    /// The center of mass in the body's local coordinate system.
    ///
    /// This is relative to the body's position, computed from attached colliders.
    #[inline]
    pub fn local_center_of_mass(&self) -> Vector {
        self.mprops.local_mprops.local_com
    }

    /// The mass-properties of this rigid-body.
    #[inline]
    pub fn mass_properties(&self) -> &RigidBodyMassProps {
        &self.mprops
    }

    /// The dominance group of this rigid-body.
    ///
    /// This method always returns `i8::MAX + 1` for non-dynamic
    /// rigid-bodies.
    #[inline]
    pub fn effective_dominance_group(&self) -> i16 {
        self.dominance.effective_group(&self.body_type)
    }

    /// Sets the axes along which this rigid-body cannot translate or rotate.
    #[inline]
    pub fn set_locked_axes(&mut self, locked_axes: LockedAxes, wake_up: bool) {
        if locked_axes != self.mprops.flags {
            if self.is_dynamic_or_kinematic() && wake_up {
                self.wake_up(true);
            }

            self.mprops.flags = locked_axes;
            self.update_world_mass_properties();
        }
    }

    /// The axes along which this rigid-body cannot translate or rotate.
    #[inline]
    pub fn locked_axes(&self) -> LockedAxes {
        self.mprops.flags
    }

    /// Locks or unlocks all rotational movement for this body.
    ///
    /// When locked, the body cannot rotate at all (useful for keeping objects upright).
    /// Use for characters that shouldn't tip over, or objects that should only slide.
    #[inline]
    pub fn lock_rotations(&mut self, locked: bool, wake_up: bool) {
        if locked != self.mprops.flags.contains(LockedAxes::ROTATION_LOCKED) {
            if self.is_dynamic_or_kinematic() && wake_up {
                self.wake_up(true);
            }

            self.mprops.flags.set(LockedAxes::ROTATION_LOCKED_X, locked);
            self.mprops.flags.set(LockedAxes::ROTATION_LOCKED_Y, locked);
            self.mprops.flags.set(LockedAxes::ROTATION_LOCKED_Z, locked);
            self.update_world_mass_properties();
        }
    }

    #[inline]
    /// Locks or unlocks rotations of this rigid-body along each cartesian axes.
    pub fn set_enabled_rotations(
        &mut self,
        allow_rotations_x: bool,
        allow_rotations_y: bool,
        allow_rotations_z: bool,
        wake_up: bool,
    ) {
        if self.mprops.flags.contains(LockedAxes::ROTATION_LOCKED_X) == allow_rotations_x
            || self.mprops.flags.contains(LockedAxes::ROTATION_LOCKED_Y) == allow_rotations_y
            || self.mprops.flags.contains(LockedAxes::ROTATION_LOCKED_Z) == allow_rotations_z
        {
            if self.is_dynamic_or_kinematic() && wake_up {
                self.wake_up(true);
            }

            self.mprops
                .flags
                .set(LockedAxes::ROTATION_LOCKED_X, !allow_rotations_x);
            self.mprops
                .flags
                .set(LockedAxes::ROTATION_LOCKED_Y, !allow_rotations_y);
            self.mprops
                .flags
                .set(LockedAxes::ROTATION_LOCKED_Z, !allow_rotations_z);
            self.update_world_mass_properties();
        }
    }

    /// Locks or unlocks rotations of this rigid-body along each cartesian axes.
    #[deprecated(note = "Use `set_enabled_rotations` instead")]
    pub fn restrict_rotations(
        &mut self,
        allow_rotations_x: bool,
        allow_rotations_y: bool,
        allow_rotations_z: bool,
        wake_up: bool,
    ) {
        self.set_enabled_rotations(
            allow_rotations_x,
            allow_rotations_y,
            allow_rotations_z,
            wake_up,
        );
    }

    /// Locks or unlocks all translational movement for this body.
    ///
    /// When locked, the body cannot move from its position (but can still rotate).
    /// Use for rotating platforms, turrets, or objects fixed in space.
    #[inline]
    pub fn lock_translations(&mut self, locked: bool, wake_up: bool) {
        if locked != self.mprops.flags.contains(LockedAxes::TRANSLATION_LOCKED) {
            if self.is_dynamic_or_kinematic() && wake_up {
                self.wake_up(true);
            }

            self.mprops
                .flags
                .set(LockedAxes::TRANSLATION_LOCKED, locked);
            self.update_world_mass_properties();
        }
    }

    #[inline]
    /// Locks or unlocks rotations of this rigid-body along each cartesian axes.
    pub fn set_enabled_translations(
        &mut self,
        allow_translation_x: bool,
        allow_translation_y: bool,
        #[cfg(feature = "dim3")] allow_translation_z: bool,
        wake_up: bool,
    ) {
        #[cfg(feature = "dim2")]
        if self.mprops.flags.contains(LockedAxes::TRANSLATION_LOCKED_X) != allow_translation_x
            && self.mprops.flags.contains(LockedAxes::TRANSLATION_LOCKED_Y) != allow_translation_y
        {
            // Nothing to change.
            return;
        }
        #[cfg(feature = "dim3")]
        if self.mprops.flags.contains(LockedAxes::TRANSLATION_LOCKED_X) != allow_translation_x
            && self.mprops.flags.contains(LockedAxes::TRANSLATION_LOCKED_Y) != allow_translation_y
            && self.mprops.flags.contains(LockedAxes::TRANSLATION_LOCKED_Z) != allow_translation_z
        {
            // Nothing to change.
            return;
        }

        if self.is_dynamic_or_kinematic() && wake_up {
            self.wake_up(true);
        }

        self.mprops
            .flags
            .set(LockedAxes::TRANSLATION_LOCKED_X, !allow_translation_x);
        self.mprops
            .flags
            .set(LockedAxes::TRANSLATION_LOCKED_Y, !allow_translation_y);
        #[cfg(feature = "dim3")]
        self.mprops
            .flags
            .set(LockedAxes::TRANSLATION_LOCKED_Z, !allow_translation_z);
        self.update_world_mass_properties();
    }

    #[inline]
    #[deprecated(note = "Use `set_enabled_translations` instead")]
    /// Locks or unlocks rotations of this rigid-body along each cartesian axes.
    pub fn restrict_translations(
        &mut self,
        allow_translation_x: bool,
        allow_translation_y: bool,
        #[cfg(feature = "dim3")] allow_translation_z: bool,
        wake_up: bool,
    ) {
        self.set_enabled_translations(
            allow_translation_x,
            allow_translation_y,
            #[cfg(feature = "dim3")]
            allow_translation_z,
            wake_up,
        )
    }

    /// Are the translations of this rigid-body locked?
    #[cfg(feature = "dim2")]
    pub fn is_translation_locked(&self) -> bool {
        self.mprops
            .flags
            .contains(LockedAxes::TRANSLATION_LOCKED_X | LockedAxes::TRANSLATION_LOCKED_Y)
    }

    /// Are the translations of this rigid-body locked?
    #[cfg(feature = "dim3")]
    pub fn is_translation_locked(&self) -> bool {
        self.mprops.flags.contains(LockedAxes::TRANSLATION_LOCKED)
    }

    /// Are the rotations of this rigid-body locked?
    #[cfg(feature = "dim2")]
    pub fn is_rotation_locked(&self) -> bool {
        self.mprops.flags.contains(LockedAxes::ROTATION_LOCKED_Z)
    }

    /// Returns `true` for each rotational degrees of freedom locked on this rigid-body.
    #[cfg(feature = "dim3")]
    pub fn is_rotation_locked(&self) -> [bool; 3] {
        [
            self.mprops.flags.contains(LockedAxes::ROTATION_LOCKED_X),
            self.mprops.flags.contains(LockedAxes::ROTATION_LOCKED_Y),
            self.mprops.flags.contains(LockedAxes::ROTATION_LOCKED_Z),
        ]
    }

    /// Enables or disables full ("bullet") CCD: fast dynamic bodies already sweep **fixed**
    /// colliders automatically (unless [`IntegrationParameters::max_ccd_substeps`] is `0`); this
    /// upgrades the body to also sweep **kinematic and dynamic** bodies at extra CPU cost —
    /// for projectiles that must not tunnel through other moving bodies. A bullet never
    /// sweeps another bullet, so two bullets can still tunnel through each other.
    pub fn enable_ccd(&mut self, enabled: bool) {
        self.ccd.ccd_enabled = enabled;
    }

    /// Checks if full ("bullet") CCD is enabled: whether this body sweeps against all bodies
    /// rather than only fixed colliders. Independent from whether CCD is *active* this frame
    /// ([`RigidBody::is_ccd_active`]) and from the automatic fixed-collider CCD of fast dynamic bodies.
    pub fn is_ccd_enabled(&self) -> bool {
        self.ccd.ccd_enabled
    }

    /// Sets the maximum prediction distance Soft Continuous Collision-Detection.
    ///
    /// When set to 0, soft-CCD is disabled. Soft-CCD helps prevent tunneling especially of
    /// slow-but-thin to moderately fast objects. The soft CCD prediction distance indicates how
    /// far in the object’s path the CCD algorithm is allowed to inspect. Large values can impact
    /// performance badly by increasing the work needed from the broad-phase.
    ///
    /// It is a generally cheaper variant of regular CCD (that can be enabled with
    /// [`RigidBody::enable_ccd`] since it relies on predictive constraints instead of
    /// shape-cast and substeps.
    pub fn set_soft_ccd_prediction(&mut self, prediction_distance: Real) {
        self.ccd.soft_ccd_prediction = prediction_distance;
    }

    /// The soft-CCD prediction distance for this rigid-body.
    ///
    /// See the documentation of [`RigidBody::set_soft_ccd_prediction`] for additional details on
    /// soft-CCD.
    pub fn soft_ccd_prediction(&self) -> Real {
        self.ccd.soft_ccd_prediction
    }

    /// Allow (or disallow) this body to exceed the angular speed cap.
    ///
    /// By default angular velocity is clamped each substep to ~45°/step to keep CCD reliable;
    /// pass `true` for bodies that must spin fast, e.g. wheels.
    pub fn set_allow_fast_rotation(&mut self, allow: bool) {
        self.ccd.allow_fast_rotation = allow;
    }

    /// Is this body allowed to exceed the angular speed cap?
    ///
    /// See [`RigidBody::set_allow_fast_rotation`].
    pub fn is_fast_rotation_allowed(&self) -> bool {
        self.ccd.allow_fast_rotation
    }

    // This is different from `is_ccd_enabled`. This checks that CCD
    // is active for this rigid-body, i.e., if it was seen to move fast
    // enough to justify a CCD run.
    /// Is CCD active for this rigid-body?
    ///
    /// Set for *any* dynamic body moving faster than an automatically-computed threshold (which
    /// then sweeps fixed colliders), not only bodies with [`RigidBody::is_ccd_enabled`] — which
    /// only says whether the body is upgraded to sweep all bodies, independently of its velocity.
    pub fn is_ccd_active(&self) -> bool {
        self.ccd.ccd_active
    }

    /// Recalculates mass, center of mass, and inertia from attached colliders.
    ///
    /// Normally automatic, but call this if you modify collider shapes/masses at runtime.
    /// Only needed after directly modifying colliders without going through the builder.
    pub fn recompute_mass_properties_from_colliders(&mut self, colliders: &ColliderSet) {
        self.mprops.recompute_mass_properties_from_colliders(
            colliders,
            &self.colliders,
            self.body_type,
            &self.pos.position,
        );
    }

    /// Adds extra mass on top of collider-computed mass.
    ///
    /// Total mass = collider masses + this additional mass. Use when you want to make
    /// a body heavier without changing collider densities.
    ///
    /// # Example
    /// ```
    /// # use rapier3d::prelude::*;
    /// # let mut bodies = RigidBodySet::new();
    /// # let body = bodies.insert(RigidBodyBuilder::dynamic());
    /// // Add 50kg to make this body heavier
    /// bodies[body].set_additional_mass(50.0, true);
    /// ```
    ///
    /// Angular inertia is automatically scaled to match the mass increase.
    /// Updated automatically at next physics step or call `recompute_mass_properties_from_colliders()`.
    #[inline]
    pub fn set_additional_mass(&mut self, additional_mass: Real, wake_up: bool) {
        self.do_set_additional_mass_properties(
            RigidBodyAdditionalMassProps::Mass(additional_mass),
            wake_up,
        )
    }

    /// Sets the rigid-body's additional mass-properties.
    ///
    /// This is only the "additional" mass-properties because the total mass-properties of the
    /// rigid-body is equal to the sum of this additional mass-properties and the mass computed from
    /// the colliders (with non-zero densities) attached to this rigid-body.
    ///
    /// That total mass-properties (which include the attached colliders’ contributions)
    /// will be updated at the name physics step, or can be updated manually with
    /// [`Self::recompute_mass_properties_from_colliders`].
    ///
    /// This will override any previous mass-properties set by [`Self::set_additional_mass`],
    /// [`Self::set_additional_mass_properties`], [`RigidBodyBuilder::additional_mass`], or
    /// [`RigidBodyBuilder::additional_mass_properties`] for this rigid-body.
    ///
    /// If `wake_up` is `true` then the rigid-body will be woken up if it was
    /// put to sleep because it did not move for a while.
    #[inline]
    pub fn set_additional_mass_properties(&mut self, props: MassProperties, wake_up: bool) {
        self.do_set_additional_mass_properties(
            RigidBodyAdditionalMassProps::MassProps(props),
            wake_up,
        )
    }

    fn do_set_additional_mass_properties(
        &mut self,
        props: RigidBodyAdditionalMassProps,
        wake_up: bool,
    ) {
        let new_mprops = Some(Box::new(props));

        if self.mprops.additional_local_mprops != new_mprops {
            self.changes.insert(RigidBodyChanges::LOCAL_MASS_PROPERTIES);
            self.mprops.additional_local_mprops = new_mprops;

            if self.is_dynamic_or_kinematic() && wake_up {
                self.wake_up(true);
            }
        }
    }

    /// Returns handles of all colliders attached to this body.
    ///
    /// Use to iterate over a body's collision shapes or to modify them.
    ///
    /// # Example
    /// ```
    /// # use rapier3d::prelude::*;
    /// # let mut bodies = RigidBodySet::new();
    /// # let mut colliders = ColliderSet::new();
    /// # let body = bodies.insert(RigidBodyBuilder::dynamic());
    /// # colliders.insert_with_parent(ColliderBuilder::ball(0.5), body, &mut bodies);
    /// for collider_handle in bodies[body].colliders() {
    ///     if let Some(collider) = colliders.get_mut(*collider_handle) {
    ///         collider.set_friction(0.5);
    ///     }
    /// }
    /// ```
    pub fn colliders(&self) -> &[ColliderHandle] {
        &self.colliders.0[..]
    }

    /// Checks if this is a dynamic body (moves via forces and collisions).
    ///
    /// Dynamic bodies are fully simulated and respond to gravity, forces, and collisions.
    pub fn is_dynamic(&self) -> bool {
        self.body_type == RigidBodyType::Dynamic
    }

    /// Checks if this is a kinematic body (moves via direct velocity/position control).
    ///
    /// Kinematic bodies move by setting velocity directly, not by applying forces.
    pub fn is_kinematic(&self) -> bool {
        self.body_type.is_kinematic()
    }

    /// Is this rigid-body a dynamic rigid-body or a kinematic rigid-body?
    ///
    /// This method is mostly convenient internally where kinematic and dynamic rigid-body
    /// are subject to the same behavior.
    pub fn is_dynamic_or_kinematic(&self) -> bool {
        self.body_type.is_dynamic_or_kinematic()
    }

    /// The offset index in the solver’s active set, or `u32::MAX` if
    /// the rigid-body isn’t dynamic or kinematic.
    // TODO: is this really necessary? Could we just always assign u32::MAX
    //       to all the fixed bodies active set offsets?
    pub fn effective_active_set_offset(&self) -> u32 {
        if self.is_dynamic_or_kinematic() {
            self.ids.active_set_id
        } else {
            u32::MAX
        }
    }

    /// Checks if this is a fixed body (never moves, infinite mass).
    ///
    /// Fixed bodies are static geometry: walls, floors, terrain. They never move
    /// and are not affected by any forces or collisions.
    pub fn is_fixed(&self) -> bool {
        self.body_type == RigidBodyType::Fixed
    }

    /// The mass of this rigid body in kilograms.
    ///
    /// Returns zero for fixed bodies (which technically have infinite mass).
    /// Mass is computed from attached colliders' shapes and densities.
    pub fn mass(&self) -> Real {
        self.mprops.local_mprops.mass()
    }

    /// The predicted position of this rigid-body.
    ///
    /// If this rigid-body is kinematic this value is set by the `set_next_kinematic_position`
    /// method and is used for estimating the kinematic body velocity at the next timestep.
    /// For non-kinematic bodies, this value is currently unspecified.
    pub fn next_position(&self) -> &Pose {
        &self.pos.next_position
    }

    /// The gravity scale multiplier for this body.
    ///
    /// - `1.0` (default) = normal gravity
    /// - `0.0` = no gravity (floating)
    /// - `2.0` = double gravity (heavy/fast falling)
    /// - Negative values = reverse gravity (objects fall upward!)
    pub fn gravity_scale(&self) -> Real {
        self.forces.gravity_scale
    }

    /// Sets how much gravity affects this body (multiplier).
    ///
    /// # Examples
    /// ```
    /// # use rapier3d::prelude::*;
    /// # let mut bodies = RigidBodySet::new();
    /// # let body = bodies.insert(RigidBodyBuilder::dynamic());
    /// bodies[body].set_gravity_scale(0.0, true);  // Zero-G (space)
    /// bodies[body].set_gravity_scale(0.1, true);  // Moon gravity
    /// bodies[body].set_gravity_scale(2.0, true);  // Extra heavy
    /// ```
    pub fn set_gravity_scale(&mut self, scale: Real, wake_up: bool) {
        if self.forces.gravity_scale != scale {
            if wake_up && self.activation.sleeping {
                self.changes.insert(RigidBodyChanges::SLEEP);
                self.activation.sleeping = false;
            }

            self.forces.gravity_scale = scale;
        }
    }

    /// The dominance group of this rigid-body.
    pub fn dominance_group(&self) -> i8 {
        self.dominance.0
    }

    /// The dominance group of this rigid-body.
    pub fn set_dominance_group(&mut self, dominance: i8) {
        if self.dominance.0 != dominance {
            self.changes.insert(RigidBodyChanges::DOMINANCE);
            self.dominance.0 = dominance
        }
    }

    /// Adds a collider to this rigid-body.
    pub(crate) fn add_collider_internal(
        &mut self,
        co_handle: ColliderHandle,
        co_parent: &ColliderParent,
        co_pos: &mut ColliderPosition,
        co_shape: &ColliderShape,
        co_mprops: &ColliderMassProps,
    ) {
        self.colliders.attach_collider(
            self.body_type,
            &mut self.changes,
            &mut self.ccd,
            &mut self.mprops,
            &self.pos,
            co_handle,
            co_pos,
            co_parent,
            co_shape,
            co_mprops,
        )
    }

    /// Removes a collider from this rigid-body.
    pub(crate) fn remove_collider_internal(&mut self, handle: ColliderHandle) {
        if let Some(i) = self.colliders.0.iter().position(|e| *e == handle) {
            self.changes.set(RigidBodyChanges::COLLIDERS, true);
            self.colliders.0.swap_remove(i);
        }
    }

    /// Forces this body to sleep immediately (stop simulating it).
    ///
    /// Sleeping bodies are excluded from physics simulation until disturbed. Use to manually
    /// deactivate bodies you know won't move for a while.
    ///
    /// The body will auto-wake if:
    /// - Hit by a moving object
    /// - Connected via joint to a moving body
    /// - Manually woken with `wake_up()`
    pub fn sleep(&mut self) {
        self.activation.sleep();
        self.vels = RigidBodyVelocity::zero();
    }

    /// Wakes up this body if it's sleeping, making it active in the simulation.
    ///
    /// # Parameters
    /// * `strong` - If `true`, guarantees the body stays awake for multiple frames.
    ///   If `false`, it might sleep again immediately if conditions are met.
    ///
    /// Use after manually moving a sleeping body or to keep it active temporarily.
    pub fn wake_up(&mut self, strong: bool) {
        if self.activation.sleeping {
            self.changes.insert(RigidBodyChanges::SLEEP);
        }

        self.activation.wake_up(strong);
    }

    /// Is this rigid body sleeping?
    pub fn is_sleeping(&self) -> bool {
        // TODO: should we:
        // - return false for fixed bodies.
        // - return true for non-sleeping dynamic bodies.
        // - return true only for kinematic bodies with non-zero velocity?
        self.activation.sleeping
    }

    /// Returns `true` if the body has non-zero linear or angular velocity.
    ///
    /// Useful for checking if an object is actually moving vs sitting still.
    pub fn is_moving(&self) -> bool {
        #[cfg(feature = "dim2")]
        let angvel_is_nonzero = self.vels.angvel != 0.0;
        #[cfg(feature = "dim3")]
        let angvel_is_nonzero = self.vels.angvel != Default::default();
        self.vels.linvel != Default::default() || angvel_is_nonzero
    }

    /// Returns both linear and angular velocity as a combined structure.
    ///
    /// Most users should use `linvel()` and `angvel()` separately instead.
    pub fn vels(&self) -> &RigidBodyVelocity<Real> {
        &self.vels
    }

    /// The current linear velocity (speed and direction of movement).
    ///
    /// This is how fast the body is moving in units per second. Use with [`set_linvel()`](Self::set_linvel)
    /// to directly control the body's movement speed.
    pub fn linvel(&self) -> Vector {
        self.vels.linvel
    }

    /// The current angular velocity (rotation speed) in 2D.
    ///
    /// Returns radians per second. Positive = counter-clockwise, negative = clockwise.
    #[cfg(feature = "dim2")]
    pub fn angvel(&self) -> Real {
        self.vels.angvel
    }

    /// The current angular velocity (rotation speed) in 3D.
    ///
    /// Returns a vector in radians per second around each axis (X, Y, Z).
    #[cfg(feature = "dim3")]
    pub fn angvel(&self) -> AngVector {
        self.vels.angvel
    }

    /// Set both the angular and linear velocity of this rigid-body.
    ///
    /// If `wake_up` is `true` then the rigid-body will be woken up if it was
    /// put to sleep because it did not move for a while.
    pub fn set_vels(&mut self, vels: RigidBodyVelocity<Real>, wake_up: bool) {
        self.set_linvel(vels.linvel, wake_up);
        #[cfg(feature = "dim2")]
        self.set_angvel(vels.angvel, wake_up);
        #[cfg(feature = "dim3")]
        self.set_angvel(vels.angvel, wake_up);
    }

    /// Sets how fast this body is moving (linear velocity).
    ///
    /// This directly sets the body's velocity without applying forces. Use for:
    /// - Player character movement
    /// - Kinematic object control
    /// - Instantly changing an object's speed
    ///
    /// For physics-based movement, consider using [`apply_impulse()`](Self::apply_impulse) or
    /// [`add_force()`](Self::add_force) instead for more realistic behavior.
    ///
    /// # Example
    /// ```
    /// # use rapier3d::prelude::*;
    /// # let mut bodies = RigidBodySet::new();
    /// # let body = bodies.insert(RigidBodyBuilder::dynamic());
    /// // Make the body move to the right at 5 units/second
    /// bodies[body].set_linvel(Vector::new(5.0, 0.0, 0.0), true);
    /// ```
    pub fn set_linvel(&mut self, linvel: Vector, wake_up: bool) {
        if self.vels.linvel != linvel {
            match self.body_type {
                RigidBodyType::Dynamic | RigidBodyType::KinematicVelocityBased => {
                    self.vels.linvel = linvel;
                    if wake_up {
                        self.wake_up(true)
                    }
                }
                RigidBodyType::Fixed | RigidBodyType::KinematicPositionBased => {}
            }
        }
    }

    /// The angular velocity of this rigid-body.
    ///
    /// If `wake_up` is `true` then the rigid-body will be woken up if it was
    /// put to sleep because it did not move for a while.
    #[cfg(feature = "dim2")]
    pub fn set_angvel(&mut self, angvel: Real, wake_up: bool) {
        if self.vels.angvel != angvel {
            match self.body_type {
                RigidBodyType::Dynamic | RigidBodyType::KinematicVelocityBased => {
                    self.vels.angvel = angvel;
                    if wake_up {
                        self.wake_up(true)
                    }
                }
                RigidBodyType::Fixed | RigidBodyType::KinematicPositionBased => {}
            }
        }
    }

    /// The angular velocity of this rigid-body.
    ///
    /// If `wake_up` is `true` then the rigid-body will be woken up if it was
    /// put to sleep because it did not move for a while.
    #[cfg(feature = "dim3")]
    pub fn set_angvel(&mut self, angvel: AngVector, wake_up: bool) {
        if self.vels.angvel != angvel {
            match self.body_type {
                RigidBodyType::Dynamic | RigidBodyType::KinematicVelocityBased => {
                    self.vels.angvel = angvel;
                    if wake_up {
                        self.wake_up(true)
                    }
                }
                RigidBodyType::Fixed | RigidBodyType::KinematicPositionBased => {}
            }
        }
    }

    /// The current position (translation + rotation) of this rigid body in world space.
    ///
    /// Returns an `SimdPose` which combines both translation and rotation.
    /// For just the position vector, use [`translation()`](Self::translation) instead.
    #[inline]
    pub fn position(&self) -> &Pose {
        &self.pos.position
    }

    /// The current position vector of this rigid body (world coordinates).
    ///
    /// This is just the XYZ location, without rotation. For the full pose (position + rotation),
    /// use [`position()`](Self::position).
    #[inline]
    pub fn translation(&self) -> Vector {
        self.pos.position.translation
    }

    /// Teleports this rigid body to a new position (world coordinates).
    ///
    /// ⚠️ **Warning**: This instantly moves the body, ignoring physics! The body will "teleport"
    /// without checking for collisions in between. Use this for:
    /// - Respawning objects
    /// - Level transitions
    /// - Resetting positions
    ///
    /// For smooth physics-based movement, use velocities or forces instead.
    ///
    /// # Parameters
    /// * `wake_up` - If `true`, prevents the body from immediately going back to sleep
    #[inline]
    pub fn set_translation(&mut self, translation: Vector, wake_up: bool) {
        if self.pos.position.translation != translation
            || self.pos.next_position.translation != translation
        {
            self.changes.insert(RigidBodyChanges::POSITION);
            self.pos.position.translation = translation;
            self.pos.next_position.translation = translation;

            // Update the world mass-properties so torque application remains valid.
            self.update_world_mass_properties();

            // TODO: Do we really need to check that the body isn't dynamic?
            if wake_up && self.is_dynamic_or_kinematic() {
                self.wake_up(true)
            }
        }
    }

    /// The current rotation/orientation of this rigid body.
    #[inline]
    pub fn rotation(&self) -> &Rotation {
        &self.pos.position.rotation
    }

    /// Instantly rotates this rigid body to a new orientation.
    ///
    /// ⚠️ **Warning**: This teleports the rotation, ignoring physics! See [`set_translation()`](Self::set_translation) for details.
    #[inline]
    pub fn set_rotation(&mut self, rotation: Rotation, wake_up: bool) {
        if self.pos.position.rotation != rotation || self.pos.next_position.rotation != rotation {
            self.changes.insert(RigidBodyChanges::POSITION);
            self.pos.position.rotation = rotation;
            self.pos.next_position.rotation = rotation;

            // Update the world mass-properties so torque application remains valid.
            self.update_world_mass_properties();

            // TODO: Do we really need to check that the body isn't dynamic?
            if wake_up && self.is_dynamic_or_kinematic() {
                self.wake_up(true)
            }
        }
    }

    /// Teleports this body to a new position and rotation (ignoring physics).
    ///
    /// ⚠️ **Warning**: Instantly moves the body without checking for collisions!
    /// For position-based kinematic bodies, this also resets their interpolated velocity to zero.
    ///
    /// Use for respawning, level transitions, or resetting positions.
    pub fn set_position(&mut self, pos: Pose, wake_up: bool) {
        if self.pos.position != pos || self.pos.next_position != pos {
            self.changes.insert(RigidBodyChanges::POSITION);
            self.pos.position = pos;
            self.pos.next_position = pos;

            // Update the world mass-properties so torque application remains valid.
            self.update_world_mass_properties();

            // TODO: Do we really need to check that the body isn't dynamic?
            if wake_up && self.is_dynamic_or_kinematic() {
                self.wake_up(true)
            }
        }
    }

    /// For position-based kinematic bodies: sets where the body should rotate to by next frame.
    ///
    /// Only works for `KinematicPositionBased` bodies. Rapier computes the angular velocity
    /// needed to reach this rotation smoothly.
    pub fn set_next_kinematic_rotation(&mut self, rotation: Rotation) {
        if self.is_kinematic() {
            self.pos.next_position.rotation = rotation;

            if self.pos.position.rotation != rotation {
                self.wake_up(true);
            }
        }
    }

    /// For position-based kinematic bodies: sets where the body should move to by next frame.
    ///
    /// Only works for `KinematicPositionBased` bodies. Rapier computes the velocity
    /// needed to reach this position smoothly.
    pub fn set_next_kinematic_translation(&mut self, translation: Vector) {
        if self.is_kinematic() {
            self.pos.next_position.translation = translation;

            if self.pos.position.translation != translation {
                self.wake_up(true);
            }
        }
    }

    /// For position-based kinematic bodies: sets the target pose (position + rotation) for next frame.
    ///
    /// Only works for `KinematicPositionBased` bodies. Combines translation and rotation control.
    pub fn set_next_kinematic_position(&mut self, pos: Pose) {
        if self.is_kinematic() {
            self.pos.next_position = pos;

            if self.pos.position != pos {
                self.wake_up(true);
            }
        }
    }

    /// Predicts the next position of this rigid-body, by integrating its velocity and forces
    /// by a time of `dt`.
    pub(crate) fn predict_position_using_velocity_and_forces_with_max_dist(
        &self,
        dt: Real,
        max_dist: Real,
    ) -> Pose {
        let new_vels = self.forces.integrate(dt, &self.vels, &self.mprops);
        // Compute the clamped dt such that the body doesn't travel more than `max_dist`.
        let linvel_norm = new_vels.linvel.length();
        let clamped_linvel = linvel_norm.min(max_dist * crate::utils::inv(dt));
        let clamped_dt = dt * clamped_linvel * crate::utils::inv(linvel_norm);
        new_vels.integrate(
            clamped_dt,
            &self.pos.position,
            &self.mprops.local_mprops.local_com,
        )
    }

    /// Calculates where this body will be after `dt` seconds, considering current velocity AND forces.
    ///
    /// Useful for predicting future positions or implementing custom integration.
    /// Accounts for gravity and applied forces.
    pub fn predict_position_using_velocity_and_forces(&self, dt: Real) -> Pose {
        self.pos
            .integrate_forces_and_velocities(dt, &self.forces, &self.vels, &self.mprops)
    }

    /// Calculates where this body will be after `dt` seconds, considering only current velocity (not forces).
    ///
    /// Like `predict_position_using_velocity_and_forces()` but ignores applied forces.
    /// Useful when you only care about inertial motion without acceleration.
    pub fn predict_position_using_velocity(&self, dt: Real) -> Pose {
        self.vels
            .integrate(dt, &self.pos.position, &self.mprops.local_mprops.local_com)
    }

    pub(crate) fn update_world_mass_properties(&mut self) {
        self.mprops
            .update_world_mass_properties(self.body_type, &self.pos.position);
    }
}

include!("rigid_body_more.rs");

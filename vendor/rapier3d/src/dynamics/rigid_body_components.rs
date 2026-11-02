#[cfg(doc)]
use super::IntegrationParameters;
use crate::alloc_prelude::*;
use crate::control::PdErrors;
#[cfg(doc)]
use crate::control::PidController;
use crate::dynamics::MassProperties;
use crate::geometry::{
    ColliderChanges, ColliderHandle, ColliderMassProps, ColliderParent, ColliderPosition,
    ColliderSet, ColliderShape, ModifiedColliders,
};
use crate::math::{AngVector, AngularInertia, Pose, Real, Rotation, Vector};
use crate::utils::{
    AngularInertiaOps, CrossProduct, DotProduct, PoseOps, ScalarType, SimdRealCopy,
};
use num::Zero;
#[cfg(feature = "dim2")]
use parry::math::Rot2;

/// The type of a body, governing the way it is affected by external forces.
#[deprecated(note = "renamed as RigidBodyType")]
pub type BodyStatus = RigidBodyType;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
/// The type of a rigid body, determining how it responds to forces and movement.
pub enum RigidBodyType {
    /// Fully simulated - responds to forces, gravity, and collisions.
    ///
    /// Use for: Falling objects, projectiles, physics-based characters, anything that should
    /// behave realistically under physics simulation.
    Dynamic = 0,

    /// Never moves - has infinite mass and is unaffected by anything.
    ///
    /// Use for: Static level geometry, walls, floors, terrain, buildings.
    Fixed = 1,

    /// Controlled by setting next position - pushes but isn't pushed.
    ///
    /// You control this by setting where it should be next frame. Rapier computes the
    /// velocity needed to get there. The body can push dynamic bodies but nothing can
    /// push it back (one-way interaction).
    ///
    /// Use for: Animated platforms, objects controlled by external animation systems.
    KinematicPositionBased = 2,

    /// Controlled by setting velocity - pushes but isn't pushed.
    ///
    /// You control this by setting its velocity directly. It moves predictably regardless
    /// of what it hits. Can push dynamic bodies but nothing can push it back (one-way interaction).
    ///
    /// Use for: Moving platforms, elevators, doors, player-controlled characters (when you want
    /// direct control rather than physics-based movement).
    KinematicVelocityBased = 3,
    // Semikinematic, // A kinematic that performs automatic CCD with the fixed environment to avoid traversing it?
    // Disabled,
}

impl RigidBodyType {
    /// Is this rigid-body fixed (i.e. cannot move)?
    pub fn is_fixed(self) -> bool {
        self == RigidBodyType::Fixed
    }

    /// Is this rigid-body dynamic (i.e. can move and be affected by forces)?
    pub fn is_dynamic(self) -> bool {
        self == RigidBodyType::Dynamic
    }

    /// Is this rigid-body kinematic (i.e. can move but is unaffected by forces)?
    pub fn is_kinematic(self) -> bool {
        self == RigidBodyType::KinematicPositionBased
            || self == RigidBodyType::KinematicVelocityBased
    }

    /// Is this rigid-body a dynamic rigid-body or a kinematic rigid-body?
    ///
    /// This method is mostly convenient internally where kinematic and dynamic rigid-body
    /// are subject to the same behavior.
    pub fn is_dynamic_or_kinematic(self) -> bool {
        self != RigidBodyType::Fixed
    }
}

bitflags::bitflags! {
    #[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    /// Flags describing how the rigid-body has been modified by the user.
    pub struct RigidBodyChanges: u32 {
        /// Flag indicating that this rigid-body is in the modified rigid-body set.
        const IN_MODIFIED_SET = 1 << 0;
        /// Flag indicating that the `RigidBodyPosition` component of this rigid-body has been modified.
        const POSITION    = 1 << 1;
        /// Flag indicating that the `RigidBodyActivation` component of this rigid-body has been modified.
        const SLEEP       = 1 << 2;
        /// Flag indicating that the `RigidBodyColliders` component of this rigid-body has been modified.
        const COLLIDERS   = 1 << 3;
        /// Flag indicating that the `RigidBodyType` component of this rigid-body has been modified.
        const TYPE        = 1 << 4;
        /// Flag indicating that the `RigidBodyDominance` component of this rigid-body has been modified.
        const DOMINANCE   = 1 << 5;
        /// Flag indicating that the local mass-properties of this rigid-body must be recomputed.
        const LOCAL_MASS_PROPERTIES = 1 << 6;
        /// Flag indicating that the rigid-body was enabled or disabled.
        const ENABLED_OR_DISABLED = 1 << 7;
    }
}

impl Default for RigidBodyChanges {
    fn default() -> Self {
        RigidBodyChanges::empty()
    }
}

#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Copy, PartialEq)]
/// The position of this rigid-body.
pub struct RigidBodyPosition {
    /// The world-space position of the rigid-body.
    pub position: Pose,
    /// The next position of the rigid-body.
    ///
    /// At the beginning of the timestep, and when the
    /// timestep is complete we must have position == next_position
    /// except for position-based kinematic bodies.
    ///
    /// The next_position is updated after the velocity and position
    /// resolution. Then it is either validated (ie. we set position := set_position)
    /// or clamped by CCD.
    pub next_position: Pose,
}

impl Default for RigidBodyPosition {
    fn default() -> Self {
        Self {
            position: Pose::IDENTITY,
            next_position: Pose::IDENTITY,
        }
    }
}

impl RigidBodyPosition {
    /// Computes the velocity need to travel from `self.position` to `self.next_position` in
    /// a time equal to `1.0 / inv_dt`.
    #[must_use]
    pub fn interpolate_velocity(&self, inv_dt: Real, local_com: Vector) -> RigidBodyVelocity<Real> {
        let pose_err = self.pose_errors(local_com);
        RigidBodyVelocity {
            linvel: pose_err.linear * inv_dt,
            angvel: pose_err.angular * inv_dt,
        }
    }

    /// Compute new positions after integrating the given forces and velocities.
    ///
    /// This uses a symplectic Euler integration scheme.
    #[must_use]
    pub fn integrate_forces_and_velocities(
        &self,
        dt: Real,
        forces: &RigidBodyForces,
        vels: &RigidBodyVelocity<Real>,
        mprops: &RigidBodyMassProps,
    ) -> Pose {
        let new_vels = forces.integrate(dt, vels, mprops);
        let local_com = mprops.local_mprops.local_com;
        new_vels.integrate(dt, &self.position, &local_com)
    }

    /// Computes the difference between [`Self::next_position`] and [`Self::position`].
    ///
    /// This error measure can for example be used for interpolating the velocity between two poses,
    /// or be given to the [`PidController`].
    ///
    /// Note that interpolating the velocity can be done more conveniently with
    /// [`Self::interpolate_velocity`].
    pub fn pose_errors(&self, local_com: Vector) -> PdErrors {
        let com = self.position * local_com;
        let shift = Pose::from_translation(com);
        let dpos = shift.inverse() * self.next_position * self.position.inverse() * shift;

        let angular;
        #[cfg(feature = "dim2")]
        {
            angular = dpos.rotation.angle();
        }
        #[cfg(feature = "dim3")]
        {
            angular = dpos.rotation.to_scaled_axis();
        }
        let linear = dpos.translation;

        PdErrors { linear, angular }
    }
}

impl<T> From<T> for RigidBodyPosition
where
    Pose: From<T>,
{
    fn from(position: T) -> Self {
        let position = position.into();
        Self {
            position,
            next_position: position,
        }
    }
}

bitflags::bitflags! {
    #[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    /// Flags affecting the behavior of the constraints solver for a given contact manifold.
    pub struct AxesMask: u8 {
        /// The translational X axis.
        const LIN_X = 1 << 0;
        /// The translational Y axis.
        const LIN_Y = 1 << 1;
        /// The translational Z axis.
        #[cfg(feature = "dim3")]
        const LIN_Z = 1 << 2;
        /// The rotational X axis.
        #[cfg(feature = "dim3")]
        const ANG_X = 1 << 3;
        /// The rotational Y axis.
        #[cfg(feature = "dim3")]
        const ANG_Y = 1 << 4;
        /// The rotational Z axis.
        const ANG_Z = 1 << 5;
    }
}

impl Default for AxesMask {
    fn default() -> Self {
        AxesMask::empty()
    }
}

bitflags::bitflags! {
    #[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    /// Flags that lock specific movement axes to prevent translation or rotation.
    ///
    /// Use this to constrain body movement to specific directions/axes. Common uses:
    /// - **2D games in 3D**: Lock Z translation and X/Y rotation to keep everything in the XY plane
    /// - **Upright characters**: Lock rotations to prevent tipping over
    /// - **Sliding objects**: Lock rotation while allowing translation
    /// - **Spinning objects**: Lock translation while allowing rotation
    ///
    /// # Example
    /// ```
    /// # use rapier3d::prelude::*;
    /// # let mut bodies = RigidBodySet::new();
    /// # let body_handle = bodies.insert(RigidBodyBuilder::dynamic());
    /// # let body = bodies.get_mut(body_handle).unwrap();
    /// // Character that can't tip over (rotation locked, but can move)
    /// body.set_locked_axes(LockedAxes::ROTATION_LOCKED, true);
    ///
    /// // Object that slides but doesn't rotate
    /// body.set_locked_axes(LockedAxes::ROTATION_LOCKED, true);
    ///
    /// // 2D game in 3D engine (lock Z movement and X/Y rotation)
    /// body.set_locked_axes(
    ///     LockedAxes::TRANSLATION_LOCKED_Z |
    ///     LockedAxes::ROTATION_LOCKED_X |
    ///     LockedAxes::ROTATION_LOCKED_Y,
    ///     true
    /// );
    /// ```
    pub struct LockedAxes: u8 {
        /// Prevents movement along the X axis.
        const TRANSLATION_LOCKED_X = 1 << 0;
        /// Prevents movement along the Y axis.
        const TRANSLATION_LOCKED_Y = 1 << 1;
        /// Prevents movement along the Z axis.
        const TRANSLATION_LOCKED_Z = 1 << 2;
        /// Prevents all translational movement.
        const TRANSLATION_LOCKED = Self::TRANSLATION_LOCKED_X.bits() | Self::TRANSLATION_LOCKED_Y.bits() | Self::TRANSLATION_LOCKED_Z.bits();
        /// Prevents rotation around the X axis.
        const ROTATION_LOCKED_X = 1 << 3;
        /// Prevents rotation around the Y axis.
        const ROTATION_LOCKED_Y = 1 << 4;
        /// Prevents rotation around the Z axis.
        const ROTATION_LOCKED_Z = 1 << 5;
        /// Prevents all rotational movement.
        const ROTATION_LOCKED = Self::ROTATION_LOCKED_X.bits() | Self::ROTATION_LOCKED_Y.bits() | Self::ROTATION_LOCKED_Z.bits();
    }
}

/// Mass and angular inertia added to a rigid-body on top of its attached colliders’ contributions.
#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RigidBodyAdditionalMassProps {
    /// Mass properties to be added as-is.
    MassProps(MassProperties),
    /// Mass to be added to the rigid-body. This will also automatically scale
    /// the attached colliders total angular inertia to account for the added mass.
    Mass(Real),
}

impl Default for RigidBodyAdditionalMassProps {
    fn default() -> Self {
        RigidBodyAdditionalMassProps::MassProps(MassProperties::default())
    }
}

#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
// #[repr(C)]
/// The mass properties of a rigid-body.
pub struct RigidBodyMassProps {
    /// The world-space center of mass of the rigid-body.
    pub world_com: Vector,
    /// The inverse mass taking into account translation locking.
    pub effective_inv_mass: Vector,
    /// The square-root of the world-space inverse angular inertia tensor of the rigid-body,
    /// taking into account rotation locking.
    pub effective_world_inv_inertia: AngularInertia,
    /// The local mass properties of the rigid-body.
    pub local_mprops: MassProperties,
    /// Flags for locking rotation and translation.
    pub flags: LockedAxes,
    /// Mass-properties of this rigid-bodies, added to the contributions of its attached colliders.
    pub additional_local_mprops: Option<Box<RigidBodyAdditionalMassProps>>,
    /// Conservative bound on the distance of any shape point from the local center of mass;
    /// the sleep metric and the CCD fast-body criterion use it to turn angular velocity into
    /// a farthest-point speed. Refreshed with the mass properties; `0` for collider-less bodies.
    #[cfg_attr(feature = "serde-serialize", serde(default))]
    pub(crate) max_extent: Real,
}

impl Default for RigidBodyMassProps {
    fn default() -> Self {
        Self {
            flags: LockedAxes::empty(),
            local_mprops: MassProperties::zero(),
            additional_local_mprops: None,
            world_com: Vector::ZERO,
            effective_inv_mass: Vector::ZERO,
            effective_world_inv_inertia: AngularInertia::zero(),
            max_extent: 0.0,
        }
    }
}

impl From<LockedAxes> for RigidBodyMassProps {
    fn from(flags: LockedAxes) -> Self {
        Self {
            flags,
            ..Self::default()
        }
    }
}

impl From<MassProperties> for RigidBodyMassProps {
    fn from(local_mprops: MassProperties) -> Self {
        Self {
            local_mprops,
            ..Default::default()
        }
    }
}

impl RigidBodyMassProps {
    /// The mass of the rigid-body.
    #[must_use]
    pub fn mass(&self) -> Real {
        crate::utils::inv(self.local_mprops.inv_mass)
    }

    /// The effective mass (that takes the potential translation locking into account) of
    /// this rigid-body.
    #[must_use]
    pub fn effective_mass(&self) -> Vector {
        self.effective_inv_mass.map(crate::utils::inv)
    }

    /// The square root of the effective world-space angular inertia (that takes the potential rotation locking into account) of
    /// this rigid-body.
    #[must_use]
    pub fn effective_angular_inertia(&self) -> AngularInertia {
        #[allow(unused_mut)] // mut needed in 3D.
        let mut ang_inertia = self.effective_world_inv_inertia;

        // Make the matrix invertible.
        #[cfg(feature = "dim3")]
        {
            if self.flags.contains(LockedAxes::ROTATION_LOCKED_X) {
                ang_inertia.m11 = 1.0;
            }
            if self.flags.contains(LockedAxes::ROTATION_LOCKED_Y) {
                ang_inertia.m22 = 1.0;
            }
            if self.flags.contains(LockedAxes::ROTATION_LOCKED_Z) {
                ang_inertia.m33 = 1.0;
            }
        }

        #[allow(unused_mut)] // mut needed in 3D.
        let mut result = ang_inertia.inverse();

        // Remove the locked axes again.
        #[cfg(feature = "dim3")]
        {
            if self.flags.contains(LockedAxes::ROTATION_LOCKED_X) {
                result.m11 = 0.0;
            }
            if self.flags.contains(LockedAxes::ROTATION_LOCKED_Y) {
                result.m22 = 0.0;
            }
            if self.flags.contains(LockedAxes::ROTATION_LOCKED_Z) {
                result.m33 = 0.0;
            }
        }

        result
    }

    /// Recompute the mass-properties of this rigid-bodies based on its currently attached colliders.
    pub fn recompute_mass_properties_from_colliders(
        &mut self,
        colliders: &ColliderSet,
        attached_colliders: &RigidBodyColliders,
        body_type: RigidBodyType,
        position: &Pose,
    ) {
        let added_mprops = self
            .additional_local_mprops
            .as_ref()
            .map(|mprops| **mprops)
            .unwrap_or_else(|| RigidBodyAdditionalMassProps::MassProps(MassProperties::default()));

        self.local_mprops = MassProperties::default();

        for handle in &attached_colliders.0 {
            if let Some(co) = colliders.get(*handle) {
                if co.is_enabled() {
                    if let Some(co_parent) = co.parent {
                        let to_add = co
                            .mprops
                            .mass_properties(&*co.shape)
                            .transform_by(&co_parent.pos_wrt_parent);
                        self.local_mprops += to_add;
                    }
                }
            }
        }

        match added_mprops {
            RigidBodyAdditionalMassProps::MassProps(mprops) => {
                self.local_mprops += mprops;
            }
            RigidBodyAdditionalMassProps::Mass(mass) => {
                let prev_mass = self.local_mprops.mass();
                if prev_mass > 0.0 {
                    self.local_mprops.set_mass(prev_mass + mass, true);
                } else {
                    // The colliders contribute no mass, so `set_mass` has no angular
                    // inertia to rescale and the body could never rotate. Derive it (and the
                    // CoM) from the shapes at unit density, rescaled to the additional mass.
                    let mut unit_mprops = MassProperties::default();
                    for handle in &attached_colliders.0 {
                        if let Some(co) = colliders.get(*handle) {
                            if co.is_enabled() {
                                if let Some(co_parent) = co.parent {
                                    unit_mprops += co
                                        .shape
                                        .mass_properties(1.0)
                                        .transform_by(&co_parent.pos_wrt_parent);
                                }
                            }
                        }
                    }

                    if unit_mprops.mass() > 0.0 {
                        unit_mprops.set_mass(mass, true);
                        self.local_mprops += unit_mprops;
                    } else {
                        // No shape to derive an inertia from: just set the mass.
                        self.local_mprops.set_mass(mass, true);
                    }
                }
            }
        }

        self.recompute_max_extent(colliders, attached_colliders);
        self.update_world_mass_properties(body_type, position);
    }

    /// Refreshes [`Self::max_extent`] from the attached colliders' bounding
    /// spheres, measured about the local center of mass.
    pub(crate) fn recompute_max_extent(
        &mut self,
        colliders: &ColliderSet,
        attached_colliders: &RigidBodyColliders,
    ) {
        let local_com = self.local_mprops.local_com;
        let mut max_extent: Real = 0.0;
        for handle in &attached_colliders.0 {
            if let Some(co) = colliders.get(*handle) {
                if co.is_enabled() {
                    if let Some(co_parent) = co.parent {
                        let sphere = co
                            .shape
                            .compute_local_bounding_sphere()
                            .transform_by(&co_parent.pos_wrt_parent);
                        let extent = (sphere.center - local_com).length() + sphere.radius;
                        max_extent = max_extent.max(extent);
                    }
                }
            }
        }
        self.max_extent = max_extent;
    }

    /// Conservative bound on the distance of any point of the body's shapes
    /// from its local center of mass. `0` for collider-less bodies.
    ///
    /// Used by the sleep metric and the CCD fast-body criterion to turn angular
    /// velocity into a farthest-point speed.
    #[inline]
    pub fn max_extent(&self) -> Real {
        self.max_extent
    }

    /// Update the world-space mass properties of `self`, taking into account the new position.
    pub fn update_world_mass_properties(&mut self, body_type: RigidBodyType, position: &Pose) {
        self.world_com = self.local_mprops.world_com(position);
        self.effective_inv_mass = Vector::splat(self.local_mprops.inv_mass);
        self.effective_world_inv_inertia = self.local_mprops.world_inv_inertia(&position.rotation);

        // Take into account translation/rotation locking.
        if !body_type.is_dynamic() || self.flags.contains(LockedAxes::TRANSLATION_LOCKED_X) {
            self.effective_inv_mass.x = 0.0;
        }

        if !body_type.is_dynamic() || self.flags.contains(LockedAxes::TRANSLATION_LOCKED_Y) {
            self.effective_inv_mass.y = 0.0;
        }

        #[cfg(feature = "dim3")]
        if !body_type.is_dynamic() || self.flags.contains(LockedAxes::TRANSLATION_LOCKED_Z) {
            self.effective_inv_mass.z = 0.0;
        }

        #[cfg(feature = "dim2")]
        {
            if !body_type.is_dynamic() || self.flags.contains(LockedAxes::ROTATION_LOCKED_Z) {
                self.effective_world_inv_inertia = 0.0;
            }
        }
        #[cfg(feature = "dim3")]
        {
            if !body_type.is_dynamic() || self.flags.contains(LockedAxes::ROTATION_LOCKED_X) {
                self.effective_world_inv_inertia.m11 = 0.0;
                self.effective_world_inv_inertia.m12 = 0.0;
                self.effective_world_inv_inertia.m13 = 0.0;
            }

            if !body_type.is_dynamic() || self.flags.contains(LockedAxes::ROTATION_LOCKED_Y) {
                self.effective_world_inv_inertia.m22 = 0.0;
                self.effective_world_inv_inertia.m12 = 0.0;
                self.effective_world_inv_inertia.m23 = 0.0;
            }
            if !body_type.is_dynamic() || self.flags.contains(LockedAxes::ROTATION_LOCKED_Z) {
                self.effective_world_inv_inertia.m33 = 0.0;
                self.effective_world_inv_inertia.m13 = 0.0;
                self.effective_world_inv_inertia.m23 = 0.0;
            }
        }
    }
}

#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Copy, PartialEq)]
/// The velocities of this rigid-body.
// repr(C): `as_vector` reinterprets this struct as a flat vector with the
// linear part first, so the field order must be guaranteed.
#[repr(C)]
pub struct RigidBodyVelocity<T: ScalarType> {
    /// The linear velocity of the rigid-body.
    pub linvel: T::Vector,
    /// The angular velocity of the rigid-body.
    pub angvel: T::AngVector,
}

impl Default for RigidBodyVelocity<Real> {
    fn default() -> Self {
        Self::zero()
    }
}

impl RigidBodyVelocity<Real> {
    /// Create a new rigid-body velocity component.
    #[must_use]
    #[cfg(feature = "dim2")]
    pub fn new(linvel: Vector, angvel: AngVector) -> Self {
        Self { linvel, angvel }
    }

    /// Create a new rigid-body velocity component.
    #[must_use]
    #[cfg(feature = "dim3")]
    pub fn new(linvel: Vector, angvel: AngVector) -> Self {
        Self {
            linvel: Vector::new(linvel.x, linvel.y, linvel.z),
            angvel: AngVector::new(angvel.x, angvel.y, angvel.z),
        }
    }

    /// Converts a slice to a rigid-body velocity.
    ///
    /// The slice must contain at least 3 elements: the `slice[0..2]` contains
    /// the linear velocity and the `slice[2]` contains the angular velocity.
    #[must_use]
    #[cfg(feature = "dim2")]
    pub fn from_slice(slice: &[Real]) -> Self {
        Self {
            linvel: Vector::new(slice[0], slice[1]),
            angvel: slice[2],
        }
    }

    /// Converts a slice to a rigid-body velocity.
    ///
    /// The slice must contain at least 6 elements: the `slice[0..3]` contains
    /// the linear velocity and the `slice[3..6]` contains the angular velocity.
    #[must_use]
    #[cfg(feature = "dim3")]
    pub fn from_slice(slice: &[Real]) -> Self {
        Self {
            linvel: Vector::new(slice[0], slice[1], slice[2]),
            angvel: AngVector::new(slice[3], slice[4], slice[5]),
        }
    }

    /// Velocities set to zero.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            linvel: Default::default(),
            angvel: Default::default(),
        }
    }

    /// Are both the linear and angular velocities finite (neither NaN nor infinite)?
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.linvel.is_finite() && self.angvel.is_finite()
    }

    /// This velocity seen as a slice.
    ///
    /// The linear part is stored first.
    #[inline]
    pub fn as_slice(&self) -> &[Real] {
        self.as_vector().as_slice()
    }

    /// This velocity seen as a mutable slice.
    ///
    /// The linear part is stored first.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [Real] {
        self.as_vector_mut().as_mut_slice()
    }

    /// This velocity seen as a vector.
    ///
    /// The linear part is stored first.
    #[inline]
    #[cfg(feature = "dim2")]
    pub fn as_vector(&self) -> &na::Vector3<Real> {
        unsafe { core::mem::transmute(self) }
    }

    /// This velocity seen as a mutable vector.
    ///
    /// The linear part is stored first.
    #[inline]
    #[cfg(feature = "dim2")]
    pub fn as_vector_mut(&mut self) -> &mut na::Vector3<Real> {
        unsafe { core::mem::transmute(self) }
    }

    /// This velocity seen as a vector.
    ///
    /// The linear part is stored first.
    #[inline]
    #[cfg(feature = "dim3")]
    pub fn as_vector(&self) -> &na::Vector6<Real> {
        unsafe { core::mem::transmute(self) }
    }

    /// This velocity seen as a mutable vector.
    ///
    /// The linear part is stored first.
    #[inline]
    #[cfg(feature = "dim3")]
    pub fn as_vector_mut(&mut self) -> &mut na::Vector6<Real> {
        unsafe { core::mem::transmute(self) }
    }

    /// Return `self` rotated by `rotation`.
    #[must_use]
    #[cfg(feature = "dim2")]
    pub fn transformed(self, rotation: &Rotation) -> Self {
        Self {
            linvel: *rotation * self.linvel,
            angvel: self.angvel,
        }
    }

    /// Return `self` rotated by `rotation`.
    #[must_use]
    #[cfg(feature = "dim3")]
    pub fn transformed(self, rotation: &Rotation) -> Self {
        Self {
            linvel: *rotation * self.linvel,
            angvel: *rotation * self.angvel,
        }
    }

    /// The approximate kinetic energy of this rigid-body.
    ///
    /// This approximation does not take the rigid-body's mass and angular inertia
    /// into account. Some physics engines call this the "mass-normalized kinetic
    /// energy".
    #[must_use]
    pub fn pseudo_kinetic_energy(&self) -> Real {
        0.5 * (self.linvel.length_squared() + self.angvel.gdot(self.angvel))
    }

    /// The velocity of the given world-space point on this rigid-body.
    #[must_use]
    #[cfg(feature = "dim2")]
    pub fn velocity_at_point(&self, point: Vector, world_com: Vector) -> Vector {
        let dpt = point - world_com;
        self.linvel + self.angvel.gcross(dpt)
    }

    /// The velocity of the given world-space point on this rigid-body.
    #[must_use]
    #[cfg(feature = "dim3")]
    pub fn velocity_at_point(&self, point: Vector, world_com: Vector) -> Vector {
        let dpt = point - world_com;
        self.linvel + self.angvel.gcross(dpt)
    }

    /// Are these velocities exactly equal to zero?
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.linvel == Vector::ZERO && self.angvel == AngVector::default()
    }

    /// The kinetic energy of this rigid-body.
    #[must_use]
    #[profiling::function]
    pub fn kinetic_energy(&self, rb_mprops: &RigidBodyMassProps) -> Real {
        let mut energy = (rb_mprops.mass() * self.linvel.length_squared()) / 2.0;

        #[cfg(feature = "dim2")]
        if !num::Zero::is_zero(&rb_mprops.effective_world_inv_inertia) {
            let inertia = 1.0 / rb_mprops.effective_world_inv_inertia;
            energy += inertia * self.angvel * self.angvel / 2.0;
        }

        #[cfg(feature = "dim3")]
        if !rb_mprops.effective_world_inv_inertia.is_zero() {
            let inertia = rb_mprops.effective_world_inv_inertia.inverse_unchecked();
            energy += self.angvel.gdot(inertia * self.angvel) / 2.0;
        }

        energy
    }

    /// Applies an impulse at the center-of-mass of this rigid-body.
    /// The impulse is applied right away, changing the linear velocity.
    /// This does nothing on non-dynamic bodies.
    pub fn apply_impulse(&mut self, rb_mprops: &RigidBodyMassProps, impulse: Vector) {
        self.linvel += impulse * rb_mprops.effective_inv_mass;
    }

    /// Applies an angular impulse at the center-of-mass of this rigid-body.
    /// The impulse is applied right away, changing the angular velocity.
    /// This does nothing on non-dynamic bodies.
    #[cfg(feature = "dim2")]
    pub fn apply_torque_impulse(&mut self, rb_mprops: &RigidBodyMassProps, torque_impulse: Real) {
        self.angvel += rb_mprops.effective_world_inv_inertia * torque_impulse;
    }

    /// Applies an angular impulse at the center-of-mass of this rigid-body.
    /// The impulse is applied right away, changing the angular velocity.
    /// This does nothing on non-dynamic bodies.
    #[cfg(feature = "dim3")]
    pub fn apply_torque_impulse(&mut self, rb_mprops: &RigidBodyMassProps, torque_impulse: Vector) {
        self.angvel += rb_mprops.effective_world_inv_inertia * torque_impulse;
    }

    /// Applies an impulse at the given world-space point of this rigid-body.
    /// The impulse is applied right away, changing the linear and/or angular velocities.
    /// This does nothing on non-dynamic bodies.
    #[cfg(feature = "dim2")]
    pub fn apply_impulse_at_point(
        &mut self,
        rb_mprops: &RigidBodyMassProps,
        impulse: Vector,
        point: Vector,
    ) {
        let torque_impulse = (point - rb_mprops.world_com).perp_dot(impulse);
        self.apply_impulse(rb_mprops, impulse);
        self.apply_torque_impulse(rb_mprops, torque_impulse);
    }

    /// Applies an impulse at the given world-space point of this rigid-body.
    /// The impulse is applied right away, changing the linear and/or angular velocities.
    /// This does nothing on non-dynamic bodies.
    #[cfg(feature = "dim3")]
    pub fn apply_impulse_at_point(
        &mut self,
        rb_mprops: &RigidBodyMassProps,
        impulse: Vector,
        point: Vector,
    ) {
        let torque_impulse = (point - rb_mprops.world_com).cross(impulse);
        self.apply_impulse(rb_mprops, impulse);
        self.apply_torque_impulse(rb_mprops, torque_impulse);
    }
}

impl<T: ScalarType> RigidBodyVelocity<T> {
    /// Returns the update velocities after applying the given damping.
    #[must_use]
    pub fn apply_damping(&self, dt: T, damping: &RigidBodyDamping<T>) -> Self {
        let one = T::one();
        RigidBodyVelocity {
            linvel: self.linvel * (one / (one + dt * damping.linear_damping)),
            angvel: self.angvel * (one / (one + dt * damping.angular_damping)),
        }
    }

    /// Integrate the velocities in `self` to compute obtain new positions when moving from the given
    /// initial position `init_pos`.
    #[must_use]
    #[inline]
    #[allow(clippy::let_and_return)] // Keeping `result` binding for potential renormalization
    pub fn integrate(&self, dt: T, init_pos: &T::Pose, local_com: &T::Vector) -> T::Pose {
        let com = *init_pos * *local_com;
        let result = init_pos
            .append_translation(-com)
            .append_rotation(self.angvel * dt)
            .append_translation(com + self.linvel * dt);
        // TODO: is renormalization really useful?
        // result.rotation.renormalize_fast();
        result
    }
}

impl RigidBodyVelocity<Real> {
    /// Same as [`Self::integrate`] but with the angular part linearized and the local
    /// center-of-mass assumed to be zero.
    #[inline]
    #[cfg(feature = "dim2")]
    pub(crate) fn integrate_linearized(
        &self,
        dt: Real,
        translation: &mut Vector,
        rotation: &mut Rotation,
    ) {
        let dang = self.angvel * dt;
        let new_cos = rotation.re - dang * rotation.im;
        let new_sin = rotation.im + dang * rotation.re;
        *rotation = Rot2::from_cos_sin_unchecked(new_cos, new_sin);
        // NOTE: don't use renormalize_fast since the linearization might cause more drift.
        rotation.normalize_mut();
        *translation += self.linvel * dt;
    }

    /// Same as [`Self::integrate`] but with the angular part linearized and the local
    /// center-of-mass assumed to be zero.
    #[inline]
    #[cfg(feature = "dim3")]
    pub(crate) fn integrate_linearized(
        &self,
        dt: Real,
        translation: &mut Vector,
        rotation: &mut Rotation,
    ) {
        // Rotations linearization is inspired from
        // https://ahrs.readthedocs.io/en/latest/filters/angular.html (not using the matrix form).
        let hang = self.angvel * (dt * 0.5);
        // Quaternion identity + `hang` seen as a quaternion.
        let id_plus_hang = Rotation::from_xyzw(hang.x, hang.y, hang.z, 1.0);
        *rotation = id_plus_hang * *rotation;
        *rotation = rotation.normalize();
        *translation += self.linvel * dt;
    }
}

impl core::ops::Mul<Real> for RigidBodyVelocity<Real> {
    type Output = Self;

    fn mul(self, rhs: Real) -> Self {
        RigidBodyVelocity {
            linvel: self.linvel * rhs,
            angvel: self.angvel * rhs,
        }
    }
}

impl core::ops::Add<RigidBodyVelocity<Real>> for RigidBodyVelocity<Real> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        RigidBodyVelocity {
            linvel: self.linvel + rhs.linvel,
            angvel: self.angvel + rhs.angvel,
        }
    }
}

impl core::ops::AddAssign<RigidBodyVelocity<Real>> for RigidBodyVelocity<Real> {
    fn add_assign(&mut self, rhs: Self) {
        self.linvel += rhs.linvel;
        self.angvel += rhs.angvel;
    }
}

impl core::ops::Sub<RigidBodyVelocity<Real>> for RigidBodyVelocity<Real> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        RigidBodyVelocity {
            linvel: self.linvel - rhs.linvel,
            angvel: self.angvel - rhs.angvel,
        }
    }
}

impl core::ops::SubAssign<RigidBodyVelocity<Real>> for RigidBodyVelocity<Real> {
    fn sub_assign(&mut self, rhs: Self) {
        self.linvel -= rhs.linvel;
        self.angvel -= rhs.angvel;
    }
}

#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Copy, PartialEq)]
/// Damping factors to progressively slow down a rigid-body.
pub struct RigidBodyDamping<T> {
    /// Damping factor for gradually slowing down the translational motion of the rigid-body.
    pub linear_damping: T,
    /// Damping factor for gradually slowing down the angular motion of the rigid-body.
    pub angular_damping: T,
}

impl<T: SimdRealCopy> Default for RigidBodyDamping<T> {
    fn default() -> Self {
        Self {
            linear_damping: T::zero(),
            angular_damping: T::zero(),
        }
    }
}

include!("rigid_body_components_more.rs");

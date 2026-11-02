impl Multibody {
    /// Computes the ids of all the links between the root and the link identified by `link_id`.
    pub fn kinematic_branch(&self, link_id: usize) -> Vec<usize> {
        let mut branch = vec![]; // Perf: avoid allocation.
        let mut curr_id = Some(link_id);

        while let Some(id) = curr_id {
            branch.push(id);
            curr_id = self.links[id].parent_id();
        }

        branch.reverse();
        branch
    }

    /// Apply forward-kinematics to compute the position of a single link of this multibody.
    ///
    /// If `out_jacobian` is `Some`, this will simultaneously compute the new jacobian of this link.
    /// If `displacement` is `Some`, the generalized position considered during transform propagation
    /// is the sum of the current position of `self` and this `displacement`.
    // TODO: this shares a lot of code with `forward_kinematics` and `update_body_jacobians`, except
    //       that we are only traversing a single kinematic chain. Could this be refactored?
    pub fn forward_kinematics_single_link(
        &self,
        bodies: &RigidBodySet,
        link_id: usize,
        displacement: Option<&[Real]>,
        out_jacobian: Option<&mut Jacobian<Real>>,
    ) -> Pose {
        let branch = self.kinematic_branch(link_id);
        self.forward_kinematics_single_branch(bodies, &branch, displacement, out_jacobian)
    }

    /// Apply forward-kinematics to compute the position of a single sorted branch of this multibody.
    ///
    /// The given `branch` must have the following properties:
    /// - It must be sorted, i.e., `branch[i] < branch[i + 1]`.
    /// - All the indices must be part of the same kinematic branch.
    /// - If a link is `branch[i]`, then `branch[i - 1]` must be its parent.
    ///
    /// In general, this method shouldn’t be used directly and [`Self::forward_kinematics_single_link`]
    /// should be preferred since it computes the branch indices automatically.
    ///
    /// If you want to calculate the branch indices manually, see [`Self::kinematic_branch`].
    ///
    /// If `out_jacobian` is `Some`, this will simultaneously compute the new jacobian of this branch.
    /// This represents the body jacobian for the last link in the branch.
    ///
    /// If `displacement` is `Some`, the generalized position considered during transform propagation
    /// is the sum of the current position of `self` and this `displacement`.
    // TODO: this shares a lot of code with `forward_kinematics` and `update_body_jacobians`, except
    //       that we are only traversing a single kinematic chain. Could this be refactored?
    #[profiling::function]
    pub fn forward_kinematics_single_branch(
        &self,
        bodies: &RigidBodySet,
        branch: &[usize],
        displacement: Option<&[Real]>,
        mut out_jacobian: Option<&mut Jacobian<Real>>,
    ) -> Pose {
        if let Some(out_jacobian) = out_jacobian.as_deref_mut() {
            if out_jacobian.ncols() != self.ndofs {
                *out_jacobian = Jacobian::zeros(self.ndofs);
            } else {
                out_jacobian.fill(0.0);
            }
        }

        let mut parent_link: Option<MultibodyLink> = None;

        for i in branch {
            let mut link = self.links[*i];

            if let Some(displacement) = displacement {
                link.joint
                    .apply_displacement(&displacement[link.assembly_id..]);
            }

            let parent_to_world;

            if let Some(parent_link) = parent_link {
                link.local_to_parent = link.joint.body_to_parent();
                link.local_to_world = parent_link.local_to_world * link.local_to_parent;

                {
                    let parent_rb = &bodies[parent_link.rigid_body];
                    let link_rb = &bodies[link.rigid_body];
                    let c0 = parent_link.local_to_world * parent_rb.mprops.local_mprops.local_com;
                    let c2 = link.local_to_world
                        * Vector::from(link.joint.data.local_frame2.translation);
                    let c3 = link.local_to_world * link_rb.mprops.local_mprops.local_com;

                    link.shift02 = c2 - c0;
                    link.shift23 = c3 - c2;
                }

                parent_to_world = parent_link.local_to_world;

                if let Some(out_jacobian) = out_jacobian.as_deref_mut() {
                    let (mut link_j_v, parent_j_w) =
                        out_jacobian.rows_range_pair_mut(0..DIM, DIM..DIM + ANG_DIM);
                    let shift_tr = vect_to_na(link.shift02).gcross_matrix_tr();
                    link_j_v.gemm(1.0, &shift_tr, &parent_j_w, 1.0);
                }
            } else {
                link.local_to_parent = link.joint.body_to_parent();
                link.local_to_world = link.local_to_parent;
                parent_to_world = Pose::IDENTITY;
            }

            if let Some(out_jacobian) = out_jacobian.as_deref_mut() {
                let ndofs = link.joint.ndofs();
                let mut tmp = SMatrix::<Real, SPATIAL_DIM, SPATIAL_DIM>::zeros();
                let mut link_joint_j = tmp.columns_mut(0, ndofs);
                let mut link_j_part = out_jacobian.columns_mut(link.assembly_id, ndofs);
                link.joint.jacobian(
                    &(parent_to_world.rotation * link.joint.data.local_frame1.rotation),
                    &mut link_joint_j,
                );
                link_j_part += link_joint_j;

                {
                    let (mut link_j_v, link_j_w) =
                        out_jacobian.rows_range_pair_mut(0..DIM, DIM..DIM + ANG_DIM);
                    let shift_tr = vect_to_na(link.shift23).gcross_matrix_tr();
                    link_j_v.gemm(1.0, &shift_tr, &link_j_w, 1.0);
                }
            }

            parent_link = Some(link);
        }

        parent_link
            .map(|link| link.local_to_world)
            .unwrap_or(Pose::IDENTITY)
    }

    /// The total number of freedoms of this multibody.
    #[inline]
    pub fn ndofs(&self) -> usize {
        self.ndofs
    }

    pub(crate) fn fill_jacobians(
        &self,
        link_id: usize,
        unit_force: Vector,
        unit_torque: AngVector,
        j_id: &mut usize,
        jacobians: &mut DVector,
    ) -> (Real, Real) {
        if self.ndofs == 0 {
            return (0.0, 0.0);
        }

        let wj_id = *j_id + self.ndofs;
        let force = Force {
            linear: unit_force,
            #[cfg(feature = "dim2")]
            angular: unit_torque,
            #[cfg(feature = "dim3")]
            angular: unit_torque,
        };

        let link = &self.links[link_id];
        let mut out_j = jacobians.rows_mut(*j_id, self.ndofs);
        self.body_jacobians[link.internal_id].tr_mul_to(force.as_vector(), &mut out_j);

        // TODO: Optimize with a copy_nonoverlapping?
        for i in 0..self.ndofs {
            jacobians[wj_id + i] = jacobians[*j_id + i];
        }

        {
            let mut out_invm_j = jacobians.rows_mut(wj_id, self.ndofs);
            self.augmented_mass_indices
                .with_rearranged_rows_mut(&mut out_invm_j, |out_invm_j| {
                    self.inv_augmented_mass.solve_mut(out_invm_j);
                });
        }

        let j = jacobians.rows(*j_id, self.ndofs);
        let invm_j = jacobians.rows(wj_id, self.ndofs);
        *j_id += self.ndofs * 2;

        (j.dot(&invm_j), j.dot(&self.generalized_velocity()))
    }

    /// Fills `jacobians` with the relative jacobian `J = J2ᵀ·f2 − J1ᵀ·f1` of two links of `self`, then `M⁻¹·J`
    /// (e.g. a loop closure). The difference must be explicit: per-link blocks lose the `J1ᵀ·W·J2` effective-mass
    /// coupling since both act on the same generalized velocities. Cancellation-vanished rows are zeroed so the solver skips them.
    pub(crate) fn fill_relative_jacobians(
        &self,
        link_id1: usize,
        unit_force1: Vector,
        unit_torque1: AngVector,
        link_id2: usize,
        unit_force2: Vector,
        unit_torque2: AngVector,
        j_id: &mut usize,
        jacobians: &mut DVector,
    ) {
        if self.ndofs == 0 {
            return;
        }

        let wj_id = *j_id + self.ndofs;
        let force1 = Force {
            linear: unit_force1,
            angular: unit_torque1,
        };
        let force2 = Force {
            linear: unit_force2,
            angular: unit_torque2,
        };

        let link1 = &self.links[link_id1];
        let link2 = &self.links[link_id2];

        {
            let jb1 = &self.body_jacobians[link1.internal_id];
            let jb2 = &self.body_jacobians[link2.internal_id];

            // Use the (overwritten below) W·J slot as scratch for J1ᵀ·f1.
            let (mut out_j, mut scratch) =
                jacobians.rows_range_pair_mut(*j_id..*j_id + self.ndofs, wj_id..wj_id + self.ndofs);
            jb2.tr_mul_to(force2.as_vector(), &mut out_j);
            jb1.tr_mul_to(force1.as_vector(), &mut scratch);
            out_j.axpy(-1.0, &scratch, 1.0);

            // Cancellation guard: the reference scale is the magnitude of the dot-product operands,
            // not their results (which may be pure cancellation noise when the direction isn’t
            // expressible by the dofs). Rows below ~1000·ε times that scale are noise, not a constraint.
            let scale_sq = jb1.norm_squared() * force1.as_vector().norm_squared()
                + jb2.norm_squared() * force2.as_vector().norm_squared();
            let eps = Real::EPSILON * 1.0e3;
            if out_j.norm_squared() <= eps * eps * scale_sq {
                out_j.fill(0.0);
            }
        }

        // TODO: Optimize with a copy_nonoverlapping?
        for i in 0..self.ndofs {
            jacobians[wj_id + i] = jacobians[*j_id + i];
        }

        {
            let mut out_invm_j = jacobians.rows_mut(wj_id, self.ndofs);
            self.augmented_mass_indices
                .with_rearranged_rows_mut(&mut out_invm_j, |out_invm_j| {
                    self.inv_augmented_mass.solve_mut(out_invm_j);
                });
        }

        *j_id += self.ndofs * 2;
    }
}

#[cfg_attr(feature = "serde-serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
struct IndexSequence {
    first_to_remove: u32,
    index_map: Vec<usize>,
}

impl IndexSequence {
    const NONE: u32 = u32::MAX;

    fn new() -> Self {
        Self {
            first_to_remove: Self::NONE,
            index_map: vec![],
        }
    }

    /// Index of the first removed dof, assuming there is one.
    fn start(&self) -> usize {
        self.first_to_remove as usize
    }

    fn clear(&mut self) {
        self.first_to_remove = Self::NONE;
        self.index_map.clear();
    }

    fn keep(&mut self, i: usize) {
        if self.first_to_remove == Self::NONE {
            // Nothing got removed yet. No need to register any
            // special indexing.
            return;
        }

        self.index_map.push(i);
    }

    fn remove(&mut self, i: usize) {
        if self.first_to_remove == Self::NONE {
            self.first_to_remove = i as u32;
        }
    }

    fn dim_after_removal(&self, original_dim: usize) -> usize {
        if self.first_to_remove == Self::NONE {
            original_dim
        } else {
            self.start() + self.index_map.len()
        }
    }

    fn rearrange_columns<R: na::Dim, C: na::Dim, S: StorageMut<Real, R, C>>(
        &self,
        mat: &mut na::Matrix<Real, R, C, S>,
        clear_removed: bool,
    ) {
        if self.first_to_remove == Self::NONE {
            // Nothing to rearrange.
            return;
        }

        for (target_shift, source) in self.index_map.iter().enumerate() {
            let target = self.start() + target_shift;
            let (mut target_col, source_col) = mat.columns_range_pair_mut(target, *source);
            target_col.copy_from(&source_col);
        }

        if clear_removed {
            mat.columns_range_mut(self.start() + self.index_map.len()..)
                .fill(0.0);
        }
    }

    fn rearrange_rows<R: na::Dim, C: na::Dim, S: StorageMut<Real, R, C>>(
        &self,
        mat: &mut na::Matrix<Real, R, C, S>,
        clear_removed: bool,
    ) {
        if self.first_to_remove == Self::NONE {
            // Nothing to rearrange.
            return;
        }

        for mut col in mat.column_iter_mut() {
            for (target_shift, source) in self.index_map.iter().enumerate() {
                let target = self.start() + target_shift;
                col[target] = col[*source];
            }

            if clear_removed {
                col.rows_range_mut(self.start() + self.index_map.len()..)
                    .fill(0.0);
            }
        }
    }

    fn inv_rearrange_rows<R: na::Dim, C: na::Dim, S: StorageMut<Real, R, C>>(
        &self,
        mat: &mut na::Matrix<Real, R, C, S>,
    ) {
        if self.first_to_remove == Self::NONE {
            // Nothing to rearrange.
            return;
        }

        for mut col in mat.column_iter_mut() {
            for (target_shift, source) in self.index_map.iter().enumerate().rev() {
                let target = self.start() + target_shift;
                col[*source] = col[target];
                col[target] = 0.0;
            }
        }
    }

    fn with_rearranged_rows_mut<C: na::Dim, S: StorageMut<Real, Dyn, C>>(
        &self,
        mat: &mut na::Matrix<Real, Dyn, C, S>,
        mut f: impl FnMut(&mut na::MatrixViewMut<Real, Dyn, C, S::RStride, S::CStride>),
    ) {
        self.rearrange_rows(mat, true);
        let effective_dim = self.dim_after_removal(mat.nrows());
        if effective_dim > 0 {
            f(&mut mat.rows_mut(0, effective_dim));
        }
        self.inv_rearrange_rows(mat);
    }
}

#[cfg(test)]
mod test {
    use super::IndexSequence;
    use crate::alloc_prelude::*;
    use crate::dynamics::{ImpulseJointSet, IslandManager};
    #[cfg(feature = "dim3")]
    use crate::math::Vector;
    use crate::math::{Real, SPATIAL_DIM};
    use crate::prelude::{
        ColliderSet, MultibodyJointHandle, MultibodyJointSet, RevoluteJoint, RigidBodyBuilder,
        RigidBodySet,
    };
    use na::{DVector, RowDVector};

    #[test]
    fn test_multibody_append() {
        let mut bodies = RigidBodySet::new();
        let mut joints = MultibodyJointSet::new();

        let a = bodies.insert(RigidBodyBuilder::dynamic());
        let b = bodies.insert(RigidBodyBuilder::dynamic());
        let c = bodies.insert(RigidBodyBuilder::dynamic());
        let d = bodies.insert(RigidBodyBuilder::dynamic());

        #[cfg(feature = "dim2")]
        let joint = RevoluteJoint::new();
        #[cfg(feature = "dim3")]
        let joint = RevoluteJoint::new(Vector::X);

        let mb_handle = joints.insert(a, b, joint, true).unwrap();
        joints.insert(c, d, joint, true).unwrap();
        joints.insert(b, c, joint, true).unwrap();

        assert_eq!(joints.get(mb_handle).unwrap().0.ndofs, SPATIAL_DIM + 3);
    }

    #[test]
    fn test_multibody_insert() {
        let mut rnd = oorandom::Rand32::new(1234);

        for k in 0..10 {
            let mut bodies = RigidBodySet::new();
            let mut multibody_joints = MultibodyJointSet::new();

            let num_links = 100;
            let mut handles = vec![];

            for _ in 0..num_links {
                handles.push(bodies.insert(RigidBodyBuilder::dynamic()));
            }

            let mut insertion_id: Vec<_> = (0..num_links - 1).collect();

            #[cfg(feature = "dim2")]
            let joint = RevoluteJoint::new();
            #[cfg(feature = "dim3")]
            let joint = RevoluteJoint::new(Vector::X);

            match k {
                0 => {} // Remove in insertion order.
                1 => {
                    // Remove from leaf to root.
                    insertion_id.reverse();
                }
                _ => {
                    // Shuffle the vector a bit.
                    // (This test checks multiple shuffle arrangements due to k > 2).
                    for l in 0..num_links - 1 {
                        insertion_id.swap(l, rnd.rand_range(0..num_links as u32 - 1) as usize);
                    }
                }
            }

            let mut mb_handle = MultibodyJointHandle::invalid();
            for i in insertion_id {
                mb_handle = multibody_joints
                    .insert(handles[i], handles[i + 1], joint, true)
                    .unwrap();
            }

            assert_eq!(
                multibody_joints.get(mb_handle).unwrap().0.ndofs,
                SPATIAL_DIM + num_links - 1
            );
        }
    }

    #[test]
    fn test_multibody_remove() {
        let mut rnd = oorandom::Rand32::new(1234);

        for k in 0..10 {
            let mut bodies = RigidBodySet::new();
            let mut multibody_joints = MultibodyJointSet::new();
            let mut colliders = ColliderSet::new();
            let mut impulse_joints = ImpulseJointSet::new();
            let mut islands = IslandManager::new();

            let num_links = 100;
            let mut handles = vec![];

            for _ in 0..num_links {
                handles.push(bodies.insert(RigidBodyBuilder::dynamic()));
            }

            #[cfg(feature = "dim2")]
            let joint = RevoluteJoint::new();
            #[cfg(feature = "dim3")]
            let joint = RevoluteJoint::new(Vector::X);

            for i in 0..num_links - 1 {
                multibody_joints
                    .insert(handles[i], handles[i + 1], joint, true)
                    .unwrap();
            }

            match k {
                0 => {} // Remove in insertion order.
                1 => {
                    // Remove from leaf to root.
                    handles.reverse();
                }
                _ => {
                    // Shuffle the vector a bit.
                    // (This test checks multiple shuffle arrangements due to k > 2).
                    for l in 0..num_links {
                        handles.swap(l, rnd.rand_range(0..num_links as u32) as usize);
                    }
                }
            }

            for handle in handles {
                bodies.remove(
                    handle,
                    &mut islands,
                    &mut colliders,
                    &mut impulse_joints,
                    &mut multibody_joints,
                    true,
                );
            }
        }
    }

    fn test_sequence() -> IndexSequence {
        let mut seq = IndexSequence::new();
        seq.remove(2);
        seq.remove(3);
        seq.remove(4);
        seq.keep(5);
        seq.keep(6);
        seq.remove(7);
        seq.keep(8);
        seq
    }

    #[test]
    fn index_sequence_rearrange_columns() {
        let seq = test_sequence();
        let mut vec = RowDVector::from_fn(10, |_, c| c as Real);
        seq.rearrange_columns(&mut vec, true);
        assert_eq!(
            vec,
            RowDVector::from(vec![0.0, 1.0, 5.0, 6.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0])
        );
    }

    #[test]
    fn index_sequence_rearrange_rows() {
        let seq = test_sequence();
        let mut vec = DVector::from_fn(10, |r, _| r as Real);
        seq.rearrange_rows(&mut vec, true);
        assert_eq!(
            vec,
            DVector::from(vec![0.0, 1.0, 5.0, 6.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0])
        );
        seq.inv_rearrange_rows(&mut vec);
        assert_eq!(
            vec,
            DVector::from(vec![0.0, 1.0, 0.0, 0.0, 0.0, 5.0, 6.0, 0.0, 8.0, 0.0])
        );
    }

    #[test]
    fn index_sequence_with_rearranged_rows_mut() {
        let seq = test_sequence();
        let mut vec = DVector::from_fn(10, |r, _| r as Real);
        seq.with_rearranged_rows_mut(&mut vec, |v| {
            assert_eq!(v.len(), 5);
            assert_eq!(*v, DVector::from(vec![0.0, 1.0, 5.0, 6.0, 8.0]));
            *v *= 10.0;
        });
        assert_eq!(
            vec,
            DVector::from(vec![0.0, 10.0, 0.0, 0.0, 0.0, 50.0, 60.0, 0.0, 80.0, 0.0])
        );
    }
}

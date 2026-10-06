//! Primitive meshes (box, sphere, cylinder, plane, capsule): their shared
//! geometry, each slot's dimensions, and the batches they draw in.
use super::{each_shown, Feed, Writes};
use crate::{shapes, Batch, MeshId, RenderError, Vertex};
use exact_game::{Mesh, ViewModel, World};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Shape {
    Box,
    Sphere,
    Cylinder,
    Plane,
    Capsule,
}
impl Shape {
    fn of(mesh: &Mesh) -> Result<Self, RenderError> {
        Ok(match mesh {
            Mesh::Box { .. } => Self::Box,
            Mesh::Sphere { .. } => Self::Sphere,
            Mesh::Cylinder { .. } => Self::Cylinder,
            Mesh::Plane { .. } => Self::Plane,
            Mesh::Capsule { .. } => Self::Capsule,
            Mesh::Asset(name) => {
                return Err(RenderError::Scene(format!(
                    "Mesh.Asset({name}): asset meshes are not implemented"
                )))
            }
        })
    }
    fn geometry(self) -> (Vec<Vertex>, Vec<u32>) {
        let (mut vertices, indices) = match self {
            Self::Box => shapes::cube(),
            Self::Sphere => shapes::sphere(24),
            Self::Cylinder => shapes::cylinder(24),
            Self::Plane => shapes::plane(),
            Self::Capsule => shapes::capsule(0.5, 2.0, 24),
        };
        let half = vertices.len() / 2;
        for (i, v) in vertices.iter_mut().enumerate() {
            v.uv = if self == Self::Capsule {
                let sign = if i < half { 1.0 } else { -1.0 };
                v.position[1] -= sign * 0.5;
                [sign, 1.0]
            } else {
                [0.0; 2]
            };
        }
        (vertices, indices)
    }
}
fn dimensions(mesh: &Mesh) -> [f32; 3] {
    match mesh {
        Mesh::Box { size } => size.to_array(),
        Mesh::Sphere { radius } => [2.0 * radius; 3],
        Mesh::Cylinder { radius, height } => [2.0 * radius, *height, 2.0 * radius],
        Mesh::Plane { width, depth } => [*width, 1.0, *depth],
        Mesh::Capsule { radius, height } => [2.0 * radius, height * 0.5 - radius, 2.0 * radius],
        Mesh::Asset(_) => [1.0; 3], // Refused before any upload.
    }
}
pub(super) struct Group {
    mesh: MeshId,
    viewmodel: bool,
    slots: Vec<u32>,
}
impl Feed {
    /// Group every visible primitive by shape and view model, one batch each;
    /// record every primitive's dimensions, visible or not.
    pub(super) fn primitive_batches(
        &mut self,
        w: &World,
        r: &mut impl Writes,
    ) -> Result<(), RenderError> {
        self.dimensions.fill([1.0; 3]);
        for group in &mut self.groups {
            group.slots.clear();
        }
        each_shown(w, |e, mesh| {
            mesh.validate().map_err(RenderError::scene)?;
            if matches!(mesh, Mesh::Asset(_)) {
                return Ok(());
            }
            let shape = Shape::of(mesh)?;
            let slot = e.index() as usize;
            if self.dimensions.len() <= slot {
                self.dimensions.resize(slot + 1, [1.0; 3]);
            }
            self.dimensions[slot] = dimensions(mesh);
            if !w.is_visible(e) {
                return Ok(());
            }
            let viewmodel = w.has::<ViewModel>(e);
            let group = *self.shapes.entry((shape, viewmodel)).or_insert_with(|| {
                let (v, i) = shape.geometry();
                let index = self.groups.len();
                self.groups.push(Group {
                    mesh: r.mesh(&v, &i),
                    viewmodel,
                    slots: Vec::new(),
                });
                index
            });
            self.groups[group].slots.push(e.index());
            Ok::<(), RenderError>(())
        })?;
        self.batches.clear();
        self.slots.clear();
        for group in &self.groups {
            if group.slots.is_empty() {
                continue;
            }
            let start = self.slots.len() as u32;
            self.slots.extend_from_slice(&group.slots);
            let mut batch = Batch::new(group.mesh, start..self.slots.len() as u32);
            if group.viewmodel {
                batch = batch.viewmodel();
            }
            self.batches.push(batch);
        }
        Ok(())
    }
}

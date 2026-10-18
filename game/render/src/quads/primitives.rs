use super::{feed, Kind, Order, Quad, NO_LOOK};
use crate::{
    shapes,
    world::{dimensions, Shape},
    FrameInput, RenderError,
};
use exact_game::{Entity, Glow, Material, Mesh, Transform, World};
use glam::Vec3;
use std::collections::BTreeMap;

struct Primitive {
    entity: Entity,
    mesh: Mesh,
    poses: [Transform; 2],
    triangles: Vec<[Vec3; 3]>,
    material: Material,
    glow: Option<Glow>,
    opacity: f32,
}
#[derive(Default)]
pub(super) struct Primitives {
    hz: u32,
    meshes: Vec<super::Item<Mesh>>,
    items: BTreeMap<Entity, Primitive>,
}
impl Primitives {
    pub fn feed(
        &mut self,
        w: &World,
        initial: bool,
        next_tick: bool,
        parent_changed: bool,
    ) -> Result<(), RenderError> {
        self.hz = w.hz();
        feed(w, &mut self.meshes, initial, next_tick, parent_changed);
        self.items.retain(|_, p| {
            self.meshes
                .binary_search_by_key(&p.entity.index(), |m| m.entity.index())
                .is_ok_and(|i| {
                    self.meshes[i].entity == p.entity
                        && !matches!(self.meshes[i].value, Mesh::Asset(_))
                })
                && w.get::<Material>(p.entity)
                    .is_some_and(|m| (0.0..1.0).contains(&m.color[3]))
        });
        for item in &self.meshes {
            if matches!(item.value, Mesh::Asset(_)) {
                continue;
            }
            let Some(material) = w
                .get::<Material>(item.entity)
                .filter(|m| (0.0..1.0).contains(&m.color[3]))
            else {
                continue;
            };
            item.value.validate().map_err(RenderError::scene)?;
            let triangles = || {
                let (vertices, indices) = match &item.value {
                    Mesh::Ring { inner, outer } => shapes::ring(*inner, *outer, 48),
                    Mesh::Torus { radius, tube } => shapes::torus(*radius, *tube, 48),
                    Mesh::Cone { radius, height } => shapes::cone(*radius, *height, 32),
                    Mesh::Capsule { radius, height } => shapes::capsule(*radius, *height, 24),
                    mesh => {
                        let (mut v, i) = Shape::of(mesh).expect("primitive shape").geometry();
                        let size = Vec3::from(dimensions(mesh));
                        for vertex in &mut v {
                            vertex.position = (Vec3::from(vertex.position) * size).to_array();
                        }
                        (v, i)
                    }
                };
                indices
                    .chunks_exact(3)
                    .map(|t| std::array::from_fn(|i| Vec3::from(vertices[t[i] as usize].position)))
                    .collect()
            };
            let p = self.items.entry(item.entity).or_insert_with(|| Primitive {
                entity: item.entity,
                mesh: item.value.clone(),
                poses: item.poses,
                triangles: triangles(),
                material: *material,
                glow: None,
                opacity: 1.,
            });
            if p.mesh != item.value {
                p.triangles = triangles();
                p.mesh.clone_from(&item.value);
            }
            p.poses = item.poses;
            p.material = *material;
            if let Some(tint) = w.get::<exact_game::Tint>(item.entity) {
                for i in 0..4 {
                    p.material.color[i] *= tint.color[i];
                }
                for i in 0..3 {
                    p.material.emissive[i] += tint.emissive[i];
                }
            }
            p.glow = w.get::<Glow>(item.entity).map(|g| g.clone());
            p.opacity = w.drawn(item.entity).map_or(1., |d| d.opacity);
        }
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.items.values().map(|p| p.triangles.len()).sum()
    }
    pub fn frame(
        &self,
        f: &FrameInput<'_>,
        camera: &[[f32; 4]; 6],
        cull: bool,
        data: &mut Vec<Quad>,
        order: &mut Vec<Order>,
    ) {
        for p in self.items.values() {
            let matrix =
                f.displayed_matrix(p.entity, crate::world::scene::interpolate(p.poses, f.alpha));
            let reach = p
                .triangles
                .iter()
                .flatten()
                .map(|v| v.length())
                .fold(0., f32::max)
                * matrix
                    .x_axis
                    .truncate()
                    .length()
                    .max(matrix.y_axis.truncate().length())
                    .max(matrix.z_axis.truncate().length());
            if cull && !crate::cull::sphere_visible(camera, matrix.w_axis.truncate(), reach) {
                continue;
            }
            let glow = p
                .glow
                .as_ref()
                .map_or(1., |g| g.0.value_at(f.seconds, self.hz));
            let m = p.material;
            let color = [m.color[0], m.color[1], m.color[2], m.color[3] * p.opacity];
            let emissive = m.emissive.map(|c| c * glow);
            for triangle in &p.triangles {
                let [a, b, c] = triangle.map(|v| matrix.transform_point3(v));
                let index = data.len();
                data.push(super::quad(
                    a,
                    b - a,
                    c - a,
                    color,
                    [emissive[0], emissive[1], emissive[2], 0.],
                    5.,
                    0.,
                ));
                order.push(Order {
                    kind: Kind::Particle(false, NO_LOOK, false),
                    depth: -f.view.transform_point3((a + b + c) / 3.).z,
                    layer: 0,
                    slot: p.entity.index(),
                    index,
                });
            }
        }
    }
}

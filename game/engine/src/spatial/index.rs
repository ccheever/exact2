//! Derived mesh BVH. Collider indices intentionally cannot answer mesh visibility.
use super::*;
use crate::{asset::ModelBounds, Parent, Transform};
use std::cell::{Ref, RefCell};

pub(super) const SLOT_LIMIT: usize = 262_144;
const VISIT_LIMIT: usize = 1_000_000;
const NONE: usize = usize::MAX;

#[derive(Default)]
pub(crate) struct Cache(RefCell<Option<Index>>);
struct Entry {
    entity: Entity,
    inverse: Affine3A,
    mesh: Mesh,
    half: Vec3,
    center: Vec3,
    lo: Vec3,
    hi: Vec3,
}
struct Node {
    lo: Vec3,
    hi: Vec3,
    left: usize,
    right: usize,
}
pub(crate) struct Index {
    key: [u64; 12],
    entries: Vec<Entry>,
    poses: Vec<Option<Affine3A>>,
    camera: Option<(Entity, Camera)>,
    nodes: Vec<Node>,
    parent: Vec<usize>,
    child: Vec<usize>,
    next: Vec<usize>,
}
struct Ray {
    from: Vec3,
    delta: Vec3,
    direction: Vec3,
    distance: f32,
}
pub(crate) struct Sight<'a> {
    index: Ref<'a, Index>,
    excluded: Vec<u8>,
    remaining: usize,
}
fn spend(remaining: &mut usize) -> Result<(), String> {
    *remaining = remaining
        .checked_sub(1)
        .ok_or("layout visibility work budget exceeded (1000000 visits)")?;
    Ok(())
}
pub(crate) fn check_size(w: &World) -> Result<(), String> {
    if w.alive_mask.len() * 64 > SLOT_LIMIT {
        return Err(
            "layout visibility index limit exceeded (262144 entity slots, including dead slots)"
                .into(),
        );
    }
    Ok(())
}
impl Cache {
    pub(super) fn index<'a>(&'a self, w: &World) -> Result<Ref<'a, Index>, String> {
        let slots = w.alive_mask.len() * 64;
        check_size(w)?;
        // Check leases even on a cache hit, as the physics queries do.
        let _leases = w.query::<(
            Option<&Mesh>,
            Option<&Transform>,
            Option<&Parent>,
            Option<&Visible>,
            Option<&ModelBounds>,
        )>();
        let _display_leases = w.query::<(
            Option<&crate::Pose>,
            Option<&crate::Sprite>,
            Option<&crate::SocketFollow>,
            Option<&Camera>,
        )>();
        let key = [
            w.revision::<Mesh>(),
            w.revision::<Transform>(),
            w.revision::<Parent>(),
            w.revision::<Visible>(),
            w.revision::<ModelBounds>(),
            w.entities_revision(),
            w.presentation_generation(),
            w.model_revision(),
            w.revision::<crate::Pose>(),
            w.revision::<crate::Sprite>(),
            w.revision::<crate::SocketFollow>(),
            w.revision::<Camera>(),
        ];
        if self.0.borrow().as_ref().is_none_or(|i| i.key != key) {
            *self.0.borrow_mut() = Some(Index::build(w, slots, key)?);
        }
        Ok(Ref::map(self.0.borrow(), |i| i.as_ref().unwrap()))
    }
}
impl Index {
    pub fn pose(&self, entity: Entity) -> Option<Affine3A> {
        self.poses
            .get(entity.index() as usize)
            .copied()
            .flatten()
            .filter(|p| p.is_finite())
    }
    pub fn view(&self, size: Vec2) -> Option<View> {
        if !size.is_finite() || size.min_element() <= 0. {
            return None;
        }
        let (entity, camera) = self.camera?;
        Some(View {
            pose: self.pose(entity)?,
            camera,
            size,
        })
    }

    fn build(w: &World, slots: usize, key: [u64; 12]) -> Result<Self, String> {
        let mut poses = super::poses::resolve(w, slots)?;
        let camera = w.query::<&Camera>().iter().find_map(|(e, c)| {
            (c.valid()
                && poses[e.index() as usize]
                    .is_some_and(|p| p.is_finite() && p.matrix3.determinant().abs() >= 1e-12))
            .then_some((e, *c))
        });
        let rotation = camera
            .and_then(|(e, _)| poses[e.index() as usize])
            .map_or(crate::Quat::IDENTITY, |p| {
                p.to_scale_rotation_translation().1
            });
        for (e, _) in w.query::<&crate::Sprite>().iter() {
            if let Some(p) = poses[e.index() as usize] {
                let (scale, _, position) = p.to_scale_rotation_translation();
                poses[e.index() as usize] = Some(Affine3A::from_scale_rotation_translation(
                    scale.abs(),
                    rotation,
                    position,
                ));
            }
        }
        let mut index = Self {
            key,
            entries: Vec::new(),
            poses,
            camera,
            nodes: Vec::new(),
            parent: vec![NONE; slots],
            child: vec![NONE; slots],
            next: vec![NONE; slots],
        };
        for (e, p) in w.query::<&Parent>().iter() {
            if w.contains(p.0) {
                let (e, p) = (e.index() as usize, p.0.index() as usize);
                index.parent[e] = p;
                index.next[e] = index.child[p];
                index.child[p] = e;
            }
        }
        for (entity, (mesh, sprite)) in w.query::<(Option<&Mesh>, Option<&crate::Sprite>)>().iter()
        {
            if mesh.is_none() && sprite.is_none() {
                continue;
            }
            if unbounded(w, entity, mesh) || w.get::<Visible>(entity).is_some_and(|v| !v.0) {
                continue;
            }
            let Some(pose) = index.poses[entity.index() as usize] else {
                continue;
            };
            if !pose.is_finite() || pose.matrix3.determinant().abs() < 1e-12 {
                continue;
            }
            let (half, center) = bounds(w, entity, mesh);
            if !half.is_finite()
                || !center.is_finite()
                || half.min_element() < 0.0
                || half.max_element() <= 0.0
            {
                continue;
            }
            let points = corners(pose, half, center);
            let lo = points
                .into_iter()
                .fold(Vec3::splat(f32::INFINITY), Vec3::min);
            let hi = points
                .into_iter()
                .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
            index.entries.push(Entry {
                entity,
                inverse: pose.inverse(),
                mesh: match mesh {
                    Some(Mesh::Asset(_)) | None => Mesh::Box { size: half * 2.0 },
                    Some(mesh) => mesh.clone(),
                },
                half,
                center,
                lo,
                hi,
            });
        }
        if !index.entries.is_empty() {
            Self::branch(&mut index.nodes, &mut index.entries, 0);
        }
        Ok(index)
    }
    // Balanced median BVH: <=19 levels at the slot limit. select_nth is worst-case
    // linear; each level visits at most N bounds, so rebuilding is O(N log N).
    fn branch(nodes: &mut Vec<Node>, entries: &mut [Entry], offset: usize) -> usize {
        let lo = entries
            .iter()
            .fold(Vec3::splat(f32::INFINITY), |v, e| v.min(e.lo));
        let hi = entries
            .iter()
            .fold(Vec3::splat(f32::NEG_INFINITY), |v, e| v.max(e.hi));
        let at = nodes.len();
        nodes.push(Node {
            lo,
            hi,
            left: offset,
            right: NONE,
        });
        if entries.len() > 1 {
            let size = hi - lo;
            let axis = if size.x >= size.y && size.x >= size.z {
                0
            } else if size.y >= size.z {
                1
            } else {
                2
            };
            let mid = entries.len() / 2;
            entries.select_nth_unstable_by(mid, |a, b| {
                (a.lo[axis] + a.hi[axis])
                    .total_cmp(&(b.lo[axis] + b.hi[axis]))
                    .then_with(|| a.entity.index().cmp(&b.entity.index()))
            });
            let (a, b) = entries.split_at_mut(mid);
            nodes[at].left = Self::branch(nodes, a, offset);
            nodes[at].right = Self::branch(nodes, b, offset + mid);
        }
        at
    }
    fn visit(
        &self,
        at: usize,
        ray: &Ray,
        excluded: &[u8],
        mask: u8,
        remaining: &mut usize,
        hit: &mut impl FnMut(Entity, f32) -> bool,
    ) -> Result<bool, String> {
        spend(remaining)?;
        let node = &self.nodes[at];
        if !segment_box(ray.from, ray.delta, node.lo, node.hi) {
            return Ok(false);
        }
        if node.right != NONE {
            return Ok(self.visit(node.left, ray, excluded, mask, remaining, hit)?
                || self.visit(node.right, ray, excluded, mask, remaining, hit)?);
        }
        let e = &self.entries[node.left];
        if excluded[e.entity.index() as usize] & mask != 0 {
            return Ok(false);
        }
        let o = e.inverse.transform_point3(ray.from);
        if contains_origin(&e.mesh, o, e.half, e.center) {
            return Ok(false);
        }
        let d = e.inverse.transform_vector3(ray.direction);
        if let Some(t) = shape_hit(&e.mesh, o, d, e.half, e.center) {
            if t > 1e-5 && t < ray.distance - 1e-5 {
                return Ok(hit(e.entity, t));
            }
        }
        Ok(false)
    }
}
// Segment broad phase includes origins inside nodes; a BVH node is not a shell.
fn segment_box(from: Vec3, delta: Vec3, lo: Vec3, hi: Vec3) -> bool {
    let (mut near, mut far) = (0.0f32, 1.0f32);
    for axis in 0..3 {
        if delta[axis] == 0.0 {
            if from[axis] < lo[axis] || from[axis] > hi[axis] {
                return false;
            }
        } else {
            let a = (lo[axis] - from[axis]) / delta[axis];
            let b = (hi[axis] - from[axis]) / delta[axis];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return false;
            }
        }
    }
    true
}
impl<'a> Sight<'a> {
    pub fn pose(&self, entity: Entity) -> Option<Affine3A> {
        self.index.pose(entity)
    }
    pub fn view(&self, size: Vec2) -> Option<View> {
        self.index.view(size)
    }
    pub fn new(
        w: &'a World,
        subject: Entity,
        target: Option<Entity>,
        camera: Option<Entity>,
    ) -> Result<Self, String> {
        let index = w.sight.index(w)?;
        let mut sight = Self {
            excluded: vec![0; index.parent.len()],
            index,
            remaining: VISIT_LIMIT,
        };
        // Bits select the independently computed LOS and camera exclusion sets.
        sight.exclude(subject, 3)?;
        if let Some(e) = target {
            sight.exclude(e, 1)?;
        }
        if let Some(e) = camera.or(sight.index.camera.map(|(e, _)| e)) {
            sight.exclude(e, 2)?;
        }
        Ok(sight)
    }
    fn exclude(&mut self, e: Entity, mask: u8) -> Result<(), String> {
        let root = e.index() as usize;
        let mut stack = vec![root];
        while let Some(at) = stack.pop() {
            spend(&mut self.remaining)?;
            if self.excluded[at] & mask == mask {
                continue;
            }
            self.excluded[at] |= mask;
            let mut child = self.index.child[at];
            while child != NONE {
                stack.push(child);
                child = self.index.next[child];
            }
        }
        // Ancestors belong to this endpoint too; their other descendants do not.
        // Thus a shared scene root never silences sibling objects.
        let mut at = self.index.parent[root];
        while at != NONE {
            spend(&mut self.remaining)?;
            if self.excluded[at] & mask == mask {
                break;
            }
            self.excluded[at] |= mask;
            at = self.index.parent[at];
        }
        Ok(())
    }
    pub fn segment(
        &mut self,
        from: Vec3,
        to: Vec3,
        mask: u8,
        mut hit: impl FnMut(Entity, f32) -> bool,
    ) -> Result<bool, String> {
        let delta = to - from;
        let distance = delta.length();
        if !from.is_finite() || !to.is_finite() || !distance.is_finite() {
            return Err("layout visibility endpoints exceed finite segment range".into());
        }
        if self.index.nodes.is_empty() {
            return Ok(false);
        }
        if distance <= 1e-5 {
            return Ok(false);
        }
        let ray = Ray {
            from,
            delta,
            distance,
            direction: delta / distance,
        };
        self.index
            .visit(0, &ray, &self.excluded, mask, &mut self.remaining, &mut hit)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod display_tests;

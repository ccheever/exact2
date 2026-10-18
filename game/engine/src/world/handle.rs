use crate::{Component, Glow, Material, RefMut, Transform};
use std::any::Any;

impl<'w, C: Component> RefMut<'w, C> {
    /// Agent-visible name of this row's entity, or None for an unnamed entity.
    /// Available on rows returned by World::get_mut, require_mut and nearest_xz_mut.
    pub fn name(&self) -> Option<&'w str> {
        let (world, entity) = self.owner.expect("entity verb requires a World row handle");
        world.name(entity)
    }

    /// Retarget the entity's saved Glow at the end of the current tick.
    /// Intensity multiplies authored emission; duration is in seconds.
    /// Requires Glow on the entity, with the same row-borrow checks as require_mut.
    #[track_caller]
    pub fn glow_to(&mut self, intensity: f32, seconds: f32) {
        let (world, entity) = self.owner.expect("glow_to requires a World row handle");
        let now = world.tick_end();
        if let Some(glow) = (&mut **self as &mut dyn Any).downcast_mut::<Glow>() {
            glow.0.to(now, intensity, seconds);
        } else {
            world
                .require_mut::<Glow>(entity)
                .0
                .to(now, intensity, seconds);
        }
    }

    /// Set the entity's authored linear RGB colour, preserving alpha and emission.
    /// Requires Material. For a cosmetic multiply derived in present, use Tint.
    #[track_caller]
    pub fn tint(&mut self, color: [f32; 3]) {
        let (world, entity) = self.owner.expect("tint requires a World row handle");
        let apply = |material: &mut Material| material.color[..3].copy_from_slice(&color);
        if let Some(material) = (&mut **self as &mut dyn Any).downcast_mut::<Material>() {
            apply(material);
        } else {
            apply(&mut world.require_mut::<Material>(entity));
        }
    }

    /// Translate the entity's local simulated position by metres. Requires Transform.
    /// For an appearance derived in present, use Offset instead.
    #[track_caller]
    pub fn offset(&mut self, x: f32, y: f32, z: f32) {
        let (world, entity) = self.owner.expect("offset requires a World row handle");
        let delta = crate::Vec3::new(x, y, z);
        if let Some(pose) = (&mut **self as &mut dyn Any).downcast_mut::<Transform>() {
            pose.position += delta;
        } else {
            world.require_mut::<Transform>(entity).position += delta;
        }
    }
}

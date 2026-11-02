use crate::{Affine3A, Entity, Transform};

/// The rows of [`World::near`](crate::World::near) and
/// [`World::near_xz`](crate::World::near_xz): each entity in the radius with a
/// copy of its global pose, in ascending entity order.
///
/// Unlimited, rows are read as the loop reaches them, so a later row sees a
/// write an earlier iteration made. [`Near::limit`] reads every row first.
pub struct Near<I> {
    pub(super) rows: I,
}

impl<I: Iterator<Item = (Entity, Affine3A, f32)>> Near<I> {
    /// Keep the `n` nearest rows, still yielded in ascending entity order;
    /// equal distances keep the lower entity index. Every row in the radius
    /// is read when this is called, so writes made while iterating the result
    /// do not change which rows it holds: a school's neighbours are the ones
    /// that were there at the start of the loop.
    pub fn limit(self, n: usize) -> Near<std::vec::IntoIter<(Entity, Affine3A, f32)>> {
        let mut rows: Vec<_> = self.rows.collect();
        if rows.len() > n {
            // Distance, then index: a total order, so the choice is the same on every host.
            let order = |a: &(Entity, Affine3A, f32), b: &(Entity, Affine3A, f32)| {
                a.2.total_cmp(&b.2).then(a.0.index().cmp(&b.0.index()))
            };
            if n > 0 {
                rows.select_nth_unstable_by(n - 1, order);
            }
            rows.truncate(n);
            rows.sort_unstable_by_key(|row| row.0.index());
        }
        Near {
            rows: rows.into_iter(),
        }
    }
}

impl<I: Iterator<Item = (Entity, Affine3A, f32)>> Iterator for Near<I> {
    type Item = (Entity, Transform);
    fn next(&mut self) -> Option<Self::Item> {
        let (entity, pose, _) = self.rows.next()?;
        let (scale, rotation, position) = pose.to_scale_rotation_translation();
        Some((
            entity,
            Transform {
                position,
                rotation,
                scale,
            },
        ))
    }
}

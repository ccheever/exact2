//! A certificate for Parry 0.30.2's binned partition order. Refit is safe for
//! character traversal only while a fresh build would make the same partitions.
//! Static bin summaries are retained; verification visits branches with bodies.
//! A changed split falls back to the canonical build, never a different tie order.
use rapier3d::parry::{
    bounding_volume::{Aabb, BoundingVolume},
    math::Vector,
};

#[derive(Clone, Copy)]
struct Bin {
    bounds: Aabb,
    centers: Aabb,
    count: usize,
}
impl Default for Bin {
    fn default() -> Self {
        Self {
            bounds: Aabb::new_invalid(),
            centers: Aabb::new_invalid(),
            count: 0,
        }
    }
}
impl Bin {
    fn add(&mut self, bounds: &Aabb) {
        self.bounds.merge(bounds);
        let center = bounds.center();
        self.centers.mins = self.centers.mins.min(center);
        self.centers.maxs = self.centers.maxs.max(center);
        self.count += 1;
    }
    fn merge(&mut self, other: Self) {
        self.bounds.merge(&other.bounds);
        self.count += other.count;
    }
}
#[derive(Clone, Copy, PartialEq)]
struct Grid {
    axis: usize,
    min: f32,
    factor: f32,
}
impl Grid {
    fn new(bounds: Aabb) -> Self {
        let axis = bounds.extents().max_position();
        Self {
            axis,
            min: bounds.mins[axis],
            factor: 8. * (1. - 1.0e-5) / (bounds.maxs[axis] - bounds.mins[axis]),
        }
    }
    fn bin(self, center: Vector) -> usize {
        ((self.factor * (center[self.axis] - self.min)) as usize).min(7)
    }
}
fn split(bins: [Bin; 8], len: usize) -> (usize, usize, bool) {
    let mut right = bins;
    let mut acc = bins[7];
    for i in (1..7).rev() {
        acc.merge(bins[i]);
        right[i] = acc;
    }
    let mut left = bins[0];
    let mut best = f32::MAX;
    let mut plane = 0;
    let mut mid = left.count;
    for i in 0..7 {
        let cost = left.bounds.volume() * left.count as f32
            + right[i + 1].bounds.volume() * right[i + 1].count as f32;
        if cost < best {
            best = cost;
            plane = i;
            mid = left.count;
        }
        left.merge(bins[i + 1]);
    }
    let degenerate = mid == 0 || mid == len;
    (plane, if degenerate { len / 2 } else { mid }, degenerate)
}
fn centers(ids: impl Iterator<Item = u32>, bounds: &[Aabb]) -> Aabb {
    let mut result = Aabb::new_invalid();
    for id in ids {
        let p = bounds[id as usize].center();
        result.mins = result.mins.min(p);
        result.maxs = result.maxs.max(p);
    }
    result
}
#[derive(Default)]
pub(crate) struct Binned {
    root: Option<Box<Node>>,
}
struct Node {
    statics: Vec<u32>,
    dynamics: Vec<u32>,
    static_centers: Aabb,
    sides: [Aabb; 2],
    dynamic_left: Vec<u32>,
    dynamic_right: Vec<u32>,
    bins: [Bin; 8],
    grid: Grid,
    mid: usize,
    len: usize,
    ordered: bool,
    children: [Option<Box<Node>>; 2],
}
impl Binned {
    pub fn new(bounds: &[Aabb], dynamic: &[bool]) -> Self {
        let mut ids: Vec<_> = (0..bounds.len() as u32).collect();
        Self {
            root: Node::new(&mut ids, bounds, dynamic),
        }
    }
    pub fn matches(&mut self, bounds: &[Aabb]) -> bool {
        self.root.as_mut().is_none_or(|n| n.matches(bounds))
    }
}
impl Node {
    fn new(ids: &mut [u32], bounds: &[Aabb], dynamic: &[bool]) -> Option<Box<Self>> {
        if ids.len() < 2 || !ids.iter().any(|i| dynamic[*i as usize]) {
            return None;
        }
        let statics: Vec<_> = ids
            .iter()
            .copied()
            .filter(|i| !dynamic[*i as usize])
            .collect();
        let dynamics: Vec<_> = ids
            .iter()
            .copied()
            .filter(|i| dynamic[*i as usize])
            .collect();
        let grid = Grid::new(centers(ids.iter().copied(), bounds));
        let mut bins = [Bin::default(); 8];
        for &id in &statics {
            bins[grid.bin(bounds[id as usize].center())].add(&bounds[id as usize]);
        }
        let mut all = bins;
        for &id in &dynamics {
            all[grid.bin(bounds[id as usize].center())].add(&bounds[id as usize]);
        }
        let (plane, mid, degenerate) = split(all, ids.len());
        let mut ordered = true;
        if !degenerate {
            let (mut l, mut r) = (0, mid);
            while l < mid && r < ids.len() {
                while l < mid && grid.bin(bounds[ids[l] as usize].center()) <= plane {
                    l += 1;
                }
                while r < ids.len() && grid.bin(bounds[ids[r] as usize].center()) > plane {
                    r += 1;
                }
                if l < mid && r < ids.len() {
                    ids.swap(l, r);
                    ordered = false;
                    l += 1;
                    r += 1;
                }
            }
        }
        let sides = [
            centers(
                ids[..mid].iter().copied().filter(|i| !dynamic[*i as usize]),
                bounds,
            ),
            centers(
                ids[mid..].iter().copied().filter(|i| !dynamic[*i as usize]),
                bounds,
            ),
        ];
        let dynamic_left = ids[..mid]
            .iter()
            .copied()
            .filter(|i| dynamic[*i as usize])
            .collect();
        let dynamic_right = ids[mid..]
            .iter()
            .copied()
            .filter(|i| dynamic[*i as usize])
            .collect();
        let len = ids.len();
        let (left, right) = ids.split_at_mut(mid);
        Some(Box::new(Self {
            static_centers: centers(statics.iter().copied(), bounds),
            statics,
            dynamics,
            sides,
            dynamic_left,
            dynamic_right,
            bins,
            grid,
            mid,
            len,
            ordered,
            children: [
                Self::new(left, bounds, dynamic),
                Self::new(right, bounds, dynamic),
            ],
        }))
    }
    fn matches(&mut self, bounds: &[Aabb]) -> bool {
        let mut c = self.static_centers;
        if !self.dynamics.is_empty() {
            c.merge(&centers(self.dynamics.iter().copied(), bounds));
        }
        let grid = Grid::new(c);
        if grid != self.grid {
            // A moving extremum can change the grid without moving any static
            // center to another bin. The center ranges certify this in O(8).
            if !self.bins.iter().enumerate().all(|(i, b)| {
                b.count == 0 || (grid.bin(b.centers.mins) == i && grid.bin(b.centers.maxs) == i)
            }) {
                self.bins = [Bin::default(); 8];
                for &id in &self.statics {
                    self.bins[grid.bin(bounds[id as usize].center())].add(&bounds[id as usize]);
                }
            }
            self.grid = grid;
        }
        let mut bins = self.bins;
        for &id in &self.dynamics {
            bins[grid.bin(bounds[id as usize].center())].add(&bounds[id as usize]);
        }
        let (plane, mid, degenerate) = split(bins, self.len);
        if mid != self.mid {
            return false;
        }
        if degenerate {
            if !self.ordered {
                return false;
            }
        } else {
            for (side, ids) in [&self.dynamic_left, &self.dynamic_right]
                .into_iter()
                .enumerate()
            {
                let mut centers = self.sides[side];
                if !ids.is_empty() {
                    centers.merge(&crate::binned::centers(ids.iter().copied(), bounds));
                }
                if centers.mins.x <= centers.maxs.x {
                    if (side == 0 && grid.bin(centers.maxs) > plane)
                        || (side == 1 && grid.bin(centers.mins) <= plane)
                    {
                        return false;
                    }
                }
            }
        }
        self.children
            .iter_mut()
            .all(|c| c.as_mut().is_none_or(|n| n.matches(bounds)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rapier3d::parry::partitioning::{Bvh, BvhBuildStrategy};
    #[test]
    fn certified_refits_keep_canonical_traversal_and_equal_cost_winners() {
        let mut bounds: Vec<_> = (0..240)
            .map(|i| {
                let p = Vector::new((i % 17) as f32, (i % 3) as f32, (i / 17) as f32);
                Aabb::new(p, p + Vector::ONE)
            })
            .collect();
        let dynamic: Vec<_> = (0..bounds.len()).map(|i| i % 7 == 0).collect();
        let make =
            |b: &[Aabb]| Bvh::from_iter(BvhBuildStrategy::Binned, b.iter().copied().enumerate());
        let mut actual = make(&bounds);
        let mut cert = Binned::new(&bounds, &dynamic);
        let mut refits = 0;
        let mut rebuilds = 0;
        for tick in 0..200 {
            let ids: Vec<_> = dynamic
                .iter()
                .enumerate()
                .filter_map(|(i, d)| d.then_some(i as u32))
                .collect();
            for &id in &ids {
                let delta = Vector::new(if tick % 31 == 0 { -0.6 } else { 0.02 }, 0., 0.);
                bounds[id as usize].mins += delta;
                bounds[id as usize].maxs += delta;
            }
            let canonical = make(&bounds);
            if cert.matches(&bounds) {
                refits += 1;
                for &id in &ids {
                    actual.insert_or_update_partially(bounds[id as usize], id, 0.);
                }
                actual.refit_partial(&[], &ids);
            } else {
                rebuilds += 1;
                actual = make(&bounds);
                cert = Binned::new(&bounds, &dynamic);
            }
            assert_eq!(
                actual
                    .intersect_aabb(&canonical.root_aabb())
                    .collect::<Vec<_>>(),
                canonical
                    .intersect_aabb(&canonical.root_aabb())
                    .collect::<Vec<_>>()
            );
            for i in 0..40 {
                let region = Aabb::new(
                    Vector::new((i % 17) as f32, 0., (i / 3) as f32),
                    Vector::new((i % 17) as f32 + 3., 4., (i / 3) as f32 + 3.),
                );
                assert_eq!(
                    actual.intersect_aabb(&region).collect::<Vec<_>>(),
                    canonical.intersect_aabb(&region).collect::<Vec<_>>()
                );
                let cost = |n: &rapier3d::parry::partitioning::BvhNode, _: f32| {
                    if n.aabb().intersects(&region) {
                        0.
                    } else {
                        f32::MAX
                    }
                };
                assert_eq!(
                    actual.find_best(2., cost, |_, _| Some(1.)),
                    canonical.find_best(2., cost, |_, _| Some(1.))
                );
            }
        }
        assert!(
            refits > 0 && rebuilds > 0,
            "refits={refits} rebuilds={rebuilds}"
        );
    }
}

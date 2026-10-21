//! Native threshold-zero observations. @ref LLP 1005 §Intersection visibility.
use super::{DataSource, Runner};
use exact_kernel::{NodeKey, ViewId};
use exact_plan::EventKind;

/// One sampled entry; its generation and observer options travel with the sample.
#[derive(Debug, Clone)]
pub struct Intersection {
    /// The mounted view that owns the handler.
    pub view: ViewId,
    /// Edge contact counts as intersecting, even when the area is zero.
    pub is_intersecting: bool,
    /// The clipped area divided by the target's bounding box area.
    pub intersection_ratio: f64,
    key: NodeKey,
    root: Option<NodeKey>,
    margin: String,
}

pub(super) type Intersections = Vec<Intersection>;

impl<D: DataSource> Runner<D> {
    /// Sample mounted handlers after layout or a reported scroll. Ratio-only
    /// changes are silent. A new root or margin starts a new observation.
    pub fn intersect_due(&mut self) -> Vec<Intersection> {
        if !self
            .plan
            .handlers
            .iter()
            .any(|h| h.event == EventKind::Intersect)
        {
            return Vec::new();
        }
        let arena = self.kernel.arena();
        self.intersections
            .retain(|old| arena.resolve(old.key).is_some());
        let mut due: Vec<_> = self
            .ids
            .sites()
            .filter_map(|(view, node)| {
                if !self
                    .plan
                    .node(node)
                    .handlers
                    .iter()
                    .any(|h| self.plan.handler(h).event == EventKind::Intersect)
                {
                    return None;
                }
                let key = arena.key_of(view)?;
                let (root, margin, is_intersecting, intersection_ratio) =
                    crate::geometry::intersection(&self.kernel, &self.scrolled, view);
                let changed = !self.intersections.iter().any(|old| {
                    old.key == key
                        && old.root == root
                        && old.margin == margin
                        && old.is_intersecting == is_intersecting
                });
                changed.then_some(Intersection {
                    view,
                    key,
                    root,
                    margin,
                    is_intersecting,
                    intersection_ratio,
                })
            })
            .collect();
        // Existing observations keep registration order through keyed moves.
        // Fresh view ids are monotonic in mount order; changed options reobserve
        // at the end of their group, just like the browser adapter.
        due.sort_by_key(|entry| {
            (
                self.intersections
                    .iter()
                    .position(|old| {
                        old.key == entry.key && old.root == entry.root && old.margin == entry.margin
                    })
                    .unwrap_or(usize::MAX),
                entry.view,
            )
        });
        due
    }

    /// Mark before dispatch. False means an earlier handler removed/recycled
    /// the sampled view; its queued entry must not reach the replacement.
    pub fn intersect_delivered(&mut self, entry: &Intersection) -> bool {
        let arena = self.kernel.arena();
        self.intersections
            .retain(|old| arena.resolve(old.key).is_some());
        if arena.resolve(entry.key).is_none() {
            return false;
        }
        if let Some(index) = self
            .intersections
            .iter()
            .position(|old| old.key == entry.key)
        {
            let old = &self.intersections[index];
            if old.root == entry.root && old.margin == entry.margin {
                self.intersections[index] = entry.clone();
                return true;
            }
            self.intersections.remove(index);
        }
        self.intersections.push(entry.clone());
        true
    }

    /// A scroll container the host presents now stands at `(left, top)` CSS
    /// px, or the page does (`None`). Noted, never dispatched from this call.
    pub fn scrolled(&mut self, view: Option<ViewId>, left: f64, top: f64) {
        self.scrolled.note(&self.kernel, view, left, top);
    }
}

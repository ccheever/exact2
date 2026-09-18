//! Untransformed local geometry; mapping identity is distinct from event dimensions.
use super::*;
use exact_kernel::{NodeKey, TransformDragBinding};

#[derive(Clone, PartialEq)]
struct Geometry {
    dimensions: [f64; 4],
    origin: (f64, f64),
    handle_frame: Rect4,
    path: Vec<(NodeKey, (f32, f32))>,
}
impl Geometry {
    fn ready(&self) -> bool {
        self.dimensions.into_iter().all(|n| n > 0.)
    }
}
struct Entry {
    binding: TransformDragBinding,
    geometry: Option<Geometry>,
    revision: u64,
    announced: Option<[f64; 4]>,
}
#[derive(Default)]
pub(super) struct State {
    entries: BTreeMap<NodeKey, Entry>,
    next: u64,
    busy: bool,
}
impl<D: DataSource> Presenter<D> {
    fn transform_geometry(&self, binding: TransformDragBinding) -> Option<Geometry> {
        if !self.host.transform_path_ready(binding) {
            return None;
        }
        let kernel = self.host.kernel();
        let target = kernel.node_by_key(binding.target)?;
        let clip = kernel.node_by_key(binding.clip)?;
        let handle = kernel.node_by_key(binding.handle)?;
        let a = target.frame;
        let b = clip.frame;
        let vals = [a.x, a.y, a.width, a.height, b.x, b.y, b.width, b.height];
        if !vals.into_iter().all(f32::is_finite)
            || [a.width, a.height, b.width, b.height]
                .into_iter()
                .any(|n| n < 0.)
        {
            return None;
        }
        // Less than one thousandth of a layout pixel permits f32 rounding only.
        if [
            (a.x, b.x),
            (a.y, b.y),
            (a.width, b.width),
            (a.height, b.height),
        ]
        .into_iter()
        .any(|(x, y)| (x as f64 - y as f64).abs() > 1. / 1024.)
        {
            return None;
        }
        let mut path = Vec::new();
        let mut offset = (self.page.0 as f64, self.page.1 as f64);
        let mut at = Some(handle.id);
        let mut above_clip = false;
        while let Some(id) = at {
            let n = kernel.node(id)?;
            let scroll = self.scroll.get(&id).copied().unwrap_or((0., 0.));
            if !scroll.0.is_finite() || !scroll.1.is_finite() {
                return None;
            }
            path.push((n.key, scroll));
            if above_clip {
                offset.0 += scroll.0 as f64;
                offset.1 += scroll.1 as f64;
            }
            if n.key == binding.clip {
                above_clip = true;
            }
            at = n.parent;
        }
        let f = handle.frame;
        let origin = (b.x as f64 - offset.0, b.y as f64 - offset.1);
        if !origin.0.is_finite() || !origin.1.is_finite() {
            return None;
        }
        Some(Geometry {
            dimensions: [
                a.width as f64,
                a.height as f64,
                b.width as f64,
                b.height as f64,
            ],
            origin,
            handle_frame: (f.x, f.y, f.width, f.height),
            path,
        })
    }
    pub(super) fn transform_revision(&self, binding: TransformDragBinding) -> Option<u64> {
        let e = self.transform_geometry.entries.get(&binding.handle)?;
        let g = e.geometry.as_ref()?;
        (e.binding == binding && g.ready() && self.transform_geometry(binding).as_ref() == Some(g))
            .then_some(e.revision)
    }
    /// Only external layout/receipt/scroll paths call this; held transform ticks do not.
    pub(super) fn refresh_transform_geometry(&mut self) -> Option<String> {
        if self.transform_geometry.busy {
            return None;
        }
        self.transform_geometry.busy = true;
        let mut error = None;
        // A callback may change geometry once. Defer any further change until
        // another external turn; no notification or self-sustaining idle loop.
        for _ in 0..2 {
            let bindings = self.host.transform_bindings();
            let live: std::collections::BTreeSet<_> = bindings.iter().map(|b| b.handle).collect();
            self.transform_geometry
                .entries
                .retain(|key, _| live.contains(key));
            let mut actions = Vec::new();
            for binding in bindings {
                let geometry = self.transform_geometry(binding);
                let changed = self
                    .transform_geometry
                    .entries
                    .get(&binding.handle)
                    .is_none_or(|e| e.binding != binding || e.geometry != geometry);
                if changed {
                    let Some(next) = self.transform_geometry.next.checked_add(1) else {
                        error = Some("transform geometry revision exhausted".into());
                        break;
                    };
                    self.transform_geometry.next = next;
                    let previous = self.transform_geometry.entries.remove(&binding.handle);
                    let announced = previous
                        .filter(|e| e.binding == binding)
                        .and_then(|e| e.announced);
                    self.transform_geometry.entries.insert(
                        binding.handle,
                        Entry {
                            binding,
                            geometry,
                            revision: next,
                            announced,
                        },
                    );
                }
                if let Some(entry) = self.transform_geometry.entries.get_mut(&binding.handle) {
                    if let Some(g) = &entry.geometry {
                        if entry.announced != Some(g.dimensions) {
                            entry.announced = Some(g.dimensions);
                            actions.push((binding, g.dimensions));
                        }
                    }
                }
            }
            // Cancel the old mapping BEFORE any authored geometry callback.
            self.retire_pointer();
            if error.is_some() || actions.is_empty() {
                break;
            }
            for (binding, dimensions) in actions {
                if self.host.transform_drag_binding(binding.handle) != Some(binding)
                    || self
                        .transform_geometry(binding)
                        .as_ref()
                        .map(|g| g.dimensions)
                        != Some(dimensions)
                {
                    continue;
                }
                let view = self
                    .host
                    .kernel()
                    .node_by_key(binding.handle)
                    .expect("live handle")
                    .id;
                let [box_width, box_height, port_width, port_height] = dimensions;
                let dispatched = self.host.dispatch_at(
                    view,
                    Event::TransformGeometry {
                        box_width,
                        box_height,
                        port_width,
                        port_height,
                    },
                    self.host.now(),
                );
                let after = self.after_commit();
                error = error.or(dispatched).or(after);
            }
        }
        self.transform_geometry.busy = false;
        // A second callback can change mapping; refuse/cancel immediately even
        // though its dimensions are intentionally deferred to an external turn.
        self.retire_pointer();
        error
    }
}

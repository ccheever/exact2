//! Layout coordinates minus real ancestor scrolling, never inferred row padding.
use super::*;
use exact_kernel::{motion::motion_node, NodeKey};
use exact_motion::{Property, Value};
use exact_runner::ReorderBinding;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Mapping {
    pub(super) port: Rect4,
    pub(super) clip: Rect4,
    pub(super) row_width: f64,
    path: Vec<(NodeKey, (f32, f32))>,
}
impl<D: DataSource> Presenter<D> {
    pub(super) fn arrange_base(&self, key: NodeKey) -> Option<Value> {
        let node = self.host.kernel().node_by_key(key)?;
        let mut v = Value {
            x: node.frame.x as f64 - self.page.0 as f64,
            y: node.frame.y as f64 - self.page.1 as f64,
            ..Value::ZERO
        };
        let mut at = node.parent;
        while let Some(id) = at {
            let parent = self.host.kernel().node(id)?;
            let (x, y) = self.scroll_of(id);
            v.x -= x as f64;
            v.y -= y as f64;
            at = parent.parent;
        }
        finite(v).then_some(v)
    }
    pub(super) fn arrange_mapping(&self, b: ReorderBinding) -> Option<Mapping> {
        // Picture replay has its own accepted ordering. This initial adapter
        // supports ordinary live collections, never silently lifts a stale picture.
        if self.host.content_region().is_some() {
            return None;
        }
        let kernel = self.host.kernel();
        let list = kernel.node_by_key(b.list)?;
        let mut child = kernel.node_by_key(b.handle)?;
        while child.key != b.list {
            let p = self.host.presented(child.id);
            if p.scale != 1.
                || p.rotate != 0.
                || [Property::Scale, Property::Rotate]
                    .into_iter()
                    .any(|prop| self.host.engine().is_active(motion_node(child.key), prop))
                || (child.key != b.wrapper
                    && ((p.translate != (0., 0.) || p.translate_percent != (0., 0.))
                        || self
                            .host
                            .engine()
                            .is_active(motion_node(child.key), Property::Translate)))
            {
                return None;
            }
            child = kernel.node(child.parent?)?;
        }
        let base = self.arrange_base(b.list)?;
        let [top, right, bottom, left] = list.style.border_widths();
        let port = (
            base.x as f32 + left,
            base.y as f32 + top,
            (list.frame.width - left - right).max(0.),
            (list.frame.height - top - bottom).max(0.),
        );
        let mut clip = intersect(port, (0., 0., self.viewport.0, self.viewport.1));
        let mut path = Vec::new();
        let mut at = Some(list.id);
        while let Some(id) = at {
            let node = kernel.node(id)?;
            let p = self.host.presented(id);
            if (p.translate != (0., 0.) || p.translate_percent != (0., 0.))
                || p.scale != 1.
                || p.rotate != 0.
                || [Property::Translate, Property::Scale, Property::Rotate]
                    .into_iter()
                    .any(|prop| self.host.engine().is_active(motion_node(node.key), prop))
            {
                return None;
            }
            if id != list.id {
                path.push((node.key, self.scroll_of(id)));
                let (ox, oy) = effective_overflow(&node);
                if ox != Overflow::Visible || oy != Overflow::Visible {
                    let pos = self.arrange_base(node.key)?;
                    clip = intersect(
                        clip,
                        (
                            pos.x as f32,
                            pos.y as f32,
                            node.frame.width,
                            node.frame.height,
                        ),
                    );
                }
            }
            at = node.parent;
        }
        let g = self.host.runner().reorder_geometry(b.list)?;
        let m = Mapping {
            port,
            clip,
            row_width: g.row_width,
            path,
        };
        [
            port.0, port.1, port.2, port.3, clip.0, clip.1, clip.2, clip.3,
        ]
        .into_iter()
        .all(f32::is_finite)
        .then_some(m)
        .filter(|m| m.clip.2 > 0. && m.clip.3 > 0.)
    }
}
pub(super) fn finite(v: Value) -> bool {
    [v.x, v.y]
        .into_iter()
        .all(|n| n.is_finite() && n.abs() <= f32::MAX as f64)
}
fn intersect(a: Rect4, b: Rect4) -> Rect4 {
    let x = a.0.max(b.0);
    let y = a.1.max(b.1);
    (
        x,
        y,
        (a.0 + a.2).min(b.0 + b.2).max(x) - x,
        (a.1 + a.3).min(b.1 + b.3).max(y) - y,
    )
}

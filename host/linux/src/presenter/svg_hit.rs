//! Hits inside an `svg` (LLP 1055.000 D17): the element under a point, by
//! the kernel's resolved scene and `pointer-events`.
use super::*;

impl<D: DataSource> Presenter<D> {
    /// Whether `node` is an element drawn inside the `svg` `drawing`: the
    /// agent's tap on an `svg` presses the element at its centre, as a
    /// finger there would.
    pub(super) fn drawn_in(&self, node: ViewId, drawing: ViewId) -> bool {
        let kernel = self.host.kernel();
        if kernel
            .node(drawing)
            .is_none_or(|n| n.node_type != NodeType::Svg)
        {
            return false;
        }
        let ancestor = drawing;
        let mut cur = kernel.node(node).and_then(|n| n.parent);
        while let Some(p) = cur {
            if p == ancestor {
                return true;
            }
            cur = kernel.node(p).and_then(|n| n.parent);
        }
        false
    }

    /// What a point in box `b` hits: inside an `svg`, the element under it
    /// by `pointer-events`; else the box itself.
    pub(super) fn svg_hit(&self, b: &crate::paint::PaintedBox, x: f32, y: f32) -> ViewId {
        let kernel = self.host.kernel();
        let Some(node) = kernel.node(b.id).filter(|n| n.node_type == NodeType::Svg) else {
            return b.id;
        };
        let content = exact_kernel::svg::scene::content_box(&node);
        let scene =
            crate::paint::resolve_with(kernel, &node, content, &|id| self.host.presented(id));
        scene
            .hit((x - b.rect.0 - content.0, y - b.rect.1 - content.1))
            .unwrap_or(b.id)
    }
}

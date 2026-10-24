use super::*;

impl<D: DataSource> Presenter<D> {
    /// A wheel at a point (the web's sign: a positive `dy` scrolls down).
    /// A phase-less tick is its own gesture and is not split (LLP 1070 G2,
    /// Chrome's measured tick): the innermost scroll container under the point
    /// that can take ANY of its components takes those it can, clamped, and
    /// the rest is dropped. One that can take none chains to the next scroller
    /// up, then the page, unless its `overscroll-behavior` is `contain` or
    /// `none` on an axis the tick moves along: it keeps (drops) the tick.
    pub fn wheel_at(&mut self, x: f32, y: f32, dx: f32, dy: f32) {
        if ![x, y, dx, dy].iter().all(|v| v.is_finite())
            || (dx == 0.0 && dy == 0.0)
            || !self.display.contains(x, y)
        {
            return;
        }
        if self.alert.is_some() && self.hit(x, y).is_none() {
            return;
        }
        self.restore_scroll_anchors();
        // A canvas that wants input takes the wheel, and the nodes' own
        // `wheel` is still heard (review b5-b 2), as the web's element hears it.
        let canvas = self.surface_wheel(x, y, dx, dy, self.pointer_now());
        if self.wheel_event(x, y, dx, dy) || canvas {
            return;
        }
        // A UI wheel can take over a UI gesture, but does not release a game's
        // captured pointer. Browser wheel events do not end pointer capture.
        if self.contact_canvas().is_none() {
            if let Err(error) = self.pointer_cancel(self.pointer_now()) {
                self.host.log(error);
            }
        }
        let mut at = self.hit(x, y);
        // Only a node the display has no bounds for needs the collections'.
        let mut collection_limits = None;
        while let Some(id) = at {
            let kernel = self.host.kernel();
            let Some(node) = kernel.node(id) else { break };
            let bounds = self.display.bounds(kernel, id).unwrap_or_else(|| {
                let limits =
                    collection_limits.get_or_insert_with(|| self.collection_scroll_limits());
                self.brush.scroll_bounds(
                    kernel,
                    self.host.content_region(),
                    &node,
                    limits.get(&id).copied(),
                )
            });
            let (ox, oy) = bounds.axes;
            let scrolls = matches!(ox, Overflow::Scroll | Overflow::Auto)
                || matches!(oy, Overflow::Scroll | Overflow::Auto);
            let contained =
                |b: exact_kernel::OverscrollBehavior| b != exact_kernel::OverscrollBehavior::Auto;
            let stops = scrolls
                && ((dx != 0.0 && contained(node.style.overscroll_behavior_x))
                    || (dy != 0.0 && contained(node.style.overscroll_behavior_y)));
            // An SVG element has no display box: its ancestors scroll it.
            let parent = self.display.parent(kernel, id).or_else(|| {
                node.node_type
                    .is_svg_element()
                    .then_some(node.parent)
                    .flatten()
            });
            if self.scroll_container_by(id, bounds, dx, dy) {
                self.last_wheel = Some(id);
                return;
            }
            // At every requested edge, containment or the modal ends the chain.
            if stops || self.alert_root() == Some(id) {
                return;
            }
            at = parent;
        }
        if self.alert.is_some() {
            return;
        }
        let doc = self.document();
        let viewport = self.display.viewport().unwrap_or(self.viewport);
        let max = ((doc.0 - viewport.0).max(0.0), (doc.1 - viewport.1).max(0.0));
        let next = (
            (self.page.0 + dx).clamp(0.0, max.0),
            (self.page.1 + dy).clamp(0.0, max.1),
        );
        let moved = self.anchor_input(PAGE, self.page, dy, next);
        if next != self.page || moved.is_some_and(|dy| dy != 0.) {
            self.page = next;
            self.dirty = true;
            if let Some(error) = self.refresh_transform_geometry() {
                self.host.log(error);
            }
        }
    }

    /// Apply one input to a container. False means neither axis can move.
    pub(super) fn scroll_container_by(
        &mut self,
        id: ViewId,
        bounds: crate::paint::ScrollBounds,
        dx: f32,
        dy: f32,
    ) -> bool {
        let (ox, oy) = bounds.axes;
        let max = bounds.max;
        let off = self.scroll.get(&id).copied().unwrap_or((0.0, 0.0));
        let takes = |axis: Overflow, d: f32, off: f32, max: f32| {
            matches!(axis, Overflow::Scroll | Overflow::Auto)
                && d != 0.0
                && max > 0.0
                && ((d > 0.0 && off < max) || (d < 0.0 && off > 0.0))
        };
        let take_x = takes(ox, dx, off.0, max.0);
        let (input_y, input_max) = self
            .anchor_input_position(id, off.1)
            .unwrap_or((off.1, max.1));
        let take_y = takes(oy, dy, input_y, input_max);
        if !take_x && !take_y {
            return false;
        }
        let nx = if take_x {
            (off.0 + dx).clamp(0.0, max.0)
        } else {
            off.0
        };
        let ny = if take_y {
            (off.1 + dy).clamp(0.0, max.1)
        } else {
            off.1
        };
        let moved = self.anchor_input(id, off, if take_y { dy } else { 0. }, (nx, ny));
        self.scroll.insert(id, (nx, ny));
        if (nx, ny) != off || moved.is_some_and(|dy| dy != 0.) {
            self.scroll_end_input(id, off, Completion::Instant);
        }
        self.dirty = true;
        self.collection_scrolled_or_deferred(id);
        if let Some(error) = self.refresh_transform_geometry() {
            self.host.log(error);
        }
        true
    }
}

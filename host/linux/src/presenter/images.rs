//! Visibility comes from the native painter's nested scroll/clip geometry.
use super::*;

impl<D: DataSource> Presenter<D> {
    /// Loads that arrived since the last call: their sizes reach the kernel.
    /// Whether anything changed.
    pub fn poll_images(&mut self) -> bool {
        let reports = self.images.poll();
        self.apply_reports(reports)
    }

    /// Wait for every load in flight (bounded).
    pub fn wait_images(&mut self, timeout: Duration) -> bool {
        let reports = self.images.wait(timeout);
        self.apply_reports(reports)
    }

    pub(super) fn apply_reports(&mut self, reports: Vec<crate::image::Report>) -> bool {
        let any = !reports.is_empty();
        for (view, size) in reports {
            if let Some(e) = self.host.set_intrinsic(view, size) {
                eprintln!("exact: {e}");
            }
        }
        if any {
            self.dirty = true;
            self.clamp_scroll();
            self.queue_collections();
            if let Some(error) = self.refresh_transform_geometry() {
                self.host.log(error);
            }
        }
        any
    }

    pub(super) fn sync_images(&mut self) -> Option<String> {
        let epoch = self.host.kernel().epoch();
        if self.images.order.as_ref().is_none_or(|(e, _)| *e != epoch) {
            let kernel = self.host.kernel();
            let order = self
                .host
                .preorder()
                .into_iter()
                .filter(|id| {
                    kernel
                        .node(*id)
                        .is_some_and(|n| n.node_type == NodeType::Image)
                })
                .collect();
            self.images.order = Some((epoch, order));
        }
        let live = self
            .images
            .order
            .as_ref()
            .map(|(_, o)| o.clone())
            .unwrap_or_default();
        let host = &self.host;
        let boxes: std::collections::HashMap<ViewId, &crate::paint::PaintedBox> =
            self.boxes.iter().rev().map(|b| (b.id, b)).collect();
        let viewport = self.viewport;
        let reports = self
            .images
            .sync_visible(host.kernel(), &live, self.brush.scale, |id| {
                if host.route_visibility(id).0 {
                    return false;
                }
                let Some(b) = boxes.get(&id) else {
                    return true;
                };
                let (mut x, mut y, mut w, mut h) = b.rect;
                // An auto-sized first load has no natural dimensions yet. Permit
                // that point to load; its accepted backing will supply geometry.
                w = w.max(1.);
                h = h.max(1.);
                if let Some((cx, cy, cw, ch)) = b.clip {
                    let right = (x + w).min(cx + cw);
                    let bottom = (y + h).min(cy + ch);
                    x = x.max(cx);
                    y = y.max(cy);
                    w = right - x;
                    h = bottom - y;
                }
                w > 0. && h > 0. && x < viewport.0 && y < viewport.1 && x + w > 0. && y + h > 0.
            });
        let mut error = None;
        for (view, size) in reports {
            error = error.or(self.host.set_intrinsic(view, size));
            self.dirty = true;
        }
        error
    }
}

//! Visibility comes from the native painter's nested scroll/clip geometry.
use super::*;

impl<D: DataSource> Presenter<D> {
    pub(super) fn sync_images(&mut self) -> Option<String> {
        let live = self.host.preorder();
        let host = &self.host;
        let boxes = &self.boxes;
        let viewport = self.viewport;
        let reports = self
            .images
            .sync_visible(host.kernel(), &live, self.brush.scale, |id| {
                if host.route_visibility(id).0 {
                    return false;
                }
                let Some(b) = boxes.iter().find(|b| b.id == id) else {
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

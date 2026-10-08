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
        if any {
            if let Some(e) = self.host.set_intrinsics(reports) {
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
        self.sync_images_moved(&BTreeMap::new())
    }

    /// Which pictures show, when the last paint was moved since (`moved`: by
    /// scroller, how far its rows moved, in points): those rows' boxes are
    /// taken where they are now, inside their scroller's box.
    pub(crate) fn sync_images_moved(
        &mut self,
        moved: &BTreeMap<ViewId, (f32, f32)>,
    ) -> Option<String> {
        let epoch = self.host.kernel().epoch();
        // A tree whose only images are symbols (a list's icons) has nothing
        // that loads by whether it shows: once synced at this epoch, a move
        // or a paint finds the same. Walking them all at every pass and
        // paint was 8% of a fling's thread on a list of icon rows.
        if self.images.settled == Some(epoch) {
            return None;
        }
        if self.images.order.as_ref().is_none_or(|(e, _)| *e != epoch) {
            let kernel = self.host.kernel();
            let order = if kernel.has_type(NodeType::Image) {
                kernel.preorder_where(&self.host.roots(), |t, _| t == NodeType::Image)
            } else {
                Vec::new()
            };
            self.images.order = Some((epoch, order));
        }
        // Both go back below; nothing in between replaces them.
        let (order_epoch, live) = self.images.order.take().unwrap_or_default();
        // The pictures' and the row groups' own boxes (each one's first), not
        // every painted box: found once for this paint's boxes and this
        // order, and only when a box is asked for (a picture's, or a moved
        // group's: a list of icon rows asks for none, at every commit).
        let index = std::cell::OnceCell::new();
        if let Some((serial, epoch, kept)) = self.images.box_index.take() {
            if serial == self.boxes_serial && epoch == order_epoch {
                let _ = index.set(kept);
            }
        }
        let host = &self.host;
        let boxes = |id: &ViewId| {
            let index = index.get_or_init(|| {
                let mut index = std::collections::HashMap::new();
                let mut wanted: std::collections::HashSet<ViewId> = live.iter().copied().collect();
                wanted.extend(self.brush.row_groups().iter().map(|(g, _, _)| *g));
                for (i, b) in self.boxes.iter().enumerate() {
                    if wanted.contains(&b.id) {
                        index.entry(b.id).or_insert(i);
                    }
                }
                index
            });
            index.get(id).map(|&i| (i, &self.boxes[i]))
        };
        type Shift = (usize, usize, (f32, f32), Option<crate::paint::Rect4>);
        let shifts: Vec<Shift> = self
            .brush
            .row_groups()
            .iter()
            .filter_map(|(g, a, b)| {
                let d = moved.get(g)?;
                Some((*a, *b, *d, boxes(g).map(|(_, s)| s.rect)))
            })
            .collect();
        let viewport = self.viewport;
        let reports = self
            .images
            .sync_visible(host.kernel(), &live, self.brush.scale, |id| {
                if host.route_visibility(id).0 {
                    return false;
                }
                let Some((i, b)) = boxes(&id) else {
                    return true;
                };
                let (mut x, mut y, mut w, mut h) = b.rect;
                let mut clip = b.clip;
                if let Some((_, _, d, port)) =
                    shifts.iter().find(|(a, e, _, _)| (*a..*e).contains(&i))
                {
                    x += d.0;
                    y += d.1;
                    clip = *port;
                }
                // An auto-sized first load has no natural dimensions yet. Permit
                // that point to load; its accepted backing will supply geometry.
                w = w.max(1.);
                h = h.max(1.);
                if let Some((cx, cy, cw, ch)) = clip {
                    let right = (x + w).min(cx + cw);
                    let bottom = (y + h).min(cy + ch);
                    x = x.max(cx);
                    y = y.max(cy);
                    w = right - x;
                    h = bottom - y;
                }
                w > 0. && h > 0. && x < viewport.0 && y < viewport.1 && x + w > 0. && y + h > 0.
            });
        self.images.box_index = index
            .into_inner()
            .map(|index| (self.boxes_serial, order_epoch, index));
        self.images.order = Some((order_epoch, live));
        self.dirty |= !reports.is_empty();
        self.images.settled = self.images.settled_at(epoch, reports.is_empty());
        if reports.is_empty() {
            return None;
        }
        self.host.set_intrinsics(reports)
    }
}

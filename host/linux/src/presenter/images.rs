//! Visibility comes from the native painter's nested scroll/clip geometry.
use super::*;

/// `EXACT_PICTURES_AT_MOUNT=1`: every mounted picture is asked for, in view
/// or not. For a host whose rows a reader moves before this presenter hears
/// of the scroll (the Android canvas host: its window leads the travel, and
/// the reader draws a mounted row where the scroll has brought it): there a
/// row is on screen before its box is in this viewport, and a picture asked
/// for only then came frames late (heavy at 24,000 dp/s: 39% of the view
/// was pictures not yet there, against 28% once they are asked at mount;
/// most were held decoded and only waiting to be asked for).
static AT_MOUNT: std::sync::LazyLock<bool> =
    std::sync::LazyLock::new(|| std::env::var("EXACT_PICTURES_AT_MOUNT").is_ok_and(|v| v != "0"));

impl<D: DataSource> Presenter<D> {
    /// Restore the ordinary image policy after the painted carrier retires boot.
    #[cfg(target_os = "macos")]
    pub fn finish_boot_images(&mut self) {
        self.images.finish_boot();
    }

    /// A landed picture needs the next display tick, without waiting for a
    /// window-system redraw. Painting consumes this demand.
    pub fn pictures_need_paint(&self) -> bool {
        self.image_paint_pending
    }

    /// Loads that arrived since the last call: their sizes reach the kernel.
    /// Whether anything changed.
    pub fn poll_images(&mut self) -> bool {
        self.images.paint_with(self.brush.text.clone());
        let reports = self.images.poll();
        // A picture's time to ask again for a failed download has come: the
        // next paint's sync asks (`Images::sync_visible`).
        if self.images.retry_due_in().is_some_and(|d| d.is_zero()) {
            self.dirty = true;
        }
        self.apply_reports(reports)
    }

    /// A source's first picture sends its `load`, and a failure for good its
    /// `error` with the reason, to an image that hears it
    /// (LLP 1011 §4) and is still that node showing that source, assigned
    /// that time, as it is sent: since it landed, input, a timer or an
    /// earlier load's action may have changed any of them. It is sent with
    /// nothing run before its handler (`Host::dispatch_now`), so the handler
    /// sees what was checked; what the clock owes runs after, at the loop's
    /// next advance. A source changed and changed back between two of the
    /// presenter's image syncs (commits of one advance) was never shown
    /// changed, and counts as unchanged. Loads are taken one at a time, so a
    /// source an earlier load's action changes drops the rest's
    /// (`Images::sync_shown`), and at most as many as were waiting: a handler
    /// that keeps changing sources can't hold the loop. Whether any was sent.
    pub(crate) fn deliver_image_loads(&mut self) -> bool {
        let mut loads = false;
        for _ in 0..self.images.load_events.len() {
            if self.images.load_events.is_empty() {
                break;
            }
            let event = self.images.load_events.remove(0);
            let same = self.host.kernel().node(event.id).is_some_and(|n| {
                n.node_type == NodeType::Image
                    && (u64::from(n.key.index) | (u64::from(n.key.generation) << 32)) == event.key
                    && n.props.str(PropId::ImageSource) == Some(event.source.as_str())
            }) && self.images.assignment(event.id) == Some(event.assignment);
            // Its `load`, or its `error` with the reason (as Apple sends it).
            let (kind, sent) = match event.error {
                None => (EventKind::Load, Event::Load),
                Some(message) => (EventKind::Error, Event::Media(EventKind::Error, message)),
            };
            if !same || !(self.host.runner().handlers_of(event.id)).contains(&kind) {
                continue;
            }
            let sent = self.host.dispatch_now(event.id, sent);
            let after = self.after_commit();
            if let Some(error) = sent.or(after) {
                self.host.log(error);
            }
            loads = true;
        }
        loads
    }

    /// Milliseconds until the pictures owe a turn: at once while a `load`
    /// waits to be sent (a picture from the cache lands in a paint's sync,
    /// with no decode to wake the loop), else when a picture on screen asks
    /// again for a failed download.
    pub(crate) fn image_turn_due_in_ms(&self) -> Option<f64> {
        if !self.images.load_events.is_empty() {
            return Some(0.0);
        }
        self.images.retry_due_in().map(|d| d.as_secs_f64() * 1000.0)
    }

    /// Wait for every load in flight (bounded).
    pub fn wait_images(&mut self, timeout: Duration) -> bool {
        self.images.paint_with(self.brush.text.clone());
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
            self.image_paint_pending = true;
            self.clamp_scroll();
            self.queue_collections();
            if let Some(error) = self.refresh_transform_geometry() {
                self.host.log(error);
            }
        }
        // After the sizes.
        let loads = self.deliver_image_loads();
        any || loads
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
        self.images.screen = (viewport.0 * self.brush.scale, viewport.1 * self.brush.scale);
        self.images.paint_with(self.brush.text.clone());
        // Where its painted box is in the viewport: None for a picture with no
        // painted box (not painted yet, or under `display: none`).
        let boot_visible_only = self.images.boot_visible_only;
        let placed = |id: ViewId| -> Option<bool> {
            if host.route_visibility(id).0 {
                return Some(false);
            }
            let Some((i, b)) = boxes(&id) else {
                return boot_visible_only.then(|| {
                    host.kernel().node(id).is_some_and(|n| {
                        let f = n.frame;
                        f.x < viewport.0
                            && f.y < viewport.1
                            && f.x + f.width.max(1.) > 0.
                            && f.y + f.height.max(1.) > 0.
                    })
                });
            };
            let (mut x, mut y, mut w, mut h) = b.rect;
            let mut clip = b.clip;
            if let Some((_, _, d, port)) = shifts.iter().find(|(a, e, _, _)| (*a..*e).contains(&i))
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
            Some(w > 0. && h > 0. && x < viewport.0 && y < viewport.1 && x + w > 0. && y + h > 0.)
        };
        // In the viewport, its box painted there: what a retry and a download's urgency go by.
        let on_screen = |id: ViewId| placed(id) == Some(true);
        // Asked for: in the viewport or with no box yet (a first load), or
        // mounted where pictures are asked at mount.
        let visible = |id: ViewId| {
            if *AT_MOUNT {
                return !host.route_visibility(id).0;
            }
            placed(id).unwrap_or(true)
        };
        let reports =
            (self.images).sync_shown(host.kernel(), &live, self.brush.scale, &visible, &on_screen);
        self.images.box_index = index
            .into_inner()
            .map(|index| (self.boxes_serial, order_epoch, index));
        self.images.order = Some((order_epoch, live));
        let mut reports = reports;
        reports.extend(self.pictures_out());
        self.dirty |= !reports.is_empty();
        self.image_paint_pending |= !reports.is_empty();
        self.images.settled = self.images.settled_at(epoch, reports.is_empty());
        if reports.is_empty() {
            return None;
        }
        self.host.set_intrinsics(reports)
    }
}

#[cfg(test)]
#[path = "images_tests.rs"]
mod tests;

//! The agent's `reveal` (ledger F7, shop F11): before a tap or a type, a node
//! whose middle is out of view is scrolled to the middle of its nearest
//! scroll containers, then of the page — the web's `scrollIntoView` (block
//! centre), as the web host's `navigation.js` asks of the browser. Offsets
//! move as a wheel's do (`wheel_at`), clamped to each container's travel.
use super::*;

impl<D: DataSource> Presenter<D> {
    /// `{"revealed":id,"scrolled":…}`, with where the middle moved from and to.
    pub fn reveal(&mut self, id: ViewId) -> Result<String, String> {
        let middle = |b: &PaintedBox| (b.rect.0 + b.rect.2 / 2.0, b.rect.1 + b.rect.3 / 2.0);
        let inside =
            |(x, y): (f32, f32), r: Rect4| x >= r.0 && y >= r.1 && x < r.0 + r.2 && y < r.1 + r.3;
        let b = self
            .box_of(id)
            .ok_or_else(|| format!("no view {id} on screen"))?;
        let from = middle(&b);
        let (vw, vh) = self.display.viewport().unwrap_or(self.viewport);
        if inside(from, (0.0, 0.0, vw, vh)) && b.clip.is_none_or(|c| inside(from, c)) {
            return Ok(format!("{{\"revealed\":{id},\"scrolled\":false}}"));
        }
        // The scroll containers above it, innermost first, with their axes and travel.
        let mut scrollers = Vec::new();
        let collection_limits = self.collection_scroll_limits();
        let kernel = self.host.kernel();
        let mut at = self.display.parent(kernel, id);
        while let Some(a) = at {
            let Some(node) = kernel.node(a) else { break };
            let bounds = self.display.bounds(kernel, a).unwrap_or_else(|| {
                self.brush.scroll_bounds(
                    kernel,
                    self.host.content_region(),
                    &node,
                    collection_limits.get(&a).copied(),
                )
            });
            // `auto` scrolls as `scroll` does (a wheel moves it: `wheel_at`).
            let scrolls = |o| matches!(o, Overflow::Scroll | Overflow::Auto);
            let (ox, oy) = bounds.axes;
            if scrolls(ox) || scrolls(oy) {
                scrollers.push((a, scrolls(ox), scrolls(oy), bounds.max));
            }
            at = self.display.parent(kernel, a);
        }
        for (a, sx, sy, max) in scrollers {
            let (Some(target), Some(scroller)) = (self.box_of(id), self.box_of(a)) else {
                continue;
            };
            if inside(middle(&target), scroller.rect) {
                continue;
            }
            let off = self.scroll.get(&a).copied().unwrap_or((0.0, 0.0));
            let (dx, dy) = delta(target.rect, scroller.rect);
            let next = (
                if sx {
                    (off.0 + dx).clamp(0.0, max.0)
                } else {
                    off.0
                },
                if sy {
                    (off.1 + dy).clamp(0.0, max.1)
                } else {
                    off.1
                },
            );
            if next != off {
                self.scroll.insert(a, next);
                self.dirty = true;
                self.collection_scrolled(a);
            }
        }
        let target = self
            .box_of(id)
            .ok_or_else(|| format!("no view {id} on screen"))?;
        if !inside(middle(&target), (0.0, 0.0, vw, vh)) {
            let doc = self.document();
            let max = ((doc.0 - vw).max(0.0), (doc.1 - vh).max(0.0));
            let (dx, dy) = delta(target.rect, (0.0, 0.0, vw, vh));
            let next = (
                (self.page.0 + dx).clamp(0.0, max.0),
                (self.page.1 + dy).clamp(0.0, max.1),
            );
            if next != self.page {
                self.page = next;
                self.dirty = true;
            }
        }
        if let Some(error) = self.refresh_transform_geometry() {
            self.host.log(error);
        }
        let to = self.box_of(id).map_or(from, |b| middle(&b));
        Ok(format!(
            "{{\"revealed\":{id},\"scrolled\":{},\"from\":[{},{}],\"to\":[{},{}]}}",
            to != from,
            num(r2(from.0)),
            num(r2(from.1)),
            num(r2(to.0)),
            num(r2(to.1))
        ))
    }
}

/// How far a port's offset moves to show `target`'s middle, as the web's
/// `scrollIntoView({block: "center", inline: "nearest"})`: down, to its
/// middle; across, only as far as its box takes. Zero on an axis whose
/// middle is in the port already.
fn delta(target: Rect4, port: Rect4) -> (f32, f32) {
    let (mx, my) = (target.0 + target.2 / 2.0, target.1 + target.3 / 2.0);
    let dx = if mx < port.0 {
        target.0 - port.0
    } else if mx >= port.0 + port.2 {
        target.0 + target.2 - (port.0 + port.2)
    } else {
        0.0
    };
    let dy = if my < port.1 || my >= port.1 + port.3 {
        my - (port.1 + port.3 / 2.0)
    } else {
        0.0
    };
    (dx, dy)
}

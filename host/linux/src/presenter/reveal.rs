//! The agent's `reveal` (ledger F7, shop F11): before a tap or a type, a node
//! whose middle is out of view is scrolled to the middle of its nearest
//! scroll containers, then of the page — the web's `scrollIntoView` (block
//! centre), as the web host's `navigation.js` asks of the browser. Offsets
//! move as a wheel's do (`wheel_at`), clamped to each container's travel.
//! An app's `scrollIntoView("element-id", block=, inline=)` (minesweeper F3)
//! walks the same containers, aligning by CSSOM View's rules.
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
        self.scroll_into(id, delta);
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

    /// An app's `scrollIntoView("element-id", block=, inline=)`: the node
    /// with that HTML `id`, aligned in each scroll container above it, then
    /// the page. `behavior="smooth"` lands at once here (no animation).
    pub(super) fn scroll_element_into_view(&mut self, args: &[exact_plan::Value]) {
        let text = |i: usize| args.get(i).and_then(exact_plan::Value::as_str);
        let name = text(0).unwrap_or_default();
        let kernel = self.host.kernel();
        let Some(id) = kernel
            .find_by_id(name)
            .first()
            .and_then(|key| kernel.node_by_key(*key))
            .map(|n| n.id)
        else {
            self.host.log(format!(
                "scrollIntoView \"{name}\" refused: no live node with that id"
            ));
            return;
        };
        let (block, inline) = (text(1).unwrap_or("start"), text(2).unwrap_or("nearest"));
        self.scroll_into(id, |t, p| {
            (
                aligned(inline, t.0, t.0 + t.2, p.0, p.2) - p.0,
                aligned(block, t.1, t.1 + t.3, p.1, p.3) - p.1,
            )
        });
    }

    /// An app's `scrollBy("element-id", x, y)`: that scroll container moves
    /// by the pixels given, clamped to its travel, as the web's
    /// `Element.scrollBy`; an element that does not scroll does not move.
    pub(super) fn scroll_element_by(&mut self, args: &[exact_plan::Value]) {
        let name = args
            .first()
            .and_then(exact_plan::Value::as_str)
            .unwrap_or_default();
        let by = |i: usize| {
            args.get(i)
                .and_then(exact_plan::Value::as_number)
                .unwrap_or(0.0) as f32
        };
        let collection_limits = self.collection_scroll_limits();
        let kernel = self.host.kernel();
        let Some(node) = kernel
            .find_by_id(name)
            .first()
            .and_then(|key| kernel.node_by_key(*key))
        else {
            self.host.log(format!(
                "scrollBy \"{name}\" refused: no live node with that id"
            ));
            return;
        };
        let id = node.id;
        let bounds = self.display.bounds(kernel, id).unwrap_or_else(|| {
            self.brush.scroll_bounds(
                kernel,
                self.host.content_region(),
                &node,
                collection_limits.get(&id).copied(),
            )
        });
        let scrolls = |o| matches!(o, Overflow::Scroll | Overflow::Auto);
        let (ox, oy) = bounds.axes;
        let off = self.scroll.get(&id).copied().unwrap_or((0.0, 0.0));
        let next = (
            if scrolls(ox) {
                (off.0 + by(1)).clamp(0.0, bounds.max.0)
            } else {
                off.0
            },
            if scrolls(oy) {
                (off.1 + by(2)).clamp(0.0, bounds.max.1)
            } else {
                off.1
            },
        );
        if next != off {
            self.scroll.insert(id, next);
            self.dirty = true;
            self.collection_scrolled(id);
        }
    }

    /// Each scroll container above `id`, innermost first, then the page,
    /// moves by what `by(target, port)` asks (viewport coordinates), clamped
    /// to its travel.
    fn scroll_into(&mut self, id: ViewId, by: impl Fn(Rect4, Rect4) -> (f32, f32)) {
        let (vw, vh) = self.display.viewport().unwrap_or(self.viewport);
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
            // `auto` scrolls as `scroll` does (a `scroll`'s default, `wheel_at`).
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
            let off = self.scroll.get(&a).copied().unwrap_or((0.0, 0.0));
            let (dx, dy) = by(target.rect, scroller.rect);
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
        if let Some(target) = self.box_of(id) {
            let doc = self.document();
            let max = ((doc.0 - vw).max(0.0), (doc.1 - vh).max(0.0));
            let (dx, dy) = by(target.rect, (0.0, 0.0, vw, vh));
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

/// Where a port starting at `p0`, `size` long, goes on one axis to align the
/// target `t0..t1` by CSS's `start`, `center`, `end` or `nearest` (CSSOM
/// View, "scroll an element into view"), before clamping.
fn aligned(align: &str, t0: f32, t1: f32, p0: f32, size: f32) -> f32 {
    let (p1, length) = (p0 + size, t1 - t0);
    match align {
        "start" => t0,
        "end" => t1 - size,
        "center" => (t0 + t1) / 2.0 - size / 2.0,
        // Nothing when it shows whole or covers the port; else the edge it
        // is past, or the far one when it is larger than the port.
        _ if (t0 >= p0 && t1 <= p1) || (t0 < p0 && t1 > p1) => p0,
        _ if (t0 < p0 && length <= size) || (t1 > p1 && length > size) => t0,
        _ => t1 - size,
    }
}

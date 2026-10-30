//! The agent's `reveal` (ledger F7, shop F11): before a tap or a type, a node
//! whose middle is out of view is scrolled to the middle of its nearest
//! scroll containers, then of the page — the web's `scrollIntoView` (block
//! centre), as the web host's `navigation.js` asks of the browser. Offsets
//! move as a wheel's do (`wheel_at`), clamped to each container's travel.
//! An app's `scrollIntoView("element-id", block=, inline=)` (minesweeper F3)
//! walks the same containers, aligning by CSSOM View's rules.
use super::*;

pub(super) enum Cause {
    InsetAdjustment,
    FieldNavigation,
    /// A remote's arrow moved the focus (LLP 1115.001.003 D4).
    FocusMove,
}

impl<D: DataSource> Presenter<D> {
    /// `{"revealed":id,"scrolled":…}`, with where the middle moved from and to.
    pub fn reveal(&mut self, id: ViewId) -> Result<String, String> {
        // The page under an open box is not scrolled: a tap there dismisses
        // a light-dismiss box where it lands, and is refused under any other.
        if !self.alert_allows(id) {
            return if self.alert_light_dismiss() {
                Ok(format!("{{\"revealed\":{id},\"scrolled\":false}}"))
            } else {
                Err(format!("view {id} is inert under the open surface"))
            };
        }
        // A row of the top layer's open surface is no box of the page's:
        // it is in view in its window (LLP 1115.001 D10). Nor is what a
        // shell's bar stands for (D11): it is in view in the bar.
        if self.surface_row(id) || self.bar_node(id) {
            return Ok(format!("{{\"revealed\":{id},\"scrolled\":false}}"));
        }
        let middle = |b: &PaintedBox| (b.rect.0 + b.rect.2 / 2.0, b.rect.1 + b.rect.3 / 2.0);
        let inside =
            |(x, y): (f32, f32), r: Rect4| x >= r.0 && y >= r.1 && x < r.0 + r.2 && y < r.1 + r.3;
        let b = self
            .box_of(id)
            .ok_or_else(|| format!("no view {id} on screen"))?;
        let from = middle(&b);
        let (vw, vh) = self.display.viewport().unwrap_or(self.viewport);
        // (Cleared of a sticky box: scrolled, whatever an attached display's boxes say before its next frame.)
        let cleared = if inside(from, (0.0, 0.0, vw, vh)) && b.clip.is_none_or(|c| inside(from, c))
        {
            // In view, but perhaps under a sticky box of its own scroller: scrolled clear of it, as a person would.
            if !self.clear_of_sticky(id, from) {
                return Ok(format!("{{\"revealed\":{id},\"scrolled\":false}}"));
            }
            true
        } else {
            self.scroll_into(id, false, true, delta);
            let at = self.box_of(id).map_or(from, |b| middle(&b));
            self.clear_of_sticky(id, at)
        };
        let to = self.box_of(id).map_or(from, |b| middle(&b));
        Ok(format!(
            "{{\"revealed\":{id},\"scrolled\":{},\"from\":[{},{}],\"to\":[{},{}]}}",
            to != from || cleared,
            num(r2(from.0)),
            num(r2(from.1)),
            num(r2(to.0)),
            num(r2(to.1))
        ))
    }

    /// Scroll `id` by `dy` points, clamped to its travel; whether it moved (the Canvas host's bench driver's, and a
    /// tap's clear of a sticky box).
    pub(crate) fn scroll_by(&mut self, id: ViewId, dy: f32) -> bool {
        self.restore_scroll_anchors();
        let kernel = self.host.kernel();
        let Some(node) = kernel.node(id) else {
            return false;
        };
        let bounds = self.display.bounds(kernel, id).unwrap_or_else(|| {
            let limits = self.collection_scroll_limits();
            self.brush.scroll_bounds(
                kernel,
                self.host.content_region(),
                &node,
                limits.get(&id).copied(),
            )
        });
        let off = self.scroll.get(&id).copied().unwrap_or((0.0, 0.0));
        let ny = (off.1 + dy).clamp(0.0, bounds.max.1.max(0.0));
        let moved = self.anchor_input(id, off, dy, (off.0, ny));
        if ny == off.1 && moved.is_none_or(|dy| dy == 0.) {
            return false;
        }
        self.scroll.insert(id, (off.0, ny));
        self.scroll_end_input(id, off, Completion::Instant);
        self.dirty = true;
        self.collection_scroll_input(id, off);
        if let Some(error) = self.refresh_transform_geometry() {
            self.host.log(error);
        }
        true
    }

    /// A target whose middle (`at`) a sticky box of its own scroller covers, as the tap's own hit test finds it (the
    /// design tool's edit dock; theme2's 14:49 ruling): that scroller (or the page) scrolled so the middle lands in the
    /// band of its port the box leaves. Never a box that holds the target, nor one of another scroller or the top
    /// layer, which no scroll takes away: the tap still refuses there. Whether it scrolled.
    fn clear_of_sticky(&mut self, id: ViewId, at: (f32, f32)) -> bool {
        // (An open surface of the top layer, a popover or an alert, is over the page: no scroll clears it.)
        if self.surface_open() || self.alert_inert() {
            return false;
        }
        let Some((cover, scroller)) = self.sticky_at(id, at) else {
            return false;
        };
        let (_, vh) = self.display.viewport().unwrap_or(self.viewport);
        // The port, what of it the window shows (the page's own, for the root).
        let is_root = self
            .host
            .kernel()
            .node(scroller)
            .is_some_and(|n| n.parent.is_none());
        let port = if is_root { None } else { self.box_of(scroller) }.map_or((0.0, vh), |b| {
            (b.rect.1.max(0.0), (b.rect.1 + b.rect.3).min(vh))
        });
        let (mut top, mut bottom) = port;
        let mid = (top + bottom) / 2.0;
        // Less each cover: the band above one whose middle is in the port's lower half, below one in its upper; its
        // middle hit-tested again, a second box there (a header across from a dock) narrows it too.
        let mut next = Some(cover);
        for _ in 0..4 {
            let Some(c) = next.and_then(|c| self.box_of(c)) else {
                break;
            };
            if c.rect.1 + c.rect.3 / 2.0 > mid {
                bottom = bottom.min(c.rect.1);
            } else {
                top = top.max(c.rect.1 + c.rect.3);
            }
            if bottom <= top {
                return false;
            }
            next = self
                .sticky_at(id, (at.0, (top + bottom) / 2.0))
                .filter(|&(_, s)| s == scroller)
                .map(|(c, _)| c);
        }
        let dy = at.1 - (top + bottom) / 2.0;
        // (A root that scrolls itself moves by its own offset; one that doesn't, by the page's.)
        self.scroll_by(scroller, dy) || (is_root && self.scroll_page_by(dy))
    }

    /// The page scrolled by `dy` points, within its travel, as `scroll_into`'s page step: whether it moved.
    fn scroll_page_by(&mut self, dy: f32) -> bool {
        let (vw, vh) = self.display.viewport().unwrap_or(self.viewport);
        let doc = self.document();
        let max = ((doc.0 - vw).max(0.0), (doc.1 - vh).max(0.0));
        let next = (self.page.0, (self.page.1 + dy).clamp(0.0, max.1));
        let moved = self.anchor_input(PAGE, self.page, dy, next);
        if next == self.page && moved.is_none_or(|dy| dy == 0.) {
            return false;
        }
        self.page = next;
        self.dirty = true;
        if let Some(error) = self.refresh_transform_geometry() {
            self.host.log(error);
        }
        true
    }

    /// The sticky box a hit at `at` lands in, and its scroller, as the kernel's sticky constraint names it, holding
    /// `id`; none for a hit on `id` itself, a box holding it, or one of the top layer (whose sticky is none).
    fn sticky_at(&mut self, id: ViewId, at: (f32, f32)) -> Option<(ViewId, ViewId)> {
        let hit = self.hit(at.0, at.1)?;
        if hit == id || self.is_inside(hit, id) {
            return None;
        }
        let kernel = self.host.kernel();
        let mut cover = Some(hit);
        let constraint = loop {
            let node = kernel.node(cover?)?;
            if node.layout_position() == exact_kernel::PositionType::Sticky {
                break kernel.sticky_constraint(node.key)?;
            }
            cover = node.parent;
        };
        let cover = cover?;
        (!self.is_inside(id, cover) && self.is_inside(id, constraint.scroller))
            .then_some((cover, constraint.scroller))
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
        self.scroll_into(id, true, true, |t, p| {
            (
                aligned(inline, t.0, t.0 + t.2, p.0, p.2) - p.0,
                aligned(block, t.1, t.1 + t.3, p.1, p.3) - p.1,
            )
        });
    }

    /// The runner's `scrollToFragment(visit, fragment)`, once a route's
    /// content has arrived (LLP 1153 D5): the node with that HTML `id` in the
    /// visit's own route (a route beneath may hold the same id) at the start
    /// of each scroll container above it, as `scrollIntoView` with
    /// `block="start"`; `top`, or no such node, the route's scrollers at
    /// their tops. A wheel turned since the router asked for it cancels it.
    pub(super) fn scroll_to_fragment(&mut self, args: &[exact_plan::Value]) {
        let visit = args.first().and_then(exact_plan::Value::as_number);
        let fragment = args
            .get(1)
            .and_then(exact_plan::Value::as_str)
            .unwrap_or_default();
        if self
            .fragment_armed
            .take()
            .is_some_and(|(_, wheels)| wheels != self.wheels)
        {
            self.host.log(format!(
                "fragment #{fragment}: the person scrolled first; the route stays where it is"
            ));
            return;
        }
        let key = visit.map(|v| format!("{v}")).unwrap_or_default();
        let kernel = self.host.kernel();
        let Some(route) = kernel
            .preorder_where(&self.host.roots(), |_, props| {
                props.str(PropId::NavigationKey) == Some(key.as_str())
                    && props.str(PropId::NavigationBack).is_none()
            })
            .first()
            .copied()
        else {
            self.host.log(format!(
                "fragment #{fragment}: no route of visit {key} is on the screen"
            ));
            return;
        };
        let id = exact_route::fragment_id(fragment);
        let target = [Some(fragment), id.as_deref()]
            .into_iter()
            .flatten()
            .filter(|name| !name.is_empty())
            .find_map(|name| {
                (kernel.find_by_id(name).into_iter())
                    .filter_map(|key| kernel.node_by_key(key).map(|n| n.id))
                    .find(|n| self.is_inside(*n, route))
            });
        if let Some(target) = target {
            self.scroll_into(target, true, true, |t, p| {
                (
                    aligned("nearest", t.0, t.0 + t.2, p.0, p.2) - p.0,
                    aligned("start", t.1, t.1 + t.3, p.1, p.3) - p.1,
                )
            });
            return;
        }
        if !fragment.is_empty() && !id.is_some_and(|id| id.eq_ignore_ascii_case("top")) {
            self.host.log(format!("fragment #{fragment}: no element in the route of visit {key} has that id; the route goes to its top"));
        }
        let scrolled: Vec<_> = (self.scroll.iter())
            .filter(|(id, off)| {
                **off != (0.0, 0.0) && (**id == route || self.is_inside(**id, route))
            })
            .map(|(id, off)| (*id, *off))
            .collect();
        for (id, off) in scrolled {
            self.scroll.insert(id, (0.0, 0.0));
            self.scroll_end_input(id, off, Completion::Instant);
            self.collection_scroll_input(id, off);
            self.dirty = true;
        }
    }

    /// An app's `scrollBy("element-id", x, y)`: that scroll container moves
    /// by the pixels given, clamped to its travel, as the web's
    /// `Element.scrollBy`; an element that does not scroll does not move.
    pub(super) fn scroll_element_by(&mut self, args: &[exact_plan::Value]) {
        self.restore_scroll_anchors();
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
        let moved = self.anchor_input(id, off, if scrolls(oy) { by(2) } else { 0. }, next);
        if next != off || moved.is_some_and(|dy| dy != 0.) {
            self.scroll.insert(id, next);
            self.scroll_end_input(id, off, Completion::Instant);
            self.dirty = true;
            self.collection_scroll_input(id, off);
        }
    }

    /// Move each scroll container above `id`, then the page, only as far as
    /// it must to show the field: `scrollIntoView` with `nearest` on both axes.
    pub(super) fn keep_in_view(&mut self, id: ViewId, cause: Cause) {
        self.scroll_into(
            id,
            false,
            matches!(cause, Cause::FieldNavigation | Cause::FocusMove),
            |t, p| {
                (
                    aligned("nearest", t.0, t.0 + t.2, p.0, p.2) - p.0,
                    aligned("nearest", t.1, t.1 + t.3, p.1, p.3) - p.1,
                )
            },
        );
    }

    /// Each scroll container above `id`, innermost first, then the page,
    /// moves by what `by(target, port)` asks (viewport coordinates), clamped
    /// to its travel.
    fn scroll_into(
        &mut self,
        id: ViewId,
        spacing: bool,
        completion: bool,
        by: impl Fn(Rect4, Rect4) -> (f32, f32),
    ) {
        self.restore_scroll_anchors();
        let (vw, vh) = self.display.viewport().unwrap_or(self.viewport);
        // The scroll containers above it, innermost first, with their axes and travel.
        let mut scrollers = Vec::new();
        let collection_limits = self.collection_scroll_limits();
        let kernel = self.host.kernel();
        let margin = kernel
            .node(id)
            .map(|n| {
                let r = &n.style.rare;
                [
                    r.scroll_margin_top,
                    r.scroll_margin_right,
                    r.scroll_margin_bottom,
                    r.scroll_margin_left,
                ]
                .map(|d| d.at_basis(kernel.arena().env(), 0.0))
            })
            .unwrap_or([0.0; 4]);
        let mut at = self.scroll_parent(id);
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
                let r = &node.style.rare;
                let padding = [
                    r.scroll_padding_top,
                    r.scroll_padding_right,
                    r.scroll_padding_bottom,
                    r.scroll_padding_left,
                ];
                scrollers.push((
                    a,
                    scrolls(ox),
                    scrolls(oy),
                    bounds.max,
                    padding,
                    node.style.border_widths(),
                ));
            }
            if self.alert_root() == Some(a) {
                break;
            }
            at = self.scroll_parent(a);
        }
        for (a, sx, sy, max, padding, borders) in scrollers {
            let (Some(target), Some(scroller)) = (self.box_of(id), self.box_of(a)) else {
                continue;
            };
            let port = inset(self.brush.split_port(a, scroller.rect), borders);
            let basis = [port.3, port.2, port.3, port.2];
            let padding = std::array::from_fn(|i| {
                padding[i]
                    .at_basis(self.host.kernel().arena().env(), basis[i])
                    .max(0.0)
            });
            let off = self.scroll.get(&a).copied().unwrap_or((0.0, 0.0));
            let (target, port) = if spacing {
                (inset(target.rect, margin.map(|n| -n)), inset(port, padding))
            } else {
                (target.rect, port)
            };
            let (dx, dy) = by(target, port);
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
            let moved = self.anchor_input(a, off, if sy { dy } else { 0. }, next);
            if next != off || moved.is_some_and(|dy| dy != 0.) {
                self.scroll.insert(a, next);
                if completion {
                    self.scroll_end_input(a, off, Completion::Instant);
                }
                self.dirty = true;
                self.collection_scroll_input(a, off);
            }
        }
        if self.alert_inert() {
            return;
        }
        if let Some(target) = self.box_of(id) {
            let doc = self.document();
            let max = ((doc.0 - vw).max(0.0), (doc.1 - vh).max(0.0));
            let target = if spacing {
                inset(target.rect, margin.map(|n| -n))
            } else {
                target.rect
            };
            let (dx, dy) = by(target, (0.0, 0.0, vw, vh));
            let next = (
                (self.page.0 + dx).clamp(0.0, max.0),
                (self.page.1 + dy).clamp(0.0, max.1),
            );
            let moved = self.anchor_input(PAGE, self.page, dy, next);
            if next != self.page || moved.is_some_and(|dy| dy != 0.) {
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

fn inset(rect: Rect4, [top, right, bottom, left]: [f32; 4]) -> Rect4 {
    (
        rect.0 + left,
        rect.1 + top,
        (rect.2 - left - right).max(0.0),
        (rect.3 - top - bottom).max(0.0),
    )
}

#[cfg(test)]
#[path = "fragment_tests.rs"]
mod fragment_tests;

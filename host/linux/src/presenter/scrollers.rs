//! Scrolling by a shell that owns the gesture (LLP 1115.001 D7.1 to D7.3):
//! Android's reader takes a finger's drag and its fling with the platform's
//! own classes, moves the scroller's drawing itself, and tells the presenter
//! after. The presenter never recognizes a scroll. What crosses is this:
//!
//! - **Which scrollers are under a point** ([`Presenter::scrollers_at`]):
//!   the scroll containers from the node hit there outward, then the page,
//!   each with the axes it takes part on, how far it scrolls, where it is,
//!   and the box it shows in; and which axes the presenter's own gestures
//!   there claim (a swipe row sideways, a reorder grip up and down, a `pan`
//!   where its `touch-action` does not give the axis to scrolling). A drag
//!   along a claimed axis stays the presenter's, by the rules it has.
//! - **The reader moved one** ([`Presenter::reader_scrolled`]): by a
//!   distance, clamped here. The reader is the authority while its gesture
//!   or fling runs.
//! - **What the reader is told back** ([`Presenter::reader_scrollers`]):
//!   where each scroller it was told of is now and how far it scrolls, with
//!   a revision that moves on whenever the offset is not where the reader's
//!   own moves left it: the app wrote `scrollTop`, revealed a node, a
//!   reorder dragged the list along. The reader ends its fling at a new
//!   revision and takes the offset told; a move it sent before it knew is
//!   dropped here by its old revision. A list's own correction of its
//!   offset (its rows turned out taller than estimated) is not such a move:
//!   the content stays where it is, and the reader's fling goes on.
//!
//! A scroll that wins a touch cancels that touch here
//! ([`Presenter::pointer_cancel`]): no press fires for it.
use super::contact::Candidate;
use super::*;
use std::collections::BTreeSet;

#[cfg(test)]
mod hidden_tests;

/// The page itself, scrolled where the document is larger than the viewport
/// and no scroller holds it: no node has this id.
pub const PAGE: ViewId = 0;

/// One scroller as a shell is told it, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scroller {
    /// The scroll container's node, or [`PAGE`].
    pub id: ViewId,
    /// Whether it scrolls sideways, and up and down: the axis overflows and
    /// has somewhere to go.
    pub axes: (bool, bool),
    /// The farthest offset on each axis.
    pub max: (f32, f32),
    /// Where it is.
    pub offset: (f32, f32),
    /// The box it shows in, in the viewport: x, y, width, height.
    pub port: [f32; 4],
    /// Whether the painter keeps its rows apart, so a shell that replays
    /// them can move them without a paint.
    pub node: bool,
}

/// What is under a point, for a shell deciding what a drag there is.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Under {
    /// The scrollers there, innermost first. Each one's `axes` are those it
    /// takes part on for a drag from this point: an `overscroll-behavior`
    /// other than `auto` ends the chain on its axis.
    pub chain: Vec<Scroller>,
    /// The axes (sideways; up and down) a gesture of the presenter's own
    /// there would take.
    pub claims: (bool, bool),
    /// The text field the shell hosts that a touch there is for, when the
    /// node hit is one (LLP 1115.001 D8): the touch is then that field's,
    /// and the presenter hears of the focus it takes, not of the touch.
    pub field: Option<ViewId>,
}

/// What a reader that scrolls by itself has been told: per scroller, the
/// revision it knows and where its own moves left the offset.
#[derive(Default)]
pub(super) struct Reader {
    tracked: BTreeMap<ViewId, (u32, (f32, f32))>,
    /// The last statement [`Presenter::reader_scrollers`] returned.
    last: Vec<i32>,
}

impl Reader {
    /// A scroller corrected its own offset from `was` to `now`: its content
    /// stays where it shows, so the reader's moves still stand.
    /// A reader that stood where it was stands where it is now, exactly: in
    /// f32, `at + now - was` can miss `now` by an ulp, which the revision's
    /// exact compare would take for an app scroll.
    pub(super) fn rebased(&mut self, id: ViewId, was: (f32, f32), now: (f32, f32)) {
        if let Some((_, at)) = self.tracked.get_mut(&id) {
            let axis = |at: f32, was: f32, now: f32| if at == was { now } else { at + now - was };
            *at = (axis(at.0, was.0, now.0), axis(at.1, was.1, now.1));
        }
    }
}

const X: i32 = 1;
const Y: i32 = 2;
const NODE: i32 = 4;

fn scrolls(o: Overflow) -> bool {
    matches!(o, Overflow::Scroll | Overflow::Auto)
}

impl<D: DataSource> Presenter<D> {
    /// The page as a scroller: the document past the viewport.
    pub(super) fn page_scroller(&self) -> Scroller {
        let doc = self.document();
        let viewport = self.display.viewport().unwrap_or(self.viewport);
        let max = ((doc.0 - viewport.0).max(0.0), (doc.1 - viewport.1).max(0.0));
        Scroller {
            id: PAGE,
            axes: (max.0 > 0.0, max.1 > 0.0),
            max,
            offset: self.page,
            port: [0.0, 0.0, viewport.0, viewport.1],
            node: false,
        }
    }

    /// Scroller `id` as it stands, without its port (a painted box, looked
    /// up only for a touch); `None` for a node that is no scroll container.
    pub(super) fn scroller(&self, id: ViewId, limits: &BTreeMap<ViewId, f32>) -> Option<Scroller> {
        if id == PAGE {
            return Some(self.page_scroller());
        }
        let kernel = self.host.kernel();
        let node = kernel.node(id)?;
        let bounds = self.display.bounds(kernel, id).unwrap_or_else(|| {
            self.brush.scroll_bounds(
                kernel,
                self.host.content_region(),
                &node,
                limits.get(&id).copied(),
            )
        });
        let (ox, oy) = bounds.axes;
        if !scrolls(ox) && !scrolls(oy) {
            return None;
        }
        let max = (bounds.max.0.max(0.0), bounds.max.1.max(0.0));
        Some(Scroller {
            id,
            axes: (scrolls(ox) && max.0 > 0.0, scrolls(oy) && max.1 > 0.0),
            max,
            offset: self.scroll.get(&id).copied().unwrap_or((0.0, 0.0)),
            port: [0.0; 4],
            node: self.brush.row_groups().iter().any(|g| g.0 == id),
        })
    }

    /// The scrollers under a point of the viewport, innermost first, and
    /// the axes the presenter's own gestures there claim.
    pub fn scrollers_at(&mut self, x: f32, y: f32) -> Under {
        let Some(hit) = self.hit(x, y) else {
            return Under::default();
        };
        let limits = self.collection_scroll_limits();
        let mut chain = Vec::new();
        // The axes still open to the scrollers farther out.
        let mut open = (true, true);
        let mut at = Some(hit);
        while let Some(id) = at {
            let kernel = self.host.kernel();
            let Some(node) = kernel.node(id) else { break };
            let contained =
                |b: exact_kernel::OverscrollBehavior| b != exact_kernel::OverscrollBehavior::Auto;
            let cut = (
                contained(node.style.overscroll_behavior_x),
                contained(node.style.overscroll_behavior_y),
            );
            let parent = self.scroll_parent(id).or_else(|| {
                node.node_type
                    .is_svg_element()
                    .then_some(node.parent)
                    .flatten()
            });
            if let Some(mut s) = self.scroller(id, &limits) {
                s.axes = (s.axes.0 && open.0, s.axes.1 && open.1);
                open = (open.0 && !cut.0, open.1 && !cut.1);
                if s.axes.0 || s.axes.1 {
                    if let Some(b) = self.box_of(id) {
                        s.port = shown(b.rect, b.clip);
                    }
                    chain.push(s);
                }
            }
            at = parent;
        }
        let mut page = self.page_scroller();
        page.axes = (page.axes.0 && open.0, page.axes.1 && open.1);
        if page.axes.0 || page.axes.1 {
            chain.push(page);
        }
        Under {
            chain,
            claims: self.claims_at(hit),
            field: self.hosted_at(hit),
        }
    }

    /// The axes a contact on `hit` would be the presenter's own gesture
    /// along: what its candidates accept (`contact.rs`).
    fn claims_at(&self, hit: ViewId) -> (bool, bool) {
        let kernel = self.host.kernel();
        let Some(key) = kernel.node(hit).map(|n| n.key) else {
            return (false, false);
        };
        if !self.input_live(key) {
            return (false, false);
        }
        let mut claims = (false, false);
        for candidate in self.candidates(key) {
            let (x, y) = match candidate {
                Candidate::Arrange(_) | Candidate::Height { .. } => (false, true),
                Candidate::Swipe(_) => (true, false),
                Candidate::Transform(_) | Candidate::Group(_) => (true, true),
                // A `pan` hears what its `touch-action` does not give to
                // scrolling, as a browser cancels a pointer it pans with.
                Candidate::Pan(key) => {
                    let action = kernel
                        .node_by_key(key)
                        .map_or("auto", |n| n.style.touch_action.pans().name());
                    let gives = |axis: &[&str]| {
                        matches!(action, "auto" | "manipulation")
                            || axis.iter().any(|a| action.contains(a))
                    };
                    (
                        !gives(&["pan-x", "pan-left", "pan-right"]),
                        !gives(&["pan-y", "pan-up", "pan-down"]),
                    )
                }
            };
            claims = (claims.0 || x, claims.1 || y);
        }
        claims
    }

    /// Scroll `id` to `to`, clamped to its travel on the axes it scrolls
    /// along; where it then is, or `None` when `id` is no scroll container.
    pub fn scroll_to(&mut self, id: ViewId, to: (f32, f32)) -> Option<(f32, f32)> {
        if !to.0.is_finite() || !to.1.is_finite() {
            return None;
        }
        self.restore_scroll_anchors();
        // An absolute destination supersedes the previous position's correction.
        self.cancel_anchor_correction(id);
        let result = self.scroll_to_completion(id, to, Completion::Instant, None);
        self.arm_scroll_anchors();
        result.map(|_| {
            if id == PAGE {
                self.page
            } else {
                self.scroll_of(id)
            }
        })
    }

    fn scroll_to_completion(
        &mut self,
        id: ViewId,
        to: (f32, f32),
        completion: Completion,
        relative: Option<(f32, f32)>,
    ) -> Option<(f32, f32)> {
        if !to.0.is_finite() || !to.1.is_finite() {
            return None;
        }
        let limits = self.collection_scroll_limits();
        let s = self.scroller(id, &limits)?;
        let next = (
            if s.axes.0 {
                to.0.clamp(0.0, s.max.0)
            } else {
                s.offset.0
            },
            if s.axes.1 {
                to.1.clamp(0.0, s.max.1)
            } else {
                s.offset.1
            },
        );
        let moved = relative
            .and_then(|by| self.anchor_input(id, s.offset, if s.axes.1 { by.1 } else { 0. }, next));
        if next == s.offset && moved.is_none_or(|dy| dy == 0.) {
            return Some(next);
        }
        if completion == Completion::Reader {
            if let Some((_, accepted)) = self.reader.tracked.get_mut(&id) {
                // A scroll handler may rebase this position before dispatch returns.
                *accepted = next;
            }
        }
        // A reader's move carries the list's own pending position across it (`ack.rs`).
        if completion == Completion::Reader {
            self.keep_own_position(id);
        }
        if id == PAGE {
            self.page = next;
        } else {
            self.scroll.insert(id, next);
            self.scroll_end_input(id, s.offset, completion);
            self.collection_scroll_input(id, s.offset);
        }
        self.dirty = true;
        if let Some(error) = self.refresh_transform_geometry() {
            self.host.log(error);
        }
        Some(next)
    }

    /// The reader moved scroller `id` by `by`, knowing revision `rev` of
    /// it: how far it went here, or `None` for a move the reader sent before
    /// it knew the app had moved the scroller (dropped), or for a scroller
    /// it was never told of.
    pub fn reader_scrolled(&mut self, id: ViewId, by: (f32, f32), rev: u32) -> Option<(f32, f32)> {
        self.restore_scroll_anchors();
        if !self.reader.tracked.contains_key(&id) {
            return None;
        }
        let limits = self.collection_scroll_limits();
        let s = self.scroller(id, &limits)?;
        // Moved since the reader's last move: its revision moves on now,
        // not at the next statement, and this move is the stale one.
        if self.reader_revision(&s) != rev {
            return None;
        }
        let from = s.offset;
        let to = self.scroll_to_completion(
            id,
            (from.0 + by.0, from.1 + by.1),
            Completion::Reader,
            Some(by),
        )?;
        Some((to.0 - from.0, to.1 - from.1))
    }

    /// An agent's phased wheel supplies one reader sample on the existing clock.
    pub(crate) fn reader_wheel(&mut self, id: ViewId, dx: f32, dy: f32) -> Result<String, String> {
        if self.host.route_visibility(id).1 || self.box_of(id).is_none() {
            return Err(format!("view {id} is hidden or inert"));
        }
        let limits = self.collection_scroll_limits();
        let s = self
            .scroller(id, &limits)
            .ok_or_else(|| format!("view {id} is not a scroller"))?;
        let revision = self.reader_revision(&s);
        self.reader_scrolled(id, (dx, dy), revision)
            .ok_or_else(|| format!("view {id} refused the reader sample"))?;
        Ok(format!(
            "{{\"tapped\":{id},\"wheel\":[{},{}],\"gesture\":true}}",
            num(dx as f64),
            num(dy as f64)
        ))
    }

    /// A driver from outside moves `id` itself from now on (the benchmark's
    /// `scroll`): the reader is told nothing more of it.
    pub fn reader_forgets(&mut self, id: ViewId) {
        self.reader.tracked.remove(&id);
    }

    /// The scrollers the clamp leaves alone: each a list kept while its route is hidden (a
    /// full-screen route over it), or shown again but not yet painted where it shows. The
    /// picture holds no travel for such a list, so the clamp would put it at its top and it
    /// would show its first rows when its route came back (Excord's picture dismiss). Shown
    /// again, its first frame is bounded by its layout's travel, staged as its own clamp
    /// (`stage_shown`) with its input offset kept until that frame is acknowledged, any rows put
    /// above it while hidden kept by its anchor; painted, it is clamped as before. Only the node
    /// whose offset it is keeps it: the one a shown clamp saw, or the one the acknowledged
    /// picture painted at its id, however its offset was written (`shown_keys`). A hidden
    /// tree built again is fresh ids, clamped afresh: no stale offset.
    pub(super) fn hidden_scrollers(&mut self) -> BTreeSet<ViewId> {
        let mut held = BTreeSet::new();
        let limits = self.collection_scroll_limits();
        for id in self.scroll.keys().copied().collect::<Vec<_>>() {
            let Some(key) = self.host.kernel().node(id).map(|n| n.key) else {
                self.collection.shown_keys.remove(&id);
                continue;
            };
            let hidden = self.host.route_visibility(id).0;
            if self.display.painted_key(id) == Some(key)
                || (!hidden && self.display.witnessed(self.host.kernel(), id))
            {
                self.collection.shown_keys.insert(id, key);
            }
            if !hidden && self.display.witnessed(self.host.kernel(), id) {
                continue;
            }
            if self.collection.shown_keys.get(&id) != Some(&key) {
                continue;
            }
            if !hidden {
                let kernel = self.host.kernel();
                let node = kernel.node(id).unwrap();
                let region = self.host.content_region();
                let bounds =
                    self.brush
                        .scroll_bounds(kernel, region, &node, limits.get(&id).copied());
                self.stage_shown(id, key, bounds);
            }
            held.insert(id);
        }
        held
    }

    /// The reader's revision of scroller `id`, moved on when the offset is
    /// not where the reader's own moves left it; tracked from here on.
    fn reader_revision(&mut self, s: &Scroller) -> u32 {
        let entry = self.reader.tracked.entry(s.id).or_insert((0, s.offset));
        if entry.1 != s.offset {
            *entry = (entry.0 + 1, s.offset);
        }
        entry.0
    }

    /// [`Presenter::scrollers_at`] for a reader, in its pixels (`scale` to a
    /// point): the claims (bit 0 sideways, bit 1 up and down), a count, the
    /// hosted text field the touch is for (0: none; LLP 1115.001 D8), and
    /// eleven words a scroller: id, flags (bit 0 it takes a sideways drag
    /// from here, bit 1 one up or down, bit 2 it has rows the reader can
    /// move), revision, then as float bits its offset (x, y), its farthest
    /// offset (x, y) and its port (x, y, width, height). The reader is told
    /// of each from now on ([`Presenter::reader_scrollers`]).
    pub fn reader_under(&mut self, x: f32, y: f32, scale: f32) -> Vec<i32> {
        let under = self.scrollers_at(x / scale, y / scale);
        let mut out = vec![
            i32::from(under.claims.0) | i32::from(under.claims.1) << 1,
            under.chain.len() as i32,
            under.field.unwrap_or(0) as i32,
        ];
        for s in &under.chain {
            let rev = self.reader_revision(s);
            out.extend([s.id as i32, flags(s), rev as i32]);
            let px = [s.offset.0, s.offset.1, s.max.0, s.max.1];
            out.extend(px.iter().chain(&s.port).map(|v| bits(v * scale)));
        }
        out
    }

    /// What the reader is told after a frame of the scrollers it knows, when
    /// that is not what it was last told (`None`): nine words a scroller:
    /// id, flags (as [`Presenter::reader_under`]'s, for any drag), revision,
    /// then as float bits where it is (x, y), the offset its rows were last
    /// painted at (x, y; `drawn` names them for a scroller that has rows)
    /// and its farthest offset (x, y), in the reader's pixels. A scroller the
    /// reader knew that is not in a statement is gone (its node went, or a
    /// driver took it over): the reader forgets it.
    pub fn reader_scrollers(
        &mut self,
        drawn: &[(ViewId, (f32, f32))],
        scale: f32,
    ) -> Option<Vec<i32>> {
        if self.reader.tracked.is_empty() {
            // The last scroller it knew went: said once, with nothing in it.
            return (!std::mem::take(&mut self.reader.last).is_empty()).then(Vec::new);
        }
        let limits = self.collection_scroll_limits();
        let ids: Vec<ViewId> = self.reader.tracked.keys().copied().collect();
        let mut out = Vec::with_capacity(ids.len() * 9);
        for id in ids {
            let Some(mut s) = self.scroller(id, &limits) else {
                self.reader.tracked.remove(&id);
                continue;
            };
            let rev = self.reader_revision(&s);
            let painted = drawn.iter().find(|g| g.0 == id).map(|g| g.1);
            s.node = painted.is_some();
            let at = painted.unwrap_or(s.offset);
            out.extend([id as i32, flags(&s), rev as i32]);
            let px = [s.offset.0, s.offset.1, at.0, at.1, s.max.0, s.max.1];
            out.extend(px.iter().map(|v| bits(v * scale)));
        }
        if out == self.reader.last {
            return None;
        }
        self.reader.last.clone_from(&out);
        Some(out)
    }
}

fn flags(s: &Scroller) -> i32 {
    (if s.axes.0 { X } else { 0 })
        | (if s.axes.1 { Y } else { 0 })
        | (if s.node { NODE } else { 0 })
}

/// A box within its clip: x, y, width, height.
fn shown(r: Rect4, clip: Option<Rect4>) -> [f32; 4] {
    let Some(c) = clip else {
        return [r.0, r.1, r.2, r.3];
    };
    let (x, y) = (r.0.max(c.0), r.1.max(c.1));
    let right = (r.0 + r.2).min(c.0 + c.2);
    let bottom = (r.1 + r.3).min(c.1 + c.3);
    [x, y, (right - x).max(0.0), (bottom - y).max(0.0)]
}

fn bits(v: f32) -> i32 {
    v.to_bits() as i32
}

#[cfg(test)]
#[path = "scrollers_tests.rs"]
mod tests;

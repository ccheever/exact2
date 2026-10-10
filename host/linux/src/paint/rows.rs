//! A scroller's rows, recorded once and replayed while unchanged. A backend
//! that keeps recordings (the Canvas recorder: one Android `RenderNode` a
//! row) is asked for a row's drawing only when the row changed; otherwise
//! the walk replays its boxes and paragraphs, moved by however far the row
//! moved, and the backend draws the kept recording there. Scrolling a list
//! of unchanged rows walks none of them.
//!
//! A row is invalid once anything in it changes: a commit touching,
//! creating or destroying a node in it, a presentation value, a press, a
//! picture arriving. A layout change that moves a node only makes the row
//! suspect: kept while every node sits where it sat relative to the row.
use super::*;
use exact_kernel::id::{IdMap, IdSet};
use exact_kernel::NodeKey;

/// What changed since the last frame, by node: what the host saw.
#[derive(Default)]
pub struct Dirty {
    hard: Vec<NodeKey>,
    moved: Vec<NodeKey>,
    /// Nodes a commit bound to another item (LLP 1078).
    renewed: Vec<NodeKey>,
}

impl Dirty {
    /// A commit's nodes.
    pub fn commit(&mut self, r: &exact_kernel::CommitReceipt) {
        self.hard.extend(&r.touched);
        self.hard.extend(&r.created);
        self.hard.extend(&r.destroyed);
        self.renewed.extend(&r.renewed);
    }

    /// A layout's moved frames, and paragraphs whose exclusions changed.
    pub fn layout(&mut self, r: &exact_kernel::LayoutReceipt) {
        self.moved.extend(&r.changed);
        self.hard.extend(&r.flow_changed);
    }

    /// A node whose presentation changed.
    pub fn node(&mut self, key: NodeKey) {
        self.hard.push(key);
    }
}

/// One kept row.
struct Row {
    id: u32,
    /// The row's top-left in viewport points when recorded.
    origin: (f32, f32),
    scale: (f32, f32),
    size: (f32, f32),
    boxes: Vec<PaintedBox>,
    text: Vec<(NodeKey, Rc<Paragraph>)>,
    images: Vec<(ViewId, Option<std::sync::Weak<Bitmap>>)>,
    /// Where each picture kept apart from the row is drawn.
    slots: Vec<Slot>,
    /// Each node's frame relative to the row's (x, y) and its size.
    frames: Vec<(NodeKey, [f32; 4])>,
    /// The scrollers inside it and their offsets when recorded: one moved
    /// (a swipe) records the row again.
    scrolls: Vec<(ViewId, (f32, f32))>,
    unsupported: bool,
    /// What the row's drawing covers, relative to its origin.
    bounds: Rect4,
    seen: u64,
    /// Something in it changed: recorded again before it draws, and kept
    /// as it was when the new recording is the same.
    stale: bool,
    /// What its subtree lets escape into its list's painting, while nothing
    /// in it changed ([`Painter::refresh_ranks`]).
    order: Option<exact_kernel::paint_order::Potentials>,
}

/// A picture a backend may keep apart from its row ([`Backend::slot_begin`]):
/// the image node, its content box, its outer shape and the transform it
/// was drawn in.
type Slot = (ViewId, Rect4, Shape, Transform);

/// A row being recorded.
#[derive(Default)]
struct Capture {
    refused: bool,
    images: Vec<(ViewId, Option<std::sync::Weak<Bitmap>>)>,
    slots: Vec<Slot>,
    text: Vec<(NodeKey, Rc<Paragraph>)>,
    frames: Vec<(NodeKey, [f32; 4])>,
    scrolls: Vec<(ViewId, (f32, f32))>,
}

#[derive(Default)]
pub(super) struct Rows {
    kept: IdMap<NodeKey, Row>,
    suspect: IdSet<NodeKey>,
    next: u32,
    frame: u64,
    active: bool,
    recording: Option<Capture>,
    freed: Vec<u32>,
    /// Viewport width, scale and appearance the kept rows were drawn at.
    world: Option<(u32, u32, bool)>,
    /// `EXACT_ROWS=0` turns rows off, for comparison.
    off: Option<bool>,
    /// This walk's scrollers with rows: each one's id and the range of the
    /// walk's boxes its rows pushed.
    groups: Vec<(ViewId, usize, usize)>,
    /// The container whose children are its scroller's rows, while it is
    /// walked (see [`Painter::wraps_rows`]).
    wrapper: Option<NodeKey>,
}

impl Rows {
    /// Whether the last walk kept rows.
    pub(super) fn active(&self) -> bool {
        self.active
    }

    /// The potentials of the kept row at `key` while nothing in it changed.
    pub(super) fn order(&self, key: NodeKey) -> Option<exact_kernel::paint_order::Potentials> {
        let row = self.kept.get(&key)?;
        row.order.filter(|_| !row.stale)
    }
}

/// Room around a row's boxes for what paints outside them (shadows).
const MARGIN: f32 = 48.0;

fn union(a: Option<Rect4>, b: Rect4) -> Rect4 {
    match a {
        None => b,
        Some(a) => {
            let x0 = a.0.min(b.0);
            let y0 = a.1.min(b.1);
            let x1 = (a.0 + a.2).max(b.0 + b.2);
            let y1 = (a.1 + a.3).max(b.1 + b.3);
            (x0, y0, x1 - x0, y1 - y0)
        }
    }
}

fn within(a: Option<Rect4>, outer: Option<Rect4>) -> Option<Rect4> {
    match (a, outer) {
        (Some(a), Some(b)) => Some(intersect(a, b)),
        (a, None) => a,
        (None, b) => b,
    }
}

impl Painter {
    /// Before a frame: drop the rows the host's changes reach.
    pub(crate) fn rows_dirty(&mut self, kernel: &Kernel, dirty: Dirty) {
        if self.rows.kept.is_empty() {
            return;
        }
        // Each node climbed, with the row it is in (or none): a row's nodes,
        // and those outside rows, climb their shared ancestors once.
        let mut memo: IdMap<NodeKey, Option<NodeKey>> = IdMap::default();
        let mut path = Vec::new();
        let mut row_of = |rows: &Rows, key: NodeKey| {
            let mut at = key;
            let found = loop {
                if let Some(row) = memo.get(&at) {
                    break *row;
                }
                if rows.kept.contains_key(&at) {
                    break Some(at);
                }
                path.push(at);
                let parent = kernel.node_by_key(at).and_then(|n| n.parent);
                match parent.and_then(|p| kernel.node(p)) {
                    Some(parent) => at = parent.key,
                    None => break None,
                }
            };
            for k in path.drain(..) {
                memo.insert(k, found);
            }
            found
        };
        for key in dirty.hard {
            if let Some(row) = row_of(&self.rows, key) {
                self.rows.kept.get_mut(&row).expect("found").stale = true;
            }
        }
        // A row bound to another item is a new row (LLP 1078): its kept
        // recording is the other item's, and a backend told the row was that
        // one draws it in the row's place until the new one is made. The
        // Canvas reader did, for as long as no stream followed: a rebound
        // row coming into view showed the item it had been, in a slot that
        // item's height left short (easy at 3,000 dp/s toward the start: up
        // to five frames, three times in two seconds).
        for key in dirty.renewed {
            if let Some(row) = self.rows.kept.remove(&key) {
                self.rows.freed.push(row.id);
            }
        }
        let mut seen = IdSet::default();
        for key in dirty.moved {
            if !seen.insert(key) {
                continue;
            }
            if let Some(row) = row_of(&self.rows, key) {
                self.rows.suspect.insert(row);
            }
        }
    }

    /// At a walk's start: whether rows are kept this walk.
    pub(super) fn rows_begin(&mut self, walk: &Walk<'_, '_>) {
        let off = *self
            .rows
            .off
            .get_or_insert_with(|| std::env::var("EXACT_ROWS").is_ok_and(|v| v == "0"));
        self.rows.frame += 1;
        self.rows.groups.clear();
        self.rows.active = !off
            && self.backend.rows()
            && walk.skip.is_none()
            && walk.replay.is_none()
            && self.placements.is_empty()
            && self.lift.arrange.is_none()
            && self.flatten.is_none();
        let world = (self.viewport.0.to_bits(), self.scale.to_bits(), self.dark);
        if self.rows.world != Some(world) {
            self.rows.world = Some(world);
            let all: Vec<u32> = self.rows.kept.drain().map(|(_, r)| r.id).collect();
            self.rows.freed.extend(all);
        }
    }

    /// At a walk's end: free what no longer draws.
    pub(super) fn rows_end(&mut self) {
        if self.rows.active {
            let frame = self.rows.frame;
            let stale: Vec<NodeKey> = self
                .rows
                .kept
                .iter()
                .filter(|(_, r)| r.seen != frame)
                .map(|(k, _)| *k)
                .collect();
            for k in stale {
                let gone = self.rows.kept.remove(&k).expect("listed");
                self.rows.freed.push(gone.id);
            }
        }
        self.rows.suspect.clear();
        for id in std::mem::take(&mut self.rows.freed) {
            self.backend.row_free(id);
        }
    }

    /// A scroller's rows begin: the backend groups them; their boxes are noted.
    pub(super) fn group_begin(&mut self, walk: &Walk<'_, '_>, id: ViewId) {
        let scroll = walk.scene.scroll.get(&id).copied().unwrap_or((0.0, 0.0));
        self.backend.group_begin(id, scroll);
        self.rows
            .groups
            .push((id, walk.boxes.len(), walk.boxes.len()));
    }

    pub(super) fn group_end(&mut self, walk: &Walk<'_, '_>) {
        self.backend.group_end();
        if let Some(g) = self.rows.groups.last_mut() {
            g.2 = walk.boxes.len();
        }
    }

    /// The last walk's scrollers with rows and the range of its boxes each
    /// one's rows are: the boxes a moved paint moved.
    pub(crate) fn row_groups(&self) -> &[(ViewId, usize, usize)] {
        &self.rows.groups
    }

    /// Whether `node`'s children are rows this walk. A scroller inside a
    /// recording row moves without a commit: the row keeps its offset and
    /// is recorded again once the offset is another.
    pub(super) fn has_rows(&mut self, walk: &Walk<'_, '_>, node: &NodeRef<'_>) -> bool {
        let (x, y) = effective_overflow(node);
        if let Some(c) = &mut self.rows.recording {
            if scrolls(x) || scrolls(y) {
                let at = walk
                    .scene
                    .scroll
                    .get(&node.id)
                    .copied()
                    .unwrap_or((0.0, 0.0));
                c.scrolls.push((node.id, at));
            }
            return false;
        }
        self.rows.active && (scrolls(y) || self.rows_wrapper(node.key))
    }

    /// Whether `key` is the container walked for its scroller's rows.
    pub(super) fn rows_wrapper(&self, key: NodeKey) -> bool {
        self.rows.wrapper == Some(key)
    }

    /// Whether a scroller's `children` are one container of several whose
    /// children are the rows (`scroll > column > rows`): each of them then
    /// records, and is kept, apart. `EXACT_WRAPPED_ROWS=0` keeps the
    /// container one row, to compare.
    pub(super) fn wraps_rows(&self, walk: &Walk<'_, '_>, children: &[ViewId]) -> bool {
        static ON: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
            !std::env::var("EXACT_WRAPPED_ROWS").is_ok_and(|v| v == "0")
        });
        let [only] = children else { return false };
        *ON && walk
            .scene
            .kernel
            .node(*only)
            .is_some_and(|n| n.children().len() > 1 && !scrolls(effective_overflow(&n).1))
    }

    /// A scroller's one container, walked with its children as the rows.
    pub(super) fn wrapped(
        &mut self,
        walk: &mut Walk<'_, '_>,
        id: ViewId,
        ts: Transform,
        offset: (f32, f32),
        clip_rect: Option<Rect4>,
    ) {
        let key = walk.scene.kernel.node(id).map(|n| n.key);
        let outer = std::mem::replace(&mut self.rows.wrapper, key);
        self.node(walk, id, ts, offset, clip_rect);
        self.rows.wrapper = outer;
    }

    /// Note a node walked while a row records.
    pub(super) fn row_node(&mut self, key: NodeKey, f: exact_kernel::Frame, shown: &Presented) {
        if let Some(c) = &mut self.rows.recording {
            c.frames.push((key, [f.x, f.y, f.width, f.height]));
            c.refused |= shown.press != 1.0;
        }
    }

    /// Note a picture drawn (or missing) while a row records.
    pub(super) fn row_image(&mut self, id: ViewId, image: Option<&Arc<Bitmap>>) {
        if let Some(c) = &mut self.rows.recording {
            // Held weakly: a dropped picture's address is not reused while
            // held, so a new picture never passes for the old.
            c.images.push((id, image.map(Arc::downgrade)));
        }
    }

    /// Note a picture's slot while a row records.
    pub(super) fn row_slot(&mut self, id: ViewId, content: Rect4, outer: Shape, ts: Transform) {
        if let Some(c) = &mut self.rows.recording {
            c.slots.push((id, content, outer, ts));
        }
    }

    /// An image node's picture in its content box, once it has arrived.
    pub(super) fn picture(
        &mut self,
        walk: &Walk<'_, '_>,
        node: &NodeRef<'_>,
        content: Rect4,
        outer: Shape,
        ts: Transform,
    ) {
        let Some(img) = walk.scene.images.get(&node.id) else {
            return;
        };
        if let Some(dst) = object_fit(img.natural(), node.style.object_fit, content) {
            let shown = (walk.scene.presented)(node.id);
            let tint = image_tint(node, &shown, self.dark);
            self.backend
                .image(img, dst, &[Shape::rect(content), outer], ts, tint);
        }
    }

    /// A kept row that only pictures changed in (one arrived, or went):
    /// each is drawn again in its slot, where the backend keeps slots, and
    /// the row stands. A heavy feed's row was otherwise walked whole for
    /// each of its pictures. False: the row is recorded again.
    fn pictures_again(&mut self, walk: &Walk<'_, '_>, key: NodeKey) -> bool {
        let Some(row) = self.rows.kept.get(&key) else {
            return false;
        };
        let rid = row.id;
        let mut changed = Vec::new();
        for (i, (id, kept)) in row.images.iter().enumerate() {
            if same_picture(walk.scene.images.get(id), kept) {
                continue;
            }
            let Some(slot) = row.slots.iter().find(|slot| slot.0 == *id) else {
                return false;
            };
            changed.push((i, *slot));
        }
        for (_, (id, content, outer, ts)) in &changed {
            let Some(node) = walk.scene.kernel.node(*id) else {
                return false;
            };
            if !self.backend.slot_again(rid, *id) {
                return false;
            }
            self.picture(walk, &node, *content, *outer, *ts);
            self.backend.slot_end();
        }
        let row = self.rows.kept.get_mut(&key).expect("kept");
        for (i, (id, ..)) in changed {
            row.images[i].1 = walk.scene.images.get(&id).map(Arc::downgrade);
        }
        true
    }

    /// Note a paragraph leased while a row records.
    pub(super) fn row_text(&mut self, key: NodeKey, p: &Rc<Paragraph>) {
        if let Some(c) = &mut self.rows.recording {
            c.text.push((key, p.clone()));
        }
    }

    /// What a recording row cannot keep (a caret, a canvas, a 3D island).
    pub(super) fn row_refuse(&mut self) {
        if let Some(c) = &mut self.rows.recording {
            c.refused = true;
        }
    }

    /// Whether a kept row draws what it would record now; `pictures`: its
    /// pictures included.
    fn row_valid(
        &self,
        row: &Row,
        node: &NodeRef<'_>,
        walk: &Walk<'_, '_>,
        scale: (f32, f32),
        pictures: bool,
    ) -> bool {
        let kernel = walk.scene.kernel;
        !row.stale
            && row.scale == scale
            && row.size == (node.frame.width, node.frame.height)
            && row
                .scrolls
                .iter()
                .all(|(id, at)| walk.scene.scroll.get(id).copied().unwrap_or((0.0, 0.0)) == *at)
            && (!pictures
                || row
                    .images
                    .iter()
                    .all(|(id, kept)| same_picture(walk.scene.images.get(id), kept)))
            && (!self.rows.suspect.contains(&node.key)
                || row.frames.iter().all(|(k, f)| {
                    kernel.node_by_key(*k).is_some_and(|n| {
                        let g = n.frame;
                        (g.x - node.frame.x - f[0]).abs() < 0.5
                            && (g.y - node.frame.y - f[1]).abs() < 0.5
                            && g.width == f[2]
                            && g.height == f[3]
                    })
                }))
    }

    /// A row: replayed when kept and unchanged, else walked and recorded.
    pub(super) fn row(
        &mut self,
        walk: &mut Walk<'_, '_>,
        id: ViewId,
        ts: Transform,
        offset: (f32, f32),
        clip_rect: Option<Rect4>,
    ) {
        let Some(node) = walk.scene.kernel.node(id) else {
            return;
        };
        if ts.kx != 0.0 || ts.ky != 0.0 || (walk.scene.hidden)(id) {
            return self.node(walk, id, ts, offset, clip_rect);
        }
        let r = paint_rect(node.frame, offset);
        let origin = (ts.sx * r.0 + ts.tx, ts.sy * r.1 + ts.ty);
        let scale = (ts.sx, ts.sy);
        let frame = self.rows.frame;
        let kept = self.rows.kept.get(&node.key);
        let valid = kept.is_some_and(|row| self.row_valid(row, &node, walk, scale, true))
            || (kept.is_some_and(|row| self.row_valid(row, &node, walk, scale, false))
                && self.pictures_again(walk, node.key));
        if let Some(row) = self.rows.kept.get(&node.key).filter(|_| valid) {
            let d = (origin.0 - row.origin.0, origin.1 - row.origin.1);
            for b in &row.boxes {
                let mut b = *b;
                b.rect.0 += d.0;
                b.rect.1 += d.1;
                b.clip = within(b.clip.map(|c| (c.0 + d.0, c.1 + d.1, c.2, c.3)), clip_rect);
                b.affine = b.affine.map(|(t, r)| (t.post_translate(d.0, d.1), r));
                walk.boxes.push(b);
            }
            for (k, p) in &row.text {
                walk.text.insert(*k, p.clone());
            }
            self.damage.unsupported |= row.unsupported;
            let bounds = (
                row.bounds.0 + origin.0,
                row.bounds.1 + origin.1,
                row.bounds.2,
                row.bounds.3,
            );
            let rid = row.id;
            self.rows.kept.get_mut(&node.key).expect("kept").seen = frame;
            if !self.backend.row_culled(bounds) {
                self.backend.row_draw(rid, origin, bounds);
            }
            return;
        }
        let old = self.rows.kept.remove(&node.key);
        // Potentials found this epoch, else the old recording's when its
        // subtree is unchanged (recorded again for its size or a picture).
        let order = self
            .fresh_order
            .get(&id)
            .copied()
            .or_else(|| old.as_ref().filter(|o| !o.stale).and_then(|o| o.order));
        let previous = old.map(|old| old.id);
        self.rows.next += 1;
        let start = walk.boxes.len();
        let unsupported = std::mem::replace(&mut self.damage.unsupported, false);
        // Without a transform, the row is walked in its own coordinates (its
        // top-left at 0,0, from document positions alone): the same row records
        // the same ops wherever it scrolled to, so one recorded again unchanged
        // keeps its drawing. Its boxes move to the page after.
        let local = ts == Transform::identity();
        let (walk_offset, content) = if local {
            ((node.frame.x, node.frame.y), (0.0, 0.0))
        } else {
            (offset, origin)
        };
        self.backend.row_begin(self.rows.next, content, previous);
        self.rows.recording = Some(Capture::default());
        self.node(walk, id, ts, walk_offset, None);
        if local {
            for b in &mut walk.boxes[start..] {
                b.rect.0 += origin.0;
                b.rect.1 += origin.1;
                b.clip = b.clip.map(|c| (c.0 + origin.0, c.1 + origin.1, c.2, c.3));
                b.affine = b
                    .affine
                    .map(|(t, r)| (t.post_translate(origin.0, origin.1), r));
            }
        }
        let capture = self.rows.recording.take().expect("recording");
        let row_unsupported = std::mem::replace(&mut self.damage.unsupported, unsupported);
        self.damage.unsupported |= row_unsupported;
        let boxes: Vec<PaintedBox> = walk.boxes[start..].to_vec();
        for b in &mut walk.boxes[start..] {
            b.clip = within(b.clip, clip_rect);
        }
        // What the row can draw: each box within its clip (a scroller's
        // content box is its whole extent, a horizontal list's thousands of
        // px wide; only its port shows), one clipped away entirely nothing.
        let covered = boxes
            .iter()
            .filter_map(|b| match b.clip {
                None => Some(b.rect),
                Some(c) => Some(intersect(b.rect, c)).filter(|r| r.2 > 0.0 && r.3 > 0.0),
            })
            .fold(None, |u, r| Some(union(u, r)))
            .unwrap_or((origin.0, origin.1, 0.0, 0.0));
        let bounds = (
            covered.0 - origin.0 - MARGIN,
            covered.1 - origin.1 - MARGIN,
            covered.2 + 2.0 * MARGIN,
            covered.3 + 2.0 * MARGIN,
        );
        // The backend keeps the previous recording when this one is the same.
        let rid = self.backend.row_end(bounds);
        if let Some(old) = previous.filter(|old| *old != rid) {
            self.rows.freed.push(old);
        }
        let page = (bounds.0 + origin.0, bounds.1 + origin.1, bounds.2, bounds.3);
        if !self.backend.row_culled(page) {
            self.backend.row_draw(rid, origin, page);
        }
        let keep = !capture.refused && boxes.iter().all(|b| b.projective.is_none());
        if !keep {
            self.rows.freed.push(rid);
            return;
        }
        let (x0, y0) = (node.frame.x, node.frame.y);
        self.rows.kept.insert(
            node.key,
            Row {
                id: rid,
                origin,
                scale,
                size: (node.frame.width, node.frame.height),
                boxes,
                text: capture.text,
                images: capture.images,
                slots: capture.slots,
                frames: capture
                    .frames
                    .into_iter()
                    .map(|(k, f)| (k, [f[0] - x0, f[1] - y0, f[2], f[3]]))
                    .collect(),
                scrolls: capture.scrolls,
                unsupported: row_unsupported,
                bounds,
                seen: frame,
                stale: false,
                order,
            },
        );
    }
}

/// Whether a node's picture is the one a row was recorded with.
fn same_picture(now: Option<&Arc<Bitmap>>, kept: &Option<std::sync::Weak<Bitmap>>) -> bool {
    match (now, kept) {
        (None, None) => true,
        (Some(now), Some(then)) => then.upgrade().is_some_and(|t| Arc::ptr_eq(&t, now)),
        _ => false,
    }
}

/// Whether an overflow scrolls: `scroll`, or `auto` (a scroller's default
/// since the kernel follows CSS there).
fn scrolls(o: Overflow) -> bool {
    matches!(o, Overflow::Scroll | Overflow::Auto)
}

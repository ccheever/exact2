//! The collection pass a scroll leaves for after its frame (rows mount and
//! retire there), and how soon what it mounts must be painted.
use super::*;

/// The share of the window's lead an unhurried paint counts on, and the
/// moves it comes early by: a step is smoothed over the last few, and the
/// reader runs two a frame, so travel that picks up, or a pass that ran a
/// step late, still finds its rows painted.
const UNHURRIED_LEAD: f32 = 0.75;
const UNHURRIED_FRAMES: u32 = 2;
/// ms since a scroll step within which the feed is still stepping (a frame
/// and a half at 120 Hz).
const STEP_MS: f64 = 12.0;

impl<D: DataSource + Default> CanvasHost<D> {
    /// When only scrollers whose rows the last paint drew moved since it,
    /// and nothing else it showed changed: the move, instead of a paint.
    /// The collection pass a scroll left for after its frame (rows mount and
    /// retire there, not inside the next frame's scroll); from now on scrolls
    /// leave it. Whether a frame is wanted after it.
    pub fn refine(&mut self) -> bool {
        self.refine_slice(None, 0.0)
    }

    /// [`CanvasHost::refine`], building at most `limit` rows past what shows
    /// per list (a slice; `None`: whole windows), the scrolled list leading
    /// toward `velocity` (logical px/s). Whether a paint is wanted; the
    /// rest a slice left is [`CanvasHost::refine_pending`].
    ///
    /// While the feed travels fast, a whole-window pass (`None`) builds only
    /// the rows its measured cost fits in a frame's share of this thread
    /// ([`crate::travel::Travel::limit`]); the rest is pending, for a pass after the
    /// steps queued meanwhile. Each pass's rows then show as it ends, not
    /// all of them after the longest pass (Extra Heavy at 24k dp/s: 13-35 ms,
    /// while the view ran past the last drawn row).
    pub fn refine_slice(&mut self, limit: Option<u32>, velocity: f64) -> bool {
        let _s = Section::begin(c"exact refine");
        // While the feed travels fast its window leads that way, unless the
        // reader says how it leads.
        let lead = self.leads().filter(|_| velocity == 0.0);
        let velocity = lead.unwrap_or(velocity);
        // A pass that can wait for more travel does: the rows the travel
        // brings into the window then mount in one commit. Owed once the
        // steps stop (`CanvasHost::owed`).
        self.waiting = limit.is_none() && self.batches(lead.is_some());
        if self.waiting {
            // Its pictures do not wait: one that came into view since the
            // last pass is asked for now (heavy's placeholders showed for
            // three points more of the view at 1,000 dp/s when they did).
            self.sync_moved_pictures();
            return false;
        }
        let started = std::time::Instant::now();
        let limit = limit.or_else(|| self.travel.limit());
        self.sliced = limit.is_some();
        // A row's cost is measured only where it is used: while the feed
        // travels fast (two walks to the list each pass otherwise, 2% of
        // crypto's scrolling).
        let measure = self.travel.fast();
        let mut before = std::mem::take(&mut self.rows_before);
        if measure {
            self.feed_rows(&mut before);
            before.sort_unstable();
        }
        self.p.slice_collections(limit, velocity);
        let wanted = self.refine_inner();
        self.leftover = self.p.collections_pending();
        // Every report between this pass and the next goes with its lead (a
        // frame's own, the reader's next ask for what a slice left): one
        // without it found the rows mounted ahead past its window, and
        // retired the farthest of them for the next pass to build again.
        self.p.slice_collections(None, lead.unwrap_or(0.0));
        self.led = lead.is_some();
        if measure {
            let mut after = std::mem::take(&mut self.rows_after);
            self.feed_rows(&mut after);
            let built = after
                .iter()
                .filter(|r| before.binary_search(r).is_err())
                .count();
            self.rows_after = after;
            self.travel
                .built(built, started.elapsed().as_secs_f32() * 1000.0);
        }
        self.rows_before = before;
        wanted
    }

    /// Whether a paint may wait for the frame rows mounted out of view can
    /// first show, rather than come within [`MOVES_MOUNTED`] moves of their
    /// pass. Those paints were most of a fling's (crypto at 6,000 px/s: 34 a
    /// second, one in three vsyncs, for 36 rows a second), and all a row
    /// needed of them when it came into view was its layers' new tracks,
    /// which now go with a move. It holds while passes fill whole windows:
    /// what one mounts then lies a viewport past the view, which is what
    /// [`crate::travel::Travel::passed`] counts on. A sliced pass (fast
    /// travel, rows left pending) mounts nearer, so its paint comes as before.
    pub(super) fn unhurried(&self) -> bool {
        self.move_tracks && self.lead && !self.sliced
    }

    /// The velocity the feed's window leads by
    /// ([`crate::travel::Travel::lead`]), while the feed travels: until
    /// [`crate::travel::SETTLE_MS`] pass without a scroll step, so a pass
    /// that ran long, or a frame the reader dropped, does not end it.
    /// `EXACT_PASS_LEAD=0`: never (a viewport each side, as slow travel has).
    fn leads(&self) -> Option<f64> {
        static ON: std::sync::LazyLock<bool> =
            std::sync::LazyLock::new(|| !std::env::var("EXACT_PASS_LEAD").is_ok_and(|v| v == "0"));
        let travels = self.now() - self.scrolled_at < crate::travel::SETTLE_MS;
        self.travel.lead().filter(|_| *ON && self.lead && travels)
    }

    /// Whether a moved paint owes a paint: once moves pause (a frame
    /// without one), the paint brings boxes, hits and pictures up to date.
    /// So does a pass that waits, and a window that leads once its steps
    /// stop: owed the pass that takes it back to a viewport each side when
    /// the travel has ended (asked before that, it waits).
    pub fn owed(&self) -> bool {
        (self.moved > 0 && self.moves < 1000)
            || self.waiting
            || (self.led && self.now() - self.scrolled_at >= STEP_MS)
    }

    /// Whether this pass can wait: the windows lead, the last pass left no
    /// rows to build (a slice's rest is not kept waiting), and a scroll
    /// step came within the last frame and a half with less travel since
    /// the last pass than one is worth ([`crate::travel::Travel::waits`];
    /// under a lead, `led`, [`crate::travel::Travel::waits_led`]). Under a
    /// lead a pass also waits once the steps stop: what it would mount is
    /// past where the view came to rest, and the pass that ends the lead
    /// retires it. `EXACT_PASS_BATCH=0`: never.
    fn batches(&self, led: bool) -> bool {
        static ON: std::sync::LazyLock<bool> =
            std::sync::LazyLock::new(|| !std::env::var("EXACT_PASS_BATCH").is_ok_and(|v| v == "0"));
        if !*ON || !self.lead || self.leftover {
            return false;
        }
        let viewport = self
            .feed
            .and_then(|id| self.p.host().kernel().node(id))
            .map_or(self.viewport.1, |n| n.frame.height);
        let stepping = self.now() - self.scrolled_at < STEP_MS;
        if led {
            !stepping || self.travel.waits_led(viewport)
        } else {
            stepping && self.travel.waits(viewport)
        }
    }

    /// The feed's mounted rows, as (view, epoch): a new pair is a row this
    /// pass built (or bound to another item).
    fn feed_rows(&self, out: &mut Vec<(ViewId, u64)>) {
        out.clear();
        if let Some(id) = self.feed {
            self.p.host().collection_mounted(id, out);
        }
    }

    /// The scroller [`CanvasHost::scroll`] moves, once a scroll found it.
    pub fn feed(&self) -> Option<ViewId> {
        self.feed
    }

    /// Device pixels per logical pixel.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Whether a slice left rows to build: the reader then asks for the next
    /// pass at once. A pass that waits for more travel left none (it is
    /// asked for again at the reader's next third step, or its timer).
    pub fn refine_pending(&self) -> bool {
        !self.waiting && self.p.collections_pending()
    }

    /// Pictures coming into view while frames move: requested now, where
    /// their rows are, not at the next paint.
    fn sync_moved_pictures(&mut self) {
        if self.moved > 0 {
            if let Some(painted) = &self.painted {
                let now = self.p.scroll_offsets();
                let moved: std::collections::BTreeMap<ViewId, (f32, f32)> = painted
                    .groups
                    .iter()
                    .map(|(id, at)| {
                        let to = now.get(id).copied().unwrap_or((0.0, 0.0));
                        (*id, (at.0 - to.0, at.1 - to.1))
                    })
                    .collect();
                if let Some(e) = self.p.sync_images_moved(&moved) {
                    eprintln!("exact: {e}");
                }
            }
        }
    }

    fn refine_inner(&mut self) -> bool {
        self.scrolled = false;
        self.prefetching = true;
        self.sync_moved_pictures();
        // How soon what this pass mounts past the view can scroll in: it
        // reaches the feed's viewport past it (the window's lead).
        let lead = self
            .feed
            .and_then(|id| self.p.host().kernel().node(id))
            .map_or(self.viewport.1, |n| n.frame.height);
        let unhurried = self.unhurried();
        let (lead, soon) = if unhurried {
            (lead * UNHURRIED_LEAD, self.moves)
        } else {
            (lead, self.moves.min(MOVES_MOUNTED))
        };
        let due = self
            .travel
            .passed(lead, soon.saturating_sub(self.moved))
            .map(|frames| frames.saturating_sub(if unhurried { UNHURRIED_FRAMES } else { 0 }));
        let before = self.p.still();
        let wanted = self.p.refine_deferred(true);
        // Only the pass changed the kernel (rows out of view): no paint now.
        let after = self.p.still();
        let painted = self.painted.as_ref().map(|p| &p.still);
        match (before, after, painted) {
            (Some(b), Some(a), Some(p))
                if (b == *p || self.quiet.is_some_and(|e| p.same_at(&b, e))) && b != a =>
            {
                let epoch = self.p.host().kernel().epoch();
                if p.same_at(&a, epoch) {
                    self.quiet = Some(epoch);
                    // Rows the scroll brings in before the paint that shows
                    // them would: that paint comes sooner, or now.
                    match due {
                        Some(0) if self.moved > 0 => {
                            self.force = true;
                            return true;
                        }
                        Some(frames) => {
                            let by = self.moved + frames;
                            self.paint_by = Some(self.paint_by.map_or(by, |b| b.min(by)));
                        }
                        None => {}
                    }
                    return false;
                }
                wanted
            }
            _ => wanted,
        }
    }
}

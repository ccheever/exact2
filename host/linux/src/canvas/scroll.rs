//! The reader scrolls by itself (LLP 1115.001 D7.1 to D7.3; the boundary is
//! `presenter/scrollers.rs`): a finger's drag and its fling are the
//! reader's, with Android's own classes. It asks what is under a touch,
//! moves the scroller's rows itself where the stream gave them a node of
//! their own, and says here how far it moved which scroller. After each
//! frame it is told where the scrollers it knows are and how far they go,
//! and whether the app moved one, which ends its fling.
//!
//! The feed is the scroller a finger last moved up or down (or the one a
//! driver's `scroll` found): the one whose travel the windows lead by and
//! whose rows the collection passes are timed for (`refine.rs`).

use super::{CanvasHost, Section, GROUPS};
use crate::presenter::PAGE;
use exact_runner::DataSource;

impl<D: DataSource + Default> CanvasHost<D> {
    /// What is under a point of the view (pixels), for the reader to decide
    /// what a drag from there is: the scrollers, innermost first, and the
    /// axes the presenter's own gestures claim
    /// ([`crate::presenter::Presenter::reader_under`] has the words).
    pub fn under(&mut self, x: f32, y: f32) -> Vec<i32> {
        let mut words = self.p.reader_under(x, y, self.scale);
        let module = self.p.hit(x / self.scale, y / self.scale).filter(|id| {
            self.p
                .host()
                .kernel()
                .node(*id)
                .is_some_and(|node| node.node_type == exact_kernel::NodeType::NativeView)
        });
        let rect = module.and_then(|id| self.p.rect_of(id)).unwrap_or_default();
        words.push(module.unwrap_or(0) as i32);
        words.extend(
            [rect.0, rect.1, rect.2, rect.3].map(|value| (value * self.scale).to_bits() as i32),
        );
        words
    }

    /// The reader moved scroller `id` by (`dx`, `dy`) pixels, knowing
    /// revision `rev` of it. Its rows mount after the frame, as a driver's
    /// scroll leaves them.
    pub fn scrolled(&mut self, id: u32, dx: f32, dy: f32, rev: u32) {
        let _s = Section::begin(c"exact scroll");
        let s = self.scale;
        self.p.hold_collections(self.prefetching);
        let went = self.p.reader_scrolled(id, (dx / s, dy / s), rev);
        self.p.hold_collections(false);
        let Some((_, down)) = went.filter(|w| *w != (0.0, 0.0)) else {
            return;
        };
        self.scrolled = self.prefetching;
        self.scrolled_at = self.now();
        self.p.scroll_end_reader_sampled(id, self.scrolled_at);
        if down != 0.0 && id != PAGE {
            if self.feed != Some(id) {
                self.feed = Some(id);
                self.travel = crate::travel::Travel::default();
            }
            self.travel.scrolled(down, self.scrolled_at);
        }
    }

    /// What the reader is told of the scrollers it knows, when that is not
    /// what it was last told
    /// ([`crate::presenter::Presenter::reader_scrollers`] has the words).
    pub fn scrollers(&mut self) -> Option<Vec<i32>> {
        let scale = self.scale;
        GROUPS.with(|g| self.p.reader_scrollers(&g.borrow(), scale))
    }

    /// Before a driver's scroll: a feed whose node is gone (its page was
    /// left) is found again, and the one it moves is the driver's from now
    /// on, not the reader's.
    pub(super) fn driven(&mut self) {
        if (self.feed).is_some_and(|id| self.p.host().kernel().node(id).is_none()) {
            self.feed = None;
        }
        if let Some(id) = self.feed {
            self.p.reader_forgets(id);
        }
    }
}

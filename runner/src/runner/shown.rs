//! Where a native host shows what it does not leave in the page's flow: the
//! open top-layer boxes (dialogs, popovers, presented routes), and the
//! roots of subtrees it lays out in blocks of their own and shows in place
//! (LLP 1115.002 D8.2, LLP 1123 D2). What `frame` and `elementFromPoint`
//! read through (`Scrolled`).
use super::{DataSource, Runner};
use exact_kernel::ViewId;

/// Where a host shows a block in place: its rectangle in the local
/// coordinates of a view that holds it (`Scrolled::place`).
pub type BlockPlacement = ([f64; 4], ViewId);

impl<D: DataSource> Runner<D> {
    /// The open popovers and dialogs last shown by a native presenter.
    pub fn top_layer(&mut self, views: &[ViewId]) {
        self.scrolled.top_layer(&self.kernel, views);
    }

    /// Where the host shows the blocks of subtrees it lays out in blocks of
    /// their own without lifting them to the top layer, each the block's
    /// rectangle in the local coordinates of an ancestor of its root that
    /// holds it in the host as in the document (`None` forgets one): what
    /// `frame`, `elementFromPoint` and the intersection observer read their
    /// subtrees through, each root keeping its place among its siblings
    /// (`Scrolled::place`, LLP 1123 D2).
    pub fn place_blocks(&mut self, placements: &[(ViewId, Option<BlockPlacement>)]) {
        for &(view, placement) in placements {
            self.scrolled.place(&self.kernel, view, placement);
        }
    }

    /// Open top-layer boxes in the presenter's back-to-front order and where
    /// it shows them, in viewport points: a block's root by its block's
    /// rectangle, any other box by its border box (its placement, a
    /// keyboard's lift included) before its own CSS transform, which the
    /// runner applies. `modal` is the index of the first box of the
    /// frontmost modal presentation, a sheet or a full-screen presentation
    /// (LLP 1123 D2): it and the boxes after it are live to structural
    /// reads, and the boxes before it and the page are inert behind it, as
    /// the host makes them to touch. None while no presentation is open.
    pub fn top_layer_frames(&mut self, frames: &[(ViewId, [f64; 4])], modal: Option<usize>) {
        self.scrolled.top_layer_frames(&self.kernel, frames, modal);
    }
}

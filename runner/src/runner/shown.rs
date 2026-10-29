//! Where a native host shows what it does not leave in the page's flow: the
//! open top-layer boxes (dialogs, popovers, presented routes), and the
//! roots of subtrees it lays out in blocks of their own and shows in place
//! (LLP 1115.002 D8.2, LLP 1123 D2). What `frame` and `elementFromPoint`
//! read through (`Scrolled`).
use super::{DataSource, Runner};
use crate::geometry::TopLayerFrame;
use exact_kernel::ViewId;

/// A placement records the block's rectangle in its holding view's local
/// coordinates. A content offset names a scrollport, including at zero;
/// an absent offset names a fixed allocation (`Scrolled::place`).
pub type BlockPlacement = ([f64; 4], ViewId, Option<[f64; 2]>);

impl<D: DataSource> Runner<D> {
    /// The top-layer boxes a host last reported open, which the holds while
    /// displayed read (`crate::wakelock::displayed`).
    pub(crate) fn top_layer_open(&self) -> &crate::geometry::Scrolled {
        &self.scrolled
    }

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
    /// the host makes them to touch. None while no presentation is open. Each
    /// optional viewport clip bounds point reads and visibility without
    /// rescaling the box or its descendants (LLP 1051.000 D1).
    pub fn top_layer_frames(&mut self, frames: &[TopLayerFrame], modal: Option<usize>) {
        self.scrolled.top_layer_frames(&self.kernel, frames, modal);
    }
}

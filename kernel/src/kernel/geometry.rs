//! Geometry reads for actions (LLP 1051.000 D1, D3, D5): where layout put a
//! node, and how tall it would be at `height: auto`. Neither writes the
//! arena, a frame, the epoch or a receipt.

use super::{engine, Kernel};
use crate::generated::NodeType;
use crate::id::{Frame, NodeFlags, NodeKey};
use crate::layout::{LayoutMirror, LayoutTree};
use crate::style::taffy_style;

impl Kernel {
    /// Where layout put a node, as last laid out: its published border box in
    /// its root's space, free of transforms and scrolling, and whether it is
    /// provisional (an image or video in its subtree has no natural size yet,
    /// so lays out at zero until it loads). A node changed since keeps its
    /// box until the next layout: a read in a dispatch sees the layout last
    /// shown, not the dispatch's own writes, as the web's page does (LLP
    /// 1051.000 D1). `None` when the node is not live, is an inline run, has
    /// not been laid out since it was created (so on a kernel that doesn't
    /// lay out, the browser's), is in no root's layout or was under
    /// `display: none` when last laid out.
    pub fn laid_out_frame(&self, key: NodeKey) -> Option<(Frame, bool)> {
        let slot = self.arena.resolve(key)?;
        if self.arena.is_inline_run(slot) || self.arena.flags(slot).has(NodeFlags::CREATED) {
            return None;
        }
        self.root_above(slot)?;
        Some((self.arena.frame(slot), self.unsettled(slot)))
    }

    /// A node's border box as if its `height` were `auto`, with presented
    /// heights removed and every other style kept, under the offer its root
    /// was last laid out at. The engine tree takes the derived style, lays
    /// out without publishing and gets its styles back, as
    /// [`Kernel::measure_height_targets`] does, then lays the path out again
    /// with them, so the engine is left clean and matching what is published.
    /// The origin is the published one. `None` as for
    /// [`Kernel::laid_out_frame`], while a content region is registered
    /// (ordinary layout is refused then), and while anything under the root
    /// has changed since the last layout: the what-if would see the change,
    /// which the layout last shown doesn't have (D1).
    pub fn measure_auto_height(&mut self, key: NodeKey) -> Option<(Frame, bool)> {
        if self.region.is_some() {
            return None;
        }
        let (published, provisional) = self.laid_out_frame(key)?;
        let root_slot = self.root_above(key.index)?;
        let root = self.arena.taffy(root_slot)?;
        let node = self.arena.taffy(key.index)?;
        let tree = self.layout.as_deref().and_then(LayoutMirror::tree_ref)?;
        if tree.is_dirty(root) {
            return None;
        }
        let offer = engine(&mut self.layout).last_offer(root)?;
        let previous = engine(&mut self.layout).height_samples(self.epoch);
        engine(&mut self.layout).present_heights(&self.arena, &[]);
        let mut auto = taffy_style(&self.arena, key.index);
        auto.size.height = taffy::style::Dimension::auto();
        engine(&mut self.layout).set_style(node, auto);
        let measured = engine(&mut self.layout)
            .compute(root, offer, &self.arena, self.measurer.as_mut())
            .map(|()| engine(&mut self.layout).layout(node).size);
        if measured.is_err() {
            // A contained invalid metric may have left a zero in Taffy's
            // cache: rebuild from the columns, as measure_height_targets does.
            self.layout = Some(Box::new(LayoutTree::rebuild(&mut self.arena)));
        } else {
            engine(&mut self.layout).set_style(node, taffy_style(&self.arena, key.index));
        }
        engine(&mut self.layout).present_heights(&self.arena, &previous);
        // The committed styles again, laid out again: the same inputs as the
        // published layout, so the same engine layout, and nothing dirty.
        let root = self.arena.taffy(root_slot)?;
        let settled =
            engine(&mut self.layout).compute(root, offer, &self.arena, self.measurer.as_mut());
        if settled.is_err() {
            self.layout = Some(Box::new(LayoutTree::rebuild(&mut self.arena)));
            engine(&mut self.layout).present_heights(&self.arena, &previous);
        }
        let size = measured.ok()?;
        (size.width.is_finite() && size.height.is_finite() && size.height >= 0.0).then_some((
            Frame {
                x: published.x,
                y: published.y,
                width: size.width,
                height: size.height,
            },
            provisional,
        ))
    }

    /// The root `slot` is laid out under, when nothing between them was
    /// `display: none` when last laid out (a batch since may have changed a
    /// `display`; the layout last shown hasn't).
    fn root_above(&self, slot: u32) -> Option<u32> {
        let mut top = slot;
        let mut at = Some(slot);
        while let Some(s) = at {
            if self.arena.flags(s).has(NodeFlags::HIDDEN) {
                return None;
            }
            top = s;
            at = self.arena.parent(s);
        }
        self.arena.is_root(top).then_some(top)
    }

    /// Whether an image or video under `slot` still waits for its natural
    /// size, so the box may change when it loads (LLP 1051.000 D5). An
    /// `audio` has none to wait for (LLP 1042 §8).
    fn unsettled(&self, slot: u32) -> bool {
        self.arena.subtree(slot).into_iter().any(|s| {
            matches!(self.arena.node_type(s), NodeType::Image | NodeType::Video)
                && self.arena.intrinsic(s).is_none()
                && self.arena.props(s).str(crate::PropId::SemanticTag) != Some("audio")
        })
    }
}

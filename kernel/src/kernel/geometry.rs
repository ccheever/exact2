//! Geometry reads for actions (LLP 1051.000 D1, D3, D5): where layout put a
//! node, and how tall it would be at `height: auto`. Neither writes the
//! arena, a frame, the epoch or a receipt.

use super::{engine, Kernel};
use crate::generated::NodeType;
use crate::id::{AxisOffer, Frame, NodeFlags, NodeKey, Offer};
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

    /// A node's border-box height when its own height is left to its
    /// content at its laid-out width: CSS's `fit-content` block size, in an
    /// indefinite height. Its subtree is laid out alone, in a separate
    /// engine tree, with its published width, its padding and border as the
    /// last layout resolved them (a percentage of its containing block, a
    /// host cover in it), its min and max heights, and no height, insets or
    /// margins: nothing it is placed in (a sheet, a flex line, insets)
    /// constrains it, so no child shrinks, grows or takes a percentage of
    /// its height. The viewport is the box's own sheet and the kernel knows
    /// no unclipped screen, so the trial is lowered and measured, native
    /// fields and buttons included, with every length resolving without
    /// the viewport's height (`Dimension::without_viewport_height`): a
    /// height term loses to a cap or floor beside it, and a length resting
    /// on one is unknown (`auto`, no padding, no margin). Where its own
    /// width or horizontal edges read the height, the box fills its
    /// parent's width instead of its published one. A height a transition presents (LLP 1063) is the
    /// presented one, as in the ordinary layout. Exclusions and
    /// multi-column fragments are not settled in that tree. The ordinary engine tree, its caches, frames and the epoch
    /// are untouched (LLP 1075.003 §9.11). `None` as for
    /// [`Kernel::laid_out_frame`], or when the trial's layout fails.
    pub fn fit_content_height(&mut self, key: NodeKey) -> Option<f32> {
        use taffy::style::{Dimension, LengthPercentage, LengthPercentageAuto};
        let (frame, _) = self.laid_out_frame(key)?;
        let slot = key.index;
        let laid = self
            .layout
            .as_deref()
            .and_then(LayoutMirror::tree_ref)?
            .layout(self.arena.taffy(slot)?);
        let (pad, border) = (laid.padding, laid.border);
        let env = self.arena.env().clone();
        let reads = |d: crate::style::Dimension| d.without_viewport_height(&env).is_some();
        let unknown = |d: crate::style::Dimension| {
            d.without_viewport_height(&env) == Some(crate::Dimension::Auto)
        };
        let presented = engine(&mut self.layout).height_samples(self.epoch);
        // Lowered and measured without the viewport's height: lengths that
        // read it lose it (`Dimension::without_viewport_height`).
        let _heightless = crate::style::viewport::Heightless::enter();
        let (mut tree, nodes) = LayoutTree::of_subtree(&self.arena, slot);
        let root = nodes[&slot];
        // A margin resting on the sheet is none, not `auto`, which would
        // take free space; a height a transition presents is the presented
        // one, as in the ordinary layout.
        let derive = |s: u32, mut t: taffy::style::Style| {
            let row = self.arena.style(s);
            let zero = LengthPercentageAuto::length(0.0);
            for (d, m) in [
                (row.margin_top, &mut t.margin.top),
                (row.margin_right, &mut t.margin.right),
                (row.margin_bottom, &mut t.margin.bottom),
                (row.margin_left, &mut t.margin.left),
            ] {
                if unknown(d) {
                    *m = zero;
                }
            }
            let shown = presented
                .iter()
                .find(|p| p.node.index == s && self.arena.resolve(p.node) == Some(s));
            if let Some(p) = shown {
                t.size.height = Dimension::length(p.px);
            }
            t
        };
        for (&s, &node) in nodes.iter().filter(|(&s, _)| s != slot) {
            let row = self.arena.style(s);
            let margins = [
                row.margin_top,
                row.margin_right,
                row.margin_bottom,
                row.margin_left,
            ];
            if margins.into_iter().any(unknown) || presented.iter().any(|p| p.node.index == s) {
                tree.set_style(node, derive(s, taffy_style(&self.arena, s)));
            }
        }
        let mut style = taffy_style(&self.arena, slot);
        // Its own padding is the last layout's (a percentage of its
        // containing block) but where that read the sheet: then this
        // lowering's, a covered edge's points in it (§3.5).
        let row = self.arena.style(slot);
        let mut pad = pad;
        let lowered = |e: LengthPercentage| {
            let raw = e.into_raw();
            if raw.tag() == taffy::CompactLength::LENGTH_TAG {
                raw.value()
            } else {
                0.0
            }
        };
        for (d, edge, now) in [
            (row.padding_top, &mut pad.top, style.padding.top),
            (row.padding_right, &mut pad.right, style.padding.right),
            (row.padding_bottom, &mut pad.bottom, style.padding.bottom),
            (row.padding_left, &mut pad.left, style.padding.left),
        ] {
            if reads(d) {
                *edge = lowered(now);
            }
        }
        // A published width that read the sheet is not its width here: the
        // root then fills its parent's width, as a block does.
        let width_reads = [
            row.width,
            row.min_width,
            row.max_width,
            row.left,
            row.right,
            row.margin_left,
            row.margin_right,
            row.padding_left,
            row.padding_right,
        ]
        .into_iter()
        .any(reads);
        let available = match self.arena.parent(slot) {
            Some(p) if width_reads => self.arena.frame(p).width,
            _ => frame.width,
        };
        let inline = pad.left + pad.right + border.left + border.right;
        let width = match style.box_sizing {
            taffy::style::BoxSizing::ContentBox => (frame.width - inline).max(0.0),
            taffy::style::BoxSizing::BorderBox => frame.width,
        };
        style.size = taffy::geometry::Size {
            width: if width_reads {
                Dimension::auto()
            } else {
                Dimension::length(width)
            },
            height: Dimension::auto(),
        };
        if !width_reads {
            style.min_size.width = LengthPercentageAuto::auto();
            style.max_size.width = LengthPercentageAuto::auto();
        }
        let lp = LengthPercentage::length;
        style.padding = taffy::geometry::Rect {
            left: lp(pad.left),
            right: lp(pad.right),
            top: lp(pad.top),
            bottom: lp(pad.bottom),
        };
        style.border = taffy::geometry::Rect {
            left: lp(border.left),
            right: lp(border.right),
            top: lp(border.top),
            bottom: lp(border.bottom),
        };
        style.margin = taffy::geometry::Rect {
            left: LengthPercentageAuto::length(0.0),
            right: LengthPercentageAuto::length(0.0),
            top: LengthPercentageAuto::length(0.0),
            bottom: LengthPercentageAuto::length(0.0),
        };
        style.inset = taffy::geometry::Rect {
            left: LengthPercentageAuto::auto(),
            right: LengthPercentageAuto::auto(),
            top: LengthPercentageAuto::auto(),
            bottom: LengthPercentageAuto::auto(),
        };
        // The containing block of what it positions, as a root is.
        style.position = taffy::style::Position::Relative;
        tree.set_style(root, style);
        let offer = Offer {
            width: AxisOffer::Definite(available),
            height: AxisOffer::MaxContent,
        };
        tree.compute_mapped(root, offer, &self.arena, self.measurer.as_mut(), &|s| {
            nodes.get(&s).copied()
        })
        .ok()?;
        let height = tree.layout(root).size.height;
        (height.is_finite() && height >= 0.0).then_some(height)
    }

    /// The padding the last layout resolved, in points: left, top, right,
    /// bottom. A percentage is of the containing block's width, which in a
    /// multi-column container is the column's (CSS Multi-column §3.4).
    /// `None` when the node has no engine layout.
    pub fn resolved_padding(&self, key: NodeKey) -> Option<(f32, f32, f32, f32)> {
        let slot = self.arena.resolve(key)?;
        let node = self.arena.taffy(slot)?;
        let pad = self.layout.as_deref()?.tree_ref()?.layout(node).padding;
        Some((pad.left, pad.top, pad.right, pad.bottom))
    }

    /// The border the last layout resolved, in points: left, top, right,
    /// bottom. `None` when the node has no engine layout.
    pub fn resolved_border(&self, key: NodeKey) -> Option<(f32, f32, f32, f32)> {
        let slot = self.arena.resolve(key)?;
        let node = self.arena.taffy(slot)?;
        let b = self.layout.as_deref()?.tree_ref()?.layout(node).border;
        Some((b.left, b.top, b.right, b.bottom))
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

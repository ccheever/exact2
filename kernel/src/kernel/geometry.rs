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
    /// content: CSS's `fit-content` block size, in an indefinite height.
    /// Its subtree is laid out alone, in a separate engine tree, inside a
    /// stand-in for its parent: the parent's style at its published width,
    /// its padding and border as the last layout resolved them, and no
    /// height. The box keeps its own style but its height and its top and
    /// bottom insets, so its width is derived as the ordinary layout
    /// derives it (insets, percentages of its containing block, ratio,
    /// limits) with its block size indefinite, never imported from a width
    /// that may have followed the sheet; nothing it is placed in (a sheet, a
    /// flex line, insets) constrains its height, so no child shrinks, grows
    /// or takes a percentage of its height. In a flex row or a grid the
    /// box's in-flow siblings come too, since they set its width; the box
    /// is aligned to the line's start, so none stretches its height. Viewport
    /// lengths resolve as in the ordinary layout, every viewport unit against
    /// the window ([`crate::Env::screen`]), so the measure and the layout
    /// agree and neither reads the sheet. A height a transition presents (LLP 1063)
    /// is the presented one, as in the ordinary layout. Exclusions and
    /// multi-column fragments are not settled in that tree. The ordinary
    /// engine tree, its caches, frames and the epoch are untouched (LLP
    /// 1075.003 §9.11), but for the symbols owed a measurement, which are
    /// measured first, as the next layout would (`measure_symbols`). `None`
    /// as for [`Kernel::laid_out_frame`], or when the trial's layout fails.
    pub fn fit_content_height(&mut self, key: NodeKey) -> Option<f32> {
        use taffy::style::{Dimension, LengthPercentage, LengthPercentageAuto};
        let (frame, _) = self.laid_out_frame(key)?;
        self.measure_symbols(false);
        let slot = key.index;
        // A block's root stands in its block (LLP 1115.002 D8.2): the block's
        // width is what it is measured in, with no parent and no height, so
        // a fitted sheet whose route has its block is measured as it wraps
        // there (LLP 1123 D2, D7).
        let block = self.arena.blocks.get(slot);
        let tree_ref = self.layout.as_deref().and_then(LayoutMirror::tree_ref)?;
        let parent = if block.is_some() {
            None
        } else {
            self.arena.parent(slot)
        };
        let holder = parent.map(|p| (p, self.arena.taffy(p).map(|n| tree_ref.layout(n))));
        // Where the parent shares its inline axis among its children (a flex
        // row, a grid), the in-flow siblings come too: they set the box's
        // width (a fixed sibling beside a `flex: 1` box). In a block or a
        // column they set nothing of it and stay out.
        let mut roots = vec![slot];
        let mut shares = false;
        if let Some(p) = parent {
            let s = self.arena.style(p);
            shares = match s.display {
                crate::Display::Flex => matches!(
                    s.flex_direction,
                    crate::FlexDirection::Row | crate::FlexDirection::RowReverse
                ),
                crate::Display::Grid => true,
                _ => false,
            };
            if shares {
                roots = self
                    .arena
                    .children(p)
                    .iter()
                    .copied()
                    .filter(|&c| {
                        let cs = self.arena.style(c);
                        c == slot
                            || (cs.display != crate::Display::None
                                && crate::style::top_layer::layout_position(&self.arena, c)
                                    != crate::PositionType::Absolute)
                    })
                    .collect();
            }
        }
        let (mut tree, nodes) = LayoutTree::of_subtrees(&self.arena, &roots);
        let root = nodes[&slot];
        let presented = engine(&mut self.layout).height_samples(self.epoch);
        let derive = |s: u32, mut t: taffy::style::Style| {
            // A scroller the platform insets fits by its content alone: the
            // detent leaves those insets to UIKit (LLP 1116.000 D1.2).
            super::cover::without_inset(&self.arena, s, &mut t);
            let shown = presented
                .iter()
                .find(|p| p.node.index == s && self.arena.resolve(p.node) == Some(s));
            if let Some(p) = shown {
                t.size.height = Dimension::length(p.px);
            }
            t
        };
        for (&s, &node) in nodes.iter().filter(|(&s, _)| s != slot) {
            if presented.iter().any(|p| p.node.index == s) || super::cover::inset(&self.arena, s) {
                tree.set_style(node, derive(s, taffy_style(&self.arena, s)));
            }
        }
        let mut style = derive(slot, taffy_style(&self.arena, slot));
        style.size.height = Dimension::auto();
        style.inset.top = LengthPercentageAuto::auto();
        style.inset.bottom = LengthPercentageAuto::auto();
        // The siblings set its width alone: aligned to the start of the
        // line, it takes its own height, not a taller sibling's stretch
        // (Astra's and Grok's review of 1a9b86776).
        if shares {
            style.align_self = Some(taffy::style::AlignSelf::START);
        }
        tree.set_style(root, style);
        // The parent, as the containing block: its published width, its
        // resolved edges, no height; positioned, so it holds what the box
        // positions. Neither reads the sheet's height: the viewport height
        // units are the window's (`Env::screen`), and a percentage of the
        // parent's own height is not its width.
        let lp = LengthPercentage::length;
        let mut outer = match holder {
            Some((p, _)) => taffy_style(&self.arena, p),
            None => taffy::style::Style::default(),
        };
        let width = match block {
            Some(b) => b.width.length(),
            None => holder.map_or(frame.width, |(p, _)| self.arena.frame(p).width),
        };
        if let Some((_, Some(laid))) = holder {
            let (pad, border) = (laid.padding, laid.border);
            outer.padding = taffy::geometry::Rect {
                left: lp(pad.left),
                right: lp(pad.right),
                top: lp(pad.top),
                bottom: lp(pad.bottom),
            };
            outer.border = taffy::geometry::Rect {
                left: lp(border.left),
                right: lp(border.right),
                top: lp(border.top),
                bottom: lp(border.bottom),
            };
        }
        let width = width.max(0.0);
        outer.box_sizing = taffy::style::BoxSizing::BorderBox;
        outer.size = taffy::geometry::Size {
            width: Dimension::length(width),
            height: Dimension::auto(),
        };
        outer.min_size = taffy::geometry::Size {
            width: LengthPercentageAuto::auto(),
            height: LengthPercentageAuto::auto(),
        };
        outer.max_size = outer.min_size;
        outer.margin = taffy::geometry::Rect {
            left: LengthPercentageAuto::length(0.0),
            right: LengthPercentageAuto::length(0.0),
            top: LengthPercentageAuto::length(0.0),
            bottom: LengthPercentageAuto::length(0.0),
        };
        outer.inset = taffy::geometry::Rect {
            left: LengthPercentageAuto::auto(),
            right: LengthPercentageAuto::auto(),
            top: LengthPercentageAuto::auto(),
            bottom: LengthPercentageAuto::auto(),
        };
        outer.position = taffy::style::Position::Relative;
        let holder_node = tree.new_leaf(outer, parent.unwrap_or(slot), false);
        match parent {
            Some(p) => tree.adopt(&self.arena, p, holder_node, &nodes),
            None => tree.set_children(holder_node, &[root]),
        }
        let offer = Offer {
            width: AxisOffer::Definite(width),
            height: AxisOffer::MaxContent,
        };
        tree.compute_mapped(
            holder_node,
            offer,
            &self.arena,
            self.measurer.as_mut(),
            &|s| nodes.get(&s).copied(),
        )
        .ok()?;
        let height = tree.layout(root).size.height;
        (height.is_finite() && height >= 0.0).then_some(height)
    }

    /// The margins the last layout resolved around `key`'s border box (left,
    /// top, right, bottom), and whether that box's width is the author's (a
    /// `width` other than `auto`, 1) or its content's (0), and its resolved
    /// minimum border-box width.
    /// What a Live Activity placement's region takes around a root the bake
    /// positions absolutely (LLP 1126.001 WP10): CSS 2.1 §10.3.7 gave the
    /// root its shrink-to-fit width in that layout, and the driver draws the
    /// same layout. A read, as [`Kernel::laid_out_frame`] is: it lays nothing
    /// out, and nothing on the ordinary layout path calls it. `None` as for
    /// `laid_out_frame`.
    pub fn placement_box(&self, key: NodeKey) -> Option<[f32; 6]> {
        self.laid_out_frame(key)?;
        let slot = self.arena.resolve(key)?;
        let tree_ref = self.layout.as_deref().and_then(LayoutMirror::tree_ref)?;
        let laid = tree_ref.layout(self.arena.taffy(slot)?);
        let style = self.arena.style(slot);
        let authored = style.width != crate::Dimension::Auto;
        let basis = self
            .arena
            .parent(slot)
            .and_then(|p| self.arena.taffy(p))
            .map_or(0.0, |p| {
                let parent = tree_ref.layout(p);
                parent.size.width - parent.border.left - parent.border.right
            });
        let env = self.arena.env_for(slot);
        let mut minimum = style.min_width.at_basis(env, basis).max(0.0);
        if style.box_sizing == crate::generated::BoxSizing::ContentBox {
            let (left, _, right, _) = self.resolved_padding(key).unwrap_or_default();
            let [_, br, _, bl] = style.border_widths_in(env);
            minimum += left + right + bl + br;
        }
        let box_ = [
            laid.margin.left,
            laid.margin.top,
            laid.margin.right,
            laid.margin.bottom,
            if authored { 1.0 } else { 0.0 },
            minimum,
        ];
        box_.iter().all(|v| v.is_finite()).then_some(box_)
    }

    /// How far down its content box a `text` that is a flex container draws
    /// its lines: its text is one anonymous flex item (CSS Flexbox §4),
    /// placed by `align-items` in a row (its cross ends swapped under
    /// `wrap-reverse`) and by `justify-content` in a column. Only the
    /// vertical place moves: the width its lines wrap in stays the box's.
    /// Zero for any other node, and for a text fragmented across columns,
    /// whose lines its fragments place.
    pub fn text_line_offset(&self, node: &crate::NodeRef<'_>) -> f32 {
        use crate::generated::{
            AlignContent, AlignItems, Display, FlexDirection, FlexWrap, JustifyContent, Overflow,
        };
        let style = node.style;
        if node.node_type != NodeType::Text
            || style.display != Display::Flex
            || self.fragments(node.key).is_some()
        {
            return 0.0;
        }
        let (_, top, _, bottom) = self.resolved_padding(node.key).unwrap_or_default();
        let [border_top, _, border_bottom, _] =
            style.border_widths_in(self.arena.env_for(node.key.index));
        let room = node.frame.height - top - bottom - border_top - border_bottom;
        // The measured extent counts the top padding before the lines, and
        // the bottom one after them in a box that clips or scrolls (Taffy's leaf).
        let clips = style.overflow_x != Overflow::Visible || style.overflow_y != Overflow::Visible;
        let lines = node.content.1 - top - if clips { bottom } else { 0.0 };
        let free = (room - lines).max(0.0);
        let reverse = style.flex_wrap == FlexWrap::WrapReverse;
        let (flex_start, flex_end) = if reverse { (free, 0.0) } else { (0.0, free) };
        match style.flex_direction {
            FlexDirection::Row | FlexDirection::RowReverse => {
                // A wrapping container's one line is placed by `align-content`,
                // the item in it (as tall as it) by nothing more.
                if style.flex_wrap != FlexWrap::Nowrap {
                    match style.align_content {
                        AlignContent::Normal | AlignContent::Stretch => {}
                        AlignContent::Center
                        | AlignContent::SpaceAround
                        | AlignContent::SpaceEvenly => return free / 2.0,
                        AlignContent::End => return free,
                        AlignContent::FlexEnd => return flex_end,
                        AlignContent::FlexStart | AlignContent::SpaceBetween => return flex_start,
                        AlignContent::Start => return 0.0,
                    }
                }
                match style.align_items {
                    AlignItems::Center => free / 2.0,
                    AlignItems::End | AlignItems::SelfEnd => free,
                    AlignItems::FlexEnd => flex_end,
                    AlignItems::FlexStart => flex_start,
                    _ => 0.0,
                }
            }
            direction => {
                let reversed = direction == FlexDirection::ColumnReverse;
                match style.justify_content {
                    JustifyContent::Center
                    | JustifyContent::SpaceAround
                    | JustifyContent::SpaceEvenly => free / 2.0,
                    JustifyContent::End => free,
                    JustifyContent::FlexEnd if !reversed => free,
                    JustifyContent::FlexStart
                    | JustifyContent::Normal
                    | JustifyContent::SpaceBetween
                        if reversed =>
                    {
                        free
                    }
                    _ => 0.0,
                }
            }
        }
    }

    /// The CSS padding the last layout resolved, in points: left, top,
    /// right, bottom. A percentage is of the containing block's width, which
    /// in a multi-column container is the column's (CSS Multi-column §3.4).
    /// A host cover's part is not in it (`layout_padding`). Whichever engine
    /// laid the node out publishes it, a content region's own included;
    /// `None` before any layout or for a node it gave no box (an inline run).
    pub fn resolved_padding(&self, key: NodeKey) -> Option<(f32, f32, f32, f32)> {
        let [left, top, right, bottom] = self.arena.inset(self.arena.resolve(key)?)?.padding;
        Some((left, top, right, bottom))
    }

    /// A node's margins as its layout reads them under the kernel's
    /// environment, top/right/bottom/left in points: whole cells under a
    /// terminal's space rule (LLP 1121 stage 11 (c)), a percentage none.
    pub fn laid_margins(&self, id: crate::ViewId) -> Option<[f32; 4]> {
        use crate::style::cells::{COLUMN, ROW};
        let node = self.node(id)?;
        let (env, s) = (self.arena.env_for(node.key.index), node.style);
        let [t, r, b, l] = [
            (s.margin_top, ROW),
            (s.margin_right, COLUMN),
            (s.margin_bottom, ROW),
            (s.margin_left, COLUMN),
        ]
        .map(|(d, cell)| match d.spaced(env, cell) {
            d if d.has_percent() => 0.0,
            d => d.at_basis(env, 0.0),
        });
        Some([t, r, b, l])
    }

    /// The padding the engine laid the box out with: `resolved_padding` and
    /// what a host cover added to it (LLP 1075.003 §3.5).
    pub fn layout_padding(&self, key: NodeKey) -> Option<(f32, f32, f32, f32)> {
        let inset = self.arena.inset(self.arena.resolve(key)?)?;
        let [left, top, right, bottom] = std::array::from_fn(|i| inset.padding[i] + inset.cover[i]);
        Some((left, top, right, bottom))
    }

    /// The border the last layout resolved, in points: left, top, right,
    /// bottom, the engine's, which holds a native field's chrome too.
    pub fn resolved_border(&self, key: NodeKey) -> Option<(f32, f32, f32, f32)> {
        let [left, top, right, bottom] = self.arena.inset(self.arena.resolve(key)?)?.border;
        Some((left, top, right, bottom))
    }

    /// The root `slot` is laid out under, when nothing between them was
    /// `display: none` when last laid out (a batch since may have changed a
    /// `display`; the layout last shown hasn't). A subtree a host gave a
    /// containing block is laid out from the node that has it (LLP
    /// 1115.002 D8.2), whose flag says whether anything above hid it.
    fn root_above(&self, slot: u32) -> Option<u32> {
        let mut top = slot;
        let mut at = Some(slot);
        while let Some(s) = at {
            if self.arena.flags(s).has(NodeFlags::HIDDEN) {
                return None;
            }
            top = s;
            if self.arena.blocks.holds(s) {
                while let Some(above) = self.arena.parent(top) {
                    top = above;
                }
                return self.arena.is_root(top).then_some(s);
            }
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

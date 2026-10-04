//! `position: sticky` (LLP 1083): what a host needs to place a sticky box as
//! its scroller scrolls. Layout places the box as a relative one with no
//! offset (`style.rs`); each frame the host scrolls, it shifts the box by
//! [`StickyConstraint::offset`], as a browser does (CSS Positioned Layout 3
//! §3.4). The web's page does it itself; a native host asks this.

use super::Kernel;
use crate::generated::{Overflow, PositionType};
use crate::id::{NodeKey, ViewId};
use crate::layout::LayoutMirror;
use crate::style::Dimension;

/// One sticky box's constraint, every rectangle in its scroller's
/// border-box space at scroll offset zero (where the scroller's content
/// starts), as `[left, top, right, bottom]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StickyConstraint {
    /// The nearest ancestor that is a scroll container (`overflow` other
    /// than `visible` on either axis), else the root, whose viewport the
    /// page scrolls.
    pub scroller: ViewId,
    /// The box's border box where layout put it.
    pub natural: [f32; 4],
    /// Where its border box may go: its containing block's content box, less
    /// its own margins.
    pub limit: [f32; 4],
    /// The scrollport at offset zero, less the scroller's padding: Chrome
    /// keeps a sticky box inside its scroller's content box.
    pub port: [f32; 4],
    /// `top`, `right`, `bottom`, `left` in points; `None` where `auto`.
    /// Percentages are of [`port`](Self::port)'s height or width.
    pub insets: [Option<f32>; 4],
}

impl StickyConstraint {
    /// How far to move the box from where layout put it, with its scroller
    /// scrolled to `scroll`. Each axis is Chrome's (`StickyPositionScrolling
    /// Constraints::ComputeStickyOffset`): the end inset pulls the box back
    /// inside the scrollport, never before its limit's start; the start
    /// inset then pushes it, never past its limit's end, so it wins where
    /// both cannot hold.
    pub fn offset(&self, scroll: (f32, f32)) -> (f32, f32) {
        let [top, right, bottom, left] = self.insets;
        let axis = |scroll: f32, start: Option<f32>, end: Option<f32>, i: usize| {
            let (lo, hi) = (self.natural[i], self.natural[i + 2]);
            let mut d = 0.0;
            if let Some(inset) = end {
                let pull =
                    (self.port[i + 2] + scroll - inset - hi).max((self.limit[i] - lo).min(0.0));
                if pull < 0.0 {
                    d += pull;
                }
            }
            if let Some(inset) = start {
                let push =
                    (self.port[i] + scroll + inset - lo).min((self.limit[i + 2] - hi).max(0.0));
                if push > 0.0 {
                    d += push;
                }
            }
            d
        };
        (
            axis(scroll.0, left, right, 0),
            axis(scroll.1, top, bottom, 1),
        )
    }
}

fn length(d: Dimension, basis: f32, env: &crate::style::Env) -> Option<f32> {
    match d.resolve(env) {
        Dimension::Points(p) => Some(p),
        Dimension::Percent(p) => Some(basis * p / 100.0),
        Dimension::Calc(p, pt) => Some(basis * p / 100.0 + pt),
        _ => None,
    }
}

impl Kernel {
    /// Every live `position: sticky` node, laid out or not.
    pub fn sticky_nodes(&self) -> Vec<NodeKey> {
        self.arena
            .sticky_slots
            .iter()
            .map(|slot| self.arena.key(slot))
            .collect()
    }

    /// A sticky node's constraint as last laid out. `None` when the node is
    /// not sticky, not live, not laid out, or under `display: none`.
    pub fn sticky_constraint(&self, key: NodeKey) -> Option<StickyConstraint> {
        let arena = &self.arena;
        let slot = arena.resolve(key)?;
        let style = arena.style(slot);
        if style.position_type != PositionType::Sticky {
            return None;
        }
        self.laid_out_frame(key)?;
        let tree = self.layout.as_deref().and_then(LayoutMirror::tree_ref)?;
        let parent = arena.parent(slot)?;
        let mut scroller = parent;
        while !arena.is_root(scroller) && !scrolls(arena, scroller) {
            scroller = arena.parent(scroller)?;
        }
        let origin = arena.frame(scroller);
        let at = |slot: u32| {
            let f = arena.frame(slot);
            [
                f.x - origin.x,
                f.y - origin.y,
                f.x - origin.x + f.width,
                f.y - origin.y + f.height,
            ]
        };
        let content = |slot: u32| {
            let l = tree.layout(arena.taffy(slot)?);
            let [x0, y0, x1, y1] = at(slot);
            let (p, b) = (l.padding, l.border);
            Some([
                x0 + b.left + p.left,
                y0 + b.top + p.top,
                x1 - b.right - p.right,
                y1 - b.bottom - p.bottom,
            ])
        };
        let margin = tree.layout(arena.taffy(slot)?).margin;
        let cb = content(parent)?;
        let port = content(scroller)?;
        let (w, h) = (port[2] - port[0], port[3] - port[1]);
        let env = arena.env();
        Some(StickyConstraint {
            scroller: arena.local_id(scroller),
            natural: at(slot),
            limit: [
                cb[0] + margin.left,
                cb[1] + margin.top,
                cb[2] - margin.right,
                cb[3] - margin.bottom,
            ],
            port,
            insets: [
                length(style.top, h, env),
                length(style.right, w, env),
                length(style.bottom, h, env),
                length(style.left, w, env),
            ],
        })
    }
}

fn scrolls(arena: &crate::arena::NodeArena, slot: u32) -> bool {
    let s = arena.style(slot);
    s.overflow_x != Overflow::Visible
        || s.overflow_y != Overflow::Visible
        || (!s.mask.has(crate::generated::StyleId::OverflowY)
            && arena.node_type(slot).scrolls_by_default())
}

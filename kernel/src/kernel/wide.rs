//! The kernel as a holder of wide colours (LLP 1121 §5 stage 4): its mark,
//! published after every change to what it holds.

use super::Kernel;
use crate::layout::LayoutMirror;

impl Kernel {
    /// The wide colours this kernel holds: every row of every live node,
    /// the styles it keeps for reuse, its region's and its layout's.
    pub fn wide_mark(&mut self) -> crate::style::wide::WideMark {
        let mut m = crate::style::wide::WideMark::default();
        self.arena.wide_colors(&mut m);
        if let Some(region) = &self.region {
            region.wide_colors(&mut m);
        }
        if let Some(tree) = self.layout.as_deref().and_then(LayoutMirror::tree_ref) {
            tree.wide_colors(&mut m);
        }
        m
    }

    /// After a change: publish this kernel's mark, a superset of the wide
    /// colours it holds now (none to visit while the table holds none).
    pub(crate) fn wide_changed(&mut self) {
        let mark = if crate::style::wide::Holder::needed() {
            self.wide_mark()
        } else {
            crate::style::wide::WideMark::default()
        };
        self.publish(mark);
    }

    /// With its mark published now: what it holds from birth.
    pub(super) fn published(mut self) -> Kernel {
        let mark = self.wide_mark();
        self.publish(mark);
        self
    }

    /// Publish `mark`; an arena cloned into this kernel needs its own keep
    /// no longer.
    fn publish(&mut self, mark: crate::style::wide::WideMark) {
        self.wide.publish(mark);
        self.arena.kept = crate::style::wide::Keep::default();
    }
}

//! Live platform defaults below authored rows. @ref LLP 1115.000 D2, D4.13, D17.
use super::*;
use crate::style::metrics::{self, Column};
use std::borrow::Cow;

static HEADER: std::sync::LazyLock<StyleProps> = std::sync::LazyLock::new(|| {
    let mut style = StyleProps::default();
    for (id, name) in [
        (StyleId::BackgroundColor, "-exact-header-ground"),
        (StyleId::TextColor, "-exact-header-color"),
        (StyleId::BorderWidthBottom, "-exact-header-edge-width"),
        (StyleId::BorderStyleBottom, "-exact-header-edge-style"),
        (StyleId::BorderColorBottom, "-exact-header-edge-color"),
    ] {
        let metric = metrics::named(name).expect("schema header metric");
        let value = metrics::as_written(id, metric.neutral).expect("neutral header row");
        style.set_dynamic(id, &value).expect("schema header value");
    }
    style
});

impl NodeArena {
    pub(crate) fn is_neutral_header(&self, slot: u32) -> bool {
        metrics::column() == Column::Neutral
            && self.props(slot).str(PropId::SemanticTag) == Some("header")
            && self
                .parent(slot)
                .is_some_and(|parent| self.props(parent).get(PropId::NavigationKey).is_some())
    }

    pub(crate) fn inside_neutral_header(&self, slot: u32) -> bool {
        if metrics::column() != Column::Neutral {
            return false;
        }
        let mut parent = self.parent(slot);
        while let Some(p) = parent {
            if self.is_neutral_header(p) {
                return true;
            }
            parent = self.parent(p);
        }
        false
    }

    pub(crate) fn header_context(&self, slot: u32) -> (bool, bool) {
        (
            self.is_neutral_header(slot),
            self.inside_neutral_header(slot),
        )
    }

    pub(crate) fn header_rows() -> StyleMask {
        HEADER.mask
    }

    pub(crate) fn style_start(&self, slot: u32) -> Option<&StyleProps> {
        if self.is_neutral_header(slot) {
            Some(&HEADER)
        } else {
            self.control_text_start(slot)
        }
    }

    /// Effective own rows never alter stored ownership. Inherited text is
    /// resolved separately, including a written clear which bypasses a start.
    pub(crate) fn effective_style(&self, slot: u32) -> Cow<'_, StyleProps> {
        let own = self.style(slot);
        if self.is_neutral_header(slot) {
            let defaults = HEADER.mask.minus(own.mask.union(own.written));
            if !defaults.is_empty() {
                let mut style = own.clone();
                style.copy_rows(&HEADER, defaults);
                return Cow::Owned(style);
            }
        }
        Cow::Borrowed(own)
    }

    /// The slot whose own row supplies `id` for `slot`: the slot itself when
    /// it sets the row; for a row the schema marks inherited, the nearest
    /// logical ancestor that does; `None` when the initial value applies
    /// (CSS inheritance, LLP 1035.000 D1).
    pub fn inherited_source(&self, slot: u32, id: StyleId) -> Option<u32> {
        let mut cur = Some(slot);
        while let Some(s) = cur {
            if self.styles[s as usize].mask.has(id) {
                return Some(s);
            }
            if self
                .style_start(s)
                .is_some_and(|start| start.mask.has(id) && !self.styles[s as usize].written.has(id))
            {
                return None;
            }
            if !id.inherited() {
                return None;
            }
            cur = self.parents[s as usize];
        }
        None
    }

    /// The node's rows with the inherited rows in `rows` resolved through the
    /// logical ancestors: an own row stays; a missing inherited row takes the
    /// nearest ancestor's and is marked set; a row no ancestor sets keeps the
    /// initial value, unmarked. Rows outside `rows`, and rows the schema does
    /// not mark inherited, keep the effective own rows, including neutral
    /// header defaults below stored values and authored clears. One walk.
    pub fn computed_style(&self, slot: u32, rows: StyleMask) -> StyleProps {
        let mut out = self.effective_style(slot).into_owned();
        self.copy_inherited(slot, rows, |from, mask| out.copy_rows(from, mask));
        out
    }

    /// The style that supplies row `id` of [`NodeArena::computed_style`]:
    /// one row read without copying a whole style.
    pub fn computed_source(&self, slot: u32, id: StyleId) -> &StyleProps {
        let mut cur = Some(slot);
        while let Some(s) = cur {
            let own = &self.styles[s as usize];
            if own.mask.has(id) {
                return own;
            }
            if let Some(start) = self
                .style_start(s)
                .filter(|start| start.mask.has(id) && !own.written.has(id))
            {
                return start;
            }
            if !id.inherited() {
                return own;
            }
            cur = self.parents[s as usize];
        }
        if self.document_style.mask.has(id) {
            return &self.document_style;
        }
        &self.styles[slot as usize]
    }

    /// The computed text style without copying unrelated rows.
    pub fn text_style(&self, slot: u32) -> crate::text::TextStyle {
        crate::text::TextStyle::from_rows(&self.styles[slot as usize], |rows, take| {
            self.copy_inherited(slot, rows, take)
        })
    }

    /// Inherited values for comparison across a topology change.
    pub(crate) fn computed_inherited(&self, slot: u32) -> InheritedStyle {
        let mut out = InheritedStyle::new(&self.styles[slot as usize]);
        self.copy_inherited(slot, StyleMask::INHERITED, |from, mask| {
            out.copy_rows(from, mask)
        });
        out
    }

    pub(super) fn copy_inherited(
        &self,
        slot: u32,
        rows: StyleMask,
        mut copy: impl FnMut(&StyleProps, StyleMask),
    ) {
        let mut pending = rows
            .intersect(StyleMask::INHERITED)
            .minus(self.styles[slot as usize].mask);
        let mut cur = Some(slot);
        while !pending.is_empty() {
            let Some(p) = cur else { break };
            let ancestor: &StyleProps = &self.styles[p as usize];
            let found = pending.intersect(ancestor.mask);
            if !found.is_empty() {
                copy(ancestor, found);
                pending = pending.minus(found);
            }
            if let Some(start) = self.style_start(p) {
                let stopped = pending.intersect(start.mask).minus(ancestor.written);
                if !stopped.is_empty() {
                    copy(start, stopped);
                    pending = pending.minus(stopped);
                }
            }
            cur = self.parents[p as usize];
        }
        let found = pending.intersect(self.document_style.mask);
        if !found.is_empty() {
            copy(&self.document_style, found);
        }
    }
}

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
        (StyleId::MinHeight, "-exact-header-min-height"),
        (StyleId::PaddingTop, "-exact-header-padding-block"),
        (StyleId::PaddingBottom, "-exact-header-padding-block"),
        (StyleId::PaddingLeft, "-exact-header-padding-inline"),
        (StyleId::PaddingRight, "-exact-header-padding-inline"),
        (StyleId::PositionType, "-exact-header-position"),
        (StyleId::Top, "-exact-header-top"),
    ] {
        let metric = metrics::named(name).expect("schema header metric");
        let value = metrics::as_written(id, metric.neutral).expect("neutral header row");
        style.set_dynamic(id, &value).expect("schema header value");
    }
    style
});

static TAB_BAR: std::sync::LazyLock<StyleProps> = std::sync::LazyLock::new(|| {
    let role =
        |name| crate::ColorValue::Role(crate::style::roles::role(name).expect("schema role"));
    let mut style = StyleProps {
        background_color: Some(role("-exact-background")),
        border_width_top: metrics::length_in(
            crate::generated::metric::CONTROL_LINE,
            Column::Neutral,
            1.0,
            1.0,
        )
        .expect("neutral separator width"),
        border_style_top: crate::BorderStyle::Solid,
        ..StyleProps::default()
    };
    // Keep role identities: a static default must not capture one app's theme.
    style.border_color_top = Some(role("-exact-separator"));
    for id in [
        StyleId::BackgroundColor,
        StyleId::BorderWidthTop,
        StyleId::BorderStyleTop,
        StyleId::BorderColorTop,
    ] {
        style.mask.set(id);
    }
    style
});

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct DefaultContext {
    pub alert: u8,
    pub header: bool,
    pub inside_header: bool,
    pub tab_bar: bool,
    pub tab: bool,
    pub native_tab: bool,
}

impl NodeArena {
    pub(crate) fn refresh_sticky(&mut self, slot: u32) {
        if self
            .computed_source(slot, StyleId::PositionType)
            .position_type
            == crate::PositionType::Sticky
        {
            self.sticky_slots.insert(slot);
        } else {
            self.sticky_slots.remove(slot);
        }
    }

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

    pub(crate) fn is_neutral_tab_bar(&self, slot: u32) -> bool {
        metrics::column() == Column::Neutral
            && self.props(slot).str(PropId::AccessibilityRole) == Some("tablist")
            && self.children(slot).iter().any(|child| {
                self.is_live(*child)
                    && self.props(*child).str(PropId::AccessibilityRole) == Some("tab")
                    && self
                        .props(*child)
                        .get(PropId::AccessibilityControls)
                        .is_some()
            })
    }

    pub(crate) fn is_neutral_tab(&self, slot: u32) -> bool {
        metrics::column() == Column::Neutral
            && self.props(slot).str(PropId::AccessibilityRole) == Some("tab")
            && self
                .props(slot)
                .get(PropId::AccessibilityControls)
                .is_some()
            && self.parent(slot).is_some_and(|parent| {
                self.props(parent).str(PropId::AccessibilityRole) == Some("tablist")
            })
    }

    pub(crate) fn default_context(&self, slot: u32) -> DefaultContext {
        DefaultContext {
            alert: self.alert_context(slot),
            header: self.is_neutral_header(slot),
            inside_header: self.inside_neutral_header(slot),
            tab_bar: self.is_neutral_tab_bar(slot),
            tab: self.is_neutral_tab(slot),
            native_tab: self.neutral_tab_root(slot).is_some(),
        }
    }

    pub(crate) fn header_rows() -> StyleMask {
        HEADER.mask
    }

    pub(crate) fn tab_bar_rows() -> StyleMask {
        TAB_BAR.mask
    }

    fn structural_starts(&self, slot: u32) -> impl Iterator<Item = &StyleProps> {
        // A route header can also be a tablist. The later header rule wins
        // shared rows while the tab bar's top edge still accompanies its bottom.
        [
            self.is_neutral_header(slot).then(|| &*HEADER),
            self.is_neutral_tab_bar(slot).then(|| &*TAB_BAR),
        ]
        .into_iter()
        .flatten()
        .chain(self.alert_start(slot))
    }

    pub(crate) fn style_starts(&self, slot: u32) -> impl Iterator<Item = &StyleProps> {
        self.structural_starts(slot)
            .chain(self.control_text_start(slot))
            .chain(self.text_style_start(slot))
    }

    /// Effective own rows never alter stored ownership. Inherited text is
    /// resolved separately, including a written clear which bypasses a start.
    pub(crate) fn effective_style(&self, slot: u32) -> Cow<'_, StyleProps> {
        let own = self.style(slot);
        let mut out = Cow::Borrowed(own);
        for start in self.structural_starts(slot) {
            let defaults = start.mask.minus(out.mask.union(own.written));
            if !defaults.is_empty() {
                out.to_mut().copy_rows(start, defaults);
            }
        }
        out
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
                .style_starts(s)
                .any(|start| start.mask.has(id) && !self.styles[s as usize].written.has(id))
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
    /// not mark inherited, keep the effective own rows, including structural
    /// defaults below stored values and authored clears. One walk.
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
                .style_starts(s)
                .find(|start| start.mask.has(id) && !own.written.has(id))
            {
                return start;
            }
            if !id.inherited() {
                return own;
            }
            cur = self.parents[s as usize];
        }
        // A bare `font-size` style fills what nothing above says (LLP
        // 1115.000 D12.2).
        if let Some(style) = self.bare_text_style(slot).filter(|b| b.mask.has(id)) {
            return style;
        }
        if self.document_style.mask.has(id) {
            return &self.document_style;
        }
        &self.styles[slot as usize]
    }

    /// The slot whose text style supplies inherited row `id` for `slot`
    /// (LLP 1115.000 D12.2): one that names it through the `font`
    /// shorthand, where nothing written stops the walk first, or the
    /// nearest bare `font-size` style when nothing above says the row.
    pub fn text_style_source(&self, slot: u32, id: StyleId) -> Option<u32> {
        let mut cur = Some(slot);
        while let Some(s) = cur {
            let own = &self.styles[s as usize];
            if own.mask.has(id) {
                return None;
            }
            if self
                .text_style_start(s)
                .is_some_and(|start| start.mask.has(id) && !own.written.has(id))
            {
                return Some(s);
            }
            if self
                .style_starts(s)
                .any(|start| start.mask.has(id) && !own.written.has(id))
                || !id.inherited()
            {
                return None;
            }
            cur = self.parents[s as usize];
        }
        self.bare_text_style(slot)
            .filter(|b| b.mask.has(id))
            .and_then(|_| self.bare_text_style_source(slot))
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
        // The nearest bare `font-size` style, read once the walk passes the
        // first node that sets its own size (LLP 1115.000 D12.2).
        let mut sized = false;
        let mut bare = None;
        while !pending.is_empty() {
            let Some(p) = cur else { break };
            let ancestor: &StyleProps = &self.styles[p as usize];
            if !sized && ancestor.mask.has(StyleId::FontSize) {
                sized = true;
                bare = self.bare_text_style(p);
            }
            let found = pending.intersect(ancestor.mask);
            if !found.is_empty() {
                copy(ancestor, found);
                pending = pending.minus(found);
            }
            for start in self.style_starts(p) {
                let stopped = pending.intersect(start.mask).minus(ancestor.written);
                if !stopped.is_empty() {
                    copy(start, stopped);
                    pending = pending.minus(stopped);
                }
            }
            cur = self.parents[p as usize];
        }
        // What nothing above says, a bare style fills.
        if let Some(style) = bare {
            let found = pending.intersect(style.mask);
            if !found.is_empty() {
                copy(style, found);
                pending = pending.minus(found);
            }
        }
        let found = pending.intersect(self.document_style.mask);
        if !found.is_empty() {
            copy(&self.document_style, found);
        }
    }
}

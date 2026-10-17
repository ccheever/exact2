//! A platform text style's weight and leading (LLP 1115.000 D12.2): where a
//! node's `font-size` names a style (`-exact-headline`) and the node does not
//! write `font-weight` or `line-height`, the style's, for this process's
//! column (the Mac's `headline` is bold, its leadings its own). Named through
//! the `font` shorthand (`fontShorthand`) the style resets both, as CSS's
//! shorthand does, so it beats an inherited value; named by a bare
//! `font-size` it fills only what nothing above says. Per longhand: written
//! on the node, then a shorthand's style, then inherited, then a bare
//! style's, then CSS's default.
use super::*;
use crate::style::metrics;
use crate::style::relative::Unit;
use crate::LineHeight;
use std::sync::LazyLock;

/// The generic font stacks, the system's (`system-ui` to `ui-rounded`): the
/// ids below a declared family's (`contract/lower/src/fonts.rs`).
const GENERIC_STACKS: u16 = 8;

/// Each text style's rows in each column, as a start: its weight and its
/// leading (a ratio, or `normal`).
static STARTS: LazyLock<Vec<[StyleProps; 4]>> = LazyLock::new(|| {
    crate::TEXT_STYLES
        .iter()
        .map(|style| {
            std::array::from_fn(|column| {
                let mut rows = StyleProps {
                    font_weight: style.weights[column],
                    line_height: style.leadings[column]
                        .map_or(LineHeight::Normal, LineHeight::Number),
                    ..StyleProps::default()
                };
                rows.mask.set(StyleId::FontWeight);
                rows.mask.set(StyleId::LineHeight);
                rows
            })
        })
        .collect()
});

use crate::style::relative::column_index as index;

/// The weight Bold Text gives `weight` in the system font (LLP 1115.000
/// D12.3), as UIKit's: one step, measured on an iOS 27 simulator
/// (UIFontMetrics with `legibilityWeight` bold): 100 to 200, 200 to 300, 300
/// to 400, 400 to 600, 500 to 700, 600 to 800, 700 and on to 900; between two
/// hundreds, between their steps.
pub fn bolder(weight: u16) -> u16 {
    const STEPS: [u16; 11] = [100, 200, 300, 400, 600, 700, 800, 900, 900, 900, 1000];
    let w = weight.clamp(1, 1000);
    let (at, rest) = (usize::from(w / 100), w % 100);
    if at >= 10 {
        return 1000;
    }
    let (from, to) = (STEPS[at], STEPS[at + 1]);
    from + (to - from) * rest / 100
}

impl NodeArena {
    /// The text style `slot`'s own `font-size` names, and whether the `font`
    /// shorthand named it.
    pub(crate) fn named_text_style(&self, slot: u32) -> Option<(u8, bool)> {
        let own = self.style(slot);
        if !own.mask.has(StyleId::FontSize) {
            return None;
        }
        match own.relative.get(StyleId::FontSize) {
            Some((Unit::TextStyle(id), _)) => Some((
                id,
                self.props(slot).bool(crate::PropId::FontShorthand) == Some(true),
            )),
            _ => None,
        }
    }

    /// The rows a style named through the `font` shorthand gives `slot`: a
    /// start, beneath what the node writes and above what it inherits.
    pub(crate) fn text_style_start(&self, slot: u32) -> Option<&'static StyleProps> {
        match self.named_text_style(slot)? {
            (id, true) => Some(&STARTS.get(usize::from(id))?[index(metrics::column())]),
            _ => None,
        }
    }

    /// A run's rows inside its paragraph (`rows`, its parent's): its own
    /// over them, and its text style's where it names one (LLP 1115.000
    /// D12.2): a shorthand's beneath its own rows, a bare one's only where
    /// nothing at or above it says the row.
    pub(crate) fn run_style_rows(
        &self,
        slot: u32,
        rows: crate::text::RunRows,
    ) -> crate::text::RunRows {
        let own = self.style(slot);
        // What is said above, but for what a bare style (a further one)
        // filled there: a nearer bare style replaces that.
        let parent = self.parents[slot as usize];
        let mut above = rows.said();
        for row in [StyleId::FontWeight, StyleId::LineHeight] {
            let bare = parent
                .and_then(|p| self.text_style_source(p, row))
                .is_some_and(|s| matches!(self.named_text_style(s), Some((_, false))));
            if bare {
                above = above.minus(StyleMask::of(row));
            }
        }
        let rows = rows.child(own);
        let unsaid = |style: &StyleProps| style.mask.minus(own.mask).minus(own.written);
        if let Some(start) = self.text_style_start(slot) {
            rows.fill(start, unsaid(start))
        } else if let Some(bare) = self.bare_style_rows(slot) {
            rows.fill(bare, unsaid(bare).minus(above))
        } else {
            rows
        }
    }

    /// Whether stack `family` is the system's: as the plan said
    /// ([`crate::Kernel::set_system_stacks`]), else a generic stack; and
    /// the platform's control font, registered by the host.
    pub(crate) fn system_family(&self, family: u16) -> bool {
        self.system_stacks
            .get(usize::from(family))
            .copied()
            .unwrap_or(family < GENERIC_STACKS)
            || self
                .control_styles
                .as_ref()
                .is_some_and(|starts| starts.iter().any(|s| s.font_family == family))
    }

    /// Whether `slot`'s weight is the platform's control font's, as its host
    /// gave it, drawn at the person's text settings already: UIKit's body
    /// font is (bold under Bold Text); Android's, which a host registers at
    /// 400, is not.
    fn platform_control_weight(&self, slot: u32) -> bool {
        if metrics::column() != metrics::Column::Ios {
            return false;
        }
        let source = self.computed_source(slot, StyleId::FontWeight);
        self.control_styles
            .as_ref()
            .is_some_and(|starts| starts.iter().any(|s| std::ptr::eq(s, source)))
    }

    /// `weight` as Bold Text draws it on `slot` in stack `family` (LLP
    /// 1115.000 D12.3): heavier in the system font, the generic stacks, when
    /// the person turned it on; a family the app ships is left alone, and so
    /// is what UIKit weights itself on iOS: a native button whose title is in
    /// UIKit's own font (neither it nor its title writes a size or a weight).
    /// A symbol is stepped (its configuration names the weight). A host that
    /// paints its controls (Android's) is stepped here, and so is a weight an
    /// author gives a control in the platform's control font; not the weight
    /// that font has as iOS's host gave it (UIKit's body font, bold already).
    pub(crate) fn emboldened(&self, slot: u32, weight: u16, family: u16) -> u16 {
        if self.takes_bold_text(slot, family) {
            bolder(weight)
        } else {
            weight
        }
    }

    /// A run's style as Bold Text draws it ([`Self::emboldened`]), and
    /// whether it does.
    pub(crate) fn embolden(&self, slot: u32, style: &mut crate::text::TextStyle) {
        style.bold_text = self.takes_bold_text(slot, style.font_family);
        if style.bold_text {
            style.font_weight = bolder(style.font_weight);
        }
    }

    /// Whether Bold Text weights `slot`'s text in stack `family`
    /// ([`Self::emboldened`]).
    fn takes_bold_text(&self, slot: u32, family: u16) -> bool {
        let uikit = metrics::column() == metrics::Column::Ios;
        let fonted = |s: u32| {
            let own = self.style(s);
            own.mask.has(StyleId::FontSize) || own.mask.has(StyleId::FontWeight)
        };
        let uikit_font = |button: u32| {
            uikit && self.is_native_button(button) && !fonted(button) && !fonted(slot)
        };
        self.bold_text
            && self.system_family(family)
            && !uikit_font(slot)
            && !self.parents[slot as usize].is_some_and(uikit_font)
            && !self.platform_control_weight(slot)
    }

    /// The rows `slot`'s own bare `font-size` style gives, when its size
    /// names one and no shorthand did (a shorthand's is a start,
    /// [`Self::text_style_start`]).
    pub(crate) fn bare_style_rows(&self, slot: u32) -> Option<&'static StyleProps> {
        match self.named_text_style(slot)? {
            (id, false) => Some(&STARTS.get(usize::from(id))?[index(metrics::column())]),
            _ => None,
        }
    }

    /// The nearest bare `font-size` style at or above `slot` that gives row
    /// `id`, as CSS would have its weight inherited from where it is named:
    /// its node and its rows. A node that writes the row, a clear among
    /// them, is passed over (`inherit` there takes its parent's, a further
    /// style's fill; a clear below inherits the style's).
    pub(crate) fn bare_text_style(
        &self,
        slot: u32,
        id: StyleId,
    ) -> Option<(u32, &'static StyleProps)> {
        let mut cur = Some(slot);
        while let Some(s) = cur {
            if let Some(rows) = self.bare_style_rows(s) {
                if rows.mask.has(id) && !self.style(s).written.has(id) {
                    return Some((s, rows));
                }
            }
            cur = self.parents[s as usize];
        }
        None
    }
    /// A style's rows (weight, leading) that something at or above `slot`
    /// says: written, or a start (a shorthand's style among them); not a
    /// bare style's fill or CSS's default. A bare style below fills only
    /// the others, so a move that changes these and no value here still
    /// changes what such a descendant computes.
    pub(crate) fn said_style_rows(&self, slot: u32) -> StyleMask {
        let mut said = StyleMask::EMPTY;
        for id in [StyleId::FontWeight, StyleId::LineHeight] {
            let mut cur = Some(slot);
            while let Some(s) = cur {
                let own = self.style(s);
                if own.mask.has(id)
                    || self
                        .style_starts(s)
                        .any(|start| start.mask.has(id) && !own.written.has(id))
                {
                    said.set(id);
                    break;
                }
                cur = self.parents[s as usize];
            }
        }
        said
    }
}

impl crate::NodeRef<'_> {
    /// Whether this node names a text style through the `font` shorthand
    /// (or a heading): its weight and leading reset there.
    pub fn names_text_style_through_font(&self) -> bool {
        matches!(self.arena.named_text_style(self.slot), Some((_, true)))
    }

    /// Whether own row `id` is a heading's implied size or weight: the
    /// compiler's, no author's (LLP 1115.000 D12.2).
    pub fn heading_implies(&self, id: StyleId) -> bool {
        matches!(id, StyleId::FontSize | StyleId::FontWeight)
            && self.style.mask.has(id)
            && !self.style.written.has(id)
            && self.props.str(crate::PropId::AccessibilityRole) == Some("heading")
    }

    /// The node whose platform text style supplies inherited row `id` here
    /// (LLP 1115.000 D12.2): its weight or leading, named through the `font`
    /// shorthand with nothing written below it, or by a bare `font-size` with
    /// nothing above saying the row. `None` when no style does.
    pub fn style_source_of(&self, id: StyleId) -> Option<crate::ViewId> {
        self.arena
            .text_style_source(self.slot, id)
            .map(|s| self.arena.local_id(s))
    }

    /// Row `id` as this node's own text style gives it, when the style is
    /// named here (LLP 1115.000 D12.2): for a writer that declares a style's
    /// weight and leading where it is named and lets them inherit, as the
    /// JS target's stylesheet does.
    pub fn style_given(&self, id: StyleId) -> Option<StyleProps> {
        if self.arena.text_style_source(self.slot, id) != Some(self.slot) {
            return None;
        }
        let mut rows = StyleProps::default();
        let mut mask = crate::StyleMask::default();
        mask.set(id);
        rows.copy_rows(self.arena.computed_source(self.slot, id), mask);
        Some(rows)
    }
}

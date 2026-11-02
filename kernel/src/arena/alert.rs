//! Neutral top-layer defaults; the host admits and opens the retained subtree.
use super::*;
use crate::{generated::metric, style::metrics, Dimension, StyleValue};
use std::sync::LazyLock;

fn rows(values: &[(StyleId, &str)]) -> StyleProps {
    let mut s = StyleProps::default();
    for (id, value) in values {
        s.set_dynamic(
            *id,
            &value
                .parse::<f64>()
                .map(StyleValue::Number)
                .unwrap_or_else(|_| StyleValue::Text((*value).into())),
        )
        .expect("alert CSS row");
    }
    s
}
fn metric_row(s: &mut StyleProps, id: StyleId, metric: usize) {
    let n = metrics::length_in(metric, metrics::Column::Neutral, 16., 16.).unwrap();
    s.set_dynamic(id, &StyleValue::Number(n as f64))
        .expect("alert metric");
}
static ALERT: LazyLock<StyleProps> = LazyLock::new(|| {
    let mut s = rows(&[
        (StyleId::Display, "flex"),
        (StyleId::FlexWrap, "wrap"),
        (StyleId::BoxSizing, "border-box"),
        (StyleId::TextAlign, "start"),
        (
            StyleId::BackgroundColor,
            "-exact-secondary-grouped-background",
        ),
        (StyleId::TextColor, "-exact-label"),
        (StyleId::OverflowX, "auto"),
        (StyleId::OverflowY, "auto"),
    ]);
    for id in [
        StyleId::PaddingTop,
        StyleId::PaddingRight,
        StyleId::PaddingBottom,
        StyleId::PaddingLeft,
    ] {
        metric_row(&mut s, id, metric::ALERT_PADDING);
    }
    for id in [
        StyleId::BorderRadiusTopLeft,
        StyleId::BorderRadiusTopRight,
        StyleId::BorderRadiusBottomRight,
        StyleId::BorderRadiusBottomLeft,
    ] {
        metric_row(&mut s, id, metric::SURFACE_RADIUS);
    }
    metric_row(&mut s, StyleId::RowGap, metric::ALERT_GAP);
    metric_row(&mut s, StyleId::ColumnGap, metric::ALERT_ACTION_GAP);
    let px = |m| metrics::length_in(m, metrics::Column::Neutral, 16., 16.).unwrap();
    let alpha = match crate::generated::METRICS[metric::MODAL_SHADOW_ALPHA].neutral {
        crate::MetricValue::Number(n) => n,
        _ => unreachable!(),
    };
    s.set_dynamic(
        StyleId::BoxShadow,
        &StyleValue::Text(
            format!(
                "0 {}px {}px rgb(0 0 0 / {alpha})",
                px(metric::MODAL_SHADOW_Y),
                px(metric::MODAL_SHADOW_BLUR)
            )
            .into(),
        ),
    )
    .unwrap();
    s
});
fn surface(s: &mut StyleProps, y: usize, blur: usize, alpha: usize) {
    for id in [
        StyleId::BorderRadiusTopLeft,
        StyleId::BorderRadiusTopRight,
        StyleId::BorderRadiusBottomRight,
        StyleId::BorderRadiusBottomLeft,
    ] {
        metric_row(s, id, metric::SURFACE_RADIUS);
    }
    let px = |m| metrics::length_in(m, metrics::Column::Neutral, 16., 16.).unwrap();
    let crate::MetricValue::Number(alpha) = crate::generated::METRICS[alpha].neutral else {
        unreachable!()
    };
    s.set_dynamic(
        StyleId::BoxShadow,
        &StyleValue::Text(format!("0 {}px {}px rgb(0 0 0 / {alpha})", px(y), px(blur)).into()),
    )
    .unwrap();
}
static POPOVER: LazyLock<StyleProps> = LazyLock::new(|| {
    rows(&[
        (StyleId::OverflowX, "auto"),
        (StyleId::OverflowY, "auto"),
        (StyleId::MarginTop, "4"),
        (StyleId::MarginBottom, "4"),
    ])
});
static SURFACE: LazyLock<StyleProps> = LazyLock::new(|| {
    rows(&[
        (StyleId::OverflowX, "auto"),
        (StyleId::OverflowY, "auto"),
        (
            StyleId::BackgroundColor,
            "-exact-secondary-grouped-background",
        ),
        (StyleId::TextColor, "-exact-label"),
    ])
});
static RAISED: LazyLock<StyleProps> = LazyLock::new(|| {
    let mut s = SURFACE.clone();
    for id in [StyleId::MarginTop, StyleId::MarginBottom] {
        s.set_dynamic(id, &StyleValue::Number(4.)).unwrap();
    }
    surface(
        &mut s,
        metric::RAISED_SHADOW_Y,
        metric::RAISED_SHADOW_BLUR,
        metric::RAISED_SHADOW_ALPHA,
    );
    s
});
static DIALOG: LazyLock<StyleProps> = LazyLock::new(|| {
    let mut s = SURFACE.clone();
    for id in [
        StyleId::PaddingTop,
        StyleId::PaddingRight,
        StyleId::PaddingBottom,
        StyleId::PaddingLeft,
    ] {
        s.set_dynamic(id, &StyleValue::Text("1em".into())).unwrap();
    }
    surface(
        &mut s,
        metric::MODAL_SHADOW_Y,
        metric::MODAL_SHADOW_BLUR,
        metric::MODAL_SHADOW_ALPHA,
    );
    s
});
static TEXT: LazyLock<StyleProps> = LazyLock::new(|| {
    rows(&[
        (StyleId::FlexBasis, "100%"),
        (StyleId::TextColor, "-exact-secondary-label"),
    ])
});
static LAST_TEXT: LazyLock<StyleProps> = LazyLock::new(|| {
    let mut s = TEXT.clone();
    metric_row(&mut s, StyleId::MarginBottom, metric::ALERT_MESSAGE_GAP);
    s
});
static BUTTON: LazyLock<StyleProps> = LazyLock::new(|| {
    rows(&[
        (StyleId::FlexGrow, "1"),
        (StyleId::FlexShrink, "1"),
        (StyleId::FlexBasis, "0%"),
    ])
});
// An alert the compiler grouped (`alertPart`, contract/lower/src/alert_parts.rs)
// is a column: its message box shrinks to fit the dialog's ceiling and
// scrolls, while its title, a prompt's field and its actions keep their size
// (host/web/index.html).
static GROUPED: LazyLock<StyleProps> = LazyLock::new(|| {
    let mut s = ALERT.clone();
    s.set_dynamic(StyleId::FlexDirection, &StyleValue::Text("column".into()))
        .expect("alert CSS row");
    s.set_dynamic(StyleId::FlexWrap, &StyleValue::Text("nowrap".into()))
        .expect("alert CSS row");
    s
});
// Its title (unless it is the label's prefix, layout/alert.rs) and a prompt's
// field keep their size and read as the message does
static KEPT: LazyLock<StyleProps> = LazyLock::new(|| {
    rows(&[
        (StyleId::FlexShrink, "0"),
        (StyleId::TextColor, "-exact-secondary-label"),
    ])
});
static MESSAGE: LazyLock<StyleProps> = LazyLock::new(|| {
    let mut s = rows(&[
        (StyleId::FlexGrow, "0"),
        (StyleId::FlexShrink, "1"),
        (StyleId::MinHeight, "0"),
        (StyleId::OverflowX, "auto"),
        (StyleId::OverflowY, "auto"),
    ]);
    metric_row(&mut s, StyleId::RowGap, metric::ALERT_GAP);
    s
});
// Room at its inline end for an overlay scroll indicator, which draws there
// beside the words: on the right, and on the left in right-to-left text,
// where the layout mirrors the sides it set (`style::sides::mirror_unsaid`;
// host/web/neutral.css's `padding-inline-end`)
static MESSAGE_BOX: LazyLock<StyleProps> = LazyLock::new(|| {
    let mut s = MESSAGE.clone();
    metric_row(&mut s, StyleId::PaddingRight, metric::ALERT_SCROLL_GUTTER);
    s
});
// Its texts read as a flat alert's do. The box leaves color to them, so a
// text that inherits its color takes the alert's.
static MESSAGE_TEXT: LazyLock<StyleProps> =
    LazyLock::new(|| rows(&[(StyleId::TextColor, "-exact-secondary-label")]));
// Its actions wrap in a row, `ALERT_MESSAGE_GAP` below a row above them
// (ACTIONS), as a flat alert's buttons are below its last text; under only the
// label's title (an empty message box), the alert's gap alone (ACTIONS_FIRST)
static ACTIONS_FIRST: LazyLock<StyleProps> = LazyLock::new(|| {
    let mut s = rows(&[(StyleId::FlexShrink, "0"), (StyleId::FlexWrap, "wrap")]);
    metric_row(&mut s, StyleId::RowGap, metric::ALERT_GAP);
    metric_row(&mut s, StyleId::ColumnGap, metric::ALERT_ACTION_GAP);
    s
});
static ACTIONS: LazyLock<StyleProps> = LazyLock::new(|| {
    let mut s = ACTIONS_FIRST.clone();
    metric_row(&mut s, StyleId::MarginTop, metric::ALERT_MESSAGE_GAP);
    s
});
impl NodeArena {
    pub(crate) fn alert_title_style(&self, slot: u32) -> StyleProps {
        let mut title = self.computed_style(slot, StyleMask::INHERITED);
        title.font_size = metrics::length_in(
            metric::ALERT_TITLE,
            metrics::Column::Neutral,
            title.font_size,
            self.root_font_size(),
        )
        .unwrap();
        title.font_weight = self.emboldened(slot, 600, title.font_family);
        title
    }

    pub(crate) fn has_alert_prefix(&self, slot: u32) -> bool {
        self.alert_prefixes.contains(slot)
    }

    pub(crate) fn set_alert_prefix(&mut self, slot: u32, present: bool) {
        if present {
            self.alert_prefixes.insert(slot);
        } else {
            self.alert_prefixes.remove(slot);
        }
    }

    pub(crate) fn neutral_alert(&self, slot: u32) -> bool {
        metrics::column() == metrics::Column::Neutral
            && self.props(slot).str(PropId::SemanticTag) == Some("dialog")
            && self.props(slot).str(PropId::AccessibilityRole) == Some("alertdialog")
    }
    fn is_alert_button(&self, slot: u32) -> bool {
        self.is_button(slot) || self.is_native_button(slot)
    }
    fn alert_part(&self, slot: u32) -> Option<&str> {
        self.props(slot).str(PropId::AlertPart)
    }
    /// Whether `slot` is a neutral alert the compiler grouped: its buttons
    /// are in its `actions` box.
    pub(crate) fn grouped_alert(&self, slot: u32) -> bool {
        self.neutral_alert(slot)
            && self
                .children(slot)
                .iter()
                .any(|c| self.alert_part(*c) == Some("actions"))
    }
    /// Whether `slot` is a grouped neutral alert's `message` or `actions`
    /// box, whose children take the alert's defaults too.
    pub(crate) fn alert_group(&self, slot: u32) -> bool {
        metrics::column() == metrics::Column::Neutral
            && matches!(self.alert_part(slot), Some("message" | "actions"))
            && self.parent(slot).is_some_and(|p| self.neutral_alert(p))
    }
    pub(crate) fn alert_context(&self, slot: u32) -> u8 {
        if metrics::column() == metrics::Column::Neutral {
            let props = self.props(slot);
            if props.str(PropId::Popover).is_some()
                && props.str(PropId::AccessibilityRole) != Some("menu")
            {
                return if props.bool(PropId::DefaultSurface) == Some(true) {
                    15
                } else {
                    14
                };
            }
            if props.str(PropId::SemanticTag) == Some("dialog")
                && props.str(PropId::AccessibilityRole) != Some("alertdialog")
            {
                return 13;
            }
        }
        if self.neutral_alert(slot) {
            return if self.grouped_alert(slot) { 5 } else { 1 };
        }
        let Some(parent) = self.parent(slot) else {
            return 0;
        };
        if self.alert_group(parent) {
            return match self.alert_part(parent) {
                Some("message") => 10,
                _ if self.is_alert_button(slot) => 2,
                _ => 0,
            };
        }
        if !self.neutral_alert(parent) {
            return 0;
        }
        if self.grouped_alert(parent) {
            return match self.alert_part(slot) {
                Some("message" | "actions") if self.children(slot).is_empty() => 8,
                Some("message") => 7,
                Some("actions")
                    if self
                        .children(parent)
                        .iter()
                        .take_while(|row| **row != slot)
                        .all(|row| {
                            self.alert_part(*row) == Some("message")
                                && self.children(*row).is_empty()
                        }) =>
                {
                    12
                }
                Some("actions") => 9,
                _ => 6,
            };
        }
        if self.is_alert_button(slot) {
            return 2;
        }
        let next_button = self
            .children(parent)
            .windows(2)
            .any(|pair| pair[0] == slot && self.is_alert_button(pair[1]));
        if next_button {
            4
        } else {
            3
        }
    }
    pub(crate) fn alert_start(&self, slot: u32) -> Option<&StyleProps> {
        match self.alert_context(slot) {
            1 => Some(&ALERT),
            2 => Some(&BUTTON),
            3 => Some(&TEXT),
            4 => Some(&LAST_TEXT),
            5 => Some(&GROUPED),
            6 => Some(&KEPT),
            7 => Some(&MESSAGE_BOX),
            8 => None,
            9 => Some(&ACTIONS),
            10 => Some(&MESSAGE_TEXT),
            12 => Some(&ACTIONS_FIRST),
            13 => Some(&DIALOG),
            14 => Some(&POPOVER),
            15 => Some(&RAISED),
            _ => None,
        }
    }
    pub(crate) fn alert_rows() -> StyleMask {
        [
            &ALERT,
            &TEXT,
            &LAST_TEXT,
            &BUTTON,
            &GROUPED,
            &KEPT,
            &MESSAGE_BOX,
            &MESSAGE_TEXT,
            &ACTIONS,
            &DIALOG,
            &POPOVER,
            &RAISED,
        ]
        .into_iter()
        .fold(StyleMask::EMPTY, |mask, s| mask.union(s.mask))
    }
    pub(crate) fn alert_layout(&self, slot: u32, s: &mut taffy::style::Style) {
        // A part box its `if` leaves empty takes no room. The compiler writes
        // the box's `display`, which a default row cannot override
        // (`effective_style`), so the layout leaves it out here (the web says
        // it with `!important`).
        if self.alert_context(slot) == 8 {
            s.display = taffy::Display::None;
            return;
        }
        if !self.neutral_alert(slot) && self.alert_context(slot) != 13 {
            return;
        }
        let own = self.style(slot).mask.union(self.style(slot).written);
        let em = self.computed_source(slot, StyleId::FontSize).font_size;
        let px =
            |m| metrics::length_in(m, metrics::Column::Neutral, em, self.root_font_size()).unwrap();
        if self.neutral_alert(slot) && !own.has(StyleId::Width) {
            s.size.width = taffy::style::Dimension::length(
                px(metric::ALERT_WIDTH)
                    .min((self.env().viewport_width - px(metric::ALERT_MARGIN)).max(0.)),
            );
        }
        // Chromium's modal dialog UA ceiling: calc(100% - 2em - 6px),
        // measured independently of the Neutral sheet's authored width.
        let ceiling = Dimension::Calc(100., -2. * em - 6.).floor_lpa(self.env());
        if !own.has(StyleId::MaxWidth) {
            s.max_size.width = ceiling;
        }
        if !own.has(StyleId::MaxHeight) {
            s.max_size.height = ceiling;
        }
    }
}

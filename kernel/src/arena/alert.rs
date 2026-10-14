//! Neutral alert sheet defaults; the host admits and opens the retained subtree.
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
impl NodeArena {
    pub(crate) fn neutral_alert(&self, slot: u32) -> bool {
        metrics::column() == metrics::Column::Neutral
            && self.props(slot).str(PropId::SemanticTag) == Some("dialog")
            && self.props(slot).str(PropId::AccessibilityRole) == Some("alertdialog")
    }
    pub(crate) fn alert_context(&self, slot: u32) -> u8 {
        if self.neutral_alert(slot) {
            return 1;
        }
        let Some(parent) = self.parent(slot).filter(|p| self.neutral_alert(*p)) else {
            return 0;
        };
        if self.is_button(slot) {
            return 2;
        }
        let next_button = self
            .children(parent)
            .windows(2)
            .any(|pair| pair[0] == slot && self.is_button(pair[1]));
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
            _ => None,
        }
    }
    pub(crate) fn alert_rows() -> StyleMask {
        ALERT
            .mask
            .union(TEXT.mask)
            .union(LAST_TEXT.mask)
            .union(BUTTON.mask)
    }
    pub(crate) fn alert_layout(&self, slot: u32, s: &mut taffy::style::Style) {
        if !self.neutral_alert(slot) {
            return;
        }
        let own = self.style(slot).mask.union(self.style(slot).written);
        let em = self.computed_source(slot, StyleId::FontSize).font_size;
        let px =
            |m| metrics::length_in(m, metrics::Column::Neutral, em, self.root_font_size()).unwrap();
        if !own.has(StyleId::Width) {
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

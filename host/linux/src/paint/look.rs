//! The neutral look, where this painter is the whole of a control (LLP
//! 1115.000 D17): on a host with no platform toolkit under it (the Linux
//! desktop, Windows) what the author left unsaid is drawn as the web draws
//! it under its neutral look (`host/web/neutral.css`), from the same colour
//! column and the same metrics. Both are the schema's: the role table's
//! neutral column and the look's rows of `metrics`, which a test holds the
//! page's sheet to.
//!
//! The process says so once, before it boots a plan
//! (`app::show_neutral_look`). Android's reader does not: it has a platform
//! and paints Material (LLP 1115.001), and until its controls are Material's
//! too they are the ones this painter drew before the neutral look, which is
//! every `else` beside a [`neutral`] here.
use super::rgba;
use exact_kernel::generated::metric;
use exact_kernel::style::{metrics, roles};
use exact_kernel::ColorValue;

/// Whether this process paints the neutral look.
pub(crate) fn neutral() -> bool {
    roles::column() == roles::Column::Neutral
}

/// A length of the neutral look for a control whose font size is `em`, in
/// pixels; `index` is one of `generated::metric`. None of the rows a control
/// reads is in `rem`.
pub(crate) fn px(index: usize, em: f32) -> f32 {
    metrics::length_in(index, metrics::Column::Neutral, em, em).unwrap_or(0.0)
}

/// A ratio, a weight or an opacity of the neutral look.
pub(crate) fn number(index: usize) -> f32 {
    match exact_kernel::generated::METRICS[index].neutral {
        exact_kernel::MetricValue::Number(n) => n as f32,
        _ => 0.0,
    }
}

/// A colour role as this process shows it under a scheme.
pub(crate) fn role(name: &str, dark: bool) -> [u8; 4] {
    roles::role(name).map_or([0, 0, 0, 255], |id| {
        rgba(ColorValue::Role(id).resolve(dark))
    })
}

/// `color` at `alpha` of its own opacity.
pub(crate) fn fade(mut color: [u8; 4], alpha: f32) -> [u8; 4] {
    color[3] = (color[3] as f32 * alpha).round() as u8;
    color
}

/// The line that marks a control's edge (`--exact-line`): the label's ink at
/// the look's opacity, which reads at 3:1 on either ground.
pub(crate) fn line(dark: bool) -> [u8; 4] {
    fade(
        role("-exact-label", dark),
        number(metric::CONTROL_LINE_ALPHA),
    )
}

/// The ink that reads on `fill`: white, until the fill is light enough that
/// black reads better (CIE lightness above 49, the sheet's own threshold).
pub(crate) fn ink_on(fill: [u8; 4]) -> [u8; 4] {
    let linear = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let y = 0.2126 * linear(fill[0]) + 0.7152 * linear(fill[1]) + 0.0722 * linear(fill[2]);
    let lightness = if y > 0.008856 {
        116.0 * y.cbrt() - 16.0
    } else {
        903.3 * y
    };
    if lightness > 49.0 {
        [0, 0, 0, 255]
    } else {
        [255; 4]
    }
}

/// The accent a control paints in and the ink that reads on it: the node's
/// `accent-color` where it sets one, else the `AccentColor` role.
pub(crate) fn accents(authored: Option<[u8; 4]>, dark: bool) -> ([u8; 4], [u8; 4]) {
    let accent = authored.unwrap_or_else(|| role("AccentColor", dark));
    (accent, ink_on(accent))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ink on the neutral column's accent is the column's own
    /// `AccentColorText`: white on the light scheme's blue, black on the
    /// dark scheme's, which is what the sheet's threshold gives a page.
    #[test]
    fn the_ink_on_a_fill_is_the_one_that_reads() {
        assert_eq!(ink_on([0x0b, 0x66, 0xd6, 0xff]), [255; 4]);
        assert_eq!(ink_on([0x68, 0xa5, 0xfe, 0xff]), [0, 0, 0, 255]);
        assert_eq!(ink_on([0xff, 0xdd, 0x00, 0xff]), [0, 0, 0, 255]);
        assert_eq!(ink_on([0xc9, 0x0f, 0x1a, 0xff]), [255; 4]);
    }

    #[test]
    fn a_length_of_the_look_follows_the_controls_font_size() {
        assert_eq!(px(metric::CONTROL_HEIGHT, 16.0), 36.0);
        assert_eq!(px(metric::CONTROL_HEIGHT, 20.0), 45.0);
        assert_eq!(px(metric::CONTROL_RADIUS, 20.0), 8.0);
        assert_eq!(number(metric::CONTROL_LINE_HEIGHT), 1.25);
    }
}

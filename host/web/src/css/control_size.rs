//! `-exact-control-size` (LLP 1069.011.001 D8).
use exact_kernel::style::relative::{text_style, text_style_size, MEDIUM};
use exact_num::push_text;

/// The title's size. UIKit titles a mini or small button in the
/// subheadline text style and a medium or large one in the body. Each is
/// that style's size at CSS's `medium`, in `rem`, so it follows the root as
/// a text style does. Large's title is the body × 18/16, the ratio
/// browsers give CSS's `large`, standing in for UIKit's taller box.
pub fn font_size(size: &str) -> String {
    let style_rem =
        |name| text_style_size(text_style(name).expect("schema text style"), MEDIUM) / MEDIUM;
    let rem = match size {
        "mini" | "small" => style_rem("-exact-subheadline"),
        "large" => style_rem("-exact-body") * 18.0 / 16.0,
        _ => style_rem("-exact-body"),
    };
    format!("{rem}rem")
}

/// What a size writes over neutral.css's 44 px touch floor: `0px` for a mini
/// or small control, which keeps no touch floor, only the floor it has under
/// a mouse, as UIKit draws one at its own height on a phone.
pub fn touch_floor(size: &str) -> Option<&'static str> {
    matches!(size, "mini" | "small").then_some("0px")
}

pub(super) fn push(out: &mut String, size: &str) {
    push_text!(out, "--exact-control-font-size:{};", font_size(size));
    if let Some(floor) = touch_floor(size) {
        push_text!(out, "--exact-touch-floor:{};", floor);
    }
}

//! Explicit CSS clears use the host's mapping for non-CSS rows.
use super::{property, StyleId};

/// False lets the existing special lowering serialize the row's initial
/// value, or report an unsupported row. Those rows are not CSS properties.
pub(super) fn push(out: &mut String, id: StyleId, press: bool) -> bool {
    use StyleId::*;
    match id {
        TintColor | PlaceholderColor | ExitAnimation | DragTimeline | AnimationTimeline
        | AnimationRange | TimelineScope | LayoutTransition | PressScale | PressHaptic
        | ControlSize | ControlCornerStyle | Fit | LineClamp | RotateAxis | TranslateZ
        | TranslatePercent | SymbolRendering | SymbolPalette | SymbolValue | SymbolEffect
        | ContentTransition | ScrollEdgeEffect | HoverEffect | SmartInvert | AnimationTrigger => {
            return false
        }
        // `scale` remains the feedback product; an unset own factor is one.
        Scale if press => out.push_str("--exact-scale:1;"),
        AccentColor => out.push_str("accent-color:unset;--exact-accent:unset;"),
        _ => {
            property(out, id);
            out.push_str(":unset;");
        }
    }
    true
}

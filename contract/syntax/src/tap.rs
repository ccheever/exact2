//! Input forms for authored test steps.

/// Which input a `tap` step gives: the driver's `tap` forms (LLP 1012).
#[derive(Debug, Clone, PartialEq)]
pub enum TapForm {
    /// A press: down and up at the node's middle.
    Press,
    /// `hover`: a pointer over it.
    Hover,
    /// `dblclick`: two presses, the second a double click (feed F10).
    Dblclick,
    /// `contextmenu`: a secondary press.
    Contextmenu,
    /// `into "key"`: a virtualized list's row brought into view by its key
    /// (LLP 1070.000 §5), so a row outside the rendered window can be tapped.
    Into(String),
    /// `pinch <scale> [at x y]`: two fingers about the node's middle, or
    /// about `at` in its box (stocks: the agent's pinch, in a test file).
    Pinch {
        /// How far the fingers spread, greater than 0. `1` is no change.
        scale: f64,
        /// Where the pinch is centred, in the node's box; its middle when `None`.
        at: Option<(f64, f64)>,
    },
    /// `wheel <dx> <dy>`: the agent's wheel at the node's middle, in points,
    /// as `tap <t> wheel dx dy`: a positive `dy` scrolls on toward the
    /// content's end (the offset grows), a negative one back toward its start.
    /// Applied at once, with no momentum (a macOS scroll, where a drag
    /// selects text instead).
    Wheel {
        /// Across, positive toward the content's right.
        dx: f64,
        /// Down, positive toward the content's end.
        dy: f64,
    },
    /// `mediasession "action" [seconds]`: the media element's session
    /// action, as the platform's handler would call it (LLP 1098 D10);
    /// never a press.
    MediaSession {
        /// `play`, `pause`, or one of the six actions.
        action: String,
        /// A seek's `seekOffset` (else the element's own), `seekto`'s
        /// `seekTime` (required there).
        seconds: Option<f64>,
    },
}

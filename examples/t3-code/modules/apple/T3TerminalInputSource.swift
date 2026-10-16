#if os(macOS)
import AppKit

/// Reconnects the terminal's input context when the keyboard input source changes under it.
///
/// A source selected with `TISSelectInputSource` from another process (an input source switcher, an
/// agent's input tool) while the terminal's web view is first responder can leave WebKit's input
/// context with no input method. Every key then reaches the page as a plain keydown, not "Process",
/// carrying the keyboard layout's character, which the surface encodes as it stands (g k s, or ㅎ ㅏ ㄴ
/// under the 2-Set layout), and nothing composes until the view is focused again. Measured in a bare
/// WKWebView on macOS 26: after 4 of 6 such switches with VoiceOver off and 9 of 10 with it on; never
/// after the system's own input source shortcut (12 switches). The reference's surface under Chromium
/// sends at most the first key that way and composes from the next. Deactivating and activating the
/// context once the selected source has changed reconnects it (12 of 12 switches then composed).
final class T3TerminalInputSource {
    /// The source the focused terminal last had; nil before it was first focused.
    private var seen: String?

    /// The terminal took the focus with `source` selected.
    func focused(_ source: String?) { seen = source }

    /// The selected source is now `source`. True when the context must be reconnected: the source
    /// changed while the terminal had the active input context.
    func changed(to source: String?, active: Bool) -> Bool {
        guard source != seen else { return false }
        seen = source
        return active
    }
}
#endif

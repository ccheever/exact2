// Which `aria-keyshortcuts` button a key may reach, on every Apple host, by
// the rule the web host's input-glue.js keeps: none behind the frontmost
// shown `aria-modal` view (gallery F22: Escape left the selection mode
// behind an open dialog instead of closing it), and never Enter or Space
// while the focus is a control those keys activate (onboarding F27: Enter
// on the focused Back went to the Next button's shortcut).
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension Presenter {
    /// The frontmost shown `aria-modal` view, the most recently made of
    /// those shown: while one is up only shortcuts inside it are heard.
    var shortcutModal: NodeView? {
        chrome.ids("accessibilityModal").sorted().compactMap { views[$0] }
            .last { $0.props["accessibilityModal"] == "true" && $0.accessibilityVisible }
    }

    /// Whether `node`'s shortcut may take `key` (the web's name) with the
    /// modifiers `held` (`KeyCodes.held`), the focus being `focus`.
    func shortcutAdmits(_ node: NodeView, key: String, held: String, focus: NodeView?) -> Bool {
        if let modal = shortcutModal, node !== modal, !node.isDescendant(of: modal) { return false }
        if held.isEmpty, key == "Enter" || key == " " || key == "Space", let focus, focus !== node,
           focus.isButton || focus.handlers.contains("press") { return false }
        return true
    }
}

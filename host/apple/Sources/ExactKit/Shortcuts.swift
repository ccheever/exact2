// Which `aria-keyshortcuts` button a key may reach, on every Apple host, by
// the rule the web host's input-glue.js keeps and the Linux host's
// `shortcuts.rs` states: a button bound to a world's action (`action="buy"`)
// is one, pressed as a click presses it, so a focused canvas never hears its
// key (LLP 1132 D11). None behind the frontmost
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
        #if os(iOS) || os(tvOS)
        // A dialog in its sheet is modal with nothing written (LLP 1115.000 D11.5).
        if let dialog = menus.sheets.topDialog { return dialog }
        #endif
        return chrome.ids("accessibilityModal").sorted().compactMap { views[$0] }
            .last { $0.props["accessibilityModal"] == "true" && $0.accessibilityVisible }
    }

    /// Press `node` as its shortcut: a world's control is a down and an up
    /// of its action on the assistive contact, which moves no focus, then
    /// its authored `press`, if it has one, as the web's click runs both. A
    /// disabled button takes its key and presses nothing, as on the web.
    /// An app-wide menu item (the Mac's app menu and Help) presses it under
    /// a modal dialog too.
    func pressShortcut(_ node: NodeView, appWide: Bool = false) {
        guard !node.disabled else { return }
        #if os(macOS)
        let press = { self.press($0, outsideModal: appWide) }
        #else
        let press = { self.press($0) }
        #endif
        guard node.isSurfaceControl else { return press(node.id) }
        let contact = NodeView.assistiveContact
        if node.control("down", id: contact) { _ = node.control("up", id: contact) }
        if node.handlers.contains("press") { press(node.id) }
    }

    /// Whether `node`'s shortcut may take `key` (the web's name) with the
    /// modifiers `held` (`KeyCodes.held`), the focus being `focus`.
    func shortcutAdmits(_ node: NodeView, key: String, held: String, focus: NodeView?) -> Bool {
        if let modal = shortcutModal, !logicallyWithin(node, modal) { return false }
        if held.isEmpty, key == "Enter" || key == " " || key == "Space", let focus, focus !== node,
           focus.isButton || focus.handlers.contains("press") { return false }
        return true
    }
}

#if os(macOS)
import AppKit

/// Edit › Undo as the reference desktop app routes ⌘Z
/// (apps/desktop/src/window/DesktopApplicationMenu.ts; MIT, see LICENSE-T3).
///
/// The reference renderer sees a keystroke before Electron's menu: ⌘Z is thread.undo
/// while no editable text has the focus. The app's Undo command stands in for Edit ›
/// Undo (its chord is the host item's own, settings-shortcuts.contract `chordStandsIn`),
/// so its item routes a keystroke to the window's command and leaves editable text with
/// the focus its own Undo and Redo: since exact2 #327
/// (issues/closed/20261009-macos-textarea-native-undo.md) Exact's textarea answers `undo:`
/// and `redo:` with its own history, so they go up the responder chain as AppKit sends them.
/// Every other app command is a shortcut with no item (`-exact-menu="none"`), and the root
/// removes the menus and items the reference's bar lacks (`-exact-menu-remove`).
final class T3MenuBar: NSObject, NSMenuItemValidation {
    /// The host's command items (ShortcutsMac): titled by their button's label.
    static let undoTitle = "Undo"
    private weak var file: NSMenu?

    func arrange(_ bar: NSMenu) {
        file = bar.items.first(where: { $0.submenu?.title == "File" })?.submenu
        if let edit = bar.items.first(where: { $0.submenu?.title == "Edit" })?.submenu,
           let undo = edit.items.first(where: { $0.keyEquivalent == "z" && $0.keyEquivalentModifierMask.intersection(.deviceIndependentFlagsMask) == .command }) {
            undo.action = #selector(undo(_:))
            undo.target = self
        }
    }

    /// The host's command item for a button label in File (hidden there), or nil.
    func command(_ title: String) -> NSMenuItem? {
        file?.items.first { item in
            item.title == title && item.action != nil && item.target.map { String(describing: type(of: $0)) == "ShortcutHost" } == true
        }
    }
    private func fire(_ item: NSMenuItem) -> Bool {
        guard let action = item.action, let target = item.target,
              (target as? NSMenuItemValidation)?.validateMenuItem(item) ?? true else { return false }
        return NSApp.sendAction(action, to: target, from: item)
    }
    /// Whether the menu action came from a keystroke (the tests inject it).
    var keystroke: () -> Bool = { NSApp.currentEvent?.type == .keyDown }
    /// The editable text view that holds the focus (the tests inject it).
    var focusedText: () -> NSTextView? = {
        guard let text = NSApp.keyWindow?.firstResponder as? NSTextView, text.isEditable else { return nil }
        return text
    }
    /// Where Edit's `undo:` goes from the focused text: the first responder up its chain that answers it, as AppKit
    /// sends a menu's nil-targeted action (Exact's textarea answers it with its own history since exact2 #327; a
    /// field's editor leaves it to its window).
    private static func undoTarget(from text: NSTextView) -> NSResponder? {
        var responder: NSResponder? = text
        while let current = responder, !current.responds(to: #selector(undo(_:))) { responder = current.nextResponder }
        return responder
    }

    @objc func undo(_ sender: Any?) {
        // Editable text with the focus answers ⌘Z itself, or not at all: the reference binds thread.undo
        // `when: "!terminalFocus && !editableFocus"`, and its Edit › Undo role acts on the focused editor's history.
        if let text = focusedText() {
            _ = Self.undoTarget(from: text)?.tryToPerform(#selector(undo(_:)), with: sender)
            return
        }
        if keystroke(), let thread = command(Self.undoTitle), fire(thread) { return }
        NSApp.sendAction(#selector(undo(_:)), to: nil, from: sender)
    }

    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        guard item.action == #selector(undo(_:)) else { return true }
        if let text = focusedText() {
            guard let target = Self.undoTarget(from: text) else { return false }
            return (target as? NSMenuItemValidation)?.validateMenuItem(item) ?? true
        }
        guard let thread = command(Self.undoTitle) else { return false }
        return (thread.target as? NSMenuItemValidation)?.validateMenuItem(thread) ?? true
    }
}
#endif

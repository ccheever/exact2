// DOM's clipboard events (spreadsheet F4, F14): ⌘C, ⌘X and ⌘V — the Edit
// menu's copy:, cut: and paste:, or a hardware keyboard's on iPadOS — reach
// the focused node as `copy`, `cut` and `paste`, the nearest node with a
// handler (itself or an ancestor) hearing them, as on the web. A paste
// carries the pasteboard's plain text; a copy or cut none, its action
// writing the pasteboard (`copyText`). With no handler a cut or paste is
// not this node's, so the responder chain goes on; a copy is the text
// selection's, as before.
// A text field's or text area's own editing (its field editor, NSTextView,
// UITextField, UITextView) fires them too, before its own cut, copy or
// paste, which an action that calls `preventDefault()` cancels, as the DOM's
// default action is (#125).
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension NodeView {
    /// The handlers that make a node focusable: the web's rule that only a
    /// focusable element hears these (a clipboard event goes to the focus).
    static let focusEvents: Set<String> = ["focus", "blur", "key", "copy", "cut", "paste"]

    /// The event an edit action is, and the node that hears it here. A
    /// field's or text area's is its editor's (`fieldClipboard`).
    func clipboardTarget(_ action: Selector) -> (kind: UInt32, node: NodeView)? {
        guard field == nil, textArea == nil else { return nil }
        return clipboardHearer(action)
    }

    /// The event `action` is and its nearest handler, the node or an ancestor.
    func clipboardHearer(_ action: Selector) -> (kind: UInt32, node: NodeView)? {
        let kinds: [(Selector, String, UInt32)] = [(#selector(copy(_:)), "copy", 32), (#selector(cut(_:)), "cut", 33), (#selector(paste(_:)), "paste", 34)]
        guard let (_, name, kind) = kinds.first(where: { $0.0 == action }) else { return nil }
        #if os(macOS)
        let chain = sequence(first: self as NSView, next: \.superview)
        #else
        let chain = sequence(first: self as UIView, next: \.superview)
        #endif
        guard let node = chain.lazy.compactMap({ $0 as? NodeView }).first(where: { $0.handlers.contains(name) && !$0.disabled }) else { return nil }
        return (kind, node)
    }

    /// Whether a field's editor would be heard for `action`: the node or an ancestor has the handler.
    func hearsFieldClipboard() -> Bool {
        [#selector(copy(_:)), #selector(cut(_:)), #selector(paste(_:))].contains { clipboardHearer($0) != nil }
    }

    /// The `copy`, `cut` or `paste` this field's or text area's editor is
    /// about to perform, heard by the nearest handler (the node or an
    /// ancestor), the paste with the pasteboard's text or `text` in its
    /// place. True when an action called `preventDefault()`: the editor
    /// skips its own cut, copy or paste, as the DOM's default is cancelled.
    func fieldClipboard(_ action: Selector, text: String? = nil) -> Bool {
        guard let presenter, let (kind, node) = clipboardHearer(action) else { return false }
        presenter.defaultPrevented = false
        presenter.clipboard(node.id, kind, kind == 34 ? text ?? Self.pasteboardText ?? "" : "")
        let prevented = presenter.defaultPrevented
        presenter.defaultPrevented = false
        return prevented
    }

    /// A text view's cut copies through its own `copy(_:)`, which is no
    /// second event.
    private static var cutting = false

    /// An editor's `action` (its copy, cut or paste): the event first, then
    /// `perform`, the editor's own, unless an action prevented it.
    static func fieldEdit(_ owner: NodeView?, _ action: Selector, _ perform: () -> Void) {
        if cutting || owner?.fieldClipboard(action) != true {
            let outer = cutting
            cutting = cutting || action == #selector(cut(_:))
            defer { cutting = outer }
            perform()
        }
    }

    static var pasteboardText: String? {
        #if os(macOS)
        NSPasteboard.general.string(forType: .string)
        #elseif os(tvOS)
        nil // tvOS has no pasteboard.
        #else
        UIPasteboard.general.string
        #endif
    }

    /// Deliver `action` with the pasteboard's text (a paste's), or `text`
    /// in its place (the agent's `type <id> paste <text>`): whether a node heard it.
    @discardableResult
    func clipboard(_ action: Selector, text: String? = nil) -> Bool {
        guard let (kind, node) = clipboardTarget(action) else { return false }
        presenter?.clipboard(node.id, kind, kind == 34 ? text ?? Self.pasteboardText ?? "" : "")
        return true
    }

    #if os(macOS)
    /// Unheard, a copy is the text selection's (`TextSelection`).
    @objc func copy(_ sender: Any?) { if !clipboard(#selector(copy(_:))) { presenter?.selection.copy() } }
    @objc func cut(_ sender: Any?) { clipboard(#selector(cut(_:))) }
    @objc func paste(_ sender: Any?) { clipboard(#selector(paste(_:))) }
    /// A cut or paste is this node's only while a node hears it.
    package override func responds(to aSelector: Selector!) -> Bool {
        if [#selector(cut(_:)), #selector(paste(_:))].contains(aSelector) { return clipboardTarget(aSelector) != nil }
        return super.responds(to: aSelector)
    }
    #else
    // UIView implements none of the three: forwarding an unheard one to
    // super raised doesNotRecognizeSelector (a field's Cut, empty, crashed).
    package override func copy(_ sender: Any?) { clipboard(#selector(copy(_:))) }
    package override func cut(_ sender: Any?) { clipboard(#selector(cut(_:))) }
    package override func paste(_ sender: Any?) { clipboard(#selector(paste(_:))) }
    /// A copy, cut or paste is this node's only while a node hears it;
    /// UIResponder's default would claim it because the class implements it.
    package override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        if [#selector(copy(_:)), #selector(cut(_:)), #selector(paste(_:))].contains(action) { return clipboardTarget(action) != nil }
        return super.canPerformAction(action, withSender: sender)
    }
    #endif
}

extension NodeView {
    /// The field's or text area's selection replaced with `text`, as its
    /// editor's paste would, so its `input` follows.
    func insertAtFieldSelection(_ text: String) {
        #if os(macOS)
        guard let editor = textArea ?? field?.currentEditor() as? NSTextView else { return }
        editor.insertText(text, replacementRange: editor.selectedRange())
        #else
        guard let editor: UITextInput = textArea ?? field, let range = editor.selectedTextRange else { return }
        editor.replace(range, withText: text)
        #endif
    }
}

extension Agent {
    /// `type <id> copy|cut|paste [text]` (spreadsheet F6): the event ⌘C, ⌘X
    /// or ⌘V delivers with the focus at `v` — a paste carrying `text` as
    /// the pasteboard's, which the driver leaves alone.
    func clipboardType(_ v: NodeView, _ edit: String, _ text: String?) -> [String: Any] {
        let actions = ["copy": #selector(NodeView.copy(_:)), "cut": #selector(NodeView.cut(_:)), "paste": #selector(NodeView.paste(_:))]
        guard let action = actions[edit] else { return ["error": "type: \(edit) is not copy, cut or paste"] }
        let editing = v.field != nil || v.textArea != nil
        guard (editing ? v.clipboardHearer(action) : v.clipboardTarget(action)) != nil else { return ["error": "no \(edit) handler at view \(v.id) or above it"] }
        #if os(macOS)
        if editing {
            if let f = v.textArea, v.window?.firstResponder !== f { v.window?.makeFirstResponder(f) }
            if let f = v.field, f.currentEditor() == nil { v.window?.makeFirstResponder(f) }
        } else if v.acceptsFirstResponder, v.window?.firstResponder !== v { v.window?.makeFirstResponder(v) }
        #else
        let focus: UIView = v.textArea ?? v.field ?? v
        if focus.canBecomeFirstResponder, !focus.isFirstResponder { _ = focus.becomeFirstResponder() }
        #endif
        // A paste is ⌘V first. A `key` handler that preventDefault()s that
        // chord keeps the clipboard event from landing (drums: the driver's
        // paste skipped the key and hid that bug). Copy and cut stay the event.
        // The page's shortcuts hear the chord before the handlers, as the
        // web's capture listener does: a button declaring Meta+V takes it.
        // Its release comes up through the `keyup` handlers last, whatever
        // took the down, as the web driver's (#140).
        // Command's own keyup after it, without its own bit, as a keyboard's.
        defer { if edit == "paste" { v.presenter?.keyUp(at: v, "v", held: "Meta+", code: "KeyV"); v.presenter?.keyUp(at: v, "Meta", held: "", code: "MetaLeft") } }
        if edit == "paste", let presenter = v.presenter {
            // Command's own keydown first, as a keyboard's (Charlie, 2026-10-07).
            _ = presenter.keyDown(at: v, "Meta", held: "Meta+", code: "MetaLeft")
            if pasteShortcut(v, presenter) {
                return ["typed": Int(v.id), "clipboard": edit, "shortcut": true, "delivery": "recognized"]
            }
            if presenter.keyDown(at: v, "v", held: "Meta+", code: "KeyV") {
                return ["typed": Int(v.id), "clipboard": edit, "delivery": "recognized"]
            }
        }
        if editing {
            // An unprevented paste inserts the driver's text, without the
            // pasteboard; a copy or cut is the event alone, as the web
            // driver's (agent-keys.mjs `deliverClipboard`).
            if !v.fieldClipboard(action, text: text ?? ""), edit == "paste" { v.insertAtFieldSelection(text ?? "") }
            return ["typed": Int(v.id), "clipboard": edit, "delivery": "recognized", "value": v.fieldText]
        }
        v.clipboard(action, text: text ?? "")
        return ["typed": Int(v.id), "clipboard": edit, "delivery": "recognized"]
    }

    /// ⌘V at `v` through the shortcuts (`ShortcutsMac`, `ShortcutsIOS`):
    /// whether a button declaring it was pressed.
    private func pasteShortcut(_ v: NodeView, _ presenter: Presenter) -> Bool {
        #if os(macOS)
        // kVK_ANSI_V is 9.
        guard let window = v.window, let down = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command,
            timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil,
            characters: "v", charactersIgnoringModifiers: "v", isARepeat: false, keyCode: 9) else { return false }
        return presenter.shortcuts.perform(down)
        #elseif os(iOS)
        guard let node = presenter.shortcut(key: "v", held: "Meta+", focus: v) else { return false }
        presenter.press(node.id)
        return true
        #else
        return false
        #endif
    }
}

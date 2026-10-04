// DOM's clipboard events (spreadsheet F4, F14): ⌘C, ⌘X and ⌘V — the Edit
// menu's copy:, cut: and paste:, or a hardware keyboard's on iPadOS — reach
// the focused node as `copy`, `cut` and `paste`, the nearest node with a
// handler (itself or an ancestor) hearing them, as on the web. A paste
// carries the pasteboard's plain text; a copy or cut none, its action
// writing the pasteboard (`copyText`). With no handler a cut or paste is
// not this node's, so the responder chain goes on; a copy is the text
// selection's, as before.
// A text field's or text area's own editing is the platform's, which fires
// none of these.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension NodeView {
    /// The handlers that make a node focusable: the web's rule that only a
    /// focusable element hears these (a clipboard event goes to the focus).
    static let focusEvents: Set<String> = ["focus", "blur", "key", "copy", "cut", "paste"]

    /// The event an edit action is, and the node that hears it here.
    func clipboardTarget(_ action: Selector) -> (kind: UInt32, node: NodeView)? {
        let kinds: [(Selector, String, UInt32)] = [(#selector(copy(_:)), "copy", 32), (#selector(cut(_:)), "cut", 33), (#selector(paste(_:)), "paste", 34)]
        guard field == nil, textArea == nil, let (_, name, kind) = kinds.first(where: { $0.0 == action }) else { return nil }
        #if os(macOS)
        let chain = sequence(first: self as NSView, next: \.superview)
        #else
        let chain = sequence(first: self as UIView, next: \.superview)
        #endif
        guard let node = chain.lazy.compactMap({ $0 as? NodeView }).first(where: { $0.handlers.contains(name) && !$0.disabled }) else { return nil }
        return (kind, node)
    }

    /// Deliver `action` with the pasteboard's text (a paste's), or `text`
    /// in its place (the agent's `type <id> paste <text>`): whether a node heard it.
    @discardableResult
    func clipboard(_ action: Selector, text: String? = nil) -> Bool {
        guard let (kind, node) = clipboardTarget(action) else { return false }
        #if os(macOS)
        let pasted = NSPasteboard.general.string(forType: .string)
        #elseif os(tvOS)
        let pasted: String? = nil // tvOS has no pasteboard.
        #else
        let pasted = UIPasteboard.general.string
        #endif
        presenter?.clipboard(node.id, kind, kind == 34 ? text ?? pasted ?? "" : "")
        return true
    }

    #if os(macOS)
    /// Unheard, a copy is the text selection's (`TextSelection`).
    @objc func copy(_ sender: Any?) { if !clipboard(#selector(copy(_:))) { presenter?.selection.copy() } }
    @objc func cut(_ sender: Any?) { clipboard(#selector(cut(_:))) }
    @objc func paste(_ sender: Any?) { clipboard(#selector(paste(_:))) }
    /// A cut or paste is this node's only while a node hears it.
    override func responds(to aSelector: Selector!) -> Bool {
        if [#selector(cut(_:)), #selector(paste(_:))].contains(aSelector) { return clipboardTarget(aSelector) != nil }
        return super.responds(to: aSelector)
    }
    #else
    override func copy(_ sender: Any?) { if !clipboard(#selector(copy(_:))) { super.copy(sender) } }
    override func cut(_ sender: Any?) { if !clipboard(#selector(cut(_:))) { super.cut(sender) } }
    override func paste(_ sender: Any?) { if !clipboard(#selector(paste(_:))) { super.paste(sender) } }
    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        clipboardTarget(action) != nil || super.canPerformAction(action, withSender: sender)
    }
    #endif
}

extension Agent {
    /// `type <id> copy|cut|paste [text]` (spreadsheet F6): the event ⌘C, ⌘X
    /// or ⌘V delivers with the focus at `v` — a paste carrying `text` as
    /// the pasteboard's, which the driver leaves alone.
    func clipboardType(_ v: NodeView, _ edit: String, _ text: String?) -> [String: Any] {
        let actions = ["copy": #selector(NodeView.copy(_:)), "cut": #selector(NodeView.cut(_:)), "paste": #selector(NodeView.paste(_:))]
        guard let action = actions[edit] else { return ["error": "type: \(edit) is not copy, cut or paste"] }
        guard v.clipboardTarget(action) != nil else { return ["error": "no \(edit) handler at view \(v.id) or above it"] }
        #if os(macOS)
        if v.acceptsFirstResponder, v.window?.firstResponder !== v { v.window?.makeFirstResponder(v) }
        #else
        if v.canBecomeFirstResponder, !v.isFirstResponder { _ = v.becomeFirstResponder() }
        #endif
        v.clipboard(action, text: text ?? "")
        return ["typed": Int(v.id), "clipboard": edit, "delivery": "recognized"]
    }
}

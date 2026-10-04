// A keydown as the web dispatches one (docs/contract-grammar.md#events): at
// the focused node — the field or textarea being edited counts as its node —
// then at every ancestor with a `key` handler, innermost first, and then the
// key's default action unless a handler called `preventDefault()`. Every
// key, printable or not, on every host: calc F3 (macOS's field editor ate
// printable keys) and minesweeper F7 (no way to claim a key) in the
// x2apps diaries.
#if canImport(AppKit)
import AppKit
typealias KeyPlatformView = NSView
#else
import UIKit
typealias KeyPlatformView = UIView
#endif

extension Presenter {
    /// The `key` handlers at `target` and above it hear `name`, the path
    /// fixed before the first runs, as the DOM fixes an event's. True when
    /// one called `preventDefault()`: the caller skips the default action.
    func keyDown(at target: NodeView?, _ name: String) -> Bool {
        var path: [UInt32] = []
        var next: KeyPlatformView? = target
        while let view = next {
            if let node = view as? NodeView, views[node.id] === node, node.handlers.contains("key"), !node.disabled, !node.inert {
                path.append(node.id)
            }
            next = view.superview
        }
        var prevented = false
        for id in path {
            defaultPrevented = false
            key(id, name)
            prevented = prevented || defaultPrevented
        }
        defaultPrevented = false
        return prevented
    }
}

#if canImport(AppKit)
extension Presenter {
    /// A keydown at the window's first responder, before AppKit delivers
    /// it: true when a handler prevented its default, and the caller drops
    /// the event. An input method's composition keeps its keys.
    func keyDown(_ event: NSEvent, in window: NSWindow? = nil) -> Bool {
        guard event.type == .keyDown else { return false }
        let responder = (window ?? event.window)?.firstResponder
        if (responder as? NSTextInputClient)?.hasMarkedText() == true { return false }
        return keyDown(at: keyTarget(responder), NodeView.keyName(event))
    }
    /// The node a responder is the focus of: the node itself, or its field
    /// (and the field editor editing it) or textarea. Any other view a node
    /// holds (a web view, a native module's) keeps its keys, as an iframe's
    /// never reach the page.
    func keyTarget(_ responder: NSResponder?) -> NodeView? {
        guard let view = responder as? NSView else { return nil }
        var next: NSView? = view
        while let v = next {
            if let node = v as? NodeView {
                let owns = node === view || node.field.map { view.isDescendant(of: $0) } == true || node.textArea === view
                return owns && views[node.id] === node ? node : nil
            }
            next = v.superview
        }
        return nil
    }
}

extension NodeView {
    /// The web's `KeyboardEvent.key` for AppKit's: a named key by its name,
    /// any other by the character it types (Shift's included).
    static func keyName(_ event: NSEvent) -> String {
        if let code = KeyCodes.mac[Int(event.keyCode)], KeyCodes.named(code) { return KeyCodes.key(code) }
        return event.charactersIgnoringModifiers ?? ""
    }
}
#else
extension NodeView {
    /// The web's key names for UIKit's.
    static func keyName(_ key: UIKey) -> String {
        let code = KeyCodes.hid(key.keyCode.rawValue)
        return KeyCodes.named(code) ? KeyCodes.key(code) : key.charactersIgnoringModifiers
    }
    /// A hardware key at this node's field or textarea, before UIKit edits
    /// with it: true when a `key` handler prevented its default. An input
    /// method's composition keeps its keys.
    func editorKeyDown(_ presses: Set<UIPress>) -> Bool {
        guard !disabled, let key = presses.first?.key, (field?.markedTextRange ?? textArea?.markedTextRange) == nil else { return false }
        return presenter?.keyDown(at: self, NodeView.keyName(key)) == true
    }
}
#endif

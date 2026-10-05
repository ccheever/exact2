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

extension NodeView {
    /// `disabled` where HTML defines it: a button, an input, a control. It
    /// takes those out of focus and keys; on any other box it means nothing
    /// there, as Chrome's `<div disabled>` (LLP 1088 D7.3, amended
    /// 2026-10-04). A pressable with an `href` is the web's `<a>`, which
    /// `disabled` does not touch either. A press is still refused on any
    /// disabled node.
    var formDisabled: Bool {
        disabled && ["button", "input", "textarea", "control"].contains(kind)
            && !(kind == "button" && props["href"] != nil)
    }
}

extension KeyCodes {
    /// The chord prefix of the modifiers held, in `Event::key`'s spelling.
    static func held(shift: Bool, control: Bool, alt: Bool, meta: Bool) -> String {
        (shift ? "Shift+" : "") + (control ? "Control+" : "") + (alt ? "Alt+" : "") + (meta ? "Meta+" : "")
    }
}

extension NodeView {
    /// HTML's `tabindex` as authored, nil when absent (LLP 1088 D7.3): any
    /// explicit value makes a node focusable — by pointer, script and
    /// `autofocus` — and only one ≥ 0 a Tab stop; a missing one is never
    /// read as `0`, which would make every box a stop.
    var explicitTabIndex: Int? { props["tabIndex"].flatMap { Int($0) } }
    /// The order HTML's sequential navigation sorts by: absent is `0`.
    var tabOrder: Int { explicitTabIndex ?? 0 }
}

extension Presenter {
    /// The `key` handlers at `target` and above it hear `name` with the
    /// modifiers `held` (a chord prefix, `KeyCodes.held`), the path fixed
    /// before the first runs, as the DOM fixes an event's; one that called
    /// `stopPropagation()` is the last. True when one called
    /// `preventDefault()`: the caller skips the default action.
    func keyDown(at target: NodeView?, _ name: String, held: String = "") -> Bool {
        var path: [UInt32] = []
        var next: KeyPlatformView? = target
        while let view = next {
            if let node = view as? NodeView, views[node.id] === node, node.handlers.contains("key"), !node.formDisabled, !node.inert {
                path.append(node.id)
            }
            next = view.superview
        }
        var prevented = false
        for id in path {
            defaultPrevented = false
            propagationStopped = false
            key(id, held + name)
            prevented = prevented || defaultPrevented
            // `stopPropagation()`: no ancestor hears it (files diary F8).
            if propagationStopped { break }
        }
        defaultPrevented = false
        propagationStopped = false
        return prevented
    }
}

#if canImport(AppKit)
extension Presenter {
    /// A key event's route, for the session's local monitor and the agent
    /// alike, in the web's order: the shortcuts (input-glue's capture
    /// listener), the focus's `key` handlers, then the key's defaults here —
    /// Escape closing a popover or dialog, a dialog's Tab — which a prevented
    /// key never reaches. True when taken. `focused`: the window's focus is
    /// in this session (the shortcuts and handlers are its).
    func routeKey(_ event: NSEvent, focused: Bool, in window: NSWindow? = nil) -> Bool {
        if focused {
            if event.type == .keyDown && shortcuts.perform(event) { return true }
            if keyDown(event, in: window) { return true }
        }
        return menus.key(event) || dialogs.key(event)
    }
    /// A keydown at the window's first responder, before AppKit delivers
    /// it: true when a handler prevented its default, and the caller drops
    /// the event. An input method's composition keeps its keys.
    func keyDown(_ event: NSEvent, in window: NSWindow? = nil) -> Bool {
        guard event.type == .keyDown || event.type == .flagsChanged && KeyCodes.pressed(event) else { return false }
        let responder = (window ?? event.window)?.firstResponder
        if (responder as? NSTextInputClient)?.hasMarkedText() == true { return false }
        return keyDown(at: keyTarget(responder), NodeView.keyName(event), held: KeyCodes.held(event.modifierFlags))
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

extension KeyCodes {
    /// A flags change that pressed a modifier key, not one that released it.
    static func pressed(_ event: NSEvent) -> Bool {
        guard let code = mac[Int(event.keyCode)], modifier(code) else { return false }
        let flag: NSEvent.ModifierFlags = code.hasPrefix("Shift") ? .shift : code.hasPrefix("Control") ? .control : code.hasPrefix("Alt") ? .option : .command
        return event.modifierFlags.contains(flag)
    }
    /// The modifiers of a keyDown as the chord prefix its `key` event
    /// carries (`Event::key`): `KeyboardEvent`'s `shiftKey`, `ctrlKey`,
    /// `altKey` (Option) and `metaKey` (Command).
    static func held(_ flags: NSEvent.ModifierFlags) -> String {
        held(shift: flags.contains(.shift), control: flags.contains(.control), alt: flags.contains(.option), meta: flags.contains(.command))
    }
}

extension NodeView {
    /// The web's `KeyboardEvent.key` for AppKit's: a named key by its name,
    /// any other by the character it types (Shift's included), and with
    /// Option by the character Option types, as Chrome reports it (Option+A
    /// is "å"). Control's key stays the unmodified one (its `characters` is a
    /// control character), as the web's does.
    static func keyName(_ event: NSEvent) -> String {
        if let code = KeyCodes.mac[Int(event.keyCode)], KeyCodes.named(code) { return KeyCodes.key(code) }
        // A function key by its AppKit character (NSF1FunctionKey is U+F704,
        // through F35), whatever its virtual key: F21–F24 have none.
        if let s = event.charactersIgnoringModifiers?.unicodeScalars, s.count == 1, let f = s.first,
           (0xF704...0xF726).contains(f.value) { return "F\(f.value - 0xF703)" }
        return KeyCodes.typed(option: event.modifierFlags.contains(.option), characters: event.characters,
                              ignoringModifiers: event.charactersIgnoringModifiers ?? "")
    }
}
#else
extension KeyCodes {
    /// A hardware key's modifiers as its `key` event's chord prefix.
    static func held(_ flags: UIKeyModifierFlags) -> String {
        held(shift: flags.contains(.shift), control: flags.contains(.control), alt: flags.contains(.alternate), meta: flags.contains(.command))
    }
}

extension NodeView {
    /// The web's key names for UIKit's.
    static func keyName(_ key: UIKey) -> String {
        let code = KeyCodes.hid(key.keyCode.rawValue)
        return KeyCodes.named(code) ? KeyCodes.key(code)
            : KeyCodes.typed(option: key.modifierFlags.contains(.alternate), characters: key.characters,
                             ignoringModifiers: key.charactersIgnoringModifiers)
    }
    /// A hardware key at this node's field or textarea, before UIKit edits
    /// with it: true when a `key` handler prevented its default. An input
    /// method's composition keeps its keys.
    func editorKeyDown(_ presses: Set<UIPress>) -> Bool {
        guard !formDisabled, let key = presses.first?.key, (field?.markedTextRange ?? textArea?.markedTextRange) == nil else { return false }
        return presenter?.keyDown(at: self, NodeView.keyName(key), held: KeyCodes.held(key.modifierFlags)) == true
    }
}
#endif

extension KeyCodes {
    /// A character key's `KeyboardEvent.key`: what the key types with its
    /// modifiers when Option is down and that is a character (Chrome's on a
    /// Mac: Option+A is "å", Option+Shift+A "Å"), else what it types
    /// ignoring them but Shift. A dead key's empty text and a control
    /// character (Control held too) keep the plain one.
    static func typed(option: Bool, characters: String?, ignoringModifiers: String) -> String {
        if option, let c = characters, c.unicodeScalars.count == 1, let s = c.unicodeScalars.first,
           !CharacterSet.controlCharacters.contains(s), !(0xF700...0xF8FF).contains(s.value) {
            return c
        }
        return ignoringModifiers
    }
}

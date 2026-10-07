// A keydown as the web dispatches one (docs/contract-grammar.md#events): at
// the focused node — the field or textarea being edited counts as its node —
// then at every ancestor with a `key` handler, innermost first, and then the
// key's default action unless a handler called `preventDefault()`. Every
// key, printable or not, on every host: calc F3 (macOS's field editor ate
// printable keys) and minesweeper F7 (no way to claim a key) in the
// x2apps diaries. A keyup goes the same way to the `keyup` handlers, a
// modifier's release included, and each carries DOM's `code` and `repeat`
// (#140).
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
    /// A chord prefix with one modifier word (`Meta`) added or taken away:
    /// a modifier's own keydown holds it and its keyup no longer does, as
    /// DOM's say (`metaKey` on Meta's keydown, not on its keyup).
    static func held(_ prefix: String, _ modifier: String, _ on: Bool) -> String {
        let words = Set(prefix.split(separator: "+").map(String.init)).subtracting([modifier]).union(on ? [modifier] : [])
        return held(shift: words.contains("Shift"), control: words.contains("Control"), alt: words.contains("Alt"), meta: words.contains("Meta"))
    }
}

/// A key event for the runner (#140): DOM's `keydown` (`key`, ABI kind 6)
/// or `keyup` (43), its chord (`Shift+Meta+b`, `Event::key`), its
/// `KeyboardEvent.code` ("" when the host cannot tell) and `repeat`. It
/// prints as its chord, which the hosts' tests read.
struct KeyPress: ExpressibleByStringLiteral, CustomStringConvertible {
    var chord: String
    var code = ""
    var repeats = false
    var up = false
    init(_ chord: String, code: String = "", repeats: Bool = false, up: Bool = false) {
        self.chord = chord
        self.code = code == "Unidentified" || !code.allSatisfy({ $0.isASCII && ($0.isLetter || $0.isNumber) }) ? "" : code
        self.repeats = repeats && !up
        self.up = up
    }
    init(stringLiteral chord: String) { self.init(chord) }
    var description: String { chord }
    /// The payload `KeyboardEvent::parse` reads.
    var payload: String { code.isEmpty && !repeats ? chord : "\(chord)\n\(code)\n\(repeats)" }
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
    /// modifiers `held` (a chord prefix, `KeyCodes.held`), the physical key
    /// (`code`) and whether it is an auto-repeat, the path fixed before the
    /// first runs, as the DOM fixes an event's; one that called
    /// `stopPropagation()` is the last. True when one called
    /// `preventDefault()`: the caller skips the default action.
    func keyDown(at target: NodeView?, _ name: String, held: String = "", code: String = "", repeats: Bool = false) -> Bool {
        keyEvent("key", at: target, KeyPress(held + name, code: code, repeats: repeats))
    }
    /// The `keyup` handlers at `target` and above hear a key's release, in
    /// the same order (#140). True when one called `preventDefault()`.
    @discardableResult
    func keyUp(at target: NodeView?, _ name: String, held: String = "", code: String = "") -> Bool {
        keyEvent("keyup", at: target, KeyPress(held + name, code: code, up: true))
    }
    private func keyEvent(_ handler: String, at target: NodeView?, _ press: KeyPress) -> Bool {
        var path: [UInt32] = []
        var next: KeyPlatformView? = target
        while let view = next {
            if let node = view as? NodeView, views[node.id] === node, node.handlers.contains(handler), !node.formDisabled, !node.inert {
                path.append(node.id)
            }
            next = view.superview
        }
        var prevented = false
        for id in path {
            defaultPrevented = false
            propagationStopped = false
            key(id, press)
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
            keyboardUsed(event, in: window)
            // A keyup's handlers (#140); AppKit gets the event either way.
            keyUp(event, in: window)
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
        return keyDown(at: keyTarget(responder), NodeView.keyName(event), held: KeyCodes.held(event.modifierFlags),
                       code: KeyCodes.code(event), repeats: event.type == .keyDown && event.isARepeat)
    }
    /// A keyup at the window's first responder, a modifier's (a flags change
    /// that released it) included (#140): the `keyup` handlers there and
    /// above. A keyup has no default here, so nothing is dropped. An input
    /// method's composition keeps its keys, as its keydowns.
    func keyUp(_ event: NSEvent, in window: NSWindow? = nil) {
        guard event.type == .keyUp || event.type == .flagsChanged && KeyCodes.released(event) else { return }
        let responder = (window ?? event.window)?.firstResponder
        if (responder as? NSTextInputClient)?.hasMarkedText() == true { return }
        keyUp(at: keyTarget(responder), NodeView.keyName(event), held: KeyCodes.held(event.modifierFlags), code: KeyCodes.code(event))
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
    static func pressed(_ event: NSEvent) -> Bool { modifierDown(event) == true }
    /// A flags change that released a modifier key (#140).
    static func released(_ event: NSEvent) -> Bool { modifierDown(event) == false }
    /// Whether a flags change pressed (true) or released (false) its
    /// modifier key; nil for any other key (Caps Lock, fn). The NX_DEVICE*
    /// bits tell the sides apart, so releasing one Command while the other is
    /// held is a release (CanvasInputMac's `flags`); an event without them
    /// (a synthesized one) goes by the modifier's flag.
    static func modifierDown(_ event: NSEvent) -> Bool? {
        guard let code = mac[Int(event.keyCode)], let (side, pair, flag) = sides[code] else { return nil }
        let raw = event.modifierFlags.rawValue
        return raw & pair != 0 ? raw & side != 0 : event.modifierFlags.contains(flag)
    }
    static let sides: [String: (UInt, UInt, NSEvent.ModifierFlags)] = [
        "ShiftLeft": (0x2, 0x6, .shift), "ShiftRight": (0x4, 0x6, .shift),
        "ControlLeft": (0x1, 0x2001, .control), "ControlRight": (0x2000, 0x2001, .control),
        "AltLeft": (0x20, 0x60, .option), "AltRight": (0x40, 0x60, .option),
        "MetaLeft": (0x8, 0x18, .command), "MetaRight": (0x10, 0x18, .command),
    ]
    /// A key event's `KeyboardEvent.code`: its physical key, "" for one
    /// with no name here.
    static func code(_ event: NSEvent) -> String { mac[Int(event.keyCode)] ?? "" }
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
        return hardwareKey(key, down: true)
    }
    /// A hardware key's release at this node's field or textarea (#140).
    func editorKeyUp(_ presses: Set<UIPress>) {
        guard !formDisabled, let key = presses.first?.key, (field?.markedTextRange ?? textArea?.markedTextRange) == nil else { return }
        _ = hardwareKey(key, down: false)
    }
    /// A hardware key's `key` (down) or `keyup` handlers at this node and
    /// above, with its `code`; UIKit reports no auto-repeat on a press, so
    /// `repeat` stays false. A modifier's own keydown holds it, its keyup
    /// no longer does, as DOM's.
    func hardwareKey(_ key: UIKey, down: Bool) -> Bool {
        let code = KeyCodes.hid(key.keyCode.rawValue), name = NodeView.keyName(key)
        var held = KeyCodes.held(key.modifierFlags)
        if KeyCodes.modifier(code) { held = KeyCodes.held(held, name, down) }
        return down ? presenter?.keyDown(at: self, name, held: held, code: code) == true
            : presenter?.keyUp(at: self, name, held: held, code: code) == true
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

// The keybinding capture field (KeybindingsSettings.tsx captureKeybinding):
// while it holds focus, the next key chord becomes a T3 binding string —
// `mod+shift+k` — through Exact's `change` event; Escape sends `escape` so the
// row restores its binding. Modifier-only presses are ignored, as
// keybindingFromKeyboardEvent does. The agent's web-named chords
// (`Meta+Shift+K`) arrive through `agentInput` and take the same path.
import Foundation
import AppKit

enum T3KeyChord {
    private static let named: [String: String] = [
        "Enter": "enter", "Tab": "tab", "Escape": "esc", "Backspace": "backspace", "Delete": "delete",
        "ArrowUp": "arrowup", "ArrowDown": "arrowdown", "ArrowLeft": "arrowleft", "ArrowRight": "arrowright",
        "Home": "home", "End": "end", "PageUp": "pageup", "PageDown": "pagedown", "Space": "space", "Plus": "+",
    ]
    private static let codes: [UInt16: String] = [
        36: "enter", 76: "enter", 48: "tab", 53: "esc", 51: "backspace", 117: "delete",
        126: "arrowup", 125: "arrowdown", 123: "arrowleft", 124: "arrowright",
        115: "home", 119: "end", 116: "pageup", 121: "pagedown", 49: "space",
        122: "f1", 120: "f2", 99: "f3", 118: "f4", 96: "f5", 97: "f6", 98: "f7", 100: "f8", 101: "f9", 109: "f10", 103: "f11", 111: "f12",
    ]
    /// The binding for a key token and macOS modifiers: ⌘ is `mod`.
    static func binding(key: String, command: Bool, control: Bool, option: Bool, shift: Bool) -> String? {
        let token = key.lowercased()
        guard !token.isEmpty, !["meta", "control", "ctrl", "shift", "alt", "option"].contains(token) else { return nil }
        var parts: [String] = []
        if command { parts.append("mod") }
        if control { parts.append("ctrl") }
        if option { parts.append("alt") }
        if shift { parts.append("shift") }
        parts.append(token)
        return parts.joined(separator: "+")
    }
    /// A hardware key event; the key is the layout's character without modifiers.
    static func binding(_ event: NSEvent) -> String? {
        let flags = event.modifierFlags
        let key = codes[event.keyCode] ?? (event.charactersIgnoringModifiers ?? "")
        // Shift changes the produced character on some keys; bind the base key, as the reference's `code`-based mapping does.
        let base = key.count == 1 ? (event.characters(byApplyingModifiers: []) ?? key) : key
        return binding(key: base, command: flags.contains(.command), control: flags.contains(.control), option: flags.contains(.option), shift: flags.contains(.shift))
    }
    /// The agent's chord, e.g. `Meta+Shift+K`, `Control+ArrowUp`, `Escape`.
    static func binding(chord: String) -> String? {
        var parts = chord.split(separator: "+", omittingEmptySubsequences: false).map(String.init)
        if chord.hasSuffix("++") { parts.removeLast(2); parts.append("Plus") }
        guard let last = parts.popLast(), !last.isEmpty else { return nil }
        let key = named[last] ?? (last.hasPrefix("F") && Int(last.dropFirst()) != nil ? last.lowercased() : last)
        return binding(key: key, command: parts.contains("Meta"), control: parts.contains("Control"), option: parts.contains("Alt"), shift: parts.contains("Shift"))
    }
}

final class T3KeyRecorderView: NSView {
    weak var owner: T3KeyRecorder?
    var placeholder = "Press shortcut"
    override var acceptsFirstResponder: Bool { true }
    override var isFlipped: Bool { true }
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        // The field appears when recording begins; it takes the keyboard at once.
        if let window { DispatchQueue.main.async { [weak self] in if let self, self.window === window { window.makeFirstResponder(self) } } }
    }
    override func becomeFirstResponder() -> Bool { owner?.events.focus(); return true }
    override func resignFirstResponder() -> Bool { owner?.events.blur(); return true }
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard window?.firstResponder === self, event.type == .keyDown else { return false }
        owner?.capture(event); return true
    }
    override func keyDown(with event: NSEvent) { owner?.capture(event) }
    override func draw(_ dirtyRect: NSRect) {
        let font = NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)
        let attributes: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: NSColor.placeholderTextColor]
        let text = NSAttributedString(string: placeholder, attributes: attributes)
        let size = text.size()
        text.draw(at: NSPoint(x: 0, y: (bounds.height - size.height) / 2))
    }
}

final class T3KeyRecorder: ExactNativeInstance {
    private let field = T3KeyRecorderView(frame: .zero)
    init(props: [String: String], events: ExactNativeEvents) {
        super.init(events: events)
        field.owner = self
        apply(props)
    }
    override var view: ExactNativeView { field }
    override var focusTarget: ExactNativeView? { field }
    private func apply(_ props: [String: String]) {
        field.placeholder = props["prompt"] ?? "Press shortcut"
        field.needsDisplay = true
    }
    override func setProps(_ props: [String: String]) throws { apply(props) }
    func capture(_ event: NSEvent) {
        if event.keyCode == 53 { events.change("escape"); return }
        guard let binding = T3KeyChord.binding(event) else { return }
        events.change(binding)
    }
    override func agentInput(_ input: ExactNativeInput) throws {
        switch input {
        case .text: throw ExactNativeRefusal("the shortcut recorder takes keys, not text")
        case .key(let chord, let phase):
            if phase == "up" { return }
            if chord == "Escape" { events.change("escape"); return }
            guard let binding = T3KeyChord.binding(chord: chord) else { throw ExactNativeRefusal("no key in chord \(chord)") }
            events.change(binding)
        }
    }
}

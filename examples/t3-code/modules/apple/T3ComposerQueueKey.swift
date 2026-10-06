#if os(macOS)
import AppKit

/// thread.editQueuedMessage (⌥↑ by default; task thread-commands-and-keys), after
/// T3 Code 1e2ecbd975 ChatView.tsx: the key edits the last queued message only
/// while the composer's caret sits collapsed before everything (isCaretAtStart);
/// anywhere else it keeps moving the caret, so a second press from the first
/// paragraph reaches the queue. The client files the command as a composer key
/// button anchored `queued-edit|<chord>` (settings-shortcuts.contract), present
/// only when the reference would take the key; this decides from the caret and
/// presses it. A held key is consumed and ignored, as `event.repeat` is.
extension T3Composer {
    /// The web's chord for a key-down (keyboard-dispatch.ts ariaChord: Meta, Control, Alt, Shift, then the key).
    static func ariaChord(_ event: NSEvent) -> String {
        let flags = event.modifierFlags.intersection([.command, .control, .option, .shift])
        var parts: [String] = []
        if flags.contains(.command) { parts.append("Meta") }
        if flags.contains(.control) { parts.append("Control") }
        if flags.contains(.option) { parts.append("Alt") }
        if flags.contains(.shift) { parts.append("Shift") }
        let named: [UInt16: String] = [126: "ArrowUp", 125: "ArrowDown", 123: "ArrowLeft", 124: "ArrowRight", 36: "Enter", 76: "Enter",
                                       53: "Escape", 48: "Tab", 51: "Backspace", 117: "Delete", 116: "PageUp", 121: "PageDown", 115: "Home", 119: "End", 49: "Space"]
        guard let key = named[event.keyCode] ?? event.charactersIgnoringModifiers?.lowercased(), !key.isEmpty else { return "" }
        parts.append(key)
        return parts.joined(separator: "+")
    }

    /// Whether the composer took the key for the queued edit (true consumes it).
    func queuedEditKey(_ event: NSEvent) -> Bool {
        guard event.type == .keyDown, let view = editor.textView, view.isEditable, !view.hasMarkedText(),
              let window = view.window, event.window === window, window.firstResponder === view, window.attachedSheet == nil else { return false }
        let chord = Self.ariaChord(event)
        guard !chord.isEmpty, let key = editor.key("queued-edit|\(chord)") else { return false }
        let selection = view.selectedRange()
        guard selection.location == 0, selection.length == 0 else { return false }
        if !event.isARepeat { key.click() }
        return true
    }
}
#endif

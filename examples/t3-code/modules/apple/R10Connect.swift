#if os(macOS)
import AppKit

/// Lane r10-connect (MIT reference, see LICENSE-T3):
/// - Wake (`r10Wake`): the shell resource reads again now, so a command's state is drawn while
///   the command still waits (the pull request dialog's "Resolving pull request..." during its
///   450 ms debounce, r10-connect-timing.ts); `t3.pr` wakes the pull request panel's resource for
///   the read that follows the ghost it just showed (pr-conversation-and-refresh).
/// - Select on open (hook `t3-select-on-open`): PullRequestThreadDialog focuses its field and
///   selects its text in the frame after it opens (`focus(); select()`).
/// - Letter chords under a non-Latin source (keybindings.ts resolveEventKeys): when a ⌘/⌃ chord's
///   key reports another script (Korean 2-Set's ㅠ for B), the physical key's Latin letter
///   (KeyboardEvent.code) is the chord's key. exact2 #168 matches the declared chords
///   (`aria-keyshortcuts`) and the host's command menu items that way; it does not reach the
///   menu's standard and app-added items (Copy, Paste, Undo, Quit, Close Window, Reload, Paste as Text),
///   the terminal's web view, or this module's own key readers that read the event's characters
///   (the composer's queued-edit chords, the SnapShot shortcut). So the event is still re-issued
///   with that letter, and each of them matches it anywhere in the
///   window. Composition keeps its own keys (the composer ends it first, R9Input.swift).
/// A row that slides under a still pointer (Settle, ⌘Z) is hovered by the host itself, which
/// hit-tests a resting pointer after layout and scrolling as a browser does (exact2 #139).
final class R10Connect {
    private let agent: Bool
    private var monitor: Any?

    init(agent: Bool) {
        self.agent = agent
        DispatchQueue.main.async { [weak self] in self?.installMonitor() }
    }

    // MARK: Wake

    static func wake(_ request: [String: Any], changed: @escaping (String) -> Void, reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let topic = request["topic"] as? String ?? ""
        // `t3.pr`: the pull request panel's resource asks again for the read after the state it showed (pages-pr-detail.ts).
        guard ["t3.notify", "t3.pr"].contains(topic) else {
            return reply(["ok": false, "generation": generation, "error": ["kind": "Arguments", "message": "r10Wake names an unknown topic.", "uncertain": false]])
        }
        DispatchQueue.main.async { changed(topic); reply(["ok": true, "generation": generation, "value": [:]]) }
    }

    // MARK: Elements

    func install(_ element: ExactElement) {
        if element.hatch == .t3SelectOnOpen, element.isNew { selectOnOpen(element) }
    }
    func destroy() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }

    // MARK: Select on open

    private func selectOnOpen(_ element: ExactElement) {
        // After the batch that mounted it (autofocus runs there), as the reference's requestAnimationFrame,
        // and once more a frame later: a remounted field's carried caret (R9Input) must not undo it.
        for delay in [0.0, 0.05] {
            DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak element] in
                guard let element, element.isLive else { return }
                Self.selectAll(element)
            }
        }
    }
    /// Focus the field (when nothing else took it) and select all of its text.
    @discardableResult
    static func selectAll(_ element: ExactElement) -> Bool {
        if let field = element.textField, let window = field.window {
            if field.currentEditor() == nil { window.makeFirstResponder(field) }
            guard let editor = field.currentEditor() else { return false }
            editor.selectAll(nil)
            return true
        }
        if let view = element.textView, let window = view.window {
            if window.firstResponder !== view { window.makeFirstResponder(view) }
            view.selectAll(nil)
            return window.firstResponder === view
        }
        return false
    }

    // MARK: Chords by physical key

    /// ANSI key codes and the Latin key KeyboardEvent.code names for them (US positions).
    static let physicalKeys: [UInt16: String] = [
        0: "a", 11: "b", 8: "c", 2: "d", 14: "e", 3: "f", 5: "g", 4: "h", 34: "i", 38: "j", 40: "k", 37: "l", 46: "m",
        45: "n", 31: "o", 35: "p", 12: "q", 15: "r", 1: "s", 17: "t", 32: "u", 9: "v", 13: "w", 7: "x", 16: "y", 6: "z",
        18: "1", 19: "2", 20: "3", 21: "4", 23: "5", 22: "6", 26: "7", 28: "8", 25: "9", 29: "0",
        50: "`", 42: "\\", 33: "[", 30: "]", 43: ",", 24: "=", 27: "-", 47: ".", 39: "'", 41: ";", 44: "/",
    ]

    /// The chord re-issued with its physical key's Latin character, or nil when the event already
    /// names a Latin key (or is not a ⌘/⌃ chord, or composition owns it).
    static func latinChord(_ event: NSEvent, composing: Bool = false) -> NSEvent? {
        guard event.type == .keyDown, !composing, !event.modifierFlags.intersection([.command, .control]).isEmpty,
              let ignoring = event.charactersIgnoringModifiers, !ignoring.isEmpty,
              !ignoring.unicodeScalars.allSatisfy({ $0.isASCII }), let key = physicalKeys[event.keyCode] else { return nil }
        let letter = key.unicodeScalars.allSatisfy { CharacterSet.lowercaseLetters.contains($0) }
        let typed = letter && event.modifierFlags.contains(.shift) ? key.uppercased() : key
        return NSEvent.keyEvent(with: .keyDown, location: event.locationInWindow, modifierFlags: event.modifierFlags, timestamp: event.timestamp,
                                windowNumber: event.windowNumber, context: nil, characters: typed, charactersIgnoringModifiers: typed,
                                isARepeat: event.isARepeat, keyCode: event.keyCode)
    }
    /// Internal so tests can drive it: what the local monitor returns for `event`.
    func route(_ event: NSEvent) -> NSEvent {
        let composing = (event.window?.firstResponder as? NSTextInputClient)?.hasMarkedText() == true
        return Self.latinChord(event, composing: composing) ?? event
    }
    private func installMonitor() {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown]) { [weak self] event in self?.route(event) ?? event }
    }
}
#endif

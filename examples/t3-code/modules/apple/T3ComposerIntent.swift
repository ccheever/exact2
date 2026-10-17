#if os(macOS)
import AppKit

/// The Return gesture that pressed Send, and the ⌘/Ctrl state the send button follows
/// (T3's useShortcutModifierState). A pointer press carries its own modifiers in its
/// MouseEvent (app.contract `send`, press-modifiers.contract),
/// so the client asks here only for the newest Return key-down when a press came with
/// none, and resolves it through the server's keybindings (composer-editor-intent.ts):
/// ⌘↩ while a turn runs is the follow-up alternate (queue ↔ steer), ⌘↩ or ⌥⌘↩ in a draft
/// starts it in the background, ⌥⌘↩ on an existing thread sends and opens a new thread.
/// It stays native: a Send chord is an `aria-keyshortcuts` press, which precedes every key
/// handler and carries no modifiers, and the composer's plain Return is T3Composer's own
/// monitor's; the held ⌘ is the window's, which Exact hears only at a focused node
/// (KeyEvents.swift keyTarget). A primary click retires the Return gesture before it (a
/// plain click and a key press are alike to a MouseEvent), so a stale one never reaches a
/// later click. Recording never consumes an event.
final class T3ComposerIntent {
    private let changed: (String) -> Void
    private var monitors: [Any] = []
    private var resign: NSObjectProtocol?
    /// "meta", "control", "meta+control" or "" while no such key is held.
    private(set) var held = ""
    private var last: (modifiers: String, source: String, at: TimeInterval)?

    init(changed: @escaping (String) -> Void = { _ in }) {
        self.changed = changed
        if let flags = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged, handler: { [weak self] event in
            self?.track(event.modifierFlags); return event
        }) { monitors.append(flags) }
        if let presses = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .leftMouseDown], handler: { [weak self] event in
            self?.record(event); return event
        }) { monitors.append(presses) }
        resign = NotificationCenter.default.addObserver(forName: NSApplication.didResignActiveNotification, object: nil, queue: .main) { [weak self] _ in
            self?.track([])
        }
    }

    var status: [String: Any] { ["modifiers": held] }

    static func names(_ flags: NSEvent.ModifierFlags) -> String {
        var parts: [String] = []
        if flags.contains(.command) { parts.append("meta") }
        if flags.contains(.control) { parts.append("control") }
        if flags.contains(.option) { parts.append("alt") }
        if flags.contains(.shift) { parts.append("shift") }
        return parts.joined(separator: "+")
    }

    /// Internal so tests can drive it with constructed events.
    func track(_ flags: NSEvent.ModifierFlags) {
        let next = Self.names(flags.intersection([.command, .control]))
        guard next != held else { return }
        held = next
        changed("t3.status")
    }

    func record(_ event: NSEvent, at time: TimeInterval = ProcessInfo.processInfo.systemUptime) {
        if event.type == .leftMouseDown { last = nil; track(event.modifierFlags); return } // its modifiers ride with its press
        guard event.type == .keyDown, event.keyCode == 36 || event.keyCode == 76, !event.isARepeat else { return }
        last = (Self.names(event.modifierFlags.intersection([.command, .control, .option, .shift])), "key", time)
        track(event.modifierFlags)
    }

    /// The newest Return gesture, once: a send never reuses an earlier press.
    func take(at time: TimeInterval = ProcessInfo.processInfo.systemUptime) -> [String: Any] {
        guard let gesture = last else { return [:] }
        last = nil
        return ["modifiers": gesture.modifiers, "source": gesture.source, "ageMs": max(0, (time - gesture.at) * 1000)]
    }

    func destroy() {
        for monitor in monitors { NSEvent.removeMonitor(monitor) }
        monitors.removeAll()
        if let resign { NotificationCenter.default.removeObserver(resign) }
        resign = nil
    }

    deinit { destroy() }
}
#endif

#if os(macOS)
import AppKit

/// Lane r9-input: the composer's focus and composing (marked) text (MIT reference, see LICENSE-T3).
///
/// - Focus: AppKit posts a text view's begin-editing only with its first edit, so a
///   click (or ⌘-less focus) into an existing draft never told the window the
///   composer was focused, and ⌘B kept toggling the sidebar there. The window's
///   first responder is observed instead; when it becomes the composer's text view,
///   the view's own delegate (Exact's node, behind T3ComposerTextDelegate) hears the
///   begin-editing it would have heard on typing. Resigning already ends editing.
/// - Composing text: a ⌘ chord or a press elsewhere ends composition first, as a
///   browser's compositionend does, so ⌘B bolds under Korean 2-Set and a click is
///   never spent committing a syllable.
/// - The transcript's remembered position is Contract's: ChatColumn keeps the first visible
///   key and offset from the list's ScrollEvent and restores them with a keyed
///   `scrollIntoView` (app-main.contract, timeline-and-window-hooks KW-12b).
/// - Diagnostics: R9_INPUT_LOG=<file> logs presses, ⌘ keys and focus.
final class R9Input {
    init() {
        if let path = ProcessInfo.processInfo.environment["R9_INPUT_LOG"], !path.isEmpty { logPath = path }
        holdComposition = ProcessInfo.processInfo.environment["R9_INPUT_HOLD_COMPOSITION"] == "1"
    }
    /// Diagnostics only (R9_INPUT_HOLD_COMPOSITION=1): leave composition to AppKit on a press, for an A/B check.
    private var holdComposition = false

    // MARK: Elements

    func install(_ element: ExactElement) {
        if element.hatch == .t3Composer, let view = element.textView { attachComposer(view) }
        observeWindow()
    }
    func destroy() {
        responder = nil; observedWindow = nil
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }

    // MARK: Composer focus

    private weak var composer: NSTextView?
    private weak var observedWindow: NSWindow?
    private var responder: NSKeyValueObservation?
    private var monitor: Any?
    private(set) var composerFocused = false
    /// Begin-editing bridges sent (tests read it).
    private(set) var bridged = 0

    func attachComposer(_ view: NSTextView) {
        if composer !== view { composer = view; composerFocused = false }
        if monitor == nil {
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseUp, .rightMouseDown, .keyDown]) { [weak self] event in
                self?.handle(event)
                return event
            }
        }
        observeWindow()
    }

    /// Follows the composer's window's first responder (re-attached when the view changes windows).
    func observeWindow() {
        guard let window = composer?.window, window !== observedWindow else { return }
        observedWindow = window
        responder = window.observe(\.firstResponder, options: [.new]) { [weak self] window, _ in
            DispatchQueue.main.async { self?.responderChanged(window) }
        }
        responderChanged(window)
    }

    /// Internal so tests can call it after moving the first responder.
    func responderChanged(_ window: NSWindow) {
        guard let view = composer, view.window === window else { return }
        let focused = window.firstResponder === view
        defer { composerFocused = focused }
        guard focused, !composerFocused else { return }
        bridged += 1
        log("focus-bridge marked=\(view.hasMarkedText())")
        view.delegate?.textDidBeginEditing?(Notification(name: NSText.didBeginEditingNotification, object: view))
    }

    /// A ⌘ chord's letter: `charactersIgnoringModifiers`, or the ANSI key's letter when a
    /// non-Latin source reports another script for it.
    static func chordKey(_ event: NSEvent) -> String {
        let key = event.charactersIgnoringModifiers?.lowercased() ?? ""
        if !key.isEmpty, key.unicodeScalars.allSatisfy({ $0.isASCII }) { return key }
        return ansiLetters[event.keyCode] ?? key
    }
    static let ansiLetters: [UInt16: String] = [11: "b", 34: "i", 0: "a", 6: "z", 13: "w", 40: "k"]

    /// Ends composition in `view` (the input method drops its state, the text stays);
    /// true once no marked text is left.
    @discardableResult
    static func commitMarkedText(_ view: NSTextView) -> Bool {
        guard view.hasMarkedText() else { return true }
        view.inputContext?.discardMarkedText()
        if view.hasMarkedText() { view.unmarkText() }
        return !view.hasMarkedText()
    }

    // MARK: Events

    /// Internal so tests can drive it with synthetic events (`window` stands in for the window a
    /// synthesized event may not carry).
    func handle(_ event: NSEvent, window eventWindow: NSWindow? = nil) {
        let eventWindow = eventWindow ?? event.window
        if event.type == .leftMouseDown || event.type == .rightMouseDown, !holdComposition, let view = composer, view.hasMarkedText(), eventWindow === view.window,
           !view.bounds.contains(view.convert(event.locationInWindow, from: nil)) {
            log("commit-on-press")
            Self.commitMarkedText(view)
        }
        guard logPath != nil else { return }
        switch event.type {
        case .leftMouseDown, .leftMouseUp, .rightMouseDown:
            let hit = event.window?.contentView?.hitTest(event.locationInWindow)
            log("\(event.type == .leftMouseUp ? "up" : event.type == .leftMouseDown ? "down" : "rdown") at=\(Int(event.locationInWindow.x)),\(Int(event.locationInWindow.y)) clicks=\(event.clickCount) hit=\(hit.map { String(describing: type(of: $0)) } ?? "nil"):\(hit?.accessibilityIdentifier() ?? "") responder=\(event.window?.firstResponder.map { String(describing: type(of: $0)) } ?? "nil") marked=\(composer?.hasMarkedText() ?? false) source=\(NSTextInputContext.current?.selectedKeyboardInputSource ?? "-") key=\(NSApp.keyWindow === event.window)")
        case .keyDown where event.modifierFlags.contains(.command):
            log("key chars=\(event.characters ?? "") ignoring=\(event.charactersIgnoringModifiers ?? "") code=\(event.keyCode) chord=\(Self.chordKey(event)) responder=\(event.window?.firstResponder.map { String(describing: type(of: $0)) } ?? "nil") composerFocused=\(composerFocused) marked=\(composer?.hasMarkedText() ?? false) source=\(NSTextInputContext.current?.selectedKeyboardInputSource ?? "-")")
        default: break
        }
    }

    // MARK: Diagnostics

    private var logPath: String?
    private func log(_ line: String) {
        guard let logPath, let data = "\(Date().timeIntervalSince1970) \(line)\n".data(using: .utf8) else { return }
        if let handle = FileHandle(forWritingAtPath: logPath) { handle.seekToEndOfFile(); handle.write(data); handle.closeFile() }
        else { FileManager.default.createFile(atPath: logPath, contents: data) }
    }
}
#endif

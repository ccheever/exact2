#if os(macOS)
import AppKit
import WebKit

/// The `t3-browser` native view (app.json `modules`; browser-surface part 1): the Browser tab's page under its
/// chrome row (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975: apps/web/src/browser/BrowserSurfaceSlot.tsx,
/// HostedBrowserWebview.tsx). The page lives in T3BrowserSessions for as long as its session does; the view
/// shows it while mounted and gives it back when the panel hides or the tab changes, so the page survives both
/// (the reference's slot positions a `<webview>` that the app root owns).
///
/// Props: `tab` (the runtime tab id, `previewRuntimeTabId`), `url` (the session's URL, loaded if the page is new),
/// `profile`, `environment`. The agent can `type` and `press` into the page (real key events, as the terminal's
/// view), and its screenshots read the page (`takeSnapshot`).
///
/// Part 2 (browser-stage.contract): `fit-scale` is the presentation scale of a fixed viewport that does not fit
/// the panel (HostedBrowserWebview's `transform: scale()`): the box's bounds are its frame over that scale, so the
/// web view lays out at the full viewport (times its zoom) and is drawn scaled into the box, and the page measures
/// the viewport it asked for. `keys` (`chord=command` words) are the preview's chords (`when: previewFocus`): while
/// the page has the focus, or the URL field does (`chrome-focus`), a key monitor answers them before the menus do and
/// sends the command as the node's `key` event; with the page focused, ⌘R (or Control-R) always refreshes, as
/// Manager.ts `isPreviewRefreshShortcut` does. The page swallows the keys it handles otherwise, as Electron's guest does.
final class T3BrowserView: ExactNativeInstance {
    static let factory = ExactNativeFactory(snapshot: true) { (owner: ExactModule, props: [String: String], events: ExactNativeEvents) in
        guard let sessions = (owner as? T3BrowserSessionOwner)?.browserSessions else { throw ExactNativeRefusal("t3-browser needs the T3 module") }
        return T3BrowserView(props: props, events: events, sessions: sessions)
    }

    /// The page's box: the borrowed web view fills its bounds, which `fitScale` scales into its frame. Until the layout
    /// first sizes the box (`sized`), it scales nothing and holds no page: its first frame is a placeholder, and the
    /// page would lay out at it over the props' scale (1,578 × 1,183 for a 1280 × 800 tab fitted at 0.41, #352).
    final class Host: NSView {
        override var isFlipped: Bool { true }
        var fitScale: CGFloat = 1 { didSet { if fitScale != oldValue { applyScale() } } }
        private(set) var sized = false
        /// The layout's first size: the view makes or borrows its page then.
        var onSized: (() -> Void)?
        override func setFrameSize(_ newSize: NSSize) {
            super.setFrameSize(newSize)
            let first = !sized && newSize.width > 0 && newSize.height > 0
            if first { sized = true }
            applyScale()
            if first { onSized?() }
        }
        func applyScale() {
            guard sized else { return }
            let scale = fitScale > 0.0001 && fitScale.isFinite ? fitScale : 1
            let target = NSSize(width: frame.width / scale, height: frame.height / scale)
            if bounds.size != target { setBoundsSize(target) }
            for view in subviews where view.frame != bounds { view.frame = bounds }
        }
        override func layout() {
            super.layout()
            guard sized else { return }
            for view in subviews { view.frame = bounds }
        }
    }

    let host = Host(frame: NSRect(x: 0, y: 0, width: 640, height: 480))
    private(set) var props: [String: String]
    private(set) weak var session: T3BrowserSession?
    private let sessions: T3BrowserSessions

    /// Part 2: the preview's chords (`keys`) by chord, and the monitor that answers them.
    private(set) var previewKeys: [String: String] = [:]
    private var keyMonitor: Any?

    init(props: [String: String], events: ExactNativeEvents, sessions: T3BrowserSessions) {
        self.props = props
        self.sessions = sessions
        super.init(events: events)
        host.setAccessibilityElement(false)
        host.onSized = { [weak self] in self?.apply() }
        apply()
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, let command = self.previewCommand(for: event) else { return event }
            self.events.key(command)
            return nil
        }
    }

    override var view: ExactNativeView { host }
    override var focusTarget: ExactNativeView? { session?.web }

    override func setProps(_ next: [String: String]) throws {
        props = next
        apply()
    }

    private func apply() {
        host.fitScale = CGFloat(Double(props["fit-scale"] ?? "") ?? 1)
        previewKeys = Self.parseKeys(props["keys"] ?? "")
        let id = props["tab"] ?? ""
        guard !id.isEmpty else { return release() }
        guard host.sized else { return } // the page is made or borrowed at the box's laid-out size (Host.onSized)
        let next = sessions.ensure(id: id, url: props["url"] ?? "", profile: props["profile"] ?? "default", environment: props["environment"] ?? "", size: host.bounds.size)
        if session !== next { release() }
        session = next
        if next.web.superview !== host {
            next.web.removeFromSuperview()
            next.web.frame = host.bounds
            next.web.autoresizingMask = [.width, .height]
            host.addSubview(next.web)
        }
        sessions.publish()
    }

    /// Gives the page back: only if this view still holds it (a newer view of the same tab may have taken it).
    private func release() {
        if let web = session?.web, web.superview === host { web.removeFromSuperview(); sessions.publish() }
        session = nil
    }

    override func destroy() {
        if let keyMonitor { NSEvent.removeMonitor(keyMonitor) }
        keyMonitor = nil
        release()
    }

    // MARK: Preview keys (part 2)

    /// `chord=command` words (`Meta+r=refresh Meta+Plus=zoom-in`): the last `=` splits, so `Meta+==zoom-in` reads.
    static func parseKeys(_ value: String) -> [String: String] {
        var keys: [String: String] = [:]
        for word in value.split(separator: " ") {
            guard let split = word.lastIndex(of: "="), split != word.startIndex else { continue }
            keys[String(word[..<split])] = String(word[word.index(after: split)...])
        }
        return keys
    }

    /// The key's name in the aria-keyshortcuts grammar (keyboard-dispatch.ts `ariaChord`), from its key code so a
    /// non-Latin input source (Korean 2-Set) still names the Latin key.
    static let keyNames: [UInt16: String] = [0: "a", 11: "b", 8: "c", 2: "d", 14: "e", 3: "f", 5: "g", 4: "h", 34: "i", 38: "j", 40: "k", 37: "l", 46: "m", 45: "n", 31: "o", 35: "p",
                                              12: "q", 15: "r", 1: "s", 17: "t", 32: "u", 9: "v", 13: "w", 7: "x", 16: "y", 6: "z", 29: "0", 18: "1", 19: "2", 20: "3", 21: "4", 23: "5",
                                              22: "6", 26: "7", 28: "8", 25: "9", 24: "=", 27: "-", 69: "Plus", 78: "-"]

    /// The key's name: the layout's own character when it is ASCII (Dvorak's ⌘R is the R key wherever it sits), else
    /// the key code's ANSI name, so a non-Latin input source (Korean 2-Set) still names the Latin key.
    static func keyName(for event: NSEvent) -> String? {
        if let typed = event.charactersIgnoringModifiers?.lowercased(), typed.count == 1, let scalar = typed.unicodeScalars.first,
           scalar.isASCII, scalar.value > 32, scalar.value < 127 { return typed == "+" ? "Plus" : typed }
        return keyNames[event.keyCode]
    }

    /// The chords a key event can stand for: its modifiers and key; a symbol Shift types (`+`) also without Shift (`mod++`),
    /// and Shift-= on the key code as Plus.
    static func chords(for event: NSEvent) -> [String] {
        guard let key = keyName(for: event) else { return [] }
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        func chord(_ name: String, shift: Bool) -> String {
            ([flags.contains(.command) ? "Meta" : nil, flags.contains(.control) ? "Control" : nil, flags.contains(.option) ? "Alt" : nil, shift ? "Shift" : nil].compactMap { $0 } + [name]).joined(separator: "+")
        }
        var chords = [chord(key, shift: flags.contains(.shift))]
        let symbol = key == "Plus" || (key.count == 1 && !(key.first?.isLetter ?? true))
        if flags.contains(.shift), symbol { chords.append(chord(key, shift: false)) }
        if key == "=", flags.contains(.shift) { chords.append(chord("Plus", shift: false)) }
        return chords
    }

    /// Manager.ts `isPreviewRefreshShortcut`: R with ⌘ or Control (or both), without Shift or Option.
    static func isRefreshShortcut(_ event: NSEvent) -> Bool {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        return keyName(for: event) == "r" && (flags.contains(.command) || flags.contains(.control)) && !flags.contains(.shift) && !flags.contains(.option)
    }

    /// Whether the keyboard focus is really in a Browser URL field (`browser-url`): the field editor sits inside the field,
    /// which the host tags with its testId. One Browser surface shows at a time, and Contract's `chrome-focus` names this
    /// tab's field; together they keep a field that left without a blur from claiming the window's keys.
    func urlFieldFocused(in window: NSWindow) -> Bool {
        var view = window.firstResponder as? NSView
        while let current = view {
            if current.accessibilityIdentifier() == "browser-url" { return true }
            view = current.superview
        }
        return false
    }

    /// The preview command a key event runs: only while this page, or this tab's URL field, has the keyboard focus.
    func previewCommand(for event: NSEvent) -> String? {
        guard let web = session?.web, let window = web.window, event.window === window else { return nil }
        let pageFocused = (window.firstResponder as? NSView).map { $0 === web || $0.isDescendant(of: web) } ?? false
        // The URL field counts only while it really has the focus: Contract's `chrome-focus` can outlive a field that left
        // without a blur (the panel hidden, the tab switched).
        guard pageFocused || (props["chrome-focus"] == "true" && urlFieldFocused(in: window)) else { return nil }
        let chords = Self.chords(for: event)
        if pageFocused, Self.isRefreshShortcut(event) { return "refresh" } // isPreviewRefreshShortcut
        return chords.lazy.compactMap { self.previewKeys[$0] }.first
    }

    // MARK: Agent

    override func agentInput(_ input: ExactNativeInput) throws {
        guard let web = session?.web, let window = web.window else { throw ExactNativeRefusal("the browser page is not in a window") }
        if window.firstResponder !== web { window.makeFirstResponder(web) }
        switch input {
        case .text(let text):
            for character in text {
                let key = T3TerminalKeys.event(for: character)
                try deliver(window, key.code, characters: String(character), plain: key.plain, flags: key.flags, phase: nil)
            }
        case .key(let chord, let phase):
            guard let key = T3TerminalKeys.chord(chord) else { throw ExactNativeRefusal("unsupported key \(chord)") }
            try deliver(window, key.code, characters: key.characters, plain: key.plain, flags: key.flags, phase: phase)
        }
    }

    private func deliver(_ window: NSWindow, _ code: UInt16, characters: String, plain: String, flags: NSEvent.ModifierFlags, phase: String?) throws {
        var answered = false
        for type in phase == "up" ? [NSEvent.EventType.keyUp] : phase == "down" ? [.keyDown] : [.keyDown, .keyUp] {
            guard let event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                               windowNumber: window.windowNumber, context: nil, characters: characters,
                                               charactersIgnoringModifiers: plain, isARepeat: false, keyCode: code) else {
                throw ExactNativeRefusal("no key event for \(characters)")
            }
            // Part 2: a preview chord meets the key monitor first, as a real key does (window.sendEvent passes no monitor).
            if type == .keyDown, let command = previewCommand(for: event) { events.key(command); answered = true; continue }
            if type == .keyUp, answered { continue }
            if NSApp.keyWindow === window { NSApp.sendEvent(event) } else { window.sendEvent(event) }
        }
    }

    override func snapshot() throws -> Data {
        guard let web = session?.web else { throw ExactNativeRefusal("no browser page") }
        var png: Data?, failure: String?
        web.takeSnapshot(with: nil) { image, error in
            if let image, let tiff = image.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff) { png = rep.representation(using: .png, properties: [:]) }
            failure = error?.localizedDescription ?? (image == nil ? "no snapshot" : nil)
        }
        let deadline = Date().addingTimeInterval(3)
        while png == nil, failure == nil, Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
        guard let png else { throw ExactNativeRefusal(failure ?? "browser snapshot timed out") }
        return png
    }
}
#endif

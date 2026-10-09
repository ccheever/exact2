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
final class T3BrowserView: ExactNativeInstance {
    static let factory = ExactNativeFactory(snapshot: true) { (owner: ExactModule, props: [String: String], events: ExactNativeEvents) in
        guard let sessions = (owner as? T3BrowserSessionOwner)?.browserSessions else { throw ExactNativeRefusal("t3-browser needs the T3 module") }
        return T3BrowserView(props: props, events: events, sessions: sessions)
    }

    /// The page's box: the borrowed web view fills it.
    final class Host: NSView {
        override var isFlipped: Bool { true }
        override func layout() {
            super.layout()
            for view in subviews { view.frame = bounds }
        }
    }

    let host = Host(frame: NSRect(x: 0, y: 0, width: 640, height: 480))
    private(set) var props: [String: String]
    private(set) weak var session: T3BrowserSession?
    private let sessions: T3BrowserSessions

    init(props: [String: String], events: ExactNativeEvents, sessions: T3BrowserSessions) {
        self.props = props
        self.sessions = sessions
        super.init(events: events)
        host.setAccessibilityElement(false)
        apply()
    }

    override var view: ExactNativeView { host }
    override var focusTarget: ExactNativeView? { session?.web }

    override func setProps(_ next: [String: String]) throws {
        props = next
        apply()
    }

    private func apply() {
        let id = props["tab"] ?? ""
        guard !id.isEmpty else { return release() }
        let next = sessions.ensure(id: id, url: props["url"] ?? "", profile: props["profile"] ?? "default", environment: props["environment"] ?? "")
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

    override func destroy() { release() }

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
        for type in phase == "up" ? [NSEvent.EventType.keyUp] : phase == "down" ? [.keyDown] : [.keyDown, .keyUp] {
            guard let event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                               windowNumber: window.windowNumber, context: nil, characters: characters,
                                               charactersIgnoringModifiers: plain, isARepeat: false, keyCode: code) else {
                throw ExactNativeRefusal("no key event for \(characters)")
            }
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

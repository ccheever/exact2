import AppKit
import WebKit
import XCTest

// browser-surface part 2: the page's zoom (`pageZoom`), its appearance (`prefers-color-scheme`), a fixed viewport
// scaled to fit (T3BrowserView's `fit-scale`) and the preview's chords (the view's key monitor), against the loopback
// fixture of main.swift.
final class BrowserNavigationTests: XCTestCase {
    private var window: NSWindow!
    private var fixture: Fixture!
    private var sessions: T3BrowserSessions!
    private var sent: [(UInt32, String)] = []

    override func setUpWithError() throws {
        fixture = try Fixture()
        fixture.page("/size", "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width'><title>Size</title><style>:root{color-scheme:light dark}body{margin:0;font:16px -apple-system;background:#fff;color:#111}@media (prefers-color-scheme: dark){body{background:#111;color:#eee}}@media (max-width: 500px){h1{color:#1b8a3a}}</style><body><h1>Probe</h1><p id=p></p><input id=f><script>window.keys=0;document.addEventListener('keydown',()=>window.keys++);const show=()=>{document.getElementById('p').textContent=innerWidth+' × '+innerHeight+' · dpr '+devicePixelRatio+' · '+(matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light')};addEventListener('resize',show);matchMedia('(prefers-color-scheme: dark)').addEventListener('change',show);show()</script>")
        fixture.page("/other", "<!doctype html><title>Other</title>")
        sessions = T3BrowserSessions(agent: true, changed: { _ in })
        window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 720, height: 560), styleMask: [.titled], backing: .buffered, defer: false)
        window.orderFrontRegardless()
    }
    override func tearDown() { sessions.sync([]); window.orderOut(nil) }

    private func spin(until condition: () -> Bool, timeout: TimeInterval = 10) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }
    private func evaluate(_ web: WKWebView, _ script: String) -> String {
        var value: String?
        web.evaluateJavaScript(script) { result, error in value = error.map { "error: \($0.localizedDescription)" } ?? (result as? String) ?? "\(result ?? "nil")" }
        spin(until: { value != nil }, timeout: 5)
        return value ?? "timeout"
    }
    private func viewport(_ web: WKWebView) -> String { evaluate(web, "innerWidth + 'x' + innerHeight") }
    /// The page's viewport once WebKit has relaid it out (its web process learns a new size asynchronously).
    private func viewport(_ web: WKWebView, settlingAt expected: String) -> String {
        spin(until: { self.viewport(web) == expected }, timeout: 3)
        return viewport(web)
    }
    private func mounted(_ id: String, frame: NSRect, props extra: [String: String] = [:]) -> (T3BrowserView, T3BrowserSession) {
        let box = Unmanaged.passUnretained(self).toOpaque()
        let events = ExactNativeEvents(fn: { ctx, _, kind, bytes, length in
            let tests = Unmanaged<BrowserNavigationTests>.fromOpaque(ctx!).takeUnretainedValue()
            let text = bytes.map { String(decoding: UnsafeBufferPointer(start: $0, count: Int(length)), as: UTF8.self) } ?? ""
            tests.sent.append((kind, text))
        }, ctx: box, nonce: 1)
        let view = T3BrowserView(props: ["tab": id, "url": "\(fixture.base)/size", "profile": "default", "environment": "env-1"].merging(extra) { _, new in new }, events: events, sessions: sessions)
        view.host.frame = frame
        window.contentView!.addSubview(view.host)
        let session = sessions.sessions[id]!
        spin(until: { session.report["kind"] as? String == "Success" })
        return (view, session)
    }
    /// The page as WebKit draws it (`T3_BROWSER_TEST_DIR`, as main.swift's `save`): offscreen, so it works with the screen locked.
    private func save(_ web: WKWebView, _ name: String) {
        guard let dir = ProcessInfo.processInfo.environment["T3_BROWSER_TEST_DIR"] else { return }
        var done = false
        web.takeSnapshot(with: nil) { image, _ in
            if let image, let tiff = image.tiffRepresentation, let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) {
                try? png.write(to: URL(fileURLWithPath: dir).appendingPathComponent(name))
            }
            done = true
        }
        spin(until: { done }, timeout: 5)
    }
    private func key(_ code: UInt16, _ characters: String, _ flags: NSEvent.ModifierFlags) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                         context: nil, characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!
    }

    func testZoomIsThePagesZoomOnTheReferenceLadderAndOutlivesANavigation() throws {
        let (view, session) = mounted("tab-zoom", frame: NSRect(x: 0, y: 0, width: 600, height: 400))
        XCTAssertEqual(viewport(session.web), "600x400")
        save(session.web, "nav-zoom-100.png")
        let answer = sessions.performNavigation(["op": "browserSet", "tab": "tab-zoom", "zoom": 2, "generation": 4])
        XCTAssertEqual(answer?["generation"] as? Int, 4)
        XCTAssertEqual((answer?["value"] as? [String: Any])?["zoomFactor"] as? Double, 2)
        XCTAssertEqual(viewport(session.web, settlingAt: "300x200"), "300x200", "a 200% page lays out in half the CSS pixels, as Chromium's zoom does")
        save(session.web, "nav-zoom-200.png")
        XCTAssertEqual(((sessions.status["browserTabs"] as? [String: Any])?["tab-zoom"] as? [String: Any])?["zoomFactor"] as? Double, 2)
        XCTAssertTrue(sessions.navigate(id: "tab-zoom", url: "\(fixture.base)/other", profile: "default", environment: "env-1"))
        spin(until: { session.report["title"] as? String == "Other" })
        XCTAssertEqual(session.zoomFactor, 2)
        XCTAssertEqual(evaluate(session.web, "String(innerWidth)"), "300", "the zoom outlives a navigation")
        _ = sessions.performNavigation(["op": "browserSet", "tab": "tab-zoom", "zoom": 1.3])
        XCTAssertEqual(session.zoomFactor, 1.25, "a value off the ladder snaps to its nearest step")
        _ = sessions.performNavigation(["op": "browserSet", "tab": "tab-zoom", "zoom": 9])
        XCTAssertEqual(session.zoomFactor, 5, "500% is the top")
        // Manager.ts nextZoomLevel from the page's own zoom: steps up and down the ladder, and stops at its ends.
        _ = sessions.performNavigation(["op": "browserSet", "tab": "tab-zoom", "zoomStep": "in"])
        XCTAssertEqual(session.zoomFactor, 5)
        for expected in [4.0, 3.0, 2.5] { _ = sessions.performNavigation(["op": "browserSet", "tab": "tab-zoom", "zoomStep": "out"]); XCTAssertEqual(session.zoomFactor, expected) }
        _ = sessions.performNavigation(["op": "browserSet", "tab": "tab-zoom", "zoom": 0.25])
        _ = sessions.performNavigation(["op": "browserSet", "tab": "tab-zoom", "zoomStep": "out"])
        XCTAssertEqual(session.zoomFactor, 0.25, "25% is the bottom")
        _ = sessions.performNavigation(["op": "browserSet", "tab": "tab-zoom", "zoomStep": "in"])
        XCTAssertEqual(session.zoomFactor, 0.33)
        XCTAssertEqual((sessions.performNavigation(["op": "browserSet", "tab": "missing", "zoom": 2])?["value"] as? [String: Any])?["done"] as? Bool, false)
        XCTAssertNil(sessions.performNavigation(["op": "browserNavigate"]), "other ops go to the next area")
        view.destroy()
    }

    func testAppearanceIsWhatThePagePrefers() throws {
        let (view, session) = mounted("tab-scheme", frame: NSRect(x: 0, y: 0, width: 400, height: 300))
        let dark = "String(matchMedia('(prefers-color-scheme: dark)').matches)"
        _ = sessions.performNavigation(["op": "browserSet", "tab": "tab-scheme", "colorScheme": "dark"])
        XCTAssertEqual(session.colorScheme, "dark")
        XCTAssertEqual(evaluate(session.web, dark), "true")
        save(session.web, "nav-appearance-dark.png")
        _ = sessions.performNavigation(["op": "browserSet", "tab": "tab-scheme", "colorScheme": "light"])
        XCTAssertEqual(evaluate(session.web, dark), "false")
        save(session.web, "nav-appearance-light.png")
        _ = sessions.performNavigation(["op": "browserSet", "tab": "tab-scheme", "colorScheme": "system"])
        XCTAssertEqual(session.colorScheme, "system")
        window.appearance = NSAppearance(named: .darkAqua)
        spin(until: { self.evaluate(session.web, dark) == "true" }, timeout: 3)
        XCTAssertEqual(evaluate(session.web, dark), "true", "System follows the window (the app's theme)")
        window.appearance = NSAppearance(named: .aqua)
        spin(until: { self.evaluate(session.web, dark) == "false" }, timeout: 3)
        XCTAssertEqual(evaluate(session.web, dark), "false")
        _ = sessions.performNavigation(["op": "browserSet", "tab": "tab-scheme", "colorScheme": "sepia"])
        XCTAssertEqual(session.colorScheme, "system", "anything else is refused")
        window.appearance = nil
        view.destroy()
    }

    func testAFixedViewportScaledToFitKeepsItsCSSSize() throws {
        let scale = window.backingScaleFactor
        // iPhone 12 Pro (390 × 844) shown at half size: the box is 195 × 422, the page still 390 × 844.
        let (view, session) = mounted("tab-fit", frame: NSRect(x: 10, y: 32, width: 195, height: 422), props: ["fit-scale": "0.5"])
        XCTAssertEqual(viewport(session.web), "390x844")
        save(session.web, "nav-fit-iphone-12-pro.png")
        XCTAssertEqual(session.web.frame.size, NSSize(width: 390, height: 844), "the web view lays out at the full viewport")
        XCTAssertEqual(evaluate(session.web, "String(devicePixelRatio)"), scale.truncatingRemainder(dividingBy: 1) == 0 ? String(Int(scale)) : "\(scale)", "presentation only: the page's device pixels are the screen's")
        // At 150% the same viewport is 585 × 1266 points, here shown at a third.
        _ = sessions.performNavigation(["op": "browserSet", "tab": "tab-fit", "zoom": 1.5])
        try view.setProps(["tab": "tab-fit", "url": "", "profile": "default", "environment": "env-1", "fit-scale": "\(1.0 / 3.0)"])
        view.host.frame = NSRect(x: 10, y: 32, width: 195, height: 422)
        XCTAssertEqual(viewport(session.web, settlingAt: "390x844"), "390x844", "zoomed and fitted, the page keeps its viewport")
        // Fill: the box is the panel and the scale 1.
        try view.setProps(["tab": "tab-fit", "url": "", "profile": "default", "environment": "env-1", "fit-scale": "1"])
        _ = sessions.performNavigation(["op": "browserSet", "tab": "tab-fit", "zoom": 1])
        view.host.frame = NSRect(x: 0, y: 0, width: 640, height: 480)
        XCTAssertEqual(viewport(session.web, settlingAt: "640x480"), "640x480")
        view.destroy()
    }

    func testThePreviewsChordsAreAnsweredWhileThePageOrTheURLFieldHasTheFocus() throws {
        let keys = "Meta+r=refresh Meta+l=focus-url Meta+==zoom-in Meta+Plus=zoom-in Meta+-=zoom-out Meta+0=reset-zoom"
        XCTAssertEqual(T3BrowserView.parseKeys(keys)["Meta+="], "zoom-in")
        let (view, session) = mounted("tab-keys", frame: NSRect(x: 0, y: 0, width: 400, height: 300), props: ["keys": keys])
        let field = NSTextField(frame: NSRect(x: 420, y: 10, width: 100, height: 22))
        window.contentView!.addSubview(field)
        window.makeFirstResponder(field)
        XCTAssertNil(view.previewCommand(for: key(37, "l", .command)), "focus elsewhere: the app's own keys")
        // A stale `chrome-focus` (the URL field left without a blur) claims nothing while another field has the focus.
        try view.setProps(["tab": "tab-keys", "url": "", "profile": "default", "environment": "env-1", "keys": keys, "chrome-focus": "true"])
        XCTAssertNil(view.previewCommand(for: key(37, "l", .command)), "a stale chrome-focus with the composer (or any other field) focused")
        let url = NSTextField(frame: NSRect(x: 420, y: 40, width: 100, height: 22))
        url.setAccessibilityIdentifier("browser-url") // the host tags a Contract field with its testId
        window.contentView!.addSubview(url)
        window.makeFirstResponder(url)
        XCTAssertEqual(view.previewCommand(for: key(37, "l", .command)), "focus-url", "the URL field really has the focus")
        XCTAssertEqual(view.previewCommand(for: key(37, "ㅣ", .command)), "focus-url", "named by key code under the Korean 2-Set source")
        XCTAssertEqual(view.previewCommand(for: key(35, "l", .command)), "focus-url", "named by the layout's character first (Dvorak puts L on the ANSI P key)")
        url.removeFromSuperview()
        try view.setProps(["tab": "tab-keys", "url": "", "profile": "default", "environment": "env-1", "keys": keys, "chrome-focus": "false"])
        window.makeFirstResponder(session.web)
        XCTAssertEqual(view.previewCommand(for: key(24, "=", .command)), "zoom-in")
        XCTAssertEqual(view.previewCommand(for: key(24, "+", [.command, .shift])), "zoom-in", "⌘+ is Shift-= on the key")
        XCTAssertEqual(view.previewCommand(for: key(27, "-", .command)), "zoom-out")
        XCTAssertEqual(view.previewCommand(for: key(29, "0", .command)), "reset-zoom")
        XCTAssertNil(view.previewCommand(for: key(13, "w", .command)), "every other chord is the page's or the app's")
        XCTAssertNil(view.previewCommand(for: key(15, "r", [.command, .shift])), "⇧⌘R is not Refresh")
        try view.setProps(["tab": "tab-keys", "url": "", "profile": "default", "environment": "env-1", "keys": ""])
        XCTAssertEqual(view.previewCommand(for: key(15, "r", .command)), "refresh", "⌘R in the page refreshes whatever the bindings say")
        XCTAssertEqual(view.previewCommand(for: key(15, "r", .control)), "refresh")
        XCTAssertEqual(view.previewCommand(for: key(15, "r", [.command, .control])), "refresh", "⌃⌘R too (Manager.ts: Meta or Control)")
        XCTAssertNil(view.previewCommand(for: key(15, "r", [.command, .option])), "not with Option")
        // Through the app's event loop: the monitor answers before the page and the menus, and the page never sees it.
        try view.setProps(["tab": "tab-keys", "url": "", "profile": "default", "environment": "env-1", "keys": keys])
        _ = evaluate(session.web, "window.keys = 0; 'ok'")
        sent.removeAll()
        NSApp.sendEvent(key(24, "=", .command))
        spin(until: { !self.sent.isEmpty }, timeout: 2)
        XCTAssertEqual(sent.map(\.1), ["zoom-in"])
        XCTAssertEqual(sent.first?.0, 5, "sent as the node's key event")
        XCTAssertEqual(evaluate(session.web, "String(window.keys)"), "0", "the chord never reached the page")
        window.sendEvent(key(0, "a", [])) // the test window is not key: deliver as T3BrowserView.deliver does
        spin(until: { self.evaluate(session.web, "String(window.keys)") != "0" }, timeout: 2)
        XCTAssertEqual(evaluate(session.web, "String(window.keys)"), "1", "a plain key reaches the page")
        view.destroy()
        sent.removeAll()
        NSApp.sendEvent(key(24, "=", .command))
        RunLoop.main.run(until: Date().addingTimeInterval(0.1))
        XCTAssertTrue(sent.isEmpty, "a destroyed view's monitor is gone")
        field.removeFromSuperview()
    }
}

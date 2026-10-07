import AppKit
import WebKit
import XCTest

// X8 (exact2 #107, main #186): the pointer the agent now sends — real mouse events through
// `NSApplication.sendEvent`, a wheel placed in the window — reaches T3 Code's Ghostty surface in the
// terminal's web view: a drag selects, a double click a word, a triple click a line, a wheel scrolls
// the scrollback. Events are built the way `AgentMac.swift` / `AgentMouseMac.swift` build them.
final class TerminalPointerTests: XCTestCase {
    private var window: NSWindow!
    private var view: T3TerminalView!

    override func setUpWithError() throws {
        let app = try XCTUnwrap(ProcessInfo.processInfo.environment["T3_APP_DIR"], "set T3_APP_DIR to examples/t3-code")
        setenv("EXACT_ASSETS", app, 1)
        try XCTSkipUnless(T3TerminalAssets.fileURL("terminal-host.js") != nil, "run terminal-host/build.mjs first")
        window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 720, height: 420), styleMask: [.titled], backing: .buffered, defer: false)
        window.orderFrontRegardless()
        window.makeKey()
        view = T3TerminalView(props: ["terminal": "pointer", "fixture": "render"], events: ExactNativeEvents(fn: { _, _, _, _, _ in }, ctx: nil, nonce: 1), agent: true)
        view.web.frame = window.contentView!.bounds
        window.contentView!.addSubview(view.web)
        spin(until: { self.view.ready || !self.view.lastError.isEmpty }, timeout: 20)
        try XCTSkipUnless(view.ready, "terminal not ready: \(view.lastError)")
        spin(until: { (self.debug()["text"] as? [String])?.contains { $0.hasPrefix("wide:") } == true })
    }
    override func tearDown() { view?.destroy(); window?.orderOut(nil) }

    private func spin(until condition: () -> Bool, timeout: TimeInterval = 10) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }
    private func debug() -> [String: Any] {
        var result: [String: Any]?
        view.debug { result = $0 }
        spin(until: { result != nil }, timeout: 5)
        return result ?? [:]
    }
    /// A window point (bottom-left origin) at `x`, `yFromTop` points down the web view.
    private func point(_ x: CGFloat, _ yFromTop: CGFloat) -> NSPoint { NSPoint(x: x, y: view.web.frame.maxY - yFromTop) }
    private func left(_ type: NSEvent.EventType, _ p: NSPoint, clicks: Int = 1, flags: NSEvent.ModifierFlags = []) {
        NSApp.sendEvent(NSEvent.mouseEvent(with: type, location: p, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                           windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: clicks, pressure: type == .leftMouseUp ? 0 : 1)!)
    }
    private static let placeInWindow: (@convention(c) (CGEvent, CGPoint) -> Void)? =
        dlsym(UnsafeMutableRawPointer(bitPattern: -2), "CGEventSetWindowLocation").map { unsafeBitCast($0, to: (@convention(c) (CGEvent, CGPoint) -> Void).self) }
    private func placed(_ cg: CGEvent, _ p: NSPoint) -> NSEvent {
        let screen = window.convertPoint(toScreen: p)
        cg.location = CGPoint(x: screen.x, y: (NSScreen.screens.first?.frame.height ?? 0) - screen.y)
        cg.setIntegerValueField(CGEventField(rawValue: 51)!, value: Int64(window.windowNumber))
        Self.placeInWindow!(cg, CGPoint(x: p.x, y: window.frame.height - p.y))
        return NSEvent(cgEvent: cg)!
    }
    private func selection() -> String { spin(until: { !self.view.selection.isEmpty }, timeout: 3); return view.selection }

    func testDragSelectsText() {
        let row = 2 * 17.0 + 8
        left(.leftMouseDown, point(4, row))
        for x in stride(from: 20.0, through: 220.0, by: 40.0) { left(.leftMouseDragged, point(x, row)) }
        left(.leftMouseUp, point(220, row))
        let text = selection()
        print("terminal X8 drag selection \(text.debugDescription)")
        XCTAssertFalse(text.isEmpty, "a drag over the canvas selects text; bridge \(view.bridgeLog)")
        // The selection popup settles after the release; under the agent it is reported, not tracked.
        spin(until: { self.view.actions.status["open"] as? Bool == true }, timeout: 3)
        print("terminal X8 selection popup \(view.actions.status)")
        XCTAssertEqual((view.actions.status["items"] as? [[String: Any]])?.compactMap { $0["label"] as? String }, ["Copy"])
    }

    func testDoubleAndTripleClickSelectWordAndLine() {
        let p = point(30, 8)
        for n in 1...2 { left(.leftMouseDown, p, clicks: n); left(.leftMouseUp, p, clicks: n) }
        let word = selection()
        print("terminal X8 double click \(word.debugDescription)")
        XCTAssertFalse(word.isEmpty)
        XCTAssertFalse(word.contains(" "), "a double click selects one word")
        // A pause ends the double click's run (the page's own click counter, surface.ts recordSelectionClick).
        RunLoop.main.run(until: Date().addingTimeInterval(0.8))
        for n in 1...3 { left(.leftMouseDown, p, clicks: n); left(.leftMouseUp, p, clicks: n) }
        spin(until: { self.view.selection != word }, timeout: 3)
        let line = view.selection
        print("terminal X8 triple click \(line.debugDescription)")
        XCTAssertGreaterThan(line.count, word.count, "a triple click selects the line")
    }

    func testRightClickReportsTheContextMenu() {
        let cg = CGEvent(mouseEventSource: nil, mouseType: .rightMouseDown, mouseCursorPosition: .zero, mouseButton: .right)!
        let up = CGEvent(mouseEventSource: nil, mouseType: .rightMouseUp, mouseCursorPosition: .zero, mouseButton: .right)!
        NSApp.sendEvent(placed(cg, point(60, 40))); NSApp.sendEvent(placed(up, point(60, 40)))
        spin(until: { self.view.actions.status["open"] as? Bool == true }, timeout: 3)
        print("terminal X8 context menu \(view.actions.status)")
        XCTAssertEqual((view.actions.status["items"] as? [[String: Any]])?.compactMap { $0["label"] as? String }, ["Copy", "Paste"])
    }

    func testClickAndMetaClickActivateALink() {
        view.resetAndWrite("open src/a.ts:10:5 or https://example.com/docs now\r\n")
        spin(until: { (self.debug()["text"] as? [String])?.first?.hasPrefix("open src") == true }, timeout: 5)
        let cell = view.web.frame.width / CGFloat(max(view.cols, 1))
        let path = point(cell * 8, 8), url = point(cell * 27, 8)
        left(.leftMouseDown, path); left(.leftMouseUp, path)
        left(.leftMouseDown, url, flags: .command); left(.leftMouseUp, url, flags: .command)
        spin(until: { self.view.bridgeLog.filter { $0.hasPrefix("link ") }.count >= 2 }, timeout: 3)
        let links = view.bridgeLog.filter { $0.hasPrefix("link ") }
        print("terminal X8 links \(links)")
        XCTAssertTrue(links.contains { $0.contains("a.ts:10:5") && $0.contains("\"metaKey\":false") }, "\(view.bridgeLog)")
        XCTAssertTrue(links.contains { $0.contains("example.com") && $0.contains("\"metaKey\":true") }, "\(view.bridgeLog)")
    }

    func testScrollbarThumbDragScrollsBack() {
        let before = debug()["text"] as? [String] ?? []
        let geometry = evaluate("const t = document.querySelector('[role=scrollbar]'); const b = t?.firstElementChild?.getBoundingClientRect(); return t ? JSON.stringify({hidden: t.hidden, x: b.x + b.width / 2, y: b.y + b.height / 2, h: b.height}) : 'none'")
        print("terminal X8 scrollbar thumb \(geometry)")
        guard let data = geometry.data(using: .utf8), let thumb = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let x = thumb["x"] as? Double, let y = thumb["y"] as? Double else { return XCTFail("no thumb: \(geometry)") }
        left(.leftMouseDown, point(x, y))
        for step in 1...6 { left(.leftMouseDragged, point(x, y - Double(step) * 40)) }
        left(.leftMouseUp, point(x, y - 240))
        spin(until: { (self.debug()["text"] as? [String] ?? []) != before }, timeout: 5)
        let after = debug()["text"] as? [String] ?? []
        print("terminal X8 thumb drag first row before \(before.first ?? "") after \(after.first ?? "")")
        XCTAssertNotEqual(before, after, "dragging the scrollbar thumb up scrolls back")
    }
    private func evaluate(_ script: String) -> String {
        var value: String?
        view.web.callAsyncJavaScript(script, arguments: [:], in: nil, in: .page) { result in
            if case .success(let any) = result { value = (any as? String) ?? "\(any)" } else { value = "error" }
        }
        spin(until: { value != nil }, timeout: 5)
        return value ?? "timeout"
    }

    func testWheelScrollsTheScrollback() {
        let before = debug()["text"] as? [String] ?? []
        let cg = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 2, wheel1: 600, wheel2: 0, wheel3: 0)!
        NSApp.sendEvent(placed(cg, point(200, 100)))
        spin(until: { (self.debug()["text"] as? [String] ?? []) != before }, timeout: 5)
        let after = debug()["text"] as? [String] ?? []
        print("terminal X8 wheel first row before \(before.first ?? "") after \(after.first ?? "")")
        XCTAssertNotEqual(before, after, "a wheel at a point over the canvas scrolls back")
    }
}

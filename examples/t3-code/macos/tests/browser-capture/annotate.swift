import AppKit
import WebKit
import XCTest

// Annotate's overlay (assets/browser-annotate.js) against fixture pages in a window, run as data code runs it
// (browser-annotate.ts): installed and started in a content world of the page, its outcome awaited through `next()`.
// `callAsyncJavaScript` in a named content world stands in for `@exact/browser-automation`'s `callFunction` and
// `evaluate` with a `sandbox`. Real AppKit mouse and key events go into the web view where the reference's user drives
// the guest (a click selects, Enter attaches, ⌘Enter sends, Escape cancels, drags draw and mark regions); the closed
// shadow root's controls are reached through the overlay's own `test` calls, which the page cannot see. Typing letters
// stays out of real keys (the Mac's input source may compose them). The React read and the crop are data code's, tested
// in browser-capture.test.ts and browser-frames.test.ts.
final class BrowserAnnotateTests: XCTestCase {
    /// The tab's web view takes the first click into an inactive window, as a frame's does.
    final class AnnotateWebView: WKWebView {
        override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    }
    final class Navigation: NSObject, WKNavigationDelegate {
        var finished = 0
        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { finished += 1 }
    }
    /// One `next()` in flight: its JSON once it resolves, or the error that ended it.
    final class Outcome {
        var value: String?
        var error: String?
        var done: Bool { value != nil || error != nil }
    }

    private var fixture: CaptureFixture!
    private var window: NSWindow!
    private var web: AnnotateWebView!
    private let navigation = Navigation()
    private let world = WKContentWorld.world(name: "t3-annotate")
    private var eventNumber = 0

    static let page = """
    <!doctype html><title>Annotate me</title>
    <style>body{margin:0;font:16px system-ui} #save{position:absolute;left:40px;top:60px;width:120px;height:40px} #note{position:absolute;left:40px;top:140px;width:200px;height:30px} .card{position:absolute;left:300px;top:60px;width:160px;height:100px;background:#eee}</style>
    <button id=save class="primary big">Save</button><p id=note>Some note</p><div class=card><span class=inner>Card</span></div>
    <script>window.clicks = 0; document.getElementById('save').addEventListener('click', () => { window.clicks += 1; });</script>
    """
    static let overlaySource: String = {
        let root = ProcessInfo.processInfo.environment["EXACT_ASSETS"] ?? ""
        return (try? String(contentsOfFile: "\(root)/assets/browser-annotate.js", encoding: .utf8)) ?? ""
    }()

    override func setUpWithError() throws {
        XCTAssertFalse(Self.overlaySource.isEmpty, "EXACT_ASSETS names the app directory")
        fixture = try CaptureFixture()
        fixture.page("/", Self.page)
        fixture.page("/other", "<!doctype html><title>Other</title>other")
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        web = AnnotateWebView(frame: NSRect(x: 0, y: 0, width: 800, height: 600), configuration: configuration)
        // As the app's agent runs: a window behind others keeps painting and running animation frames.
        let occlusion = Selector(("_setWindowOcclusionDetectionEnabled:"))
        if web.responds(to: occlusion) { web.perform(occlusion, with: false) }
        web.navigationDelegate = navigation
        window = NSWindow(contentRect: NSRect(x: 60, y: 60, width: 800, height: 600), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = web
        window.orderFrontRegardless()
        try load("/")
    }

    override func tearDown() {
        window.orderOut(nil)
        window.contentView = nil
    }

    // MARK: Helpers

    private func load(_ path: String) throws {
        let before = navigation.finished
        web.load(URLRequest(url: URL(string: "\(fixture.base)\(path)")!))
        CaptureSpin.until({ self.navigation.finished > before })
        XCTAssertGreaterThan(navigation.finished, before, "\(path) loaded")
    }
    /// A function body in the overlay's world, as data code's `callFunction` runs it; its value as a string.
    private func call(_ body: String, _ arguments: [String: Any] = [:]) -> String {
        var value: String?
        web.callAsyncJavaScript(body, arguments: arguments, in: nil, in: world) { result in
            switch result {
            case .success(let output): value = (output as? String) ?? "\(output)"
            case .failure(let error): value = "error: \(error.localizedDescription)"
            }
        }
        CaptureSpin.until({ value != nil }, timeout: 5)
        return value ?? "timeout"
    }
    private func overlay() -> [String: Any]? {
        let text = call("return globalThis.__t3codeAnnotate ? globalThis.__t3codeAnnotate.state() : 'null'")
        return (try? JSONSerialization.jsonObject(with: Data(text.utf8))) as? [String: Any]
    }
    /// browser-annotate.ts startAnnotate: the overlay installs once per document, then starts in the app's theme.
    private func start() {
        let started = call("if (!globalThis.__t3codeAnnotate) {\n\(Self.overlaySource)\n}\nreturn String(globalThis.__t3codeAnnotate.start(theme) === true);",
                           ["theme": ["primary": "rgb(37, 99, 235)"]])
        XCTAssertEqual(started, "true")
        XCTAssertNotNil(overlay(), "the overlay started")
    }
    /// browser-annotate.ts awaitPick: one `next()`, left running until the person picks or cancels.
    private func next() -> Outcome {
        let outcome = Outcome()
        web.callAsyncJavaScript("return globalThis.__t3codeAnnotate ? await globalThis.__t3codeAnnotate.next() : null", arguments: [:], in: nil, in: world) { result in
            switch result {
            case .success(let output): outcome.value = (output as? String) ?? "null"
            case .failure(let error): outcome.error = error.localizedDescription
            }
        }
        return outcome
    }
    private func settled(_ outcome: Outcome) throws -> [String: Any] {
        CaptureSpin.until({ outcome.done })
        let text = try XCTUnwrap(outcome.value, "next() resolved")
        return try XCTUnwrap((try? JSONSerialization.jsonObject(with: Data(text.utf8))) as? [String: Any])
    }
    private func test(_ call: String) {
        XCTAssertEqual(self.call("return String(globalThis.__t3codeAnnotate.test.\(call))"), "true", call)
    }
    /// The element's centre in CSS pixels (the page's world).
    private func centre(_ selector: String) -> NSPoint {
        let text = CaptureSpin.evaluate(web, "(() => { const r = document.querySelector('\(selector)').getBoundingClientRect(); return JSON.stringify([r.left + r.width / 2, r.top + r.height / 2]); })()")
        let values = (try? JSONSerialization.jsonObject(with: Data(text.utf8))) as? [Double] ?? [0, 0]
        return NSPoint(x: values[0], y: values[1])
    }
    private func windowPoint(_ point: NSPoint) -> NSPoint {
        web.convert(web.isFlipped ? point : NSPoint(x: point.x, y: web.bounds.height - point.y), to: nil)
    }
    private func mouse(_ type: NSEvent.EventType, _ point: NSPoint, flags: NSEvent.ModifierFlags = []) {
        eventNumber += 1
        let event = NSEvent.mouseEvent(with: type, location: windowPoint(point), modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                       windowNumber: window.windowNumber, context: nil, eventNumber: eventNumber, clickCount: 1, pressure: type == .leftMouseUp ? 0 : 1)!
        window.sendEvent(event)
    }
    /// A real click: down and up at a page point.
    private func click(_ point: NSPoint) {
        mouse(.leftMouseDown, point)
        mouse(.leftMouseUp, point)
        CaptureSpin.wait(0.2)
    }
    /// A real drag in steps, a frame apart.
    private func drag(from start: NSPoint, to end: NSPoint, steps: Int = 6) {
        mouse(.leftMouseDown, start)
        for step in 1...steps {
            let t = CGFloat(step) / CGFloat(steps)
            mouse(.leftMouseDragged, NSPoint(x: start.x + (end.x - start.x) * t, y: start.y + (end.y - start.y) * t))
            CaptureSpin.wait(0.03) // WebKit coalesces pointer moves within a frame
        }
        mouse(.leftMouseUp, end)
        CaptureSpin.wait(0.25)
    }
    /// A real key (Return, Escape) into the web view.
    private func key(_ code: UInt16, _ characters: String, flags: NSEvent.ModifierFlags = []) {
        if window.firstResponder !== web { window.makeFirstResponder(web) }
        for type in [NSEvent.EventType.keyDown, .keyUp] {
            let event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                                         context: nil, characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!
            window.sendEvent(event)
        }
        CaptureSpin.wait(0.2)
    }

    // MARK: Tests

    func testCommentDirectionFollowsItsTextOnARightToLeftPage() {
        _ = CaptureSpin.evaluate(web, "document.documentElement.dir = 'rtl'; 'ok'")
        start()
        XCTAssertEqual(overlay()?["direction"] as? String, "ltr", "the toolbar keeps its layout")
        test("setComment('Make this clearer')")
        XCTAssertEqual(overlay()?["commentDirection"] as? String, "ltr")
        test("setComment('مرحبا')")
        XCTAssertEqual(overlay()?["commentDirection"] as? String, "rtl")
    }

    func testTheOverlayShowsItsToolsAndKeepsItsShadowRootFromThePage() throws {
        start()
        let state = try XCTUnwrap(overlay())
        XCTAssertEqual(state["tool"] as? String, "select")
        let tools = (state["tools"] as? [[String: Any]] ?? []).map { "\($0["label"] ?? "")|\($0["title"] ?? "")" }
        XCTAssertEqual(tools, ["Select|Select elements (V)", "Region|Draw a region or marquee-select elements (R)", "Draw|Draw freehand (D)", "Erase|Remove an annotation target (E)"])
        XCTAssertEqual(state["editorShown"] as? Bool, false, "the editor waits for a target")
        XCTAssertEqual(state["submitTitle"] as? String, "Attach annotation and screenshot (Enter)")
        XCTAssertEqual(state["placeholder"] as? String, "Describe the change…")
        // The page's own world sees neither the overlay's object nor inside its closed shadow root.
        XCTAssertEqual(CaptureSpin.evaluate(web, "typeof globalThis.__t3codeAnnotate"), "undefined")
        XCTAssertEqual(CaptureSpin.evaluate(web, "String(document.querySelector('[data-t3code-annotation-ui]').shadowRoot)"), "null")
        XCTAssertEqual(CaptureSpin.evaluate(web, "document.documentElement.getAttribute('data-t3code-annotation-tool')"), "select")
        // The tools' keys (V, R, D, E) switch the tool.
        for (key, tool) in [("d", "draw"), ("r", "marquee"), ("e", "erase"), ("v", "select")] {
            _ = call("window.dispatchEvent(new KeyboardEvent('keydown', { key: '\(key)' })); return 'ok'")
            XCTAssertEqual(overlay()?["tool"] as? String, tool)
        }
    }

    func testARealClickSelectsAnElementAndEnterResolvesNextWithThePick() throws {
        start()
        let outcome = next()
        click(centre("#save"))
        let state = try XCTUnwrap(overlay())
        XCTAssertEqual(state["selected"] as? [String], ["button#save.primary.big"], "the click picked the button")
        XCTAssertEqual(state["editorShown"] as? Bool, true)
        XCTAssertEqual(CaptureSpin.evaluate(web, "String(window.clicks)"), "0", "the click never reached the page")
        XCTAssertFalse(outcome.done, "next() waits for the person")
        test("setComment('Make this clearer')")
        key(36, "\r")
        let result = try settled(outcome)
        XCTAssertEqual(result["type"] as? String, "picked")
        XCTAssertEqual(result["submission"] as? String, "attach")
        let payload = try XCTUnwrap(result["annotation"] as? [String: Any])
        XCTAssertEqual(payload["comment"] as? String, "Make this clearer")
        XCTAssertEqual(payload["pageTitle"] as? String, "Annotate me")
        let target = try XCTUnwrap((payload["elements"] as? [[String: Any]])?.first)
        let element = try XCTUnwrap(target["element"] as? [String: Any])
        XCTAssertEqual(element["tagName"] as? String, "button")
        XCTAssertEqual(element["selector"] as? String, "#save")
        XCTAssertTrue((element["htmlPreview"] as? String ?? "").contains(">Save</button>"))
        XCTAssertTrue((element["styles"] as? String ?? "").contains("width: 120px;"))
        let rect = try XCTUnwrap(target["rect"] as? [String: Any])
        XCTAssertEqual(rect["x"] as? Double, 40); XCTAssertEqual(rect["width"] as? Double, 120)
        // The crop's rect: the target grown by 20 pixels, which data code clips and captures.
        let crop = try XCTUnwrap(result["rect"] as? [String: Any])
        XCTAssertEqual([crop["x"], crop["y"], crop["width"], crop["height"]].map { ($0 as? NSNumber)?.doubleValue ?? -1 }, [20, 40, 160, 80])
        // The picked element is marked for the page world's React read until the overlay comes down after the crop.
        XCTAssertEqual(CaptureSpin.evaluate(web, "String(document.querySelectorAll('[data-t3code-pick]').length)"), "1")
        XCTAssertEqual(call("globalThis.__t3codeAnnotate.captured(); return 'ok'"), "ok")
        XCTAssertNil(overlay())
        XCTAssertEqual(CaptureSpin.evaluate(web, "String(document.querySelectorAll('[data-t3code-annotation-ui]').length)"), "0")
    }

    func testCommandEnterSends() throws {
        start()
        let outcome = next()
        click(centre("#save"))
        key(36, "\r", flags: .command)
        XCTAssertEqual(try settled(outcome)["submission"] as? String, "send", "⌘Enter sends")
    }

    func testEscapeResolvesNextCancelledAndTakesTheOverlayDown() throws {
        start()
        let outcome = next()
        click(centre("#save"))
        key(53, "\u{1b}")
        XCTAssertEqual(try settled(outcome)["type"] as? String, "cancelled")
        XCTAssertNil(overlay())
        XCTAssertEqual(CaptureSpin.evaluate(web, "String(document.documentElement.hasAttribute('data-t3code-annotation-tool'))"), "false")
        XCTAssertEqual(CaptureSpin.evaluate(web, "String(document.querySelectorAll('[data-t3code-annotation-ui]').length)"), "0")
    }

    func testAMarqueeOverEmptySpaceMarksARegionDrawMakesAStrokeAndEraseRemovesOne() throws {
        start()
        let outcome = next()
        test("press('marquee')")
        drag(from: NSPoint(x: 560, y: 300), to: NSPoint(x: 700, y: 420))
        XCTAssertEqual(overlay()?["regions"] as? Int, 1, "nothing under the marquee: a region")
        XCTAssertEqual((overlay()?["selected"] as? [Any])?.count, 0)
        test("press('marquee')")
        drag(from: NSPoint(x: 292, y: 56), to: NSPoint(x: 470, y: 170)) // starts under the toolbar (y 10–52)
        XCTAssertEqual(overlay()?["selected"] as? [String], ["span.inner"], "a marquee over the card selects its leaf")
        test("press('draw')")
        drag(from: NSPoint(x: 100, y: 400), to: NSPoint(x: 300, y: 480))
        drag(from: NSPoint(x: 100, y: 520), to: NSPoint(x: 300, y: 560))
        XCTAssertEqual(overlay()?["strokes"] as? Int, 2)
        test("press('erase')")
        click(NSPoint(x: 620, y: 360))
        XCTAssertEqual(overlay()?["regions"] as? Int, 0, "erase removed the region under the press")
        click(NSPoint(x: 300, y: 560))
        XCTAssertEqual(overlay()?["strokes"] as? Int, 1, "erase removed the stroke under the press")
        test("press('submit')")
        let payload = try XCTUnwrap(try settled(outcome)["annotation"] as? [String: Any])
        XCTAssertEqual((payload["regions"] as? [Any])?.count, 0)
        let stroke = try XCTUnwrap((payload["strokes"] as? [[String: Any]])?.first)
        XCTAssertEqual(stroke["color"] as? String, "rgb(37, 99, 235)", "the stroke wears the theme's primary")
        XCTAssertEqual(stroke["width"] as? Int, 4)
        XCTAssertGreaterThan((stroke["points"] as? [Any])?.count ?? 0, 2)
        XCTAssertEqual((payload["elements"] as? [Any])?.count, 1)
    }

    func testAStyleChangeIsRecordedWithItsPreviousValueAndRestoredAtTeardown() throws {
        start()
        let outcome = next()
        click(centre("#save"))
        test("press('adjust')")
        XCTAssertEqual(overlay()?["expanded"] as? Bool, true)
        test("setField('Opacity', '0.5')")
        let change = try XCTUnwrap((overlay()?["styleChanges"] as? [[String: Any]])?.first)
        XCTAssertEqual(change["property"] as? String, "opacity")
        XCTAssertEqual(change["previousValue"] as? String, "1")
        XCTAssertEqual(change["value"] as? String, "0.5")
        XCTAssertEqual(CaptureSpin.evaluate(web, "getComputedStyle(document.getElementById('save')).opacity"), "0.5", "the page shows the edit")
        test("setComment('Fade it')")
        test("press('submit')")
        let recorded = try XCTUnwrap(((try settled(outcome)["annotation"] as? [String: Any])?["styleChanges"] as? [[String: Any]])?.first)
        XCTAssertEqual(recorded["selector"] as? String, "#save", "the change names the element's selector")
        XCTAssertEqual(call("globalThis.__t3codeAnnotate.captured(); return 'ok'"), "ok")
        XCTAssertEqual(CaptureSpin.evaluate(web, "getComputedStyle(document.getElementById('save')).opacity"), "1", "restored at teardown")
        XCTAssertEqual(CaptureSpin.evaluate(web, "document.getElementById('save').getAttribute('style') || ''"), "")
    }

    /// The framework aborts an evaluation in a document that a new one replaced (FrameAutomation.swift `committed`), which
    /// ends data code's `next()`; here the old document's promise is only seen never to resolve with a pick.
    func testANavigationDuringAPickLeavesNoOverlayAndNoPick() throws {
        start()
        let outcome = next()
        click(centre("#save"))
        try load("/other")
        CaptureSpin.wait(0.5)
        XCTAssertFalse((outcome.value ?? "").contains("picked"), outcome.value ?? "")
        XCTAssertNil(overlay(), "the new document has no overlay")
        XCTAssertEqual(CaptureSpin.evaluate(web, "String(document.querySelectorAll('[data-t3code-pick], [data-t3code-annotation-ui]').length)"), "0")
    }

    func testCancelSettlesTheWaitingNextAndANewPickStartsClean() throws {
        start()
        let outcome = next()
        XCTAssertEqual(call("globalThis.__t3codeAnnotate.cancel(); return 'ok'"), "ok")
        XCTAssertEqual(try settled(outcome)["type"] as? String, "cancelled")
        XCTAssertNil(overlay(), "cancel() takes the overlay down")
        start()
        let second = next()
        CaptureSpin.wait(0.2)
        XCTAssertFalse(second.done, "start() dropped the outcomes the last pick left")
        XCTAssertEqual(call("globalThis.__t3codeAnnotate.cancel(); return 'ok'"), "ok")
        XCTAssertEqual(try settled(second)["type"] as? String, "cancelled")
    }
}

import AppKit
import WebKit
import XCTest

// browser-surface part 3 (capture): Annotate (T3BrowserAnnotate.swift, assets/browser-annotate.js) against fixture pages
// in a window, driven by real AppKit mouse and key events into the web view where the reference's user drives the
// guest (a click selects, Enter attaches, ⌘Enter sends, Escape cancels, drags draw and mark regions); the closed
// shadow root's controls are reached through the overlay's own content world (`__t3codeAnnotate.test`), which the
// page cannot see. Typing letters stays out of real keys (the Mac's input source may compose them).
final class BrowserAnnotateTests: XCTestCase {
    /// The tab's web view takes the first click into an inactive window, as T3BrowserWebView does.
    final class AnnotateWebView: WKWebView {
        override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    }
    /// Calls `navigated()` as T3BrowserSession does when a main-frame navigation starts.
    final class Navigation: NSObject, WKNavigationDelegate {
        weak var annotation: T3BrowserAnnotation?
        var finished = 0
        func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) { annotation?.navigated() }
        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { finished += 1 }
    }

    private var fixture: CaptureFixture!
    private var window: NSWindow!
    private var configuration: WKWebViewConfiguration!
    private var web: AnnotateWebView!
    private var annotation: T3BrowserAnnotation!
    private var images: URL!
    private let navigation = Navigation()
    private var changes = 0
    private var eventNumber = 0

    static let page = """
    <!doctype html><title>Annotate me</title>
    <style>body{margin:0;font:16px system-ui} #save{position:absolute;left:40px;top:60px;width:120px;height:40px} #note{position:absolute;left:40px;top:140px;width:200px;height:30px} .card{position:absolute;left:300px;top:60px;width:160px;height:100px;background:#eee}</style>
    <button id=save class="primary big">Save</button><p id=note>Some note</p><div class=card><span class=inner>Card</span></div>
    <script>window.clicks = 0; document.getElementById('save').addEventListener('click', () => { window.clicks += 1; });</script>
    """

    override func setUpWithError() throws {
        fixture = try CaptureFixture()
        fixture.page("/", Self.page)
        fixture.page("/other", "<!doctype html><title>Other</title>other")
        fixture.page("/react", """
        <!doctype html><title>React</title><style>body{margin:0} #go{position:absolute;left:40px;top:60px;width:120px;height:40px}</style>
        <button id=go>Go</button>
        <script>
        function SubmitButton() {} function Form() {}
        const form = { type: Form, _debugSource: { fileName: '/repo/src/App.tsx', lineNumber: 3, columnNumber: 1 }, _debugOwner: null };
        const component = { type: SubmitButton, _debugSource: { fileName: '/repo/src/Form.tsx', lineNumber: 8, columnNumber: 7 }, _debugOwner: form, return: form };
        document.getElementById('go').__reactFiber$abc = { type: 'button', _debugSource: { fileName: '/repo/src/Button.tsx', lineNumber: 12, columnNumber: 5 }, _debugOwner: component, return: component };
        </script>
        """)
        images = CaptureSpin.outputDirectory("annotate-images")
        configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        annotation = T3BrowserAnnotation(configuration: configuration, imageDirectory: images)
        annotation.changed = { [weak self] in self?.changes += 1 }
        web = AnnotateWebView(frame: NSRect(x: 0, y: 0, width: 800, height: 600), configuration: configuration)
        annotation.page = web
        // As the app's agent runs: a window behind others keeps painting and running animation frames.
        let occlusion = Selector(("_setWindowOcclusionDetectionEnabled:"))
        if web.responds(to: occlusion) { web.perform(occlusion, with: false) }
        navigation.annotation = annotation
        web.navigationDelegate = navigation
        window = NSWindow(contentRect: NSRect(x: 60, y: 60, width: 800, height: 600), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = web
        window.orderFrontRegardless()
        try load("/")
    }

    override func tearDown() {
        annotation.close()
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
    private func overlay() -> [String: Any]? {
        let text = CaptureSpin.evaluate(web, "globalThis.__t3codeAnnotate ? globalThis.__t3codeAnnotate.state() : 'null'", world: .defaultClient)
        return (try? JSONSerialization.jsonObject(with: Data(text.utf8))) as? [String: Any]
    }
    private func start() {
        annotation.start(theme: ["primary": "rgb(37, 99, 235)"])
        CaptureSpin.until({ self.overlay() != nil })
        XCTAssertNotNil(overlay(), "the overlay started")
    }
    private func test(_ call: String) {
        XCTAssertEqual(CaptureSpin.evaluate(web, "String(globalThis.__t3codeAnnotate.test.\(call))", world: .defaultClient), "true", call)
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
    private func click(_ point: NSPoint, flags: NSEvent.ModifierFlags = []) {
        mouse(.leftMouseDown, point, flags: flags)
        mouse(.leftMouseUp, point, flags: flags)
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
    /// The page with its overlay as a PNG in `T3_BROWSER_TEST_DIR` (evidence; nothing is asserted on it).
    private func save(_ name: String) {
        guard let directory = ProcessInfo.processInfo.environment["T3_BROWSER_TEST_DIR"] else { return }
        var done = false
        web.takeSnapshot(with: nil) { image, _ in
            if let image, let cgImage = image.cgImage(forProposedRect: nil, context: nil, hints: nil),
               let png = NSBitmapImageRep(cgImage: cgImage).representation(using: .png, properties: [:]) {
                try? png.write(to: URL(fileURLWithPath: directory).appendingPathComponent(name))
            }
            done = true
        }
        CaptureSpin.until({ done }, timeout: 5)
    }
    private func settled() -> [String: Any]? {
        CaptureSpin.until({ self.annotation.report["ready"] as? Bool == true })
        return annotation.take(serial: annotation.serial)
    }

    // MARK: Tests

    func testTheOverlayShowsItsToolsAndKeepsItsShadowRootFromThePage() throws {
        start()
        let state = try XCTUnwrap(overlay())
        XCTAssertEqual(state["tool"] as? String, "select")
        let tools = (state["tools"] as? [[String: Any]] ?? []).map { "\($0["label"] ?? "")|\($0["title"] ?? "")" }
        XCTAssertEqual(tools, ["Select|Select elements (V)", "Region|Draw a region or marquee-select elements (R)", "Draw|Draw freehand (D)", "Erase|Remove an annotation target (E)"])
        XCTAssertEqual(state["editorShown"] as? Bool, false, "the editor waits for a target")
        XCTAssertEqual(state["submitTitle"] as? String, "Attach annotation and screenshot (Enter)")
        XCTAssertEqual(state["placeholder"] as? String, "Describe the change…")
        XCTAssertEqual(annotation.report["active"] as? Bool, true)
        // The page's own world sees neither the overlay's object nor inside its closed shadow root.
        XCTAssertEqual(CaptureSpin.evaluate(web, "typeof globalThis.__t3codeAnnotate"), "undefined")
        XCTAssertEqual(CaptureSpin.evaluate(web, "String(document.querySelector('[data-t3code-annotation-ui]').shadowRoot)"), "null")
        XCTAssertEqual(CaptureSpin.evaluate(web, "document.documentElement.getAttribute('data-t3code-annotation-tool')"), "select")
        // The tools' keys (V, R, D, E) switch the tool; a key in the overlay's own controls would not.
        _ = CaptureSpin.evaluate(web, "window.dispatchEvent(new KeyboardEvent('keydown', { key: 'd' })); 'ok'", world: .defaultClient)
        XCTAssertEqual(overlay()?["tool"] as? String, "draw")
        _ = CaptureSpin.evaluate(web, "window.dispatchEvent(new KeyboardEvent('keydown', { key: 'r' })); 'ok'", world: .defaultClient)
        XCTAssertEqual(overlay()?["tool"] as? String, "marquee")
        _ = CaptureSpin.evaluate(web, "window.dispatchEvent(new KeyboardEvent('keydown', { key: 'e' })); 'ok'", world: .defaultClient)
        XCTAssertEqual(overlay()?["tool"] as? String, "erase")
        _ = CaptureSpin.evaluate(web, "window.dispatchEvent(new KeyboardEvent('keydown', { key: 'v' })); 'ok'", world: .defaultClient)
        XCTAssertEqual(overlay()?["tool"] as? String, "select")
    }

    func testARealClickSelectsAnElementAndEnterAttachesItWithACrop() throws {
        start()
        click(centre("#save"))
        let state = try XCTUnwrap(overlay())
        XCTAssertEqual(state["selected"] as? [String], ["button#save.primary.big"], "the click picked the button")
        XCTAssertEqual(state["editorShown"] as? Bool, true)
        XCTAssertEqual(CaptureSpin.evaluate(web, "String(window.clicks)"), "0", "the click never reached the page")
        test("setComment('Make this clearer')")
        save("annotate-select.png")
        key(36, "\r")
        let result = try XCTUnwrap(settled())
        XCTAssertEqual(result["submission"] as? String, "attach")
        XCTAssertEqual(result["screenshotFailed"] as? Bool, false)
        let payload = try XCTUnwrap(result["annotation"] as? [String: Any])
        XCTAssertEqual(payload["comment"] as? String, "Make this clearer")
        XCTAssertEqual(payload["pageTitle"] as? String, "Annotate me")
        XCTAssertTrue(payload["screenshot"] is NSNull, "the page never supplies the crop")
        let target = try XCTUnwrap((payload["elements"] as? [[String: Any]])?.first)
        let element = try XCTUnwrap(target["element"] as? [String: Any])
        XCTAssertEqual(element["tagName"] as? String, "button")
        XCTAssertEqual(element["selector"] as? String, "#save")
        XCTAssertTrue((element["htmlPreview"] as? String ?? "").contains(">Save</button>"))
        XCTAssertTrue((element["styles"] as? String ?? "").contains("width: 120px;"))
        XCTAssertTrue(element["componentName"] is NSNull, "no React on this page")
        let rect = try XCTUnwrap(target["rect"] as? [String: Any])
        XCTAssertEqual(rect["x"] as? Double, 40); XCTAssertEqual(rect["width"] as? Double, 120)
        // The crop is the target grown by 20 pixels: (20, 40) 160 × 80, written as a PNG draft image.
        let screenshot = try XCTUnwrap(result["screenshot"] as? [String: Any])
        let crop = try XCTUnwrap(screenshot["cropRect"] as? [String: Any])
        XCTAssertEqual([crop["x"], crop["y"], crop["width"], crop["height"]].map { ($0 as? NSNumber)?.doubleValue ?? -1 }, [20, 40, 160, 80])
        XCTAssertEqual(screenshot["name"] as? String, "preview-annotation-\(payload["id"] as? String ?? "").png")
        let id = try XCTUnwrap(screenshot["id"] as? String)
        XCTAssertNotNil(UUID(uuidString: id))
        let file = images.appendingPathComponent("\(id).png")
        let rep = try XCTUnwrap(NSBitmapImageRep(data: try Data(contentsOf: file)))
        let scale = window.backingScaleFactor
        XCTAssertEqual(rep.pixelsWide, Int(160 * scale)); XCTAssertEqual(rep.pixelsHigh, Int(80 * scale))
        XCTAssertEqual(screenshot["width"] as? Int, rep.pixelsWide)
        XCTAssertEqual(screenshot["sizeBytes"] as? Int, try Data(contentsOf: file).count)
        // The overlay is gone and left no mark on the page.
        CaptureSpin.until({ self.overlay() == nil })
        XCTAssertNil(overlay())
        XCTAssertEqual(CaptureSpin.evaluate(web, "String(document.querySelectorAll('[data-t3code-pick], [data-t3code-annotation-ui]').length)"), "0")
        XCTAssertEqual(annotation.report["active"] as? Bool, false)
        XCTAssertNotNil(annotation.take(serial: annotation.serial), "the result stays for an answer that was let go and asks again")
        XCTAssertNil(annotation.take(serial: annotation.serial - 1), "only the newest pick's")
    }

    func testReactsFiberGivesTheComponentNameSourceAndOwnerStack() throws {
        try load("/react")
        start()
        click(centre("#go"))
        XCTAssertEqual(overlay()?["selected"] as? [String], ["button#go"])
        key(36, "\r", flags: .command)
        let result = try XCTUnwrap(settled())
        XCTAssertEqual(result["submission"] as? String, "send", "⌘Enter sends")
        let element = try XCTUnwrap(((result["annotation"] as? [String: Any])?["elements"] as? [[String: Any]])?.first?["element"] as? [String: Any])
        XCTAssertEqual(element["componentName"] as? String, "SubmitButton")
        let source = try XCTUnwrap(element["source"] as? [String: Any])
        XCTAssertEqual(source["fileName"] as? String, "/repo/src/Button.tsx")
        XCTAssertEqual(source["lineNumber"] as? Int, 12)
        XCTAssertEqual(source["functionName"] as? String, "SubmitButton")
        let stack = try XCTUnwrap(element["stack"] as? [[String: Any]])
        XCTAssertEqual(stack.map { $0["functionName"] as? String }, ["SubmitButton", "Form"])
        XCTAssertEqual(stack.first?["fileName"] as? String, "/repo/src/Form.tsx")
        XCTAssertEqual(CaptureSpin.evaluate(web, "String(document.querySelectorAll('[data-t3code-pick]').length)"), "0", "the page world's lookup removed the mark")
    }

    func testEscapeCancelsAndTakesTheOverlayDown() throws {
        start()
        click(centre("#save"))
        key(53, "\u{1b}")
        let result = try XCTUnwrap(settled())
        XCTAssertEqual(result["cancelled"] as? Bool, true)
        XCTAssertNil(overlay())
        XCTAssertEqual(CaptureSpin.evaluate(web, "String(document.documentElement.hasAttribute('data-t3code-annotation-tool'))"), "false")
        XCTAssertEqual(CaptureSpin.evaluate(web, "String(document.querySelectorAll('[data-t3code-annotation-ui]').length)"), "0")
        XCTAssertEqual(annotation.report["active"] as? Bool, false)
    }

    func testAMarqueeOverEmptySpaceMarksARegionDrawMakesAStrokeAndEraseRemovesOne() throws {
        start()
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
        save("annotate-region-draw.png")
        test("press('erase')")
        click(NSPoint(x: 620, y: 360))
        XCTAssertEqual(overlay()?["regions"] as? Int, 0, "erase removed the region under the press")
        click(NSPoint(x: 300, y: 560))
        XCTAssertEqual(overlay()?["strokes"] as? Int, 1, "erase removed the stroke under the press")
        test("press('submit')")
        let result = try XCTUnwrap(settled())
        let payload = try XCTUnwrap(result["annotation"] as? [String: Any])
        XCTAssertEqual((payload["regions"] as? [Any])?.count, 0)
        let stroke = try XCTUnwrap((payload["strokes"] as? [[String: Any]])?.first)
        XCTAssertEqual(stroke["color"] as? String, "rgb(37, 99, 235)", "the stroke wears the theme's primary")
        XCTAssertEqual(stroke["width"] as? Int, 4)
        XCTAssertGreaterThan((stroke["points"] as? [Any])?.count ?? 0, 2)
        XCTAssertEqual((payload["elements"] as? [Any])?.count, 1)
        XCTAssertEqual(result["screenshotFailed"] as? Bool, false)
    }

    func testAStyleChangeIsRecordedWithItsPreviousValueAndRestoredAtTeardown() throws {
        start()
        click(centre("#save"))
        test("press('adjust')")
        XCTAssertEqual(overlay()?["expanded"] as? Bool, true)
        test("setField('Opacity', '0.5')")
        let change = try XCTUnwrap((overlay()?["styleChanges"] as? [[String: Any]])?.first)
        XCTAssertEqual(change["property"] as? String, "opacity")
        XCTAssertEqual(change["previousValue"] as? String, "1")
        XCTAssertEqual(change["value"] as? String, "0.5")
        XCTAssertEqual(CaptureSpin.evaluate(web, "getComputedStyle(document.getElementById('save')).opacity"), "0.5", "the page shows the edit")
        save("annotate-style-panel.png")
        test("setComment('Fade it')")
        test("press('submit')")
        let result = try XCTUnwrap(settled())
        let recorded = try XCTUnwrap(((result["annotation"] as? [String: Any])?["styleChanges"] as? [[String: Any]])?.first)
        XCTAssertEqual(recorded["selector"] as? String, "#save", "the change names the element's selector")
        CaptureSpin.until({ self.overlay() == nil })
        XCTAssertEqual(CaptureSpin.evaluate(web, "getComputedStyle(document.getElementById('save')).opacity"), "1", "restored at teardown")
        XCTAssertEqual(CaptureSpin.evaluate(web, "document.getElementById('save').getAttribute('style') || ''"), "")
    }

    func testANavigationDuringAPickSettlesWithNothing() throws {
        start()
        click(centre("#save"))
        try load("/other")
        let result = try XCTUnwrap(settled())
        XCTAssertEqual(result["cancelled"] as? Bool, true)
        XCTAssertEqual(annotation.report["active"] as? Bool, false)
    }

    func testANewPickReplacesTheOneInFlightAndAPopupsMessagesAreIgnored() throws {
        start()
        XCTAssertEqual(annotation.serial, 0)
        // A pop-up shares the tab's configuration, so the handler; its posts are not the tab's.
        let popup = AnnotateWebView(frame: NSRect(x: 0, y: 0, width: 300, height: 200), configuration: configuration)
        let popupNavigation = Navigation()
        popup.navigationDelegate = popupNavigation
        popup.load(URLRequest(url: URL(string: "\(fixture.base)/other")!))
        CaptureSpin.until({ popupNavigation.finished > 0 })
        _ = CaptureSpin.evaluate(popup, "window.webkit.messageHandlers.t3BrowserAnnotate.postMessage({ type: 'cancelled' }); 'ok'", world: .defaultClient)
        CaptureSpin.wait(0.3)
        XCTAssertEqual(annotation.report["active"] as? Bool, true, "the pop-up's cancel was ignored")
        XCTAssertEqual(annotation.serial, 0)
        // The page's own world cannot post to the handler at all.
        XCTAssertEqual(CaptureSpin.evaluate(web, "String(!!(window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.t3BrowserAnnotate))"), "false")
        start()
        XCTAssertEqual(annotation.serial, 1, "the replaced pick settled")
        XCTAssertEqual(annotation.take(serial: 1)?["cancelled"] as? Bool, true)
        XCTAssertEqual(annotation.report["active"] as? Bool, true)
        annotation.cancel()
        XCTAssertEqual(annotation.serial, 2)
        CaptureSpin.until({ self.overlay() == nil })
        XCTAssertNil(overlay(), "cancel() takes the overlay down")
    }

    func testTheCaptureRectAndThePageWorldsFindingsAreCheckedLikeTheReference() {
        XCTAssertEqual(T3BrowserAnnotation.normalizeCaptureRect(["x": 10.6, "y": -3, "width": 20.2, "height": 5]), CGRect(x: 10, y: 0, width: 21, height: 5))
        XCTAssertNil(T3BrowserAnnotation.normalizeCaptureRect(["x": 0, "y": 0, "width": 0, "height": 5]))
        XCTAssertNil(T3BrowserAnnotation.normalizeCaptureRect(["x": 0, "y": 0, "width": "wide", "height": 5]))
        XCTAssertNil(T3BrowserAnnotation.normalizeCaptureRect(nil))
        let annotation: [String: Any] = ["id": "annotation_1", "elements": [["id": "element_1", "element": ["tagName": "button", "componentName": NSNull(), "source": NSNull(), "stack": []]]]]
        let long = String(repeating: "x", count: 900)
        let merged = T3BrowserAnnotation.merge(annotation, react: ["element_1": ["componentName": long, "stack": [["functionName": "A", "fileName": NSNull(), "lineNumber": Double.infinity, "columnNumber": true], "bogus"], "source": NSNull()]])
        let element = ((merged["elements"] as? [[String: Any]])?.first?["element"] as? [String: Any]) ?? [:]
        XCTAssertEqual((element["componentName"] as? String)?.count, 500, "bounded")
        let frame = (element["stack"] as? [[String: Any]])?.first ?? [:]
        XCTAssertEqual(frame["functionName"] as? String, "A")
        XCTAssertTrue(frame["lineNumber"] is NSNull, "a non-finite number becomes null")
        XCTAssertTrue(frame["columnNumber"] is NSNull, "a boolean is not a number")
        XCTAssertEqual((element["stack"] as? [Any])?.count, 1, "a malformed frame is dropped")
        XCTAssertEqual((element["source"] as? [String: Any])?["functionName"] as? String, "A", "no source: the stack's first frame")
    }
}

import AppKit
import Network
import WebKit
import XCTest

// browser-surface part 5: the previewAutomation host's executor (T3BrowserAutomation.swift, its scripts and
// native input) against a loopback fixture page: every operation's answer as `previewAutomation.respond` carries
// it, evaluate, the snapshot (Playwright's ARIA snapshot, the screenshot, console and network diagnostics), click
// by locator and by point (native in a window, DOM events out of one), type, press (native keys and the macOS
// editing commands), scroll, wait for, status, open and navigate readiness, the host's error answers, the colour
// scheme and Mute. T3 Code 1e2ecbd975 (MIT, see LICENSE-T3) is the reference for the expected answers.
// `T3_APP_DIR` names the example (the vendored Playwright script); `T3_BROWSER_TEST_DIR` receives screenshots.
final class Fixture {
    private let listener: NWListener
    private(set) var paths: [String] = []
    var routes: [String: (Int, String, Data, Double)] = [:]
    private(set) var port: UInt16 = 0
    private let queue = DispatchQueue(label: "automation-fixture")

    init() throws {
        listener = try NWListener(using: .tcp, on: .any)
        let ready = DispatchSemaphore(value: 0)
        listener.stateUpdateHandler = { if case .ready = $0 { ready.signal() } }
        listener.newConnectionHandler = { [weak self] connection in self?.serve(connection) }
        listener.start(queue: queue)
        _ = ready.wait(timeout: .now() + 5)
        port = listener.port?.rawValue ?? 0
    }
    var base: String { "http://127.0.0.1:\(port)" }
    var seen: [String] { queue.sync { paths } }
    func page(_ path: String, _ html: String, delay: Double = 0) { routes[path] = (200, "text/html", Data(html.utf8), delay) }

    private func serve(_ connection: NWConnection) {
        connection.start(queue: queue)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { [weak self] data, _, _, _ in
            guard let self, let data, let head = String(data: data, encoding: .utf8)?.split(separator: "\r\n").first else { return connection.cancel() }
            let path = String(head.split(separator: " ").dropFirst().first ?? "")
            self.paths.append(path)
            let route = String(path.split(separator: "?").first ?? "")
            let (status, type, body, delay) = self.routes[route] ?? (404, "text/html", Data("<title>Not found</title>missing".utf8), 0)
            var response = Data("HTTP/1.1 \(status) \(status == 200 ? "OK" : "Not Found")\r\nContent-Type: \(type)\r\nContent-Length: \(body.count)\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n".utf8)
            response.append(body)
            self.queue.asyncAfter(deadline: .now() + delay) { connection.send(content: response, completion: .contentProcessed { _ in connection.cancel() }) }
        }
    }
    deinit { listener.cancel() }
}

let formPage = """
<!doctype html><title>Form page</title>
<style>body { margin: 0; font: 16px -apple-system; } button { width: 120px; height: 40px; } .tall { height: 3000px; }</style>
<h1>Automation fixture</h1>
<form id=form onsubmit="event.preventDefault(); document.getElementById('submitted').textContent = 'submitted ' + document.getElementById('name').value">
  <label>Name <input id=name name=name></label>
  <button id=go type=button onclick="window.clicks = (window.clicks || 0) + 1; window.trusted = event.isTrusted; this.textContent = 'Go ' + window.clicks">Go</button>
  <textarea id=notes aria-label=Notes></textarea>
</form>
<input id=locked readonly value=locked>
<p id=submitted></p>
<div id=later></div>
<div class=tall></div>
<script>
  window.keys = []; window.log = [];
  for (const type of ['pointerdown', 'mousedown', 'mouseup', 'click']) document.addEventListener(type, (event) => window.log.push(type + '@' + Math.round(event.clientX) + ',' + Math.round(event.clientY) + ':' + event.detail), true);
  document.addEventListener('keydown', (event) => window.keys.push(event.key + ':' + event.isTrusted));
  console.warn('fixture warning', 42);
  fetch('/missing.json').catch(() => {});
  setTimeout(() => { document.getElementById('later').textContent = 'Ready later'; }, 400);
</script>
"""

final class BrowserAutomationTests: XCTestCase {
    private var window: NSWindow!
    private var fixture: Fixture!
    private var sessions: T3BrowserSessions!
    private var automation: T3BrowserAutomation!
    private var responses: [String: [String: Any]] = [:]
    private var calls: [(String, [String: Any])] = []
    /// `preview.open`'s answer in the tests: a new Idle tab.
    private var openSnapshot: [String: Any] = [:]
    private var serial = 0

    override func setUpWithError() throws {
        fixture = try Fixture()
        fixture.page("/form", formPage)
        fixture.page("/b", "<!doctype html><title>Page B</title><p>second page</p>")
        fixture.page("/slow", "<!doctype html><title>Slow</title><p>slow page</p>", delay: 1.2)
        sessions = T3BrowserSessions(agent: true, changed: { _ in })
        automation = T3BrowserAutomation(sessions: sessions)
        sessions.created = { [weak automation] in automation?.prepare($0) }
        sessions.ended = { [weak automation] in automation?.forget($0) }
        automation.rpc = { [weak self] method, payload, _, done in
            guard let self else { return }
            self.calls.append((method, payload))
            if method == "previewAutomation.respond", let id = payload["requestId"] as? String { self.responses[id] = payload; return done(["ok": true]) }
            if method == "preview.open" { return done(["ok": true, "value": self.openSnapshot]) }
            done(["ok": true, "value": [:]])
        }
        window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 900, height: 700), styleMask: [.titled], backing: .buffered, defer: false)
        window.orderFrontRegardless()
    }
    override func tearDown() { sessions.sync([]); window.orderOut(nil) }

    private func spin(until condition: () -> Bool, timeout: TimeInterval = 10) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }
    private func js(_ web: WKWebView, _ script: String) -> Any? {
        var value: Any?, done = false
        web.evaluateJavaScript(script) { result, _ in value = result; done = true }
        spin(until: { done }, timeout: 5)
        return value
    }
    private static let runtime = T3BrowserAutomation.runtimeId(environment: "env-1", thread: "thread-1", epoch: "epoch-1", tab: "tab-1")

    /// A tab's page, in the window (`visible`) or not, loaded at the form page.
    @discardableResult
    private func page(_ path: String = "/form", visible: Bool = true, id: String = runtime) -> T3BrowserSession {
        let session = sessions.ensure(id: id, url: "\(fixture.base)\(path)", profile: "default", environment: "env-1")
        if visible {
            session.web.frame = window.contentView!.bounds
            window.contentView!.addSubview(session.web)
        }
        spin(until: { session.navigation.kind == "Success" })
        return session
    }
    private func plan(_ operation: String, _ input: [String: Any] = [:], extra: [String: Any] = [:], timeoutMs: Int = 15_000, runtimeId: String? = runtime, tabId: String? = "tab-1") -> [String: Any] {
        serial += 1
        var plan: [String: Any] = ["requestId": "preview-\(serial)", "connectionId": "connection-1", "clientId": "preview-client", "operation": operation, "input": input,
                                   "environmentId": "env-1", "threadId": "thread-1", "timeoutMs": timeoutMs, "generation": 1,
                                   "deadline": Date().timeIntervalSince1970 * 1000 + Double(timeoutMs) - min(1_500, ceil(Double(timeoutMs) * 0.2))]
        if let runtimeId { plan["runtimeId"] = runtimeId }
        if let tabId { plan["tabId"] = tabId }
        for (key, value) in extra { plan[key] = value }
        return plan
    }
    /// Runs one plan to its `previewAutomation.respond`.
    @discardableResult
    private func run(_ plan: [String: Any], timeout: TimeInterval = 20) -> [String: Any] {
        let id = plan["requestId"] as! String
        XCTAssertEqual(automation.perform(plan)["accepted"] as? Bool, true)
        spin(until: { self.responses[id] != nil }, timeout: timeout)
        return responses[id] ?? [:]
    }
    private func result(_ response: [String: Any], file: StaticString = #filePath, line: UInt = #line) -> [String: Any] {
        XCTAssertEqual(response["ok"] as? Bool, true, "\(response["error"] ?? "no answer")", file: file, line: line)
        return response["result"] as? [String: Any] ?? [:]
    }
    private func error(_ response: [String: Any]) -> [String: Any] {
        XCTAssertEqual(response["ok"] as? Bool, false)
        return response["error"] as? [String: Any] ?? [:]
    }

    // MARK: Responses

    func testEveryAnswerGoesBackOnItsConnectionOnce() {
        page()
        let request = plan("status")
        let response = run(request)
        XCTAssertEqual(response["clientId"] as? String, "preview-client")
        XCTAssertEqual(response["connectionId"] as? String, "connection-1")
        XCTAssertEqual(response["requestId"] as? String, request["requestId"] as? String)
        XCTAssertEqual(automation.perform(request)["duplicate"] as? Bool, true, "a plan delivered again (a let-go answer sent it) runs once")
        XCTAssertEqual(calls.filter { $0.0 == "previewAutomation.respond" }.count, 1)
    }

    // MARK: Evaluate

    func testEvaluateReturnsSerializableValuesFromThePage() {
        let session = page()
        XCTAssertEqual(run(plan("evaluate", ["expression": "document.title"]))["result"] as? String, "Form page")
        XCTAssertEqual(run(plan("evaluate", ["expression": "new Promise(resolve => setTimeout(() => resolve({ a: [1, 2] }), 50))"]))["result"] as? NSDictionary, ["a": [1, 2]] as NSDictionary, "a Promise is awaited by default")
        XCTAssertEqual(run(plan("evaluate", ["expression": "const answer = 40; answer + 2"]))["result"] as? Int, 42, "a script runs as a script")
        XCTAssertNil(run(plan("evaluate", ["expression": "undefined"]))["result"], "undefined has no value (the MCP handler answers null)")
        XCTAssertEqual(run(plan("evaluate", ["expression": "document.body"]))["result"] as? NSDictionary, [:] as NSDictionary, "a DOM node by value is {}, as CDP's returnByValue")
        XCTAssertEqual(run(plan("evaluate", ["expression": "window.clicks = 7"]))["result"] as? Int, 7)
        XCTAssertEqual(js(session.web, "window.clicks") as? Int, 7, "evaluate runs in the page's world: it may mutate page state")
        let thrown = error(run(plan("evaluate", ["expression": "(() => { throw new Error('boom') })()"])))
        XCTAssertEqual(thrown["_tag"] as? String, "PreviewAutomationExecutionError")
        XCTAssertFalse((thrown["message"] as? String ?? "").contains("boom"), "the cause stays in the host log")
        let large = error(run(plan("evaluate", ["expression": "'x'.repeat(70000)"])))
        XCTAssertEqual(large["_tag"] as? String, "PreviewAutomationExecutionError", "a result over 64 KB is refused")
    }

    // MARK: Snapshot

    func testSnapshotReportsThePageItsDiagnosticsAndAScreenshot() throws {
        let session = page()
        spin(until: { (self.js(session.web, "document.getElementById('later').textContent") as? String) == "Ready later" })
        _ = run(plan("click", ["locator": "role=button[name='Go']"]))
        let snapshot = result(run(plan("snapshot")))
        XCTAssertEqual(snapshot["url"] as? String, "\(fixture.base)/form")
        XCTAssertEqual(snapshot["title"] as? String, "Form page")
        XCTAssertEqual(snapshot["loading"] as? Bool, false)
        XCTAssertTrue((snapshot["visibleText"] as? String ?? "").contains("Automation fixture"))
        let elements = snapshot["interactiveElements"] as? [[String: Any]] ?? []
        XCTAssertTrue(elements.contains { $0["selector"] as? String == "#go" && $0["tag"] as? String == "button" }, "\(elements)")
        XCTAssertTrue(elements.contains { $0["selector"] as? String == "#notes" && $0["name"] as? String == "Notes" })
        let tree = try XCTUnwrap(snapshot["accessibilityTree"] as? [String: Any])
        XCTAssertEqual(tree["format"] as? String, "playwright-aria-snapshot")
        XCTAssertTrue((tree["snapshot"] as? String ?? "").contains("button \"Go 1\""), "\(tree)")
        let console = snapshot["consoleEntries"] as? [[String: Any]] ?? []
        XCTAssertTrue(console.contains { $0["level"] as? String == "warning" && $0["text"] as? String == "fixture warning 42" && $0["source"] as? String == "console" }, "\(console)")
        let network = snapshot["networkEntries"] as? [[String: Any]] ?? []
        XCTAssertTrue(network.contains { ($0["url"] as? String ?? "").hasSuffix("/missing.json") && $0["status"] as? Int == 404 && $0["failed"] as? Bool == true }, "\(network)")
        let actions = snapshot["actionTimeline"] as? [[String: Any]] ?? []
        XCTAssertEqual(actions.first?["action"] as? String, "click")
        XCTAssertEqual(actions.first?["status"] as? String, "succeeded")
        XCTAssertEqual(actions.last?["action"] as? String, "snapshot")
        let shot = try XCTUnwrap(snapshot["screenshot"] as? [String: Any])
        XCTAssertEqual(shot["mimeType"] as? String, "image/png")
        XCTAssertLessThanOrEqual(shot["width"] as? Int ?? 0, 1280)
        let png = try XCTUnwrap(Data(base64Encoded: shot["data"] as? String ?? ""))
        XCTAssertEqual(NSBitmapImageRep(data: png)?.pixelsWide, shot["width"] as? Int)
        if let dir = ProcessInfo.processInfo.environment["T3_BROWSER_TEST_DIR"] { try png.write(to: URL(fileURLWithPath: dir).appendingPathComponent("automation-snapshot.png")) }
    }

    func testAPageInNoWindowStillAnswersASnapshot() throws {
        page(visible: false)
        let snapshot = result(run(plan("snapshot")))
        XCTAssertEqual(snapshot["title"] as? String, "Form page")
        let shot = try XCTUnwrap(snapshot["screenshot"] as? [String: Any])
        XCTAssertGreaterThan(shot["width"] as? Int ?? 0, 0)
    }

    // MARK: Input

    func testClickByLocatorAndByPointIsANativeClickInAWindow() {
        let session = page()
        _ = result(run(plan("click", ["locator": "role=button[name='Go']"])))
        XCTAssertEqual(js(session.web, "window.clicks") as? Int, 1)
        XCTAssertEqual(js(session.web, "window.trusted") as? Bool, true, "an NSEvent click is trusted in the page")
        let point = js(session.web, "(() => { const r = document.getElementById('go').getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; })()") as? [Double] ?? [0, 0]
        _ = result(run(plan("click", ["x": point[0], "y": point[1]])))
        XCTAssertEqual(js(session.web, "window.clicks") as? Int, 2, "\(js(session.web, "window.log.join(' ')") ?? "")")
        _ = result(run(plan("click", ["selector": "#go"])))
        XCTAssertEqual(js(session.web, "window.clicks") as? Int, 3, "a legacy CSS selector is css=…")
        XCTAssertNotEqual(window.firstResponder, session.web, "the page lets the window's focus go after the click")
        let missing = error(run(plan("click", ["locator": "role=button[name='Nope']"], timeoutMs: 3_000)))
        XCTAssertEqual(missing["_tag"] as? String, "PreviewAutomationExecutionError")
        let outside = error(run(plan("click", ["x": 5_000, "y": 10])))
        XCTAssertEqual(outside["_tag"] as? String, "PreviewAutomationExecutionError", "coordinates outside the viewport")
        let pointer = automation.status["browserAutomation"] as? [String: Any]
        let tab = (pointer?["tabs"] as? [String: Any])?[Self.runtime] as? [String: Any]
        XCTAssertEqual((tab?["pointer"] as? [String: Any])?["phase"] as? String, "click", "the agent cursor's last event")
    }

    func testClicksInQuickSuccessionEachLand() {
        let session = page()
        let point = js(session.web, "(() => { const r = document.getElementById('go').getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; })()") as? [Double] ?? [0, 0]
        for _ in 0..<4 { _ = result(run(plan("click", ["x": point[0], "y": point[1]]))) }
        XCTAssertEqual(js(session.web, "window.clicks") as? Int, 4)
    }

    func testAClickOnAPageInNoWindowIsDOMEvents() {
        let session = page(visible: false)
        _ = result(run(plan("click", ["locator": "role=button[name='Go']"])))
        XCTAssertEqual(js(session.web, "window.clicks") as? Int, 1)
        XCTAssertEqual(js(session.web, "window.trusted") as? Bool, false, "X1 path B: a hidden page gets untrusted DOM events")
    }

    func testTypeInsertsTextAndClearReplacesIt() {
        let session = page()
        _ = result(run(plan("type", ["locator": "#name", "text": "Ada"])))
        XCTAssertEqual(js(session.web, "document.getElementById('name').value") as? String, "Ada")
        XCTAssertNotEqual(window.firstResponder, session.web, "typing edits the page without taking the window's focus")
        _ = result(run(plan("type", ["locator": "#name", "text": "Grace", "clear": true])))
        XCTAssertEqual(js(session.web, "document.getElementById('name').value") as? String, "Grace")
        _ = result(run(plan("type", ["text": " Hopper"])))
        XCTAssertEqual(js(session.web, "document.getElementById('name').value") as? String, "Grace Hopper", "no target: the focused element")
        let locked = error(run(plan("type", ["selector": "#locked", "text": "x"])))
        XCTAssertEqual(locked["_tag"] as? String, "PreviewAutomationTargetNotEditableError")
        XCTAssertEqual((locked["detail"] as? [String: Any])?["selectorKind"] as? String, "selector")
        XCTAssertEqual((locked["detail"] as? [String: Any])?["selectorLength"] as? Int, 7)
    }

    func testPressSendsNativeKeysAndEditingCommands() {
        let session = page()
        _ = result(run(plan("type", ["locator": "#name", "text": "Ada"])))
        _ = result(run(plan("press", ["key": "Enter"], extra: ["key": ["chord": "Enter", "text": "\r"]])))
        spin(until: { (self.js(session.web, "document.getElementById('submitted').textContent") as? String) == "submitted Ada" }, timeout: 3)
        XCTAssertEqual(js(session.web, "document.getElementById('submitted').textContent") as? String, "submitted Ada", "Enter submits the form")
        XCTAssertTrue((js(session.web, "window.keys.join(',')") as? String ?? "").contains("Enter:true"), "a native key is trusted")
        _ = result(run(plan("press", ["key": "b"], extra: ["key": ["chord": "b", "text": "b"]])))
        XCTAssertEqual(js(session.web, "document.getElementById('name').value") as? String, "Adab", "a printable key types its character")
        let selectAll = "(() => { let element = document.activeElement; if (!element) return; element.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', metaKey: true, bubbles: true })); document.execCommand('selectAll'); element.dispatchEvent(new KeyboardEvent('keyup', { key: 'a', metaKey: true, bubbles: true })); })()"
        _ = result(run(plan("press", ["key": "a", "modifiers": ["Meta"]], extra: ["key": ["chord": "Meta+a", "commands": ["selectAll"], "editing": selectAll]])))
        XCTAssertEqual(js(session.web, "document.getElementById('name').selectionEnd - document.getElementById('name').selectionStart") as? Int, 4, "⌘A selects the field's text in the page")
        XCTAssertNotEqual(window.firstResponder, session.web, "the page lets the window's focus go after the key")
    }

    func testScrollWaitForAndTheirFailures() {
        let session = page()
        _ = result(run(plan("scroll", ["deltaY": 400])))
        XCTAssertEqual(js(session.web, "Math.round(window.scrollY)") as? Int, 400)
        _ = result(run(plan("waitFor", ["text": "Ready later", "timeoutMs": 5_000])))
        _ = result(run(plan("waitFor", ["locator": "text=Ready later", "urlIncludes": "/form"])))
        let missing = error(run(plan("waitFor", ["text": "Never", "timeoutMs": 300])))
        XCTAssertEqual(missing["_tag"] as? String, "PreviewAutomationExecutionError", "the desktop's wait timeout is an execution failure at the response")
        let scrollMissing = error(run(plan("scroll", ["selector": "#nothing", "deltaY": 10])))
        XCTAssertEqual(scrollMissing["_tag"] as? String, "PreviewAutomationExecutionError")
    }

    // MARK: Status, navigate, open

    func testStatusReadsThePageAndFallsBackToTheSnapshot() {
        page()
        let status = result(run(plan("status", extra: ["viewportSetting": ["_tag": "fill"]])))
        XCTAssertEqual(status["available"] as? Bool, true)
        XCTAssertEqual(status["visible"] as? Bool, true)
        XCTAssertEqual(status["tabId"] as? String, "tab-1")
        XCTAssertEqual(status["url"] as? String, "\(fixture.base)/form")
        XCTAssertEqual(status["title"] as? String, "Form page")
        XCTAssertEqual(status["loading"] as? Bool, false)
        XCTAssertEqual((status["viewportSetting"] as? [String: Any])?["_tag"] as? String, "fill")
        XCTAssertEqual((status["viewport"] as? [String: Any])?["width"] as? Int, 900)
        let fallback = result(run(plan("status", extra: ["fallback": ["url": "https://example.com/", "title": "Example", "loading": false]], runtimeId: "[\"none\"]", tabId: "tab-9")))
        XCTAssertEqual(fallback["available"] as? Bool, false)
        XCTAssertEqual(fallback["visible"] as? Bool, false)
        XCTAssertEqual(fallback["url"] as? String, "https://example.com/")
        let none = result(run(plan("status", runtimeId: nil, tabId: nil)))
        XCTAssertTrue(none["tabId"] is NSNull, "no tab: tabId null")
    }

    func testNavigateWaitsForItsReadiness() {
        let session = page()
        let started = Date()
        let status = result(run(plan("navigate", ["url": "\(fixture.base)/slow"], extra: ["navigate": ["url": "\(fixture.base)/slow"]])))
        XCTAssertGreaterThan(Date().timeIntervalSince(started), 1.0, "load readiness waits for the slow page")
        XCTAssertEqual(status["title"] as? String, "Slow")
        XCTAssertEqual(status["loading"] as? Bool, false)
        let quick = result(run(plan("navigate", ["url": "\(fixture.base)/b", "readiness": "none"], extra: ["navigate": ["url": "\(fixture.base)/b"]])))
        XCTAssertEqual(quick["url"] as? String, "\(fixture.base)/b")
        spin(until: { session.navigation.kind == "Success" })
        let late = error(run(plan("navigate", ["url": "\(fixture.base)/slow", "timeoutMs": 300], extra: ["navigate": ["url": "\(fixture.base)/slow"]])))
        XCTAssertEqual(late["_tag"] as? String, "PreviewAutomationTimeoutError")
        XCTAssertEqual((late["detail"] as? [String: Any])?["readiness"] as? String, "load")
    }

    func testOpenCreatesATabAndWaitsForItsPage() {
        openSnapshot = ["threadId": "thread-1", "tabId": "tab-7", "navStatus": ["_tag": "Loading", "url": "\(fixture.base)/b", "title": ""], "canGoBack": false, "canGoForward": false, "updatedAt": "2026-10-09T00:00:00.000Z"]
        let runtime7 = T3BrowserAutomation.runtimeId(environment: "env-1", thread: "thread-1", epoch: "epoch-1", tab: "tab-7")
        let request = plan("open", ["url": "\(fixture.base)/b"], extra: ["open": ["create": ["threadId": "thread-1", "url": "\(fixture.base)/b", "viewport": ["_tag": "fill"], "profileId": "default"], "epoch": "epoch-1", "present": true]], runtimeId: nil, tabId: nil)
        XCTAssertEqual(automation.perform(request)["accepted"] as? Bool, true)
        spin(until: { (self.automation.status["browserAutomation"] as? [String: Any]).flatMap { $0["opened"] as? [[String: Any]] }?.isEmpty == false })
        let opened = ((automation.status["browserAutomation"] as? [String: Any])?["opened"] as? [[String: Any]])?.last
        XCTAssertEqual((opened?["snapshot"] as? [String: Any])?["tabId"] as? String, "tab-7", "the data module adopts the new tab from the status")
        XCTAssertEqual(opened?["present"] as? Bool, true)
        XCTAssertEqual(calls.first { $0.0 == "preview.open" }?.1["url"] as? String, "\(fixture.base)/b")
        // The data module syncs the page (browserSync); the host waits for it.
        let session = page("/b", visible: true, id: runtime7)
        spin(until: { self.responses[request["requestId"] as! String] != nil })
        let status = result(responses[request["requestId"] as! String] ?? [:])
        XCTAssertEqual(status["tabId"] as? String, "tab-7")
        XCTAssertEqual(status["available"] as? Bool, true)
        XCTAssertEqual(status["visible"] as? Bool, true)
        XCTAssertEqual(session.web.url?.path, "/b")
    }

    func testOpenReusingATabNavigatesIt() {
        let session = page()
        let status = result(run(plan("open", ["url": "\(fixture.base)/b"], extra: ["open": ["tabId": "tab-1", "runtimeId": Self.runtime, "url": "\(fixture.base)/b", "needsOverlay": true]])))
        XCTAssertEqual(status["title"] as? String, "Page B")
        XCTAssertEqual(session.web.url?.path, "/b")
        XCTAssertFalse(calls.contains { $0.0 == "preview.open" }, "a reused tab opens no session")
    }

    // MARK: Errors

    func testTheHostsErrorAnswers() {
        let unavailable = error(run(plan("click", ["x": 1, "y": 1], runtimeId: nil, tabId: nil)))
        XCTAssertEqual(unavailable["_tag"] as? String, "PreviewAutomationTabNotFoundError")
        XCTAssertEqual(unavailable["message"] as? String, "Preview automation target for click request preview-1 is unavailable on environment env-1 thread thread-1 (tab unassigned, bridge available).")
        let overlay = error(run(plan("snapshot", timeoutMs: 600)))
        XCTAssertEqual(overlay["_tag"] as? String, "PreviewAutomationTimeoutError", "no page within the host budget")
        XCTAssertLessThanOrEqual((overlay["detail"] as? [String: Any])?["timeoutMs"] as? Int ?? 9_999, 480)
        page()
        let stop = error(run(plan("recordingStop")))
        XCTAssertEqual(stop["_tag"] as? String, "PreviewAutomationExecutionError")
        XCTAssertEqual(stop["message"] as? String, "Preview automation request preview-3 found no active recording for tab tab-1 on environment env-1 thread thread-1.")
        let start = error(run(plan("recordingStart")))
        XCTAssertEqual(start["_tag"] as? String, "PreviewAutomationExecutionError", "recording is part 3's: capture unavailable")
        let planned = error(run(plan("navigate", extra: ["failure": ["_tag": "PreviewAutomationTabNotFoundError", "message": "planned", "detail": ["requestId": "x"]]])))
        XCTAssertEqual(planned["message"] as? String, "planned", "a failure the data module planned is answered as it is")
        let fixed = error(run(plan("resize", ["mode": "freeform", "width": 800, "height": 600], extra: ["viewport": ["_tag": "freeform", "width": 800, "height": 600]])))
        XCTAssertEqual(fixed["_tag"] as? String, "PreviewAutomationExecutionError", "a fixed viewport is part 2's")
        let fill = result(run(plan("resize", ["mode": "fill"], extra: ["viewport": ["_tag": "fill"]])))
        XCTAssertEqual((fill["viewport"] as? [String: Any])?["width"] as? Int, 900)
        XCTAssertEqual(calls.last { $0.0 == "preview.resize" }?.1["tabId"] as? String, "tab-1")
    }

    // MARK: Appearance and Mute

    func testTheColourSchemeIsThePagesPreferredScheme() {
        let session = page()
        _ = result(run(plan("setColorScheme", ["colorScheme": "dark"])))
        XCTAssertEqual(js(session.web, "matchMedia('(prefers-color-scheme: dark)').matches") as? Bool, true)
        _ = result(run(plan("setColorScheme", ["colorScheme": "light"])))
        XCTAssertEqual(js(session.web, "matchMedia('(prefers-color-scheme: dark)').matches") as? Bool, false)
        let status = (automation.status["browserAutomation"] as? [String: Any])?["tabs"] as? [String: Any]
        XCTAssertEqual((status?[Self.runtime] as? [String: Any])?["colorScheme"] as? String, "light")
    }

    func testMuteSilencesTheDocumentsMediaAndTheAudibleStateFollowsPlayback() {
        // One second of a 440 Hz tone as a WAV.
        var samples = Data()
        for index in 0..<8000 { let value = Int16(sin(Double(index) * 2 * .pi * 440 / 8000) * 8000); withUnsafeBytes(of: value.littleEndian) { samples.append(contentsOf: $0) } }
        var wav = Data("RIFF".utf8); withUnsafeBytes(of: UInt32(36 + samples.count).littleEndian) { wav.append(contentsOf: $0) }; wav.append(Data("WAVEfmt ".utf8))
        for value: UInt32 in [16] { withUnsafeBytes(of: value.littleEndian) { wav.append(contentsOf: $0) } }
        for value: UInt16 in [1, 1] { withUnsafeBytes(of: value.littleEndian) { wav.append(contentsOf: $0) } }
        for value: UInt32 in [8000, 16000] { withUnsafeBytes(of: value.littleEndian) { wav.append(contentsOf: $0) } }
        for value: UInt16 in [2, 16] { withUnsafeBytes(of: value.littleEndian) { wav.append(contentsOf: $0) } }
        wav.append(Data("data".utf8)); withUnsafeBytes(of: UInt32(samples.count).littleEndian) { wav.append(contentsOf: $0) }; wav.append(samples)
        fixture.routes["/tone.wav"] = (200, "audio/wav", wav, 0)
        fixture.page("/media", "<!doctype html><title>Media</title><audio id=tone src=/tone.wav loop></audio>")
        let session = page("/media")
        let tabState = { (self.automation.status["browserAutomation"] as? [String: Any]).flatMap { $0["tabs"] as? [String: Any] }?[Self.runtime] as? [String: Any] ?? [:] }
        XCTAssertTrue(automation.setMuted(Self.runtime, true))
        _ = js(session.web, "document.getElementById('tone').play(); 'ok'")
        spin(until: { tabState()["audible"] as? Bool == true }, timeout: 5)
        XCTAssertEqual(tabState()["audible"] as? Bool, true, "audible as Chromium's tab state: playing with sound the page asked for")
        XCTAssertEqual(tabState()["muted"] as? Bool, true)
        XCTAssertEqual(js(session.web, "document.getElementById('tone').muted") as? Bool, true, "Mute silences the element")
        XCTAssertTrue(automation.setMuted(Self.runtime, false))
        spin(until: { (self.js(session.web, "document.getElementById('tone').muted") as? Bool) == false }, timeout: 3)
        XCTAssertEqual(js(session.web, "document.getElementById('tone').muted") as? Bool, false, "Unmute gives the page its own muted state back")
        _ = js(session.web, "document.getElementById('tone').pause(); 'ok'")
        spin(until: { tabState()["audible"] as? Bool == false }, timeout: 3)
        XCTAssertEqual(tabState()["audible"] as? Bool, false)
        XCTAssertFalse(automation.setMuted("missing", true))
    }
}

_ = NSApplication.shared
NSApp.setActivationPolicy(.accessory)
let suite = XCTestSuite(forTestCaseClass: BrowserAutomationTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)

import AppKit
import Network
import WebKit
import XCTest

// A download from a page the agent drove last goes into the artifact directory with
// no dialog; a person's goes through the save dialog (Manager.ts installDownloadHandler, agentDrivenWebContents,
// READ_ONLY_CONTROL_ACTIONS, handleHumanInput, navigate; T3 Code 1e2ecbd975, MIT, see LICENSE-T3). The page shows the save
// dialogs a person's app shows (`dialogs` on, as outside agent mode); the person's input goes through
// `NSApplication.sendEvent`, as the window server's does, the agent's through the automation host. Self-contained (its
// own loopback page) and written against API the feature tip already had, so the same file runs against the tip's
// modules for the "before" record.
final class DownloadPage {
    private let listener: NWListener
    private(set) var port: UInt16 = 0
    private let queue = DispatchQueue(label: "download-fixture")
    static let page = """
    <!doctype html><title>Downloads</title><style>body { margin: 0; font: 16px -apple-system; } a { display: block; width: 200px; height: 40px; }</style>
    <a id=get href="/export.csv" download>Export CSV</a><p id=out></p>
    """
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
    private func serve(_ connection: NWConnection) {
        connection.start(queue: queue)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { data, _, _, _ in
            guard let data, let head = String(data: data, encoding: .utf8)?.split(separator: "\r\n").first else { return connection.cancel() }
            let path = String(head.split(separator: " ").dropFirst().first ?? "")
            let csv = path.hasPrefix("/export.csv")
            let body = Data((csv ? "a,b\n1,2\n" : DownloadPage.page).utf8)
            var response = Data("HTTP/1.1 200 OK\r\nContent-Type: \(csv ? "text/csv" : "text/html")\r\nContent-Length: \(body.count)\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n".utf8)
            response.append(body)
            connection.send(content: response, completion: .contentProcessed { _ in connection.cancel() })
        }
    }
    deinit { listener.cancel() }
}

final class BrowserDownloadTests: XCTestCase {
    private var window: NSWindow!
    private var fixture: DownloadPage!
    private var sessions: T3BrowserSessions!
    private var automation: T3BrowserAutomation!
    private var artifacts: URL!
    private var responses: [String: [String: Any]] = [:]
    private var serial = 0
    private var asked = 0
    private static let runtime = T3BrowserAutomation.runtimeId(environment: "env-1", thread: "thread-1", epoch: "epoch-1", tab: "tab-1")

    override func setUpWithError() throws {
        fixture = try DownloadPage()
        artifacts = FileManager.default.temporaryDirectory.appendingPathComponent("t3-downloads-\(UUID().uuidString)", isDirectory: true)
        sessions = T3BrowserSessions(agent: true, changed: { _ in })
        sessions.artifactDirectoryOverride = artifacts
        automation = T3BrowserAutomation(sessions: sessions)
        sessions.created = { [weak automation] in automation?.prepare($0) }
        sessions.ended = { [weak automation] in automation?.forget($0) }
        automation.rpc = { [weak self] method, payload, _, _, done in
            if method == "previewAutomation.respond", let id = payload["requestId"] as? String { self?.responses[id] = payload }
            done(["ok": true, "value": [:]])
        }
        window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 640, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.orderFrontRegardless()
    }
    override func tearDown() {
        if let sheet = window.attachedSheet { window.endSheet(sheet, returnCode: .cancel) }
        sessions.sync([]); window.orderOut(nil); try? FileManager.default.removeItem(at: artifacts)
    }

    private func spin(until condition: () -> Bool, timeout: TimeInterval = 10) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }
    /// The page in the window, with the save dialogs a person's app shows.
    private func page() -> T3BrowserSession {
        let session = sessions.ensure(id: Self.runtime, url: "\(fixture.base)/", profile: "default", environment: "env-1")
        session.dialogs = true
        session.web.frame = window.contentView!.bounds
        window.contentView!.addSubview(session.web)
        spin(until: { session.navigation.kind == "Success" })
        return session
    }
    private func run(_ operation: String, _ input: [String: Any] = [:], extra: [String: Any] = [:]) -> [String: Any] {
        serial += 1
        let id = "preview-\(serial)"
        var plan: [String: Any] = ["requestId": id, "connectionId": "connection-1", "clientId": "preview-client", "operation": operation, "input": input,
                                   "environmentId": "env-1", "threadId": "thread-1", "tabId": "tab-1", "runtimeId": Self.runtime, "timeoutMs": 15_000, "generation": 1, "budgetMs": 13_500]
        for (key, value) in extra { plan[key] = value }
        XCTAssertEqual(automation.perform(plan)["accepted"] as? Bool, true)
        spin(until: { self.responses[id] != nil }, timeout: 20)
        return responses[id] ?? [:]
    }
    private func savedFiles() -> [String] { ((try? FileManager.default.contentsOfDirectory(atPath: artifacts.path)) ?? []).filter { $0.hasPrefix("browser-download-") }.sorted() }
    /// The download ended one way or the other: a file in the artifact directory, or the save dialog on the window.
    private func settle(files: Int) { spin(until: { self.window.attachedSheet != nil || self.savedFiles().count >= files }, timeout: 8) }
    /// The save dialog was asked: cancelled, as a person's Cancel.
    private func dismissSaveDialog() -> Bool {
        guard let sheet = window.attachedSheet else { return false }
        asked += 1
        window.endSheet(sheet, returnCode: .cancel)
        spin(until: { self.window.attachedSheet == nil }, timeout: 3)
        return true
    }
    /// A person's click on the link: the mouse events the window server sends, through the application.
    private func personClicks(_ session: T3BrowserSession) {
        let point = session.web.convert(NSPoint(x: 60, y: session.web.isFlipped ? 20 : session.web.bounds.height - 20), to: nil)
        for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
            let event = NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                                           context: nil, eventNumber: 0, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!
            NSApp.sendEvent(event)
        }
    }

    func testTheAgentsClickSavesIntoTheArtifactDirectoryWithNoDialog() {
        let session = page()
        let response = run("click", ["locator": "#get"])
        XCTAssertEqual(response["ok"] as? Bool, true, "\(response)")
        settle(files: 1)
        XCTAssertFalse(dismissSaveDialog(), "installDownloadHandler: a download from the page the agent drove last asks no one")
        spin(until: { (session.report["downloads"] as? [[String: Any]] ?? []).contains { $0["state"] as? String == "done" } }, timeout: 5)
        XCTAssertEqual(savedFiles().count, 1)
        XCTAssertTrue(savedFiles().first?.hasSuffix("-export.csv") == true, "browser-download-<id>-<name>: \(savedFiles())")
    }

    func testAPersonsClickAsksWithTheSaveDialogBeforeAndAfterTheAgentDroveThePage() {
        let session = page()
        personClicks(session)
        settle(files: 1)
        XCTAssertTrue(dismissSaveDialog(), "a person's download asks where")
        XCTAssertTrue(savedFiles().isEmpty)
        _ = run("click", ["locator": "#get"])
        settle(files: 1)
        XCTAssertFalse(dismissSaveDialog())
        XCTAssertEqual(savedFiles().count, 1, "the agent's: the artifact directory")
        personClicks(session) // handleHumanInput: the page is the person's again
        settle(files: 2)
        XCTAssertTrue(dismissSaveDialog(), "after the agent, a person's download asks again")
        XCTAssertEqual(savedFiles().count, 1)
    }

    func testReadingThePageLeavesItThePersonsAndANavigationHandsItBack() {
        let session = page()
        _ = run("snapshot")
        _ = run("waitFor", ["text": "Export CSV", "timeoutMs": 3_000])
        pageClicksLink(session)
        settle(files: 1)
        XCTAssertTrue(dismissSaveDialog(), "READ_ONLY_CONTROL_ACTIONS: a snapshot and a wait do not make the page agent-driven")
        _ = run("evaluate", ["expression": "document.getElementById('out').textContent = 'set'"])
        _ = run("navigate", ["url": "\(fixture.base)/"], extra: ["navigate": ["url": "\(fixture.base)/"]])
        pageClicksLink(session)
        settle(files: 1)
        XCTAssertTrue(dismissSaveDialog(), "a navigation hands the page back (the agent's next action marks it again)")
        XCTAssertTrue(savedFiles().isEmpty)
    }
    /// A download the page itself starts (its script), not an input: it follows the page's last driver.
    private func pageClicksLink(_ session: T3BrowserSession) {
        var done = false
        session.web.evaluateJavaScript("document.getElementById('get').click(); 1") { _, _ in done = true }
        spin(until: { done }, timeout: 3)
    }
}

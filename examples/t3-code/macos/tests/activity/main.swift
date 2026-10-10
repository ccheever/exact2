import AppKit
import XCTest

/// A view that counts the mouse moves AppKit sends it (as a focused ExactKit node with a `hover` would hear them).
private final class MoveCounter: NSView {
    private(set) var moves = 0
    override var acceptsFirstResponder: Bool { true }
    override func mouseMoved(with event: NSEvent) { moves += 1 }
}

final class ActivityTests: XCTestCase {
    func testExpiresInteractionIndependentlyOfWindowFocus() {
        // "expires interaction independently of window focus"
        XCTAssertTrue(T3ActivityScopes.recentlyInteracted(0, at: 45_000))
        XCTAssertFalse(T3ActivityScopes.recentlyInteracted(0, at: 45_001))
    }
    // pr-conversation-and-refresh: the pull request panel's idle rule reads the reporter's last interaction (Unix ms).
    func testLastInteractionIsAWallClockInstantFromLaunch() {
        let before = Date().timeIntervalSince1970 * 1000
        let reporter = T3ActivityReporter(persistent: false, dataDirectory: nil, observeWindows: false)
        let at = reporter.lastInteractionAt()
        XCTAssertGreaterThanOrEqual(at, before)
        XCTAssertLessThanOrEqual(at, Date().timeIntervalSince1970 * 1000)
        reporter.destroy()
    }
    func testRejectsFutureTimestamps() {
        // "rejects future timestamps"
        XCTAssertFalse(T3ActivityScopes.recentlyInteracted(100, at: 99))
    }
    func testRetainsAnObservedSubscriptionUntilItsReturnedFinalizerRuns() {
        // "retains an observed subscription until its returned finalizer runs"
        let scopes = T3ActivityScopes()
        let first = scopes.retain(environment: "e", method: "subscribeVcsStatus", payload: ["cwd": "/repo"])!
        let second = scopes.retain(environment: "e", method: "subscribeVcsStatus", payload: ["cwd": "/repo"])!
        XCTAssertEqual(scopes.values("e").count, 1)
        first(); first(); XCTAssertEqual(scopes.values("e").count, 1)
        second(); XCTAssertTrue(scopes.values("e").isEmpty)
        XCTAssertNil(scopes.retain(environment: "e", method: "subscribeShell", payload: [:]))
    }
    func testKeepsDelimiterContainingEnvironmentAndScopeValuesDistinct() {
        // "keeps delimiter-containing environment and scope values distinct"
        let scopes = T3ActivityScopes()
        let first = scopes.retain(environment: "a:vcs-status", method: "subscribeVcsStatus", payload: ["cwd": "b"])!
        let second = scopes.retain(environment: "a", method: "subscribeVcsStatus", payload: ["cwd": "vcs-status:b"])!
        XCTAssertEqual(scopes.values("a").first?["cwd"] as? String, "vcs-status:b")
        XCTAssertEqual(scopes.values("a:vcs-status").first?["cwd"] as? String, "b")
        first(); XCTAssertEqual(scopes.values("a").count, 1); second()
    }
    func testSingleQueuePayloadAndDisconnectedEnvironment() {
        let reporter = T3ActivityReporter(persistent: false, dataDirectory: nil, observeWindows: false)
        defer { reporter.destroy() }
        let a = UUID(), b = UUID(), scope = UUID(), received = expectation(description: "debounced reports")
        received.expectedFulfillmentCount = 1
        reporter.connect(a, environment: "a") { value, finished in
            defer { finished() }
            XCTAssertEqual(value["clientKind"] as? String, "desktop-renderer")
            XCTAssertEqual(value["ttlMs"] as? Int, 45_000)
            XCTAssertEqual(value["recentlyInteracted"] as? Bool, true)
            XCTAssertEqual(value["environmentId"] as? String, "a")
            XCTAssertNotNil(ISO8601DateFormatter().date(from: value["observedAt"] as? String ?? ""))
            XCTAssertEqual((value["scopes"] as? [[String: Any]])?.count, 2)
            received.fulfill()
        }
        reporter.connect(b, environment: "b") { _, finished in
            defer { finished() }; XCTFail("Disconnected environment received a lease") }
        reporter.retain(scope, environment: "a", method: "subscribeResourceTelemetry", payload: [:])
        reporter.disconnect(b); reporter.changed(); reporter.changed()
        wait(for: [received], timeout: 2)
    }
    func testReportRequestsCoalesceWhileThePreviousBatchIsPending() {
        let reporter = T3ActivityReporter(persistent: false, dataDirectory: nil, observeWindows: false)
        defer { reporter.destroy() }
        let first = expectation(description: "first batch"), next = expectation(description: "queued batch")
        let early = expectation(description: "no overlapping batch"); early.isInverted = true
        let lock = NSLock()
        var finishFirst: (() -> Void)?, count = 0, allowed = false
        reporter.connect(UUID(), environment: "e") { _, done in
            lock.lock(); count += 1; let index = count, permitted = allowed
            if index == 1 { finishFirst = done }; lock.unlock()
            if index == 1 { first.fulfill() }
            else { if !permitted { early.fulfill() }; done(); next.fulfill() }
        }
        wait(for: [first], timeout: 2)
        reporter.changed(); reporter.changed()
        wait(for: [early], timeout: 0.35)
        lock.lock(); allowed = true; let done = finishFirst; lock.unlock(); done?()
        wait(for: [next], timeout: 2)
        lock.lock(); XCTAssertEqual(count, 2); lock.unlock()
    }
    func testPageFactsHoldTheFirstReportAndEachChangeReportsAgain() {
        // The window facts are the page's exactPage() (exact2 #219): document.visibilityState and
        // document.hasFocus(), reported again on visibilitychange, focus and blur.
        let reporter = T3ActivityReporter(persistent: false, dataDirectory: nil, observeWindows: false, pageFacts: true)
        defer { reporter.destroy() }
        let lock = NSLock()
        var reports: [[String: Any]] = []
        let early = expectation(description: "no report before the page's facts"); early.isInverted = true
        var first: XCTestExpectation? = early, second: XCTestExpectation?
        reporter.connect(UUID(), environment: "e") { value, done in
            lock.lock(); reports.append(value); let waiting = reports.count == 1 ? first : second; lock.unlock()
            waiting?.fulfill(); done()
        }
        wait(for: [early], timeout: 0.4)
        let focusedReport = expectation(description: "the facts arrive")
        lock.lock(); first = focusedReport; lock.unlock()
        reporter.facts(visible: true, focused: true)
        wait(for: [focusedReport], timeout: 2)
        let blurred = expectation(description: "blur reports again")
        lock.lock(); second = blurred; lock.unlock()
        reporter.facts(visible: true, focused: false)
        wait(for: [blurred], timeout: 2)
        lock.lock(); defer { lock.unlock() }
        XCTAssertEqual(reports.count, 2)
        XCTAssertEqual(reports.map { $0["focused"] as? Bool }, [true, false])
        XCTAssertEqual(reports.map { $0["visible"] as? Bool }, [true, true])
        XCTAssertEqual(reports.map { $0["appState"] as? String }, ["active", "active"])
    }
    func testIdentitySurvivesPreferenceSaveAndRelaunch() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = directory.appendingPathComponent("t3-code.json")
        let first = T3ActivityReporter(persistent: true, dataDirectory: directory, observeWindows: false)
        try first.writePreferences("{\"version\":1}", to: url)
        let initial = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as! [String: Any]
        let id = initial["backgroundActivityClientId"] as? String
        XCTAssertNotNil(UUID(uuidString: id ?? ""))
        let second = T3ActivityReporter(persistent: true, dataDirectory: directory, observeWindows: false)
        try second.writePreferences("{\"version\":2}", to: url)
        let saved = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as! [String: Any]
        XCTAssertEqual(saved["backgroundActivityClientId"] as? String, id)
        XCTAssertEqual(saved["version"] as? Int, 2)
        first.destroy(); second.destroy()
    }
}
extension ActivityTests {
    /// realinput-1010f RF-5: the reporter hears the pointer through a tracking area of its own over the window's content.
    /// It leaves `acceptsMouseMovedEvents` off: a window that accepts mouse moves sends each one to its first responder,
    /// and a focused ExactKit node with a `hover` took it as the pointer over it wherever the pointer was (the Usage
    /// page's pressed Cost segment held the unpriced (i)'s hover).
    func testPointerMovesReachTheReporterAndNoFocusedView() {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 200, y: 200, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let focused = MoveCounter(frame: NSRect(x: 0, y: 0, width: 100, height: 100))
        window.contentView!.addSubview(focused)
        window.orderFrontRegardless()
        defer { window.orderOut(nil) }
        XCTAssertTrue(window.makeFirstResponder(focused))
        let reporter = T3ActivityReporter(persistent: false, dataDirectory: nil)
        reporter.connect(UUID(), environment: "e") { _, done in done() }
        let observed = expectation(description: "observing")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { observed.fulfill() }
        wait(for: [observed], timeout: 2)
        XCTAssertFalse(window.acceptsMouseMovedEvents, "the window does not send its moves to the first responder")
        let move = NSEvent.mouseEvent(with: .mouseMoved, location: NSPoint(x: 300, y: 200), modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                      windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 0, pressure: 0)!
        NSApp.sendEvent(move)
        XCTAssertEqual(focused.moves, 0, "a move away from the focused view does not reach it")
        let area = window.contentView!.trackingAreas.first { $0.owner is T3ActivityPointer }
        XCTAssertNotNil(area, "a tracking area over the window's content hears the pointer")
        XCTAssertEqual(area?.options.isSuperset(of: [.mouseMoved, .activeAlways, .inVisibleRect]), true)
        let before = reporter.lastInteractionAt()
        usleep(5_000)
        (area?.owner as? NSResponder)?.mouseMoved(with: move)
        XCTAssertGreaterThan(reporter.lastInteractionAt(), before, "each move over the content is an interaction")
        reporter.destroy()
        let destroyed = expectation(description: "destroyed")
        DispatchQueue.main.async { destroyed.fulfill() }
        wait(for: [destroyed], timeout: 2)
        XCTAssertFalse(window.contentView!.trackingAreas.contains { $0.owner is T3ActivityPointer }, "destroy removes it")
    }
}
let suite = ActivityTests.defaultTestSuite
suite.run()
exit(suite.testRun?.hasSucceeded == true ? 0 : 1)

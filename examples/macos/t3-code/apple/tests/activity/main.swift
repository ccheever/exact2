import Foundation
import XCTest

final class ActivityTests: XCTestCase {
    func testExpiresInteractionIndependentlyOfWindowFocus() {
        // "expires interaction independently of window focus"
        XCTAssertTrue(T3ActivityScopes.recentlyInteracted(0, at: 45_000))
        XCTAssertFalse(T3ActivityScopes.recentlyInteracted(0, at: 45_001))
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
let suite = ActivityTests.defaultTestSuite
suite.run()
exit(suite.testRun?.hasSucceeded == true ? 0 : 1)

import AppKit
import XCTest

// Thread notifications' native side (T3Notifications.swift) with real AppKit
// in a bundle-less test binary: no notification center exists here, so the
// module reports "unavailable" and never asks for permission. Covers the
// module requests, the Dock badge of pending posts and its clearing on focus,
// a clicked notification's open request, and the two sound files.
final class T3NotificationsTests: XCTestCase {
    func request(_ module: T3Notifications, _ body: [String: Any]) -> [String: Any] {
        var response: [String: Any] = [:]
        let done = expectation(description: "reply \(body["op"] ?? "")")
        module.perform(body) { value in response = value; done.fulfill() }
        wait(for: [done], timeout: 2)
        return response
    }

    func testStatusAndAuthorizeNeverPromptWithoutACenter() {
        _ = NSApplication.shared
        let module = T3Notifications(agent: true) { _ in }
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        let status = request(module, ["op": "notifyStatus", "generation": 4])
        XCTAssertEqual(status["ok"] as? Bool, true)
        XCTAssertEqual(status["generation"] as? Int, 4)
        let value = status["value"] as? [String: Any] ?? [:]
        XCTAssertEqual(value["authorization"] as? String, "unavailable")
        XCTAssertEqual(value["agent"] as? Bool, true)
        XCTAssertEqual(value["opened"] as? String, "")
        XCTAssertNil(value["active"], "the window's focus is the page's exactPage().hasFocus (exact2 #219), not the module's")
        let authorize = request(module, ["op": "notifyAuthorize", "generation": 4])["value"] as? [String: Any] ?? [:]
        XCTAssertEqual(authorize["requested"] as? Bool, false)
        let post = request(module, ["op": "notifyPost", "title": "Thread completed", "body": "Fixture", "tag": "env:t1", "threadId": "t1"])
        XCTAssertEqual((post["value"] as? [String: Any])?["posted"] as? Bool, false, "nothing posts without a notification center")
        XCTAssertNil(NSApp.dockTile.badgeLabel)
        let unknown = request(module, ["op": "notifyNope"])
        XCTAssertEqual(unknown["ok"] as? Bool, false)
        module.destroy()
    }

    func testBadgeCountsPendingTagsAndFocusClearsIt() {
        _ = NSApplication.shared
        var changes: [String] = []
        let module = T3Notifications(agent: true) { topic in changes.append(topic) }
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        module.record(tag: "env:t1")
        module.record(tag: "env:t2")
        module.record(tag: "env:t1") // the same thread replaces its notification
        XCTAssertEqual(module.pending, ["env:t2", "env:t1"])
        XCTAssertEqual(NSApp.dockTile.badgeLabel, "2")
        NotificationCenter.default.post(name: NSApplication.didBecomeActiveNotification, object: NSApp)
        XCTAssertEqual(module.pending, [])
        XCTAssertNil(NSApp.dockTile.badgeLabel)
        XCTAssertEqual(changes, [], "focus announces nothing: the page has its own focus fact")
        module.record(tag: "env:t3")
        _ = request(module, ["op": "notifyClear"])
        XCTAssertNil(NSApp.dockTile.badgeLabel)
        module.destroy()
    }

    func testClickedNotificationAsksTheWindowToOpenItsThread() {
        _ = NSApplication.shared
        var changes = 0
        let module = T3Notifications(agent: true) { _ in changes += 1 }
        module.open(threadId: "t1")
        module.open(threadId: "t1")
        XCTAssertEqual(module.opened, "2:t1", "each click is a new request, even for the same thread")
        XCTAssertEqual(module.openedThread, "t1")
        XCTAssertEqual(changes, 2)
        let status = request(module, ["op": "notifyStatus"])["value"] as? [String: Any] ?? [:]
        XCTAssertEqual(status["openedThread"] as? String, "t1")
        module.destroy()
    }

    func testSoundsResolveFromTheAppAssets() {
        let app = ProcessInfo.processInfo.environment["T3_APP_DIR"] ?? ""
        XCTAssertTrue(app.hasPrefix("/"), "set T3_APP_DIR to examples/t3-code")
        XCTAssertEqual(T3Notifications.soundURL("completion", assets: app, resources: nil)?.lastPathComponent, "notification-completion.mp3")
        XCTAssertEqual(T3Notifications.soundURL("input", assets: app, resources: nil)?.lastPathComponent, "notification-input.mp3")
        XCTAssertNil(T3Notifications.soundURL("completion", assets: "relative", resources: nil))
        if let url = T3Notifications.soundURL("completion", assets: app, resources: nil) {
            XCTAssertNotNil(NSSound(contentsOf: url, byReference: false), "the completion sound decodes")
        }
    }
}

let suite = XCTestSuite(forTestCaseClass: T3NotificationsTests.self)
suite.run()
let run = suite.testRun!
print("T3Notifications tests: \(run.executionCount) run, \(run.totalFailureCount) failed")
exit(run.totalFailureCount == 0 ? 0 : 1)

import XCTest
import ExactCore
@testable import ExactKit
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

@MainActor
final class StoreRefreshTests: XCTestCase {
    func testProductionObserverRefreshesPeersSkipsOwnWritesAndRecoversAtActivation() throws {
        #if os(macOS)
        _ = NSApplication.shared
        let foreground = NSApplication.didBecomeActiveNotification
        #else
        let foreground = UIApplication.didBecomeActiveNotification
        #endif
        let fixture = try isolatedStore()
        let (app, appID, root) = (fixture.app, fixture.appID, fixture.root)
        defer { fixture.restore() }
        let counter = root.appendingPathComponent("widget-store-generation")
        try Data("1000000".utf8).write(to: counter, options: .atomic)
        var reloads = 0
        var expected: XCTestExpectation?
        app.observeSharedStore(root: root, appID: appID, kinds: ["wp12-fixture"]) { kinds in
            XCTAssertEqual(kinds, ["wp12-fixture"])
            reloads += 1
            expected?.fulfill()
        }
        WidgetContainer.configure()
        XCTAssertEqual(String(cString: getenv("EXACT_APP_GROUP_ROOT")), root.path, "configure preserves the explicitly endowed test container")
        func foregroundAndDrain() {
            let delivered = expectation(description: "foreground notification delivered")
            NotificationCenter.default.post(name: foreground, object: nil)
            DispatchQueue.main.async { delivered.fulfill() }
            wait(for: [delivered], timeout: 5)
        }
        foregroundAndDrain()
        XCTAssertEqual(reloads, 0, "the installed counter is the baseline, including before first pixel")
        let plan = try fixturePlan("STORE_REFRESH")
        let readers = (0..<2).map { app.makeSession(label: "store-reader-\($0)") }
        let writer = app.makeSession(label: "store-writer")
        defer { (readers + [writer]).forEach { $0.destroy() } }
        for session in readers + [writer] {
            XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 300, height: 400)).error)
            session.apply(session.runtime.dataReady())
        }
        app.sharedStore.firstPixel()
        func text(_ session: ExactSession, _ value: String) -> Bool {
            #if os(macOS)
            session.presenter.controls.controls.values.contains { ($0 as? NSButton)?.title == value }
            #else
            session.presenter.controls.controls.values.contains { ($0 as? UIButton)?.titleLabel?.text == value }
            #endif
        }
        func tap(_ session: ExactSession, _ testID: String) throws {
            let node = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testID })
            session.apply(session.runtime.press(node.id, now: 0))
        }
        try tap(readers[0], "local")
        try tap(writer, "favourite")
        XCTAssertTrue(readers.allSatisfy { text($0, "Not a favourite") })
        expected = expectation(description: "the production Darwin observer refreshes both readers before reload")
        try Data("1000001".utf8).write(to: counter, options: .atomic)
        let ping = appID + ".widget-store"
        CFNotificationCenterPostNotification(CFNotificationCenterGetDarwinNotifyCenter(), CFNotificationName(ping as CFString), nil, nil, true)
        wait(for: [try XCTUnwrap(expected)], timeout: 5)
        expected = nil
        XCTAssertTrue(readers.allSatisfy { text($0, "Favourite") })
        XCTAssertEqual(reloads, 1)
        try tap(writer, "favourite")
        try Data("1000002".utf8).write(to: counter, options: .atomic)
        XCTAssertTrue(readers.allSatisfy { text($0, "Favourite") })
        expected = expectation(description: "didBecomeActive recovers a ping missed by a prewarmed app")
        NotificationCenter.default.post(name: foreground, object: nil)
        wait(for: [try XCTUnwrap(expected)], timeout: 5)
        expected = nil
        XCTAssertEqual(reloads, 2)
        XCTAssertTrue(readers.allSatisfy { text($0, "Not a favourite") })
        XCTAssertTrue(text(readers[0], "1"))
        XCTAssertTrue(text(readers[1], "0"))
        foregroundAndDrain()
        XCTAssertEqual(reloads, 2)

        setenv("EXACT_WIDGET_WRITER", appID, 1)
        expected = expectation(description: "an app-side commit reloads without refreshing its peers")
        try tap(readers[0], "favourite")
        wait(for: [try XCTUnwrap(expected)], timeout: 5)
        expected = nil
        XCTAssertTrue(text(readers[0], "Favourite"))
        XCTAssertTrue(text(readers[1], "Not a favourite"), "a self-published counter only reloads timelines")
        XCTAssertEqual(reloads, 3)
        foregroundAndDrain()
        XCTAssertEqual(reloads, 3)
    }

    func testBusyRefreshRetriesAndDestroyedFillDoesNotHoldCompletion() throws {
        let wasAsync = ExactSession.asyncFills
        ExactSession.asyncFills = true
        defer { ExactSession.asyncFills = wasAsync }
        let fixture = try isolatedStore()
        defer { fixture.restore() }
        let app = fixture.app
        let session = app.makeSession(label: "store-busy")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try fixturePlan("STORE_REFRESH"), size: CGSize(width: 300, height: 400)).error)
        session.apply(session.runtime.dataReady())
        let due = session.now() + 10000
        session.apply(Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: due))
        let completed = expectation(description: "a busy refresh is retried after the owner callback")
        Owner.shared.sync {
            Owner.shared.callMain {
                session.refreshSharedStore { _ in completed.fulfill() }
            }
        }
        wait(for: [completed], timeout: 5)
        XCTAssertEqual(session.timerDue, due, "an unchanged store refresh leaves the existing clock armed")
        let destroyed = app.makeSession(label: "store-destroyed")
        XCTAssertNil(destroyed.boot(plan: try fixturePlan("STORE_REFRESH"), size: CGSize(width: 300, height: 400)).error)
        // The real collection feedback starts an owner slice; its waiter is
        // cancelled synchronously when the window is destroyed.
        let node = try XCTUnwrap(destroyed.presenter.views.values.first)
        destroyed.presenter.collections.onFill?(node.id, Data())
        XCTAssertTrue(destroyed.fillInFlight)
        let released = expectation(description: "destroying a queued reader releases the reload")
        destroyed.refreshSharedStore { _ in released.fulfill() }
        destroyed.destroy()
        wait(for: [released], timeout: 5)
    }
    private func isolatedStore() throws -> (app: ExactApp, appID: String, root: URL, restore: () -> Void) {
        let app = ExactApp.shared
        let receipt = try XCTUnwrap(try JSONSerialization.jsonObject(with: Runtime.bakedCompat()) as? [String: Any])
        let appID = try XCTUnwrap((receipt["inputs"] as? [String: Any])?["app"] as? String)
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let variables = ["EXACT_APP_GROUP_ROOT": root.path, "EXACT_APP_GROUP_APP": appID, "EXACT_STORE": "real", "EXACT_WIDGET_WRITER": ""]
        let saved = variables.keys.map { key in (key, getenv(key).map { String(cString: $0) }) }
        for (key, value) in variables { setenv(key, value, 1) }
        let restore = {
            app.sharedStore.observer = nil
            for (key, value) in saved { if let value { setenv(key, value, 1) } else { unsetenv(key) } }
            app.observeSharedStore()
            try? FileManager.default.removeItem(at: root)
        }
        return (app, appID, root, restore)
    }
}

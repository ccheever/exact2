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
    #if os(iOS)
    func testPersistedSlotsReadAndWriteTheAppsDefaultsAtBoot() throws {
        let receipt = try XCTUnwrap(try JSONSerialization.jsonObject(with: Runtime.bakedCompat()) as? [String: Any])
        let appID = try XCTUnwrap((receipt["inputs"] as? [String: Any])?["app"] as? String)
        let defaults = try XCTUnwrap(UserDefaults(suiteName: appID))
        let keys = ["exact.state.favourite", "exact.state.marker", "exact.state.delay"]
        let previous = keys.map { ($0, defaults.object(forKey: $0)) }
        let variables = ["EXACT_AGENT", "EXACT_STORE", "EXACT_APP_GROUP_ROOT", "EXACT_APP_GROUP_APP"]
        let environment = variables.map { ($0, getenv($0).map { String(cString: $0) }) }
        for key in variables { unsetenv(key) }
        setenv("EXACT_STORE", "real", 1)
        defer {
            for (key, value) in previous { defaults.set(value, forKey: key) }
            defaults.synchronize()
            for (key, value) in environment { if let value { setenv(key, value, 1) } else { unsetenv(key) } }
        }
        defaults.set("true", forKey: keys[0])
        defaults.set("\"not a boolean\"", forKey: keys[1])
        defaults.set("10000", forKey: keys[2])
        XCTAssertTrue(defaults.synchronize())
        let plan = try fixturePlan("STORE_REFRESH")
        func boot() throws -> ExactSession {
            let session = ExactApp.shared.makeSession(label: "defaults-boot")
            let batch = session.boot(plan: plan, size: CGSize(width: 300, height: 400))
            XCTAssertNil(batch.error)
            return session
        }
        func slots(_ session: ExactSession) throws -> [String: Any] {
            let state = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(session.agent("{\"op\":\"state\"}").utf8)) as? [String: Any])
            return try XCTUnwrap(state["slots"] as? [String: Any])
        }
        let first = try boot()
        defer { first.destroy() }
        XCTAssertEqual(try slots(first)["favourite"] as? Bool, true, "a preference is present before data activation")
        XCTAssertEqual(try slots(first)["marker"] as? Bool, false, "an incompatible saved value uses the initializer")
        let button = try XCTUnwrap(first.presenter.views.values.first { $0.props["testId"] == "favourite" })
        first.apply(first.runtime.press(button.id, now: 0))
        XCTAssertEqual(defaults.string(forKey: keys[0]), "false", "the commit writes the preference immediately")
        first.destroy()
        let second = try boot()
        defer { second.destroy() }
        XCTAssertEqual(try slots(second)["favourite"] as? Bool, false, "a fresh runner reads the committed value")
    }
    #endif

    func testProductionObserverRefreshesEverySessionAndRecoversAtActivation() throws {
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
        let booting = ExactEnv.wall()
        for session in readers + [writer] {
            XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 300, height: 400)).error)
            session.apply(session.runtime.dataReady())
        }
        let booted = ExactEnv.wall()
        // Due 10 s after its window booted, however long the process ran
        // before: a boot starts on the live clock, not at its zero. Read
        // here, before the waits below, which under load can outlast it. A
        // task's timer is Foundation's (#1144): its row's due time.
        let due = try XCTUnwrap(readers[0].taskTimers.nextDue, "the short task's timer is armed")
        XCTAssertTrue((booting + 10000...booted + 10000).contains(due), "due \(due), booted between \(booting) and \(booted)")
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
        expected = expectation(description: "an app-side commit refreshes sibling windows before reload")
        try tap(readers[0], "favourite")
        wait(for: [try XCTUnwrap(expected)], timeout: 5)
        expected = nil
        XCTAssertTrue(text(readers[0], "Favourite"))
        XCTAssertTrue(text(readers[1], "Favourite"), "the sibling adopts the app-side write")
        XCTAssertEqual(reloads, 3)
        foregroundAndDrain()
        XCTAssertEqual(reloads, 3)

        // Main stays in this turn until both publications exist, so the
        // observer first sees the app's counter over the peer's new value.
        setenv("EXACT_WIDGET_WRITER", "", 1)
        try tap(writer, "favourite")
        let peerCounter = UInt64(try String(contentsOf: counter, encoding: .utf8))! + 1
        try Data(String(peerCounter).utf8).write(to: counter, options: .atomic)
        CFNotificationCenterPostNotification(CFNotificationCenterGetDarwinNotifyCenter(), CFNotificationName(ping as CFString), nil, nil, true)
        setenv("EXACT_WIDGET_WRITER", appID, 1)
        try tap(writer, "marker")
        let deadline = Date().addingTimeInterval(5)
        while (UInt64((try? String(contentsOf: counter, encoding: .utf8)) ?? "") ?? 0) <= peerCounter && Date() < deadline { usleep(50000) }
        XCTAssertGreaterThan(UInt64(try String(contentsOf: counter, encoding: .utf8))!, peerCounter)
        expected = expectation(description: "an app publication cannot hide a peer value")
        wait(for: [try XCTUnwrap(expected)], timeout: 5)
        expected = nil
        XCTAssertTrue((readers + [writer]).allSatisfy { text($0, "Not a favourite") && text($0, "Marked") })

        setenv("EXACT_WIDGET_WRITER", "", 1)
        try tap(writer, "wait")
        try Data(String(peerCounter + 2).utf8).write(to: counter, options: .atomic)
        var timerBatch: Batch?
        let refreshing = ExactEnv.wall()
        // The long task's timer is armed by the refresh: 20 s after it, not the short one's.
        let rearmed = { (at: Double?) in at.map { $0 != due && $0 >= refreshing + 20000 } ?? false }
        readers[0].runtime.observeBatch = { _, batch in
            if timerBatch == nil, rearmed(batch.nativeTimers?.map(\.due).min()) { timerBatch = batch }
        }
        defer { readers[0].runtime.observeBatch = nil }
        expected = expectation(description: "a timer-only store commit reaches apply")
        NotificationCenter.default.post(name: foreground, object: nil)
        wait(for: [try XCTUnwrap(expected)], timeout: 5)
        expected = nil
        let refreshed = ExactEnv.wall()
        let landed = try XCTUnwrap(timerBatch)
        XCTAssertTrue(landed.ops.isEmpty, "this commit changes only the timer")
        let landedDue = try XCTUnwrap(landed.nativeTimers?.map(\.due).min())
        XCTAssertTrue((refreshing + 20000...refreshed + 20000).contains(landedDue))
        XCTAssertEqual(readers[0].taskTimers.nextDue, landedDue)
        XCTAssertTrue(rearmed(readers[1].taskTimers.nextDue), "the sibling's long task is armed too")

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
        let bounded = expectation(description: "a persistently busy owner ends the refresh attempt")
        Owner.shared.sync {
            Owner.shared.callMain {
                session.refreshSharedStore { success in XCTAssertFalse(success); bounded.fulfill() }
                // Keep the owner in this callback while main serves retries.
                self.wait(for: [bounded], timeout: 5)
            }
        }
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
    func testForegroundDuringAnInflightRefreshIsRemembered() throws {
        let fixture = try isolatedStore()
        defer { fixture.restore() }
        let foreground = Notification.Name("wp12-test-foreground")
        let counter = fixture.root.appendingPathComponent("widget-store-generation")
        try Data("10".utf8).write(to: counter)
        var finish: ((Bool) -> Void)?
        var calls = 0
        var retried = expectation(description: "the notification during the refused refresh was remembered")
        let observer = try XCTUnwrap(WidgetContainer.observeStore(root: fixture.root, appID: fixture.appID, kinds: ["fixture"], foreground: [foreground]) { _, done in
            calls += 1
            if calls % 2 == 1 { finish = done } else { done(true); retried.fulfill() }
        })
        try Data("11".utf8).write(to: counter, options: .atomic)
        observer.check()
        NotificationCenter.default.post(name: foreground, object: nil)
        try XCTUnwrap(finish)(false)
        wait(for: [retried], timeout: 5)
        XCTAssertEqual(calls, 2)
        retried = expectation(description: "the notification during a successful refresh was remembered")
        try Data("12".utf8).write(to: counter, options: .atomic)
        observer.check()
        NotificationCenter.default.post(name: foreground, object: nil)
        try XCTUnwrap(finish)(true)
        wait(for: [retried], timeout: 5)
        XCTAssertEqual(calls, 4)
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

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
    func testPingRefreshesEveryWindowAndForegroundRecoversAMissedPing() throws {
        #if os(macOS)
        _ = NSApplication.shared
        let foreground = NSApplication.didBecomeActiveNotification
        #else
        let foreground = UIApplication.willEnterForegroundNotification
        #endif
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let variables = ["EXACT_APP_GROUP_ROOT": root.path, "EXACT_APP_GROUP_APP": "com.exact.caltrain", "EXACT_STORE": "real"]
        let saved = variables.keys.map { key in (key, getenv(key).map { String(cString: $0) }) }
        for (key, value) in variables { setenv(key, value, 1) }
        defer {
            for (key, value) in saved { if let value { setenv(key, value, 1) } else { unsetenv(key) } }
            try? FileManager.default.removeItem(at: root)
        }
        let app = ExactApp.shared
        let plan = try fixturePlan("STORE_REFRESH")
        let windows = (0..<2).map { app.makeSession(label: "store-reader-\($0)") }
        let writer = app.makeSession(label: "store-writer")
        defer { (windows + [writer]).forEach { $0.destroy() } }
        for session in windows + [writer] {
            XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 300, height: 400)).error)
            session.apply(session.runtime.dataReady())
        }
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
        try tap(windows[0], "local")
        XCTAssertTrue(windows.allSatisfy { text($0, "Not a favourite") })
        let ping = UUID().uuidString + ".widget-store"
        var turns = 0
        let arrived = expectation(description: "the Darwin ping re-asks both readers")
        let observer = WidgetStoreObserver(root: root, name: ping, foreground: foreground) {
            app.refreshStore {
                turns += 1
                if turns == 1 { arrived.fulfill() }
            }
        }
        try tap(writer, "favourite")
        XCTAssertTrue(windows.allSatisfy { text($0, "Not a favourite") }, "the write alone does not mutate a peer's heap")
        try Data("1".utf8).write(to: root.appendingPathComponent("widget-store-generation"), options: .atomic)
        CFNotificationCenterPostNotification(CFNotificationCenterGetDarwinNotifyCenter(), CFNotificationName(ping as CFString), nil, nil, true)
        wait(for: [arrived], timeout: 5)
        XCTAssertTrue(windows.allSatisfy { text($0, "Favourite") })
        observer.check()
        XCTAssertEqual(turns, 1, "an unchanged counter asks nothing again")
        try tap(writer, "favourite")
        try Data("2".utf8).write(to: root.appendingPathComponent("widget-store-generation"), options: .atomic)
        XCTAssertTrue(windows.allSatisfy { text($0, "Favourite") }, "no ping was delivered while suspended")
        NotificationCenter.default.post(name: foreground, object: nil)
        XCTAssertEqual(turns, 2)
        XCTAssertTrue(windows.allSatisfy { text($0, "Not a favourite") })
        XCTAssertTrue(text(windows[0], "1"), "window-local state survives")
        XCTAssertTrue(text(windows[1], "0"))
        NotificationCenter.default.post(name: foreground, object: nil)
        XCTAssertEqual(turns, 2)
        withExtendedLifetime(observer) {}
    }
}

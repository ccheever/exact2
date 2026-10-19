import XCTest
@testable import ExactKit

/// `onLine` loads Network only when something reads it (PageFacts): the app
/// hatch's facts start no path monitor for a module that asks the host,
/// an older module's do, and nothing waits for the monitor's first path,
/// which corrects the fact as a change.
final class OnLineIOSTests: XCTestCase {
    override func setUpWithError() throws {
        try XCTSkipIf(PageFacts.substituted, "the agent's facts stand in for the device's")
        PageFacts.stopWatchingPath()
    }
    override func tearDown() { PageFacts.stopWatchingPath() }

    func testTheAppFactsOfAModuleThatAsksStartNoMonitor() {
        XCTAssertTrue(NativeViews.appOnLine(moduleAsks: true), "online until something reads it")
        XCTAssertFalse(PageFacts.watchesPath)
    }

    func testAnOlderModulesAppFactsStartTheMonitor() {
        _ = NativeViews.appOnLine(moduleAsks: false)
        XCTAssertTrue(PageFacts.watchesPath, "it reads onLine only from the facts, and must hear its changes")
    }

    func testTheFirstReadAnswersAtOnce() {
        let start = Date()
        XCTAssertTrue(PageFacts.onLine, "online until the first path, as a browser starts")
        XCTAssertTrue(PageFacts.watchesPath)
        XCTAssertLessThan(Date().timeIntervalSince(start), 0.05, "nothing waits for the monitor's first path")
    }

    func testAFirstPathThatSaysOfflineIsAChange() {
        var heard = 0
        let tokens = PageFacts.observe { heard += 1 }
        defer { PageFacts.forget(tokens) }
        PageFacts.heard(online: true)
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
        XCTAssertEqual(heard, 0, "the same answer is no change")
        PageFacts.heard(online: false)
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
        XCTAssertFalse(PageFacts.lastOnLine)
        XCTAssertGreaterThan(heard, 0, "the plan and the hatches hear the correction")
    }

    /// A module that reads `app.onLine` in its `isNew` call (the host
    /// table's `on_line`) starts the watch, so a later offline path is a
    /// change its hatch is told, and its next read says so.
    func testAModuleThatReadsOnLineWhenNewHearsALaterOfflineChange() {
        let session = ExactApp.shared.makeSession(label: "on-line")
        defer { session.destroy() }
        let host = UnsafeMutableRawPointer(bitPattern: UInt(session.runtime.rt))
        XCTAssertFalse(PageFacts.watchesPath)
        XCTAssertEqual(hatchOnLine(host), 1)
        XCTAssertTrue(PageFacts.watchesPath)
        // The monitor's own first path (the simulator's: online) first.
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.3))
        var heard = 0
        let tokens = PageFacts.observe { heard += 1 }
        defer { PageFacts.forget(tokens) }
        PageFacts.heard(online: false)
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
        XCTAssertGreaterThan(heard, 0, "the app hatch's facts are compared again")
        XCTAssertFalse(NativeViews.appOnLine(moduleAsks: true), "and its `changed` call says offline")
        XCTAssertEqual(hatchOnLine(host), 0)
    }

    /// Off main, `on_line` answers nothing (2) and starts nothing, as the
    /// host table's other entries do; the module keeps what it was told.
    func testOnLineOffMainIsNotAnswered() {
        let session = ExactApp.shared.makeSession(label: "on-line-off-main")
        defer { session.destroy() }
        let host = UnsafeMutableRawPointer(bitPattern: UInt(session.runtime.rt))
        var answer: UInt32 = 9
        let done = expectation(description: "asked off main")
        DispatchQueue.global().async { answer = hatchOnLine(host); done.fulfill() }
        wait(for: [done], timeout: 5)
        XCTAssertEqual(answer, 2)
        XCTAssertFalse(PageFacts.watchesPath)
        XCTAssertEqual(hatchOnLine(nil), 2, "no session")
    }
}

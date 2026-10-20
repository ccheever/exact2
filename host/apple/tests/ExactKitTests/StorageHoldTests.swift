import XCTest
@testable import ExactKit

/// LLP 1097 D10: a quit or a suspension is held while the module's storage
/// lands, and let go when it has, when the bound passes, or when the
/// platform takes the hold back (an iOS background task's expiration).
final class StorageHoldTests: XCTestCase {
    final class Probe {
        var pending = true, begun = 0, ended = 0
        var expire: (() -> Void)?
        func hold(bound: TimeInterval? = nil) -> StorageHold {
            StorageHold(every: 0.01, bound: bound, pending: { self.pending },
                        begin: { expired in self.begun += 1; self.expire = expired }, end: { self.ended += 1 })
        }
    }

    func testNothingLandingTakesNoHold() {
        let probe = Probe()
        probe.pending = false
        XCTAssertFalse(probe.hold().hold())
        XCTAssertEqual(probe.begun, 0)
    }

    func testTheHoldEndsWhenTheStorageLands() {
        let probe = Probe(), hold = probe.hold()
        XCTAssertTrue(hold.hold())
        XCTAssertFalse(hold.hold(), "one hold at a time")
        hold.check()
        XCTAssertEqual(probe.ended, 0, "still landing")
        probe.pending = false
        let ended = expectation(description: "ended by the timer")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { if probe.ended == 1 { ended.fulfill() } }
        wait(for: [ended], timeout: 2)
        XCTAssertFalse(hold.holding)
    }

    func testTheBoundEndsAHoldThatNeverLands() {
        let probe = Probe(), hold = probe.hold(bound: 0.05)
        XCTAssertTrue(hold.hold())
        let ended = expectation(description: "ended at the bound")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { if probe.ended == 1 { ended.fulfill() } }
        wait(for: [ended], timeout: 2)
        XCTAssertEqual(probe.begun, 1)
    }

    func testTheExpirationEndsTheHoldOnce() {
        let probe = Probe(), hold = probe.hold()
        XCTAssertTrue(hold.hold())
        probe.expire?()
        probe.expire?()
        hold.release()
        XCTAssertEqual(probe.ended, 1)
        XCTAssertFalse(hold.holding)
    }
}

/// LLP 1069.010 Q4: a native module's quit work joins the quit's one hold.
/// The module is a table in memory whose `will_quit` keeps `done` for the
/// test to call.
final class ModuleQuitHoldTests: XCTestCase {
    typealias Done = @convention(c) (UnsafeMutableRawPointer?) -> Void
    nonisolated(unsafe) static var asked = 0
    nonisolated(unsafe) static var done: (() -> Void)?

    static func table(size: UInt32) -> [UInt8] {
        var t = [UInt8](repeating: 0, count: 216)
        func put<T>(_ value: T, _ offset: Int) { withUnsafeBytes(of: value) { for (i, b) in $0.enumerated() { t[offset + i] = b } } }
        let none: @convention(c) (UnsafeMutableRawPointer?) -> Void = { _ in }
        let willQuit: @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, Done?) -> Void = { _, ctx, fn in
            ModuleQuitHoldTests.asked += 1
            ModuleQuitHoldTests.done = { fn?(ctx) }
        }
        put(UInt32(3), 0); put(size, 4); put(UnsafeRawPointer(strdup("{}")), 8)
        for offset in [16, 24, 32, 48, 72, 80, 88, 96] { put(unsafeBitCast(none, to: UnsafeRawPointer.self), offset) }
        put(unsafeBitCast(willQuit, to: UnsafeRawPointer.self), 208)
        return t
    }

    func natives(size: UInt32 = 216) -> NativeViews {
        Self.asked = 0
        Self.done = nil
        Self.table(size: size).withUnsafeBytes { NativeViews.install(table: $0.baseAddress!) }
        let natives = NativeViews()
        natives.install(module: UnsafeMutableRawPointer(bitPattern: 1)!)
        return natives
    }

    override func tearDown() { NativeViews.uninstallTable() }

    /// Asked once however often the quit is; held, with the main thread
    /// running, until `done` arrives from another thread.
    func testTheHoldWaitsForTheModuleOnceAndKeepsTheMainThreadRunning() {
        let natives = natives()
        natives.willQuit()
        natives.willQuit()
        XCTAssertEqual(Self.asked, 1, "a repeated quit asks the module once")
        XCTAssertTrue(natives.quitWorkPending)
        var ended = 0
        let hold = StorageHold(every: 0.01, bound: 5, pending: { natives.quitWorkPending }, begin: { _ in }, end: { ended += 1 })
        XCTAssertTrue(hold.hold())
        let finished = expectation(description: "the hold ends after done")
        var turns = 0
        let ticker = Timer(timeInterval: 0.01, repeats: true) { _ in
            turns += 1
            if turns == 5 { DispatchQueue.global().async { Self.done?() } }
            if ended == 1 { finished.fulfill() }
        }
        RunLoop.main.add(ticker, forMode: .common)
        wait(for: [finished], timeout: 2)
        ticker.invalidate()
        XCTAssertGreaterThanOrEqual(turns, 5, "the main thread ran while the quit was held")
        XCTAssertFalse(natives.quitWorkPending)
        natives.willQuit()
        XCTAssertEqual(Self.asked, 1)
        XCTAssertEqual(ended, 1)
    }

    /// Work that never finishes is cut at the bound; the session ending
    /// stops the wait too.
    func testUnfinishedWorkIsCutAtTheBoundAndEndsWithTheSession() {
        let natives = natives()
        natives.willQuit()
        var ended = 0
        let hold = StorageHold(every: 0.01, bound: 0.05, pending: { natives.quitWorkPending }, begin: { _ in }, end: { ended += 1 })
        XCTAssertTrue(hold.hold())
        let cut = expectation(description: "cut at the bound")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { if ended == 1 { cut.fulfill() } }
        wait(for: [cut], timeout: 2)
        XCTAssertTrue(natives.quitWorkPending, "the work is still running; the hold let go anyway")
        natives.destroyModule()
        XCTAssertFalse(natives.quitWorkPending)
        Self.done?() // A late done after the module has gone is harmless.
    }

    /// A module made after the quit was decided (its first pixel came during
    /// the hold) is asked when it is made, and once.
    func testAModuleMadeDuringTheHoldIsAskedWhenMade() {
        Self.asked = 0
        Self.done = nil
        Self.table(size: 216).withUnsafeBytes { NativeViews.install(table: $0.baseAddress!) }
        let natives = NativeViews()
        natives.willQuit()
        XCTAssertEqual(Self.asked, 0, "no module yet")
        XCTAssertFalse(natives.quitWorkPending)
        natives.install(module: UnsafeMutableRawPointer(bitPattern: 1)!)
        XCTAssertEqual(Self.asked, 1)
        XCTAssertTrue(natives.quitWorkPending)
        natives.willQuit()
        XCTAssertEqual(Self.asked, 1)
        Self.done?()
        XCTAssertFalse(natives.quitWorkPending)
    }

    /// A module built before the entry existed is not asked and holds nothing.
    func testAModuleWithoutTheEntryHoldsNothing() {
        let natives = natives(size: 208)
        natives.willQuit()
        XCTAssertEqual(Self.asked, 0)
        XCTAssertFalse(natives.quitWorkPending)
        XCTAssertFalse(StorageHold(pending: { natives.quitWorkPending }, begin: { _ in }, end: {}).hold())
    }
}

import Foundation
import ExactCore
import XCTest
@testable import ExactKit

private final class NativeTimerTestClock: @unchecked Sendable {
    static let shared = NativeTimerTestClock()
    private let lock = NSLock()
    private var time = 0.0
    func sample() -> Double { lock.lock(); defer { lock.unlock() }; return time }
    func set(_ time: Double) { lock.lock(); self.time = time; lock.unlock() }
}

final class NativeTaskTimersTests: XCTestCase {
    private let date = Date(timeIntervalSince1970: 4_000_000_000)
    private func row(_ index: UInt32 = 0, generation: UInt64 = 0, due: Double = 100, interval: UInt32 = 100, tolerance: UInt32 = 100, once: Bool = false) -> NativeTimerWake {
        NativeTimerWake(index: index, generation: generation, due: due, interval: interval, tolerance: tolerance, once: once)
    }
    func testEachTimerKeepsItsFullToleranceAndOriginalDateWhenAnotherIsAdded() throws {
        var now = 0.0
        let timers = NativeTaskTimers(now: { now }, date: { self.date }, ready: {})
        defer { timers.cancel() }
        let a = row(due: 1000, interval: 1000), b = row(1, due: 1040, interval: 1000, tolerance: 50)
        timers.update([a], epoch: 1, enabled: true)
        let first = try XCTUnwrap(timers.timerForTests(0))
        let originalDate = first.fireDate
        now = 1040
        timers.update([a, b], epoch: 1, enabled: true)
        let second = try XCTUnwrap(timers.timerForTests(1))
        XCTAssertTrue(timers.timerForTests(0) === first)
        XCTAssertEqual(first.fireDate, originalDate)
        XCTAssertEqual(first.tolerance, 0.1)
        XCTAssertEqual(second.tolerance, 0.05)
        XCTAssertEqual(second.fireDate, date)
        timers.update([a], epoch: 1, enabled: true)
        XCTAssertFalse(second.isValid)
        XCTAssertEqual(first.fireDate, originalDate)
        XCTAssertEqual(first.tolerance, 0.1)
    }
    func testRepeatsRetainTheNativeTimerAndMergePendingDeliveries() throws {
        var now = 100.0
        let timers = NativeTaskTimers(now: { now }, date: { self.date }, ready: {})
        defer { timers.cancel() }
        let a = row()
        timers.update([a], epoch: 1, enabled: true)
        let timer = try XCTUnwrap(timers.timerForTests(0))
        timer.fire(); timer.fire()
        let delivery = try XCTUnwrap(timers.take())
        XCTAssertEqual(delivery.row, a)
        XCTAssertNil(timers.take(), "A busy owner receives a repeat once")
        timers.update([row(due: 500)], epoch: 1, enabled: true)
        XCTAssertTrue(timers.timerForTests(0) === timer, "Foundation retains the repeating phase")
        timers.complete(delivery, retry: false)
        now = 500
        timer.fire()
        XCTAssertEqual(timers.take()?.row.due, 500)
    }
    func testDeliveryOrderIsTheNativeCallbackOrder() throws {
        let timers = NativeTaskTimers(now: { 140 }, date: { self.date }, ready: {})
        defer { timers.cancel() }
        let a = row(), b = row(1, due: 140, tolerance: 50)
        timers.update([a, b], epoch: 1, enabled: true)
        try XCTUnwrap(timers.timerForTests(1)).fire()
        XCTAssertEqual(timers.take()?.row, b)
        XCTAssertNil(timers.take(), "The other due task waits for its own callback")
        try XCTUnwrap(timers.timerForTests(0)).fire()
        XCTAssertEqual(timers.take()?.row, a)
    }
    func testCancellationRekeyReloadAndTakeoverInvalidateQueuedCallbacks() throws {
        let timers = NativeTaskTimers(now: { 100 }, date: { self.date }, ready: {})
        defer { timers.cancel() }
        let a = row(), replacement = row(generation: 1)
        timers.update([a], epoch: 1, enabled: true)
        let old = try XCTUnwrap(timers.timerForTests(0))
        old.fire()
        let oldDelivery = try XCTUnwrap(timers.take())
        timers.update([replacement], epoch: 1, enabled: true)
        XCTAssertFalse(old.isValid)
        XCTAssertNil(timers.take())
        timers.complete(oldDelivery, retry: true)
        XCTAssertNil(timers.take(), "An old completion cannot resurrect a cancelled instance")
        let current = try XCTUnwrap(timers.timerForTests(0))
        current.fire()
        let currentDelivery = try XCTUnwrap(timers.take())
        timers.update([replacement], epoch: 2, enabled: true)
        XCTAssertFalse(current.isValid)
        XCTAssertNil(timers.take(), "A reload invalidates even matching timer descriptors")
        let reloaded = try XCTUnwrap(timers.timerForTests(0))
        timers.complete(currentDelivery, retry: true)
        XCTAssertNil(timers.take(), "A completion from before reload cannot requeue a matching instance")
        reloaded.fire()
        let beforeTakeover = try XCTUnwrap(timers.take())
        timers.cancel()
        timers.update([replacement], epoch: 2, enabled: true)
        timers.complete(beforeTakeover, retry: true)
        XCTAssertNil(timers.take(), "Clock takeover invalidates matching generations too")
        let resumed = try XCTUnwrap(timers.timerForTests(0))
        timers.update([replacement], epoch: 2, enabled: false)
        XCTAssertFalse(reloaded.isValid)
        XCTAssertFalse(resumed.isValid)
        XCTAssertTrue(timers.rows.isEmpty)
        XCTAssertNil(timers.nextDue)
    }
    func testHeldDeliveryRetriesOnlyWhenResumedAndCancellationRemovesIt() throws {
        let timers = NativeTaskTimers(now: { 100 }, date: { self.date }, ready: {})
        defer { timers.cancel() }
        let a = row()
        timers.update([a], epoch: 1, enabled: true)
        try XCTUnwrap(timers.timerForTests(0)).fire()
        let delivery = try XCTUnwrap(timers.take())
        XCTAssertEqual(delivery.row, a)
        timers.complete(delivery, retry: true)
        XCTAssertTrue(timers.hasReady)
        timers.update([], epoch: 1, enabled: true)
        XCTAssertFalse(timers.hasReady)
        XCTAssertNil(timers.take())
    }
    func testZeroToleranceTimersAndExplicitFrameTasksHaveIndependentDemand() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        session.clock = nil
        var batch = batchFixture(ops: [], timers: true, motion: false, clock: nil, error: nil)
        batch.nativeTimers = [row(due: session.now() + 1000, tolerance: 0)]
        session.apply(batch)
        XCTAssertNotNil(session.taskTimers.timerForTests(0))
        XCTAssertNil(session.clockTimer)
        XCTAssertFalse(session.frames.timerSoon)
        XCTAssertNil(session.frames.link)
        batch.frames = true
        session.apply(batch)
        XCTAssertTrue(session.frames.timerSoon)
        batch.frames = false
        session.apply(batch)
        XCTAssertFalse(session.frames.timerSoon)
        let timer = try XCTUnwrap(session.taskTimers.timerForTests(0))
        session.clock = session.now()
        XCTAssertFalse(timer.isValid)
        XCTAssertTrue(session.taskTimers.rows.isEmpty)
    }
    func testOneShotKeepsItsInstanceUntilTheDeliveryIsApplied() throws {
        var now = 0.0
        let timers = NativeTaskTimers(now: { now }, date: { self.date }, ready: {})
        defer { timers.cancel() }
        let a = row(once: true)
        timers.update([a], epoch: 1, enabled: true)
        let timer = try XCTUnwrap(timers.timerForTests(0))
        XCTAssertEqual(timer.fireDate, date.addingTimeInterval(0.1))
        XCTAssertEqual(timer.tolerance, 0.1)
        now = 100
        timer.fire()
        let delivery = try XCTUnwrap(timers.take())
        timers.update([a], epoch: 1, enabled: true)
        XCTAssertTrue(timers.timerForTests(0) === timer, "Keep a one-shot while delivery is outstanding")
        timers.update([], epoch: 1, enabled: true)
        timers.complete(delivery, retry: false)
        XCTAssertNil(timers.timerForTests(0))
        XCTAssertFalse(timer.isValid)
        XCTAssertFalse(timers.hasReady)
    }
    #if EXACT_LINKS_CALTRAIN
    func testNativeDeliveryCrossesTheOwnerQueueOnceAndRetainsTheRepeat() throws {
        let previous = ExactSession.asyncFills
        defer { ExactSession.asyncFills = previous }
        for asynchronous in [false, true] {
            ExactSession.asyncFills = asynchronous
            NativeTimerTestClock.shared.set(0)
            let session = ExactApp.shared.makeSession()
            defer { session.destroy() }
            session.taskTimers = NativeTaskTimers(now: { NativeTimerTestClock.shared.sample() }, date: { self.date }, ready: { [weak session] in session?.deliverNativeTimer() })
            session.runtime.setLiveClock { NativeTimerTestClock.shared.sample() }
            let boot = session.runtime.boot(width: 390, height: 844)
            XCTAssertNil(boot.error)
            XCTAssertEqual(boot.nativeTimers?.count, 1)
            session.apply(boot)
            let timer = try XCTUnwrap(session.taskTimers.timerForTests(0))
            XCTAssertFalse(session.frames.timerSoon)
            NativeTimerTestClock.shared.set(3400)
            session.holdFillForTests()
            timer.fire(); timer.fire()
            session.deliverNativeTimer()
            XCTAssertTrue(session.taskTimers.hasReady, "A busy owner retains the native delivery")
            session.publishFillForTests(0, Batch(ops: [], timers: true, motion: false, clock: nil, error: nil), generation: session.generation)
            session.landFill()
            session.deliverNativeTimer()
            Owner.shared.sync {}
            session.landFill()
            let landed = expectation(description: "Native delivery completed")
            DispatchQueue.main.async { landed.fulfill() }
            wait(for: [landed], timeout: 10)
            XCTAssertFalse(session.advanceInFlight)
            XCTAssertFalse(session.taskTimers.hasReady)
            XCTAssertTrue(session.taskTimers.timerForTests(0) === timer)
            XCTAssertEqual(session.taskTimers.rows.first?.due, 4000)
            XCTAssertFalse(session.frames.timerSoon)
        }
    }
    #endif
    func testAdmissionPostsToTheOwnerAndRechecksGatesBeforeTheNextFiring() throws {
        let bytes = try fixturePlan("TIMER_ADMISSION"), previous = ExactSession.asyncFills
        defer { ExactSession.asyncFills = previous }
        enum Replacement { case none, beforeSubmission, afterSubmission }
        let cases: [(Bool, Bool, Bool, Replacement)] = [
            (false, false, false, .none), (true, false, false, .none),
            (true, true, false, .none), (true, false, true, .none),
            (true, false, false, .beforeSubmission), (true, false, false, .afterSubmission),
        ]
        for (asynchronous, frame, cancelled, replacement) in cases {
            ExactSession.asyncFills = asynchronous
            let session = ExactApp.shared.makeSession()
            defer { session.destroy() }
            session.runtime.setLiveClock { 1_000_000_000 }
            let boot = session.runtime.bootPlan(bytes, width: 390, height: 844)
            XCTAssertNil(boot.error)
            session.apply(boot)
            XCTAssertFalse(session.advanceInFlight)
            XCTAssertTrue(session.presenter.views.values.contains { $0.props["text"] == "0" })
            session.timerAdmissions.check(frame: frame, update: true, at: 1_000_000_100)
            XCTAssertEqual(session.advanceInFlight, asynchronous && !frame)
            if cancelled { session.timerAdmissions.takeover() }
            if replacement == .beforeSubmission { session.commitForTests() }
            if replacement == .afterSubmission {
                DispatchQueue.main.async {
                    Owner.shared.sync {}
                    session.commitForTests()
                    let replacement = session.runtime.bootPlan(bytes, width: 390, height: 844)
                    XCTAssertNil(replacement.error)
                    session.apply(replacement)
                    XCTAssertFalse(session.timerAdmissions.hasPendingForTests, "A replacement discards the old cutoff before scheduling observations")
                }
            }
            let done = expectation(description: "Admission publications applied")
            func drain() {
                Owner.shared.sync {}
                session.landFill()
                if session.advanceInFlight || session.timerAdmissions.hasPendingForTests { DispatchQueue.main.async { drain() } }
                else { done.fulfill() }
            }
            DispatchQueue.main.async { drain() }
            wait(for: [done], timeout: 10)
            XCTAssertTrue(session.presenter.views.values.contains { $0.props["text"] == (cancelled || replacement == .beforeSubmission ? "0" : "1") }, "Cancellation before submission prevents the firing; a completed action carries its state through reload")
            if replacement == .beforeSubmission {
                session.timerAdmissions.check(frame: true, update: true, at: 1_000_000_100)
                XCTAssertTrue(session.presenter.views.values.contains { $0.props["text"] == "1" }, "A replacement generation admits fresh work after discarding the queued turn")
            }
            XCTAssertFalse(session.frames.timerSoon)
        }
    }

    func testMetadataDecoderRejectsMalformedNativeTimers() {
        let valid = "{\"index\":0,\"generation\":\"0\",\"due\":100,\"interval\":100,\"tolerance\":50,\"once\":false}"
        for invalid in [valid.replacingOccurrences(of: "\"0\"", with: "\"18446744073709551616\""),
                        valid.replacingOccurrences(of: "\"interval\":100", with: "\"interval\":0"),
                        valid.replacingOccurrences(of: "\"due\":100", with: "\"due\":-1"),
                        valid.replacingOccurrences(of: "\"index\":0", with: "\"index\":4294967296"),
                        valid.replacingOccurrences(of: "\"tolerance\":50", with: "\"tolerance\":0.5")] {
            let wire = "{\"ops\":[],\"timers\":true,\"nativeTimers\":[\(invalid)]}"
            XCTAssertThrowsError(try JSONDecoder().decode(Batch.self, from: Data(wire.utf8)))
        }
    }
}

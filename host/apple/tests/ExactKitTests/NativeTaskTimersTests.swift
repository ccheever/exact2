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
    /// The queue's asks for delivery, counted: each runs `deliverNativeTimer`, as the session's own queue does.
    private var asks = 0
    /// An idle session whose only work is a task's native timer (NATIVE_RETRY's `every(50, tick)`), on the live clock:
    /// nothing else applies a batch, so only the timer's own queue can deliver it again.
    private func idleTicker(async asynchronous: Bool) throws -> ExactSession {
        ExactSession.asyncFills = asynchronous
        asks = 0
        let session = ExactApp.shared.makeSession(label: "native-retry")
        session.taskTimers = NativeTaskTimers(now: { [weak session] in session?.now() ?? 0 }, ready: { [weak self, weak session] in
            self?.asks += 1
            session?.deliverNativeTimer()
        })
        XCTAssertNil(session.boot(plan: try fixturePlan("NATIVE_RETRY"), size: CGSize(width: 400, height: 600)).error)
        XCTAssertNotNil(session.taskTimers.rows.first, "the ticker's native timer")
        return session
    }
    private func ticks(_ session: ExactSession) throws -> Int {
        let state = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(session.runtime.agent(#"{"op":"state"}"#).utf8)) as? [String: Any])
        return try XCTUnwrap((state["slots"] as? [String: Any])?["ticks"] as? Int)
    }
    private func spin(until done: () -> Bool = { false }, _ what: String = "", for seconds: Double = 5) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.001)) }
        if !what.isEmpty { XCTAssertTrue(done(), what) }
    }
    /// Inside `gate` (a refusing runtime), the timer's due firing goes through the queue's own ask (no hand
    /// delivery) until that ask has run and the firing is back at the front of its queue, refused. After the gate the
    /// task fires again with nothing else applying a batch, and keeps firing. (That the firings meanwhile add none
    /// beside it is the queue's own, `testARetriedFiringAsksForItselfAndFiringsMeanwhileQueueNoneBesideIt`.)
    private func assertRetriedAndKeepsFiring(_ session: ExactSession, gate: (() -> Void) -> Void) throws {
        let row = try XCTUnwrap(session.taskTimers.rows.first)
        let timer = try XCTUnwrap(session.taskTimers.timerForTests(row.index))
        while session.now() < row.due + 1 { Thread.sleep(forTimeInterval: 0.002) }
        let base = try ticks(session), asked = asks
        gate {
            timer.fire()
            spin(until: { self.asks > asked && session.taskTimers.hasReady && !session.advanceInFlight }, "the queue's own ask ran and the refused firing was re-queued")
        }
        XCTAssertEqual(try ticks(session), base, "the refused firing did not run")
        XCTAssertTrue(session.taskTimers.hasReady || session.advanceInFlight, "kept by its queue")
        spin(until: { (try? self.ticks(session)) ?? 0 >= base + 3 }, "the retried firing ran by itself and the task kept firing")
    }
    /// LLP 1072 T5: a firing the queue delivers while main serves a callback the owner waits on (a nested run loop
    /// inside it) is answered `busy`.
    func testABusyFiringIsRetriedByItsQueueAndTheTaskKeepsFiring() throws {
        let previous = ExactSession.asyncFills
        defer { ExactSession.asyncFills = previous }
        let session = try idleTicker(async: false)
        defer { session.destroy() }
        try assertRetriedAndKeepsFiring(session) { refusing in
            Owner.shared.sync { Owner.shared.callMain { refusing() } }
        }
    }
    /// A firing the queue delivers while a reset holds the runtime, the native path synchronous (the Mac's) or posted
    /// to the owner, whose held answer lands before the re-queue (`deliverNativeTimer`'s async path).
    func testAHeldFiringIsRetriedByItsQueueAndTheTaskKeepsFiring() throws {
        let previous = ExactSession.asyncFills
        defer { ExactSession.asyncFills = previous }
        for asynchronous in [false, true] {
            let session = try idleTicker(async: asynchronous)
            defer { session.destroy() }
            try assertRetriedAndKeepsFiring(session) { refusing in
                session.runtime.held = true
                refusing()
                session.runtime.held = false
                let held = session.runtime.takeHeldCalls()
                XCTAssertTrue(held.contains(asynchronous ? "fireNativeTimerAsync(_:now:done:)" : "fireNativeTimer(_:now:)"), "the runtime held the firing: \(held)")
            }
        }
    }
    /// A retried firing (a busy or held answer) asks for itself a frame later, at the front of its queue; Foundation
    /// firings while it is pending queue none beside it, so it runs once.
    func testARetriedFiringAsksForItselfAndFiringsMeanwhileQueueNoneBesideIt() throws {
        var asked = 0
        let timers = NativeTaskTimers(now: { 100 }, date: { self.date }, ready: { asked += 1 })
        defer { timers.cancel() }
        let a = row()
        timers.update([a], epoch: 1, enabled: true)
        let timer = try XCTUnwrap(timers.timerForTests(0))
        func turns(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }
        timer.fire()
        turns(0.01)
        XCTAssertEqual(asked, 1, "the firing asks for delivery")
        timers.complete(try XCTUnwrap(timers.take()), retry: true)
        timer.fire(); timer.fire() // Foundation firings while the retry is pending
        XCTAssertEqual(asked, 1, "not at once")
        turns(0.1)
        XCTAssertEqual(asked, 2, "the retry asks for itself, once")
        XCTAssertEqual(timers.take()?.row, a, "the retried firing, at the front")
        XCTAssertNil(timers.take(), "the firings meanwhile queued none beside it")
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
            // The queue's own ask meets the fill in flight and leaves the delivery queued.
            let turned = expectation(description: "The queue's ask ran")
            DispatchQueue.main.async { turned.fulfill() }
            wait(for: [turned], timeout: 10)
            XCTAssertTrue(session.taskTimers.hasReady, "A busy owner retains the native delivery")
            session.publishFillForTests(0, Batch(ops: [], timers: true, motion: false, clock: nil, error: nil), generation: session.generation)
            session.landFill()
            // The landed fill resumes the queue: the retained delivery is asked for again, by no hand here.
            let asked = expectation(description: "The queue asks for the retained delivery")
            DispatchQueue.main.async { asked.fulfill() }
            wait(for: [asked], timeout: 10)
            XCTAssertFalse(session.taskTimers.hasReady, "The retained delivery was taken")
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

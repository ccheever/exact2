import XCTest
@testable import ExactKit

/// A timer's (or a scroll event's) batch that changes nothing only moves the clock: it does not
/// reach `apply`, as fills and list feedback already skip theirs, and the
/// next timer is armed for its deadline. A batch with ops, an error, a
/// control's viewless contents (`controls`, LLP 1069.011 §9) or new motion
/// still applies. The flag crosses the wire as `"controls":true`.
final class IdleTickTests: XCTestCase {
    func testATickThatChangesNothingOnlyMovesTheClock() throws {
        let session = ExactApp.shared.makeSession(label: "idle-tick")
        defer { session.destroy() }
        let before = session.appliedBatches
        let due = session.now() + 250
        var idle = Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: due, pending: false)
        XCTAssertTrue(session.changesNothing(idle))
        session.applyUnlessEmpty(idle)
        XCTAssertEqual(session.appliedBatches, before, "an idle tick does not reach apply")
        XCTAssertEqual(session.timerDue, due, "its deadline is kept")
        if !ExactEnv.agentMode {
            XCTAssertEqual(session.clockDue, due, "and the next timer armed for it")
            XCTAssertEqual(session.clockTimer?.isValid, true)
        }
        // A later deadline re-arms; no deadline invalidates what was armed.
        let armed = session.clockTimer
        idle.timerDueMs = due + 250
        session.applyUnlessEmpty(idle)
        if !ExactEnv.agentMode {
            XCTAssertEqual(session.clockDue, due + 250)
            XCTAssertEqual(armed?.isValid, false, "the earlier timer is replaced")
        }
        let later = session.clockTimer
        if !ExactEnv.agentMode { XCTAssertEqual(later?.isValid, true, "the successor is armed") }
        idle.timerDueMs = nil
        session.applyUnlessEmpty(idle)
        XCTAssertEqual(session.appliedBatches, before)
        XCTAssertNil(session.clockTimer)
        XCTAssertNil(session.clockDue)
        XCTAssertNotEqual(later?.isValid, true, "and invalidated")
        // A scroll event's batch is no timer's (`timers` false): the same.
        let scrolled = Batch(ops: [], timers: false, motion: false, clock: nil, error: nil, timerDueMs: due)
        session.applyUnlessEmpty(scrolled)
        XCTAssertEqual(session.appliedBatches, before, "an empty scroll event does not reach apply")
        XCTAssertEqual(session.timerDue, due)
        // Any other event's empty batch still applies: the pass reconciles a
        // native control that changed itself first, and seeks native
        // animations to the agent's clock.
        session.apply(scrolled)
        XCTAssertEqual(session.appliedBatches, before + 1, "an ordinary empty batch reaches apply")
        // Each of these is news: the tick applies.
        var controls = Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: due)
        controls.controls = true
        let failed = Batch(ops: [], timers: true, motion: false, clock: nil, error: "boom", timerDueMs: due)
        let moving = Batch(ops: [], timers: true, motion: true, clock: nil, error: nil, timerDueMs: due)
        let ops = wireBatch([["op": "props", "id": 1, "set": ["testId": "x"]]])
        for batch in [controls, failed, moving, ops] {
            XCTAssertFalse(session.changesNothing(batch))
            let count = session.appliedBatches
            session.applyUnlessEmpty(batch)
            XCTAssertEqual(session.appliedBatches, count + 1)
        }
    }

    /// A slice the owner published before an empty scroll batch lands
    /// first, and the scroll batch then still skips the pass: a fling's
    /// fills are often in flight.
    func testAnEmptyScrollBatchBehindAPublishedSliceLandsItAndSkipsThePass() throws {
        let session = ExactApp.shared.makeSession(label: "idle-behind-slice")
        defer { session.destroy() }
        let due = session.now() + 250
        session.publishTickForTests(wireBatch([["op": "props", "id": 1, "set": ["testId": "x"]]]))
        XCTAssertTrue(session.tickInFlight)
        let before = session.appliedBatches
        session.applyUnlessEmpty(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil, timerDueMs: due), scrolled: true)
        XCTAssertFalse(session.tickInFlight, "the slice landed")
        XCTAssertEqual(session.appliedBatches, before + 1, "the slice's pass, not the empty scroll batch's")
        XCTAssertEqual(session.timerDue, due, "the scroll batch's deadline, the owner's latest")
        // A report the landing owes waits for the batch, then is asked: not stranded by the skip.
        session.publishTickForTests(wireBatch([["op": "props", "id": 1, "set": ["testId": "y"]]]))
        session.oweColorsForTests()
        let held = session.appliedBatches
        session.applyUnlessEmpty(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil, timerDueMs: due), scrolled: true)
        XCTAssertEqual(session.appliedBatches, held + 2, "the slice, then the colours' report; the scroll batch skipped")
        XCTAssertNotEqual(session.timerDue, due, "the report applied after the skip, its clock the latest (an unbooted runtime's has none)")
    }

    /// What waited for a slice runs when it lands, even a slice from before
    /// a commit: a wake's pump dropped there left every later reply in the
    /// executor, whose wake re-arms only on a pump. Work bound to the
    /// presenter's views is dropped once a generation commits.
    func testWorkWaitingForAStaleSliceStillRuns() throws {
        let session = ExactApp.shared.makeSession(label: "stale-slice")
        defer { session.destroy() }
        var free = 0, bound = 0
        session.holdFillForTests()
        session.whenIdle { free += 1 }
        session.whenIdle(bound: true) { bound += 1 }
        XCTAssertEqual(free + bound, 0, "both wait for the slice")
        let before = session.appliedBatches
        let stale = wireBatch([["op": "props", "id": 1, "set": ["testId": "x"]]])
        session.publishFillForTests(1, stale, generation: session.generation - 1)
        session.drainFill()
        XCTAssertFalse(session.fillInFlight)
        XCTAssertEqual(session.appliedBatches, before, "the stale slice applies nothing")
        XCTAssertEqual(free, 1, "the pump still runs")
        XCTAssertEqual(bound, 0, "the old views' news does not")
        // A slice of this generation runs both.
        session.holdFillForTests()
        session.whenIdle { free += 1 }
        session.whenIdle(bound: true) { bound += 1 }
        session.publishFillForTests(1, Batch(ops: [], timers: false, motion: false, clock: nil, error: nil), generation: session.generation)
        session.drainFill()
        XCTAssertEqual(free, 2)
        XCTAssertEqual(bound, 1)
    }

    func testTheControlsFlagDecodes() throws {
        let data = Data(#"{"ops":[],"timers":false,"motion":false,"controls":true}"#.utf8)
        XCTAssertTrue(Batch.decode(data).controls)
        XCTAssertFalse(Batch.decode(Data(#"{"ops":[],"timers":false,"motion":false}"#.utf8)).controls)
    }
}

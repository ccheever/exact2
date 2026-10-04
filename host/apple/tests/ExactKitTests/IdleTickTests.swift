import XCTest
@testable import ExactKit

/// A timer's batch that changes nothing only moves the clock: it does not
/// reach `apply`, as fills and list feedback already skip theirs. A batch
/// with ops, an error, a control's viewless contents (`controls`, LLP
/// 1069.011 §9) or new motion still applies.
final class IdleTickTests: XCTestCase {
    func testATickThatChangesNothingOnlyMovesTheClock() throws {
        let session = ExactApp.shared.makeSession(label: "idle-tick")
        defer { session.destroy() }
        let before = session.appliedBatches
        var idle = Batch(ops: [], timers: true, motion: false, clock: nil, error: nil, timerDueMs: 250, pending: false)
        XCTAssertTrue(session.changesNothing(idle))
        session.applyTick(idle)
        XCTAssertEqual(session.appliedBatches, before, "an idle tick does not reach apply")
        XCTAssertEqual(session.timerDue, 250, "but its deadline is kept")
        idle.timerDueMs = 500
        session.applyTick(idle)
        XCTAssertEqual(session.appliedBatches, before)
        XCTAssertEqual(session.timerDue, 500)
        // Each of these is news: the tick applies.
        var controls = idle; controls.controls = true
        let failed = Batch(ops: [], timers: true, motion: false, clock: nil, error: "boom", timerDueMs: 500)
        let moving = Batch(ops: [], timers: true, motion: true, clock: nil, error: nil, timerDueMs: 500)
        let ops = wireBatch([["op": "props", "id": 1, "set": ["testId": "x"]]])
        for batch in [controls, failed, moving, ops] {
            XCTAssertFalse(session.changesNothing(batch))
            let count = session.appliedBatches
            session.applyTick(batch)
            XCTAssertEqual(session.appliedBatches, count + 1)
        }
    }
}

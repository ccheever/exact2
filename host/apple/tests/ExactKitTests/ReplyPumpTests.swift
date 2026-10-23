import Foundation
import XCTest
@testable import ExactKit

final class ReplyPumpTests: XCTestCase {
    func testAReplyWakeLeavesMainFreeWhileTheOwnerIsBusy() {
        let session = ExactApp.shared.makeSession(label: "reply-pump")
        defer { session.destroy() }
        let release = DispatchSemaphore(value: 0)
        let finished = expectation(description: "owner released by main")
        Owner.shared.post {
            XCTAssertEqual(release.wait(timeout: .now() + 5), .success,
                           "the wake must return so main can release the owner")
            finished.fulfill()
        }
        session.pumpReplies()
        release.signal()
        wait(for: [finished], timeout: 10)
        session.drainFill()
        XCTAssertFalse(session.ownerInFlight)
    }

    func testRepliesLandBeforeALaterBatchAndOldGenerationsAreDropped() {
        let session = ExactApp.shared.makeSession(label: "reply-order")
        defer { session.destroy() }
        session.clock = 0
        let before = session.appliedBatches
        session.pumpReplies()
        session.pumpReplies()
        // A synchronous operation queues after both pumps on the owner.
        Owner.shared.sync {}
        session.apply(Batch(ops: [], timers: true, motion: false, clock: nil,
                            error: nil, timerDueMs: 123))
        XCTAssertEqual(session.appliedBatches, before + 3)
        XCTAssertFalse(session.ownerInFlight)
        XCTAssertEqual(session.timerDue, 123)
        session.landFill()
        XCTAssertEqual(session.timerDue, 123, "no earlier reply may overwrite the later batch")

        let applied = session.appliedBatches
        session.pumpReplies()
        session.commitForTests()
        session.drainFill()
        XCTAssertEqual(session.appliedBatches, applied)
        XCTAssertFalse(session.ownerInFlight)
        session.pumpReplies()
        session.destroy()
        session.drainFill()
        XCTAssertFalse(session.ownerInFlight)
        XCTAssertEqual(session.appliedBatches, applied)
    }
}

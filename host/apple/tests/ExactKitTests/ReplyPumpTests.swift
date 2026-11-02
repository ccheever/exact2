import Foundation
import XCTest
@testable import ExactKit

final class ReplyPumpTests: XCTestCase {
    #if os(macOS)
    func testAReplyCapturesAnOpenModalOnMain() throws {
        let fixture = try SheetFixture()
        defer { fixture.close() }
        try fixture.press("open")
        let session = fixture.session
        XCTAssertNotNil(session.presenter.openTopLayer.modal)
        let capture = session.runtime.topLayer
        var calls = 0
        session.runtime.topLayer = {
            XCTAssertTrue(Thread.isMainThread, "native modal geometry belongs to main")
            guard Thread.isMainThread else { return ([], nil) }
            calls += 1
            return capture?() ?? ([], nil)
        }
        session.pumpReplies()
        XCTAssertEqual(calls, 1, "capture before submitting to the owner")
        session.drainFill()
        XCTAssertEqual(calls, 1)
        session.runtime.topLayer = capture
    }
    #endif

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

    func testDestroyCancelsAReplyWaitingBehindTheOwner() {
        let session = ExactApp.shared.makeSession(label: "reply-cancel")
        let entered = DispatchSemaphore(value: 0)
        Owner.shared.post {
            entered.signal()
            Owner.shared.callMain {
                XCTAssertTrue(session.runtime.destroyed, "destruction starts before the queued pump")
            }
        }
        XCTAssertEqual(entered.wait(timeout: .now() + 5), .success)
        session.pumpReplies()
        // Destroy waits on the owner and serves the barrier's main callback.
        session.runtime.destroy()
        session.destroy()
        session.drainFill()
        XCTAssertEqual(session.repliesInFlight, 0)
        XCTAssertTrue(session.replySubmissions.isEmpty)
        XCTAssertFalse(session.ownerInFlight)
    }

    func testAStaleReplyRetiresOnlyItsOwnPendingActivation() {
        for newerPending in [false, true] {
            let session = ExactApp.shared.makeSession(label: "reply-activation")
            defer { session.destroy() }
            let old = session.generation
            session.pendingActivation = (old, 17)
            session.activatedGeneration = old
            session.pumpReplies()
            session.commitForTests()
            if newerPending {
                session.pendingActivation = (session.generation, 23)
                session.activatedGeneration = session.generation
            }
            session.drainFill()
            if newerPending {
                XCTAssertEqual(session.pendingActivation?.generation, session.generation)
                XCTAssertEqual(session.pendingActivation?.token, 23)
                XCTAssertEqual(session.activatedGeneration, session.generation)
                session.pumpReplies()
                session.drainFill()
                XCTAssertNil(session.pendingActivation)
                XCTAssertEqual(session.activatedGeneration, session.generation, "the matching wake retries firstDrawn")
            } else {
                XCTAssertNil(session.pendingActivation, "a discarded generation must not keep waiting for a spent wake")
            }
        }
    }

    func testEmptyScrollAndTimerBatchesSkipPresentationWhileAReplyParses() {
        let session = ExactApp.shared.makeSession(label: "reply-scroll")
        defer { session.destroy() }
        session.clock = 0
        let release = DispatchSemaphore(value: 0)
        Owner.shared.post { XCTAssertEqual(release.wait(timeout: .now() + 5), .success) }
        session.pumpReplies()
        let before = session.appliedBatches
        let empty = Batch(ops: [], timers: false, motion: false, clock: nil, error: nil)
        XCTAssertTrue(session.changesNothing(empty))
        for _ in 0..<10 {
            session.applyUnlessEmpty(empty, scrolled: true)
            session.applyUnlessEmpty(empty)
        }
        XCTAssertEqual(session.appliedBatches, before, "an unpublished reply must not force empty presenter passes")
        XCTAssertEqual(session.repliesInFlight, 1)
        release.signal()
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

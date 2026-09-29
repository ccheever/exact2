import Foundation
import XCTest
@testable import ExactKit

// @ref LLP 1072 T1/T5 — the owner thread's contract.
final class OwnerTests: XCTestCase {
    func testSyncRunsOnTheOwnerInOrder() {
        var seen: [Int] = []
        for i in 0..<100 {
            let onOwner = Owner.shared.sync { () -> Bool in seen.append(i); return Owner.shared.isOwner }
            XCTAssertTrue(onOwner)
        }
        XCTAssertEqual(seen, Array(0..<100))
        XCTAssertFalse(Owner.shared.isOwner)
    }

    func testNestedSyncOnTheOwnerRunsInline() {
        let value = Owner.shared.sync { Owner.shared.sync { 7 } + 1 }
        XCTAssertEqual(value, 8)
    }

    func testCallMainFromTheOwnerIsServedWhileMainWaits() {
        let result = Owner.shared.sync { () -> (Bool, Int) in
            let answer = Owner.shared.callMain { () -> (Bool, Int) in (Thread.isMainThread, 41) }
            return (answer.0, answer.1 + 1)
        }
        XCTAssertTrue(result.0)
        XCTAssertEqual(result.1, 42)
    }

    func testARuntimeCallFromAServedCallbackIsRefusedNotDeadlocked() {
        let refused = Owner.shared.sync { () -> Int in
            Owner.shared.callMain { Owner.shared.sync({ 1 }, busy: -1) }
        }
        XCTAssertEqual(refused, -1)
    }

    func testANotificationFromAServedCallbackRunsAfterTheCurrentJob() {
        var order: [String] = []
        Owner.shared.sync {
            Owner.shared.callMain {
                Owner.shared.syncOrLater { order.append("later") }
                order.append("served")
            }
            order.append("job")
        }
        // The deferred job runs after the one that asked; a later sync is
        // queued behind it.
        Owner.shared.sync { order.append("next") }
        XCTAssertEqual(order, ["served", "job", "later", "next"])
    }

    func testCallMainFromAnAsynchronousOwnerJobIsServedByTheMainQueue() {
        let done = expectation(description: "served")
        Owner.shared.sync {
            Owner.shared.callMain {
                Owner.shared.syncOrLater {
                    let onMain = Owner.shared.callMain { Thread.isMainThread }
                    XCTAssertTrue(onMain)
                    DispatchQueue.main.async { done.fulfill() }
                }
            }
        }
        wait(for: [done], timeout: 5)
    }
}

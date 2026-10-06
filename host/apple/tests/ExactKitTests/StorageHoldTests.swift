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

import XCTest
@testable import ExactKit

/// On the wall the hatch clock's 4,096-call bound is one presented frame's
/// (LLP 1075.003.000.001 §2.4): a ticket ticks for as long as frames come,
/// and one frame still stops at the bound.
final class HatchClockTests: XCTestCase {
    func testAWallTicketTicksPastTheBoundAndOneFrameStopsAtIt() {
        let clock = HatchClock()
        clock.frames(1, on: true)
        let frames = HatchClock.limit + 100
        let ticked = (0..<frames).filter { clock.presented(Double($0) * 1000 / 120) }.count
        XCTAssertEqual(ticked, frames, "every frame's ticket ticks, 4,096 frames in and after")

        let crowded = HatchClock()
        for token in 1...UInt64(HatchClock.limit + 1) { crowded.frames(token, on: true) }
        XCTAssertFalse(crowded.presented(0), "one frame calls 4,096 ticks at most")
    }
}

#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// One display link for the app's life: made once, paused while no one
/// wants frames, at the highest rate its users ask for.
final class FrameClockIOSTests: XCTestCase {
    private final class Owner {}

    func testOneLinkIsMadeAndPausedWhenNoOneWantsFrames() throws {
        let clock = FrameClock()
        let a = Owner(), b = Owner()
        XCTAssertNil(clock.link, "no link before anyone wants frames")
        clock.want(a, .scroll) { _ in }
        XCTAssertTrue(clock.running)
        let link = try XCTUnwrap(clock.link)
        clock.want(b, .heavyLeaves) { _ in }
        clock.drop(a)
        XCTAssertTrue(clock.running, "b still wants frames")
        clock.drop(b)
        XCTAssertFalse(clock.running, "paused, not torn down")
        for _ in 0..<50 { clock.want(a, .scroll) { _ in }; clock.drop(a) }
        XCTAssertTrue(clock.link === link)
        XCTAssertEqual(clock.linksMade, 1)
    }

    func testTheRateIsTheHighestAsked() {
        let clock = FrameClock()
        let a = Owner(), b = Owner()
        clock.want(a, .session) { _ in }
        XCTAssertEqual(clock.link?.preferredFrameRateRange, .default)
        clock.want(b, .heavyLeaves, rate: CAFrameRateRange(minimum: 80, maximum: 120, preferred: 120)) { _ in }
        clock.want(a, .session, rate: CAFrameRateRange(minimum: 30, maximum: 60, preferred: 60)) { _ in }
        XCTAssertEqual(clock.link?.preferredFrameRateRange, CAFrameRateRange(minimum: 80, maximum: 120, preferred: 120))
        clock.drop(b)
        XCTAssertEqual(clock.link?.preferredFrameRateRange, CAFrameRateRange(minimum: 30, maximum: 60, preferred: 60))
    }

    func testUsersTickInOrderAndADroppedOneDoesNot() throws {
        let clock = FrameClock()
        let session = Owner(), scroll = Owner(), leaves = Owner()
        var ticks: [String] = []
        clock.want(scroll, .scroll) { _ in ticks.append("scroll") }
        clock.want(leaves, .heavyLeaves) { _ in ticks.append("leaves") }
        clock.want(session, .session) { _ in ticks.append("session"); clock.drop(session); clock.drop(scroll) }
        let link = try XCTUnwrap(clock.link)
        clock.fire(link)
        XCTAssertEqual(ticks, ["session", "leaves"])
        XCTAssertTrue(clock.running)
        clock.drop(leaves)
        XCTAssertFalse(clock.running)
    }

    func testAGoneOwnerStopsTicking() throws {
        let clock = FrameClock()
        var owner: Owner? = Owner()
        var ticks = 0
        clock.want(owner!, .heavyLeaves) { _ in ticks += 1 }
        owner = nil
        clock.fire(try XCTUnwrap(clock.link))
        XCTAssertEqual(ticks, 0)
        XCTAssertFalse(clock.running)
    }
}
#endif

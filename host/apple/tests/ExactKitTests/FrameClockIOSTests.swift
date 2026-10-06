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

    /// Motion asks for the display's full rate, and a paint-only cap or a
    /// timer's `.default` never lowers it (LLP 1061 D4).
    func testMotionAtTheFullRateOutranksAPaintCap() {
        let clock = FrameClock()
        let paint = Owner(), driver = Owner(), timer = Owner()
        let full = FrameClock.full(on: nil)
        XCTAssertGreaterThanOrEqual(full.maximum, 60)
        XCTAssertEqual(full.minimum, min(60, full.maximum))
        clock.want(timer, .session) { _ in }
        clock.want(paint, .svgFilter, rate: CAFrameRateRange(minimum: 30, maximum: 60, preferred: 60)) { _ in }
        clock.want(driver, .offsetDriver, rate: full) { _ in }
        XCTAssertEqual(clock.link?.preferredFrameRateRange.maximum, full.maximum)
        XCTAssertEqual(clock.link?.preferredFrameRateRange.minimum, max(30, full.minimum))
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

    /// A list's smooth correction takes the app's frames, not a link of its
    /// own, and gives them back when it ends or is cancelled.
    func testASmoothCorrectionRunsOnTheAppsClock() throws {
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 400))
        let scroll = UIScrollView(frame: window.bounds)
        scroll.contentSize = CGSize(width: 300, height: 2000)
        window.addSubview(scroll)
        window.makeKeyAndVisible()
        let made = FrameClock.shared.linksMade
        var ended: Bool?
        let driver = OffsetDriver(scroll: scroll, to: CGPoint(x: 0, y: 600), duration: 0.3, serial: 1,
                                  done: { _, finished in ended = finished }, reclamp: { _ in })
        driver.start()
        XCTAssertTrue(FrameClock.shared.wants(driver))
        XCTAssertLessThanOrEqual(FrameClock.shared.linksMade, max(made, 1))
        let link = try XCTUnwrap(FrameClock.shared.link)
        FrameClock.shared.fire(link)
        XCTAssertNil(ended)
        driver.cancel()
        XCTAssertFalse(FrameClock.shared.wants(driver))
    }
}
#endif

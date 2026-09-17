import CoreFoundation
import Foundation
import XCTest
@testable import ExactKit

private final class ClockTicks: @unchecked Sendable {
    // The production timer and this oracle run only on the main run loop.
    var count = 0
    func tick() {
        precondition(Thread.isMainThread)
        count += 1
    }
}

final class SessionClockTimerTests: XCTestCase {
    private func run(_ mode: RunLoop.Mode, seconds: TimeInterval) {
        let end = Date(timeIntervalSinceNow: seconds)
        while Date() < end && RunLoop.main.run(mode: mode, before: end) {}
    }

    private func trackingMode() -> RunLoop.Mode {
        // Model the existing AppKit tracking/common-mode membership without
        // a window, synthetic delegate notification or an agent clock.
        let mode = RunLoop.Mode("ExactSessionClockTrackingTest")
        CFRunLoopAddCommonMode(CFRunLoopGetMain(), CFRunLoopMode(rawValue: mode.rawValue as CFString))
        return mode
    }

    func testClockRunsInDefaultMode() {
        XCTAssertTrue(Thread.isMainThread)
        let ticks = ClockTicks()
        let timer = SessionClockTimer.schedule { _ in ticks.tick() }
        defer { timer.invalidate() }
        XCTAssertEqual(timer.timeInterval, 0.25)
        run(.default, seconds: 0.6)
        XCTAssertGreaterThan(ticks.count, 0)
    }

    func testClockPreservesInitialDeadline() {
        let before = Date()
        let original = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { _ in }
        let timer = SessionClockTimer.schedule { _ in }
        let after = Date()
        defer { original.invalidate(); timer.invalidate() }
        for candidate in [original, timer] {
            XCTAssertGreaterThanOrEqual(candidate.fireDate, before.addingTimeInterval(0.25))
            XCTAssertLessThanOrEqual(candidate.fireDate, after.addingTimeInterval(0.25))
        }
    }

    func testClockRunsWhileTrackingModeIsActive() {
        XCTAssertTrue(Thread.isMainThread)
        let mode = trackingMode(), ticks = ClockTicks()
        let timer = SessionClockTimer.schedule { _ in ticks.tick() }
        defer { timer.invalidate() }
        run(mode, seconds: 0.6)
        XCTAssertGreaterThan(ticks.count, 0, "tracking must not suspend the session clock")
    }

    func testClockInvalidationRemovesTrackingDelivery() {
        XCTAssertTrue(Thread.isMainThread)
        let mode = trackingMode(), ticks = ClockTicks()
        let timer = SessionClockTimer.schedule { _ in ticks.tick() }
        run(mode, seconds: 0.6)
        XCTAssertGreaterThan(ticks.count, 0)
        timer.invalidate()
        let stopped = ticks.count
        run(mode, seconds: 0.3)
        XCTAssertFalse(timer.isValid)
        XCTAssertEqual(ticks.count, stopped)
    }
}

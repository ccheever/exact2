import XCTest
@testable import ExactUpdates

final class UpdateChecksTests: XCTestCase {
    func testEveryPolicyDiscoversWhileActiveAndOnlyImmediateActivates() {
        for policy in ["next-launch", "app-decides", "immediate"] {
            var time: TimeInterval = 0
            var tick: (() -> Void)?
            var requests = 0, activations = 0, timers = 0
            let checks = ForegroundUpdateChecks(interval: 3, immediate: policy == "immediate", check: {
                requests += 1
            }, activate: {
                activations += 1
            }, now: { time }, schedule: { interval, callback in
                XCTAssertEqual(interval, 3)
                timers += 1
                tick = callback
                return { timers -= 1 }
            })
            checks.foreground(false)
            XCTAssertEqual(requests, 0)
            checks.foreground(true)
            checks.foreground(true)
            XCTAssertEqual(requests, 1)
            XCTAssertEqual(timers, 1)
            time += 3
            tick?()
            checks.completed("staged seq 2")
            XCTAssertEqual(requests, 2, policy)
            XCTAssertEqual(activations, policy == "immediate" ? 1 : 0, policy)
            checks.foreground(false)
            XCTAssertEqual(timers, 0)
            tick?()
            checks.completed("staged seq 3")
            XCTAssertEqual(requests, 2)
            XCTAssertEqual(activations, policy == "immediate" ? 1 : 0)
            time += 1
            checks.foreground(true)
            XCTAssertEqual(requests, 2, "foreground checks are debounced")
            XCTAssertEqual(activations, policy == "immediate" ? 2 : 0, "a check completed in the background waits for foreground")
            checks.foreground(false)
            time += 60
            checks.foreground(true)
            XCTAssertEqual(requests, 3)
        }
    }

    func testTheDefaultIsFifteenMinutesAndInvalidIntervalsFallBack() {
        for interval: TimeInterval? in [nil, 0, -1, .infinity, .nan] {
            let checks = ForegroundUpdateChecks(interval: interval, immediate: false, check: {}, activate: {})
            XCTAssertEqual(checks.interval, 900)
        }
    }
}

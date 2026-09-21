import CoreFoundation
import Foundation
import QuartzCore
import XCTest
@testable import ExactKit

private final class ClockTicks: @unchecked Sendable {
    // The production timer and this oracle run only on the main run loop.
    var count = 0
    var times: [Double] = []
    func tick() {
        precondition(Thread.isMainThread)
        count += 1
        times.append(CACurrentMediaTime() * 1000)
    }
}

final class SessionClockTimerTests: XCTestCase {
    func testTargetedLayoutDigestStaysWithEachSessionsNode() throws {
        #if os(macOS)
        let a = ExactApp.shared.makeSession(), b = ExactApp.shared.makeSession()
        defer { a.destroy(); b.destroy() }
        XCTAssertNil(a.boot(size: CGSize(width: 390, height: 844)).error)
        XCTAssertNil(b.boot(size: CGSize(width: 390, height: 844)).error)
        func inspect(_ session: ExactSession, mapped: Bool) throws -> [String: Any] {
            let bytes = try XCTUnwrap(session.agent("{\"op\":\"tree\"}").data(using: .utf8))
            let tree = try XCTUnwrap(JSONSerialization.jsonObject(with: bytes) as? [String: Any])
            let roots = try XCTUnwrap(tree["roots"] as? [Int])
            let id = try XCTUnwrap(roots.first)
            let reply = session.agentInstance.layout(["id": id, "plan": mapped])
            let node = try XCTUnwrap(reply["node"] as? [String: Any])
            XCTAssertEqual(node["id"] as? Int, id)
            return node
        }
        XCTAssertNil(try inspect(a, mapped: false)["planDigest"])
        let first = try inspect(a, mapped: true)
        let digest = try XCTUnwrap(first["planDigest"] as? String)
        XCTAssertEqual(digest.count, 64)
        XCTAssertNil(try inspect(b, mapped: false)["planDigest"])
        XCTAssertEqual(try inspect(b, mapped: true)["planDigest"] as? String, digest)
        XCTAssertEqual(try inspect(a, mapped: true)["planDigest"] as? String, digest)
        XCTAssertNil(first["plan"], "inspection must not carry the plan or its text")
        #endif
    }

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
        let timer = SessionClockTimer.schedule(after: 0.025) { _ in ticks.tick() }
        defer { timer.invalidate() }
        XCTAssertEqual(timer.timeInterval, 0)
        run(.default, seconds: 0.6)
        XCTAssertGreaterThan(ticks.count, 0)
    }

    func testClockPreservesInitialDeadlineAndOnlyFiresOnce() {
        let before = Date(), ticks = ClockTicks()
        let timer = SessionClockTimer.schedule(after: 0.05) { _ in ticks.tick() }
        let after = Date()
        defer { timer.invalidate() }
        XCTAssertGreaterThanOrEqual(timer.fireDate, before.addingTimeInterval(0.05))
        XCTAssertLessThanOrEqual(timer.fireDate, after.addingTimeInterval(0.05))
        run(.default, seconds: 0.2)
        XCTAssertEqual(ticks.count, 1)
        XCTAssertFalse(timer.isValid)
    }

    func testDeadlineSelectsFramesOnlyForNearTimers() {
        XCTAssertEqual(SessionClockTimer.wake(due: 116, now: 100), .frame)
        XCTAssertEqual(SessionClockTimer.wake(due: 0, now: 100), .frame, "a hitch catches up next frame")
        XCTAssertEqual(SessionClockTimer.wake(due: 1100, now: 100), .timeout(1000))
        XCTAssertEqual(SessionClockTimer.wake(due: nil, now: 100), .none)
        XCTAssertEqual(SessionClockTimer.wake(due: 116, now: 100, agent: true), .none)
    }

    func testNestedTransformBatchPreservesTimerDeadline() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        session.clock = nil
        for delta in [16.0, 1000.0] {
            let due = session.now() + delta
            let data = try JSONSerialization.data(withJSONObject: ["accepted": true,
                "batch": ["ops": [], "timers": true, "timer_due_ms": due]])
            let reply = try XCTUnwrap(TransformDragReply(data))
            XCTAssertEqual(reply.batch.timerDueMs, due)
            session.apply(reply.batch)
            if delta == 16 {
                XCTAssertTrue(session.frames.timerSoon)
                XCTAssertNotNil(session.frames.link)
            } else {
                XCTAssertFalse(session.frames.timerSoon)
                XCTAssertNotNil(session.clockTimer)
            }
        }
    }

    func testSlowTimerDoesNotRunDisplayLinkBetweenTicks() {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        session.clock = nil
        session.scheduleClock(due: session.now() + 16)
        XCTAssertTrue(session.frames.timerSoon)
        XCTAssertNotNil(session.frames.link)
        session.scheduleClock(due: session.now() + 1000)
        XCTAssertFalse(session.frames.timerSoon)
        XCTAssertNil(session.frames.link)
        XCTAssertNotNil(session.clockTimer)
        session.scheduleClock(due: nil)
        XCTAssertNil(session.clockTimer)
    }

    func testOldPollFailsTheWallClockCadenceBar() {
        let ticks = ClockTicks(), started = CACurrentMediaTime()
        let old = Timer(timeInterval: 0.25, repeats: true) { _ in ticks.tick() }
        RunLoop.main.add(old, forMode: .common)
        defer { old.invalidate() }
        run(.default, seconds: 2)
        let maxGap = zip(ticks.times, ticks.times.dropFirst()).map { $1 - $0 }.max() ?? 0
        print(String(format: "TIMER NEGATIVE CONTROL old250 elapsed_ms=%.1f wakes=%d max_gap_ms=%.2f", (CACurrentMediaTime() - started) * 1000, ticks.count, maxGap))
        XCTAssertGreaterThan(ticks.count, 1)
        XCTAssertLessThan(ticks.count, 50, "the old timer must fail the animation throughput bar")
        XCTAssertGreaterThan(maxGap, 40, "the old timer must fail the animation gap bar")
    }

    func testTraceRejectsOldPollingEvenWhenBurstContainsManyFlowOps() {
        var trace = SessionTimerTrace()
        let burst = batchFixture(ops: Array(repeating: ["op": "flow"], count: 15), timers: true, motion: false, clock: nil, error: nil)
        for now in stride(from: 0.0, to: 1000, by: 250) { XCTAssertNil(trace.record(burst, at: now)) }
        let line = trace.record(burst, at: 1000)!
        XCTAssertTrue(line.contains("flow_applies=5"), line)
        XCTAssertTrue(line.contains("flow_ops=75"), line)
        XCTAssertTrue(line.contains("max_gap_ms=250.00"), line)
    }

    func testClockRunsWhileTrackingModeIsActive() {
        XCTAssertTrue(Thread.isMainThread)
        let mode = trackingMode(), ticks = ClockTicks()
        let timer = SessionClockTimer.schedule(after: 0.025) { _ in ticks.tick() }
        defer { timer.invalidate() }
        run(mode, seconds: 0.6)
        XCTAssertGreaterThan(ticks.count, 0, "tracking must not suspend the session clock")
    }

    func testClockInvalidationRemovesTrackingDelivery() {
        XCTAssertTrue(Thread.isMainThread)
        let mode = trackingMode(), ticks = ClockTicks()
        let timer = SessionClockTimer.schedule(after: 0.025) { _ in ticks.tick() }
        run(mode, seconds: 0.6)
        XCTAssertGreaterThan(ticks.count, 0)
        timer.invalidate()
        let stopped = ticks.count
        run(mode, seconds: 0.3)
        XCTAssertFalse(timer.isValid)
        XCTAssertEqual(ticks.count, stopped)
    }
}

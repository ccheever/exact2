import Foundation
import QuartzCore
import XCTest
import CExact
#if os(macOS)
import AppKit
#endif
@testable import ExactKit

final class FrameHealthMetricsTests: XCTestCase {
    override class func setUp() {
        super.setUp()
        #if os(macOS)
        _ = NSApplication.shared
        #endif
    }
    func testMetricCursorsFinalizeWholeGapsAndKeepTheirOwnApplyWindow() throws {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        health.record(arrival: 1, target: 1.008, period: 0.008, finished: 1.001)
        let before = health.metrics()
        let cursor = try XCTUnwrap(before["cursor"] as? [String: Any])
        health.record(arrival: 1.024, target: 1.032, period: 0.008, finished: 1.025)
        var work = ExactFrameHealthWork(); work.began_ns = 1_026_000_000; work.finished_ns = 1_029_000_000; work.target_ns = 1_028_000_000
        exact_frame_health_applied(health.handle, work, 0)
        let after = health.metrics(cursor: cursor)
        let callback = try XCTUnwrap(after["callbacks"] as? [String: Any])
        XCTAssertEqual(callback["comparableIntervals"] as? String, "1")
        XCTAssertEqual(callback["observedNs"] as? String, "24000000")
        XCTAssertEqual(callback["p99BoundsNs"] as? [UInt64], [24_000_000, 48_000_000])
        XCTAssertEqual((after["apply"] as? [String: Any])?["targetOverruns"] as? String, "1")
        let window = try XCTUnwrap(after["window"] as? [String: Any])
        XCTAssertEqual(window["intervalThroughNs"] as? String, "1024000000")
        XCTAssertEqual(window["applyThroughNs"] as? String, "1029000000")
        let idleCursor = try XCTUnwrap(after["cursor"] as? [String: Any])
        health.stop()
        let idle = health.metrics(cursor: idleCursor)
        XCTAssertEqual((idle["callbacks"] as? [String: Any])?["observedNs"] as? String, "0")
        XCTAssertTrue((idle["callbacks"] as? [String: Any])?["p99BoundsNs"] is NSNull)
        health.reset()
        XCTAssertEqual(health.metrics(cursor: cursor)["error"] as? String, "baseline-required")
        XCTAssertEqual(health.metrics(cursor: ["version": 1])["error"] as? String, "baseline-required")
    }

    func testDenseCursorsStayInTheFourKBEnvelopeAndMalformedBinsFailClosed() throws {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        let snapshot = ExactFrameHealthSnapshot()
        let sample = FrameHealthMetrics.Sample(producer: health.handle, snapshot: snapshot,
            intervals: Array(repeating: UInt32.max, count: 64), applies: Array(repeating: UInt32.max, count: 64), read: UInt64.max)
        let reply = FrameHealthMetrics.reply(sample, health: health)
        XCTAssertNil(reply["error"])
        XCTAssertLessThanOrEqual(try JSONSerialization.data(withJSONObject: reply).count + 1, 4000)
        XCTAssertEqual((reply["histogram"] as? [String: Any])?["valid"] as? Bool, false)
        var invalid = health.metrics()["cursor"] as! [String: Any]
        invalid["intervals"] = Array(repeating: -1, count: 64)
        XCTAssertEqual(health.metrics(cursor: invalid)["error"] as? String, "baseline-required")
        invalid["intervals"] = Array(repeating: 0, count: 65)
        XCTAssertEqual(health.metrics(cursor: invalid)["error"] as? String, "baseline-required")
    }

    func testPlatformFrameReadsDoNotAllocateAnObserverOrStartTheLegacySampler() throws {
        guard ExactEnv.agentMode, !ExactEnv.agentFreezes else { throw XCTSkip("platform-agent regression") }
        let session = ExactApp.shared.makeSession(label: "read-without-observation")
        defer { session.destroy() }
        session.sampler?.stop(); session.sampler = nil
        let pipe = Pipe(), saved = Agent.out
        Agent.out = pipe.fileHandleForWriting
        defer { Agent.out = saved }
        session.agentInstance.handle(op: "perf", ["frames": true, "source": "scroll", "health": true], line: "")
        let missing = try XCTUnwrap(JSONSerialization.jsonObject(with: pipe.fileHandleForReading.availableData) as? [String: Any])
        XCTAssertEqual(missing["error"] as? String, "native-scroll-unobserved")
        XCTAssertNil(session.frameHealth.nativeHealth)
        session.agentInstance.handle(op: "perf", ["frames": true], line: "")
        let reply = try XCTUnwrap(JSONSerialization.jsonObject(with: pipe.fileHandleForReading.availableData) as? [String: Any])
        XCTAssertEqual(reply["format"] as? String, FrameHealthMetrics.format)
        XCTAssertNil(session.sampler)
    }

    func testLiveCaptureGuardsAllClockRequestsAndInspectorOwnershipIsIndependent() throws {
        let session = ExactApp.shared.makeSession(label: "frame-health-clock-guard")
        defer { session.destroy() }
        session.clock = nil
        let health = session.frameHealth
        health.setInspectorVisible(true)
        _ = health.setAgentCapture("start")
        for request: [String: Any] in [["take": true], ["data": true], ["settle": true], ["to": 500], ["land": true]] {
            XCTAssertEqual(session.agentInstance.clock(request)["code"] as? String, "frame-health-clock-takeover")
            XCTAssertNil(session.clock)
        }
        _ = health.setAgentCapture("stop")
        XCTAssertNotEqual(health.captureID, 0, "closing an agent lease does not close the human inspector")
        health.setInspectorVisible(false)
        XCTAssertEqual(health.captureID, 0)
        XCTAssertNotNil(session.agentInstance.clock(["take": true])["clock"])
    }
}

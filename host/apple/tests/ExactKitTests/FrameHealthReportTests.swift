import Foundation
import QuartzCore
import XCTest
import CExact
#if os(macOS)
import AppKit
#endif
@testable import ExactCore
@testable import ExactKit

final class FrameHealthReportTests: XCTestCase {
    override class func setUp() {
        super.setUp()
        #if os(macOS)
        _ = NSApplication.shared
        #endif
    }
    private func populated() -> (FrameHealth, Double) {
        let health = FrameHealth(enabled: true), start = CACurrentMediaTime()
        _ = health.setAgentCapture("start")
        health.requestedRate(120)
        for index in 0..<40 {
            let at = start + Double(index) * 0.024
            health.record(arrival: at, target: at + 0.008, period: 0.008, finished: at + 0.001)
        }
        return (health, start + 1)
    }

    func testTailIsBoundedLossAndMissingEvidenceSurviveEncodingAndExpiry() throws {
        let (health, now) = populated()
        defer { health.destroy() }
        let report = FrameHealthReport.snapshot(health: health, session: nil, now: now)
        let data = FrameHealthReport.encode(report)
        XCTAssertLessThanOrEqual(data.count + 1, 4000)
        let decoded = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let columns = try XCTUnwrap(decoded["columns"] as? [String])
        let events = try XCTUnwrap(decoded["hitches"] as? [[Any]])
        XCTAssertGreaterThan(events.count, 0)
        XCTAssertEqual(events.last?[0] as? String, "39")
        XCTAssertEqual((decoded["retention"] as? [String: Any])?["omittedForBudget"] as? Int, 16 - events.count)
        XCTAssertEqual(events.first?[columns.firstIndex(of: "gap_ns")!] as? UInt64, 24_000_000)
        XCTAssertEqual(events.first?[columns.firstIndex(of: "requested_period_ns")!] as? UInt64, 8_333_333)
        XCTAssertTrue(events.first?[columns.firstIndex(of: "owner_queued_offset_ns")!] is NSNull)
        XCTAssertEqual((decoded["retention"] as? [String: Any])?["overwrittenSinceClear"] as? String, "23")
        XCTAssertEqual((decoded["retention"] as? [String: Any])?["automaticDiskBytes"] as? Int, 0)
        XCTAssertTrue((decoded["unobserved"] as? [String])?.contains("presentation") == true)
        health.reset()
        XCTAssertEqual(health.historyStatus().retained, 16, "a reload preserves the evidence a person wants to share")
        XCTAssertEqual(health.hitch(0).generation, 0)
        XCTAssertEqual(health.snapshot().generation, 1)
        _ = health.setAgentCapture("stop")
        let expired = FrameHealthReport.snapshot(health: health, session: nil, now: now + 121)
        XCTAssertEqual((expired["hitches"] as? [[Any]])?.count, 0)
        XCTAssertEqual((expired["retention"] as? [String: Any])?["expiredAtRead"] as? Int, 16)
        XCTAssertEqual(health.historyStatus().retained, 16, "age filtering does not allocate an expiry timer or grow storage")
        let oversized = FrameHealthReport.encode(["unexpected": String(repeating: "x", count: 20_000)])
        XCTAssertLessThan(oversized.count, 1024)
        XCTAssertNotNil((try JSONSerialization.jsonObject(with: oversized) as? [String: Any])?["error"])
    }

    func testDenseEvidenceFitsFourKBWithoutChangingTimingPrecision() throws {
        let (health, now) = populated()
        defer { health.destroy() }
        var report = FrameHealthReport.snapshot(health: health, session: nil, now: now)
        let columns = try XCTUnwrap(report["columns"] as? [String])
        let rows: [[Any]] = (0..<16).map { index in
            columns.enumerated().map { column, _ -> Any in
                column < 3 ? String(index + 1) as Any : Int64(-8_999_999_999_999_991) as Any
            }
        }
        report["hitches"] = rows
        report["identity"] = ["app": String(repeating: "é", count: 80), "device": String(repeating: "🦊", count: 40)]
        let data = FrameHealthReport.encode(report)
        XCTAssertLessThanOrEqual(data.count + 1, 4000, "the carrier newline is part of the hard cap")
        let decoded = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let events = try XCTUnwrap(decoded["hitches"] as? [[Any]])
        XCTAssertGreaterThan(events.count, 0)
        XCTAssertLessThan(events.count, 16)
        XCTAssertEqual(events.last?[3] as? Int64, -8_999_999_999_999_991)
        XCTAssertEqual((decoded["retention"] as? [String: Any])?["omittedForBudget"] as? Int, 16 - events.count)
    }

    func testAgentReportAcknowledgesOnlyAfterItsAsynchronousReply() throws {
        let session = ExactApp.shared.makeSession(label: "frame-health-report-order")
        defer { session.destroy() }
        let pipe = Pipe(), saved = Agent.out, savedRoutes = Agent.routes
        Agent.out = pipe.fileHandleForWriting
        Agent.routes = [(session.label, session)]
        defer { Agent.out = saved; Agent.routes = savedRoutes }
        let done = expectation(description: "the carrier may read the next request after the report is written")
        var acknowledged = false
        Agent.handle("{\"op\":\"perf\",\"frames\":true,\"report\":true}") {
            acknowledged = true
            let data = pipe.fileHandleForReading.availableData
            XCTAssertLessThanOrEqual(data.count, 4000)
            XCTAssertNotNil(try? JSONSerialization.jsonObject(with: data))
            done.fulfill()
        }
        XCTAssertFalse(acknowledged)
        wait(for: [done], timeout: 5)
        XCTAssertTrue(acknowledged)
    }

    func testCaptureLeasesHudOwnershipAndReportBackpressureAreIndependent() throws {
        let (health, now) = populated()
        defer { health.destroy() }
        health.setHUDVisible(true)
        _ = health.setAgentCapture("stop")
        XCTAssertNotEqual(health.captureID, 0, "an agent stop cannot disable the person's monitor")
        health.setHUDVisible(false)
        XCTAssertEqual(health.captureID, 0)
        health.requestedRate(60)
        _ = health.setAgentCapture("start", duration: 1)
        XCTAssertEqual(health.requestedPeriodNs, 16_666_667, "policy changes are cached even while detail capture is paused")
        health.expireCapture(at: now + 1)
        XCTAssertEqual(health.captureID, 0, "an expired agent lease needs no timer or idle wakeup")

        let finished = expectation(description: "one bounded utility encoding completes")
        health.agentReport(session: nil, now: now + 1000) { data in
            XCTAssertTrue(Thread.isMainThread)
            XCTAssertLessThanOrEqual(data.count, FrameHealthReport.maximumBytes)
            finished.fulfill()
        }
        XCTAssertTrue(health.reportInFlight)
        var busy: [String: Any]?
        let secondProducer = FrameHealth(enabled: true)
        defer { secondProducer.destroy() }
        FrameHealthReport.prepare(health: secondProducer, session: nil, now: now) { data in
            busy = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
        }
        XCTAssertEqual(busy?["error"] as? String, "report-busy")
        var limited: [String: Any]?
        health.agentReport(session: nil, now: now + 1001) { data in
            limited = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
        }
        XCTAssertEqual(limited?["error"] as? String, "report-rate-limit")
        XCTAssertEqual(limited?["retryAfterMs"] as? Int, 14_000)
        wait(for: [finished], timeout: 5)
        XCTAssertFalse(health.reportInFlight)
    }

    func testAsynchronousTickCarriesActualOwnerAndApplyBoundaries() throws {
        let session = ExactApp.shared.makeSession(label: "frame-health-owner")
        defer { session.destroy() }
        session.clock = nil
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        _ = session.frameHealth.setAgentCapture("start")
        let done = expectation(description: "owner tick returns and applies on main")
        let frame = (CACurrentMediaTime() - 0.002 - ExactEnv.t0) * 1000
        session.runtime.tickAsync(now: session.now(), frame: frame, capture: session.frameHealth.captureID) { batch in
            DispatchQueue.main.async {
                var landed = batch
                landed.frameWorkTiming?.landed = CACurrentMediaTime()
                session.apply(landed)
                done.fulfill()
            }
        }
        wait(for: [done], timeout: 10)
        let health = session.frameHealth
        XCTAssertGreaterThan(health.historyStatus().apply_deadlines, 0)
        let events = (0..<UInt32(health.historyStatus().retained)).map { health.hitch($0) }
        let event = try XCTUnwrap(events.first { $0.flags & 4 != 0 })
        XCTAssertGreaterThan(event.work.queued_ns, 0)
        XCTAssertGreaterThanOrEqual(event.work.owner_began_ns, event.work.queued_ns)
        XCTAssertGreaterThanOrEqual(event.work.owner_finished_ns, event.work.owner_began_ns)
        XCTAssertGreaterThanOrEqual(event.work.landed_ns, event.work.owner_finished_ns)
        XCTAssertGreaterThanOrEqual(event.work.began_ns, event.work.landed_ns)
        XCTAssertGreaterThan(event.work.finished_ns, event.work.target_ns)
    }

    func testFrameCaptureAgentRequestDoesNotEnableTheLegacySampler() throws {
        let session = ExactApp.shared.makeSession(label: "frame-health-agent")
        defer { session.destroy() }
        session.clock = nil
        session.sampler?.stop(); session.sampler = nil
        let pipe = Pipe(), saved = Agent.out
        Agent.out = pipe.fileHandleForWriting
        defer { Agent.out = saved }
        let request: [String: Any] = ["op": "perf", "frames": true, "capture": "start", "durationMs": 1000]
        Agent(session: session).handle(op: "perf", request, line: "")
        let reply = try XCTUnwrap(JSONSerialization.jsonObject(with: pipe.fileHandleForReading.availableData) as? [String: Any])
        XCTAssertEqual(reply["capture"] as? Bool, true)
        XCTAssertNil(session.sampler)
        XCTAssertNotNil(session.frameHealth.setAgentCapture("start", duration: .nan)["error"])
    }
}

import XCTest
@testable import ExactCore

/// LLP 1142 D1: a late frame's interval is charged each main-thread phase's
/// time no phase inside it took, clipped to the interval, and what none took
/// is unattributed, never negative.
final class HostPhasesTests: XCTestCase {
    private var now = 0.0
    private func phases() -> HostPhases {
        let p = HostPhases()
        p.clock = { [unowned self] in now }
        p.on = true
        return p
    }
    private func ms(_ w: [String: Any]?, _ phase: String) -> Double? {
        (w?["phases"] as? [[String: Any]])?.first { $0["phase"] as? String == phase }?["ms"] as? Double
    }

    func testANestedPhaseIsChargedToItselfOnly() {
        let p = phases()
        now = 1.000
        let apply = p.begin(.apply)
        now = 1.002
        p.measure(.anchor) { now = 1.005 }
        now = 1.006
        p.end(apply)
        let w = p.window(from: 1.0, to: 1.010)
        XCTAssertEqual(ms(w, "apply"), 3)
        XCTAssertEqual(ms(w, "anchor"), 3)
        XCTAssertEqual(w?["unattributed"] as? Double, 4)
        XCTAssertNil(w?["coverage"])
    }

    func testASegmentCrossingTheWindowIsClippedToIt() {
        let p = phases()
        now = 1.000
        p.measure(.rows) { now = 1.020 }
        let w = p.window(from: 1.010, to: 1.016)
        XCTAssertEqual(ms(w, "rows"), 6)
        XCTAssertEqual(w?["unattributed"] as? Double, 0)
    }

    func testAnIdleWindowIsAllUnattributedAndAnOpenPhaseIsNotYetCharged() {
        let p = phases()
        now = 1.000
        let open = p.begin(.scroll)
        now = 1.010
        let w = p.window(from: 1.002, to: 1.010)
        XCTAssertEqual((w?["phases"] as? [Any])?.count, 0)
        XCTAssertEqual(w?["unattributed"] as? Double, 8)
        p.end(open)
        XCTAssertEqual(ms(p.window(from: 1.002, to: 1.010), "scroll"), 8)
    }

    func testAWindowBeforeRecordingOrPastWhatTheRingKeptIsPartial() {
        now = 5.0
        let p = phases()
        XCTAssertEqual(p.window(from: 4.9, to: 5.1)?["coverage"] as? String, "partial")
        for i in 0..<(HostPhases.capacity + 1) {
            now = 6.0 + Double(i) * 0.001
            p.measure(.text) { now += 0.0005 }
        }
        XCTAssertEqual(p.window(from: 6.0, to: 6.002)?["coverage"] as? String, "partial")
        XCTAssertNil(p.window(from: 6.5, to: 6.6)?["coverage"])
    }

    func testWorkCountsAreFixedAggregatesAndOffCollectsNothing() throws {
        let p = HostPhases()
        p.count(.ownerSync)
        var work = p.work()
        XCTAssertEqual((work["counts"] as? [String: Int])?["ownerSync"], 0)
        p.on = true
        p.count(.ownerSync)
        p.count(.intrinsicReports, by: 150)
        p.count(.intrinsicLayoutRequests)
        work = p.work()
        let counts = try XCTUnwrap(work["counts"] as? [String: Int])
        XCTAssertEqual(counts.count, HostPhases.Operation.allCases.count)
        XCTAssertEqual(counts["intrinsicReports"], 150)
        XCTAssertEqual(counts["intrinsicLayoutRequests"], 1)
        XCTAssertEqual(work["scope"] as? String, "process")
        XCTAssertEqual(work["site"] as? String, "unknown")
        p.on = false
        p.count(.intrinsicReports)
        XCTAssertEqual((p.work()["counts"] as? [String: Int])?["intrinsicReports"], 150)
    }

    func testNothingIsRecordedWhileOff() {
        let p = HostPhases()
        p.clock = { [unowned self] in now }
        XCTAssertFalse(p.begin(.apply))
        XCTAssertNil(p.window(from: 0, to: 1))
    }
}

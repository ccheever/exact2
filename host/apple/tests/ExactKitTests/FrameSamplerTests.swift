import Foundation
import QuartzCore
import XCTest
@testable import ExactKit

/// LLP 1079 D3–D5 on Apple: the trailer's `seq`, the sampler's arithmetic
/// against the link's own target period, its journal line, and Save Trace.
final class FrameSamplerTests: XCTestCase {
    func testTheTrailerCarriesTheTransactionsItApplies() {
        let batch = Batch.decode(Data(#"{"ops":[],"seq":[3,7],"timers":false,"motion":false,"canvas":false,"clock":0,"error":null}"#.utf8))
        XCTAssertNil(batch.error)
        XCTAssertEqual(batch.seq?.0, 3)
        XCTAssertEqual(batch.seq?.1, 7)
        XCTAssertNil(Batch.decode(Data(#"{"ops":[],"timers":false,"motion":false,"clock":0,"error":null}"#.utf8)).seq)
    }

    func testALateFrameIsCountedAgainstTheTargetPeriodJournaledAndSaved() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler, "a development build samples")
        let p = 1.0 / 120, t = ExactEnv.t0 + 10
        sampler.observe(now: t, target: t + p)                     // the baseline, never a sample
        sampler.observe(now: t + p, target: t + 2 * p)             // on time
        sampler.batch((5, 6), ms: 2.5)
        sampler.observe(now: t + 5 * p, target: t + 6 * p)         // three frames missed
        let reply = sampler.reply()
        let lifetime = try XCTUnwrap(reply["lifetime"] as? [String: Int])
        XCTAssertEqual(lifetime["presented"], 2)
        XCTAssertEqual(lifetime["late"], 1)
        XCTAssertEqual(lifetime["missed"], 3)
        let late = try XCTUnwrap((reply["late"] as? [[String: Any]])?.first)
        XCTAssertEqual(late["missed"] as? Int, 3)
        XCTAssertEqual(late["seq"] as? [UInt64], [5, 6])
        XCTAssertEqual(late["apply"] as? Double, 2.5)
        XCTAssertEqual((reply["period"] as? [String: Any])?["source"] as? String, "target")
        XCTAssertTrue(session.agent(#"{"op":"logs","since":0}"#).contains("frame late at "), "a late frame is one journal line")

        let url = try session.saveTrace().get()
        defer { try? FileManager.default.removeItem(at: url) }
        let trace = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
        for key in ["identity", "proxies", "plan", "journal", "frames", "perf"] { XCTAssertNotNil(trace[key], key) }
        XCTAssertEqual((trace["frames"] as? [String: Any])?["records"].map { ($0 as? [Any])?.count }, 2)
        XCTAssertNotNil((trace["perf"] as? [String: Any])?["sites"], "the runner measures in a development build")
    }
}

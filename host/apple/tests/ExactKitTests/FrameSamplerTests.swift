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

    /// A frame the link delivered on time is late all the same when the main
    /// thread's turn that commits it ends after the frame's target: the
    /// render server shows the last frame again.
    func testATurnPastItsFramesTargetIsAnOverrun() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler)
        let p = 1.0 / 60, t = ExactEnv.t0 + 20
        sampler.observe(now: t, target: t + p)
        sampler.turnBegan(at: t + 0.001)
        sampler.turnEnded(at: t + 0.010)                           // within the frame
        sampler.observe(now: t + p, target: t + 2 * p)
        sampler.turnBegan(at: t + p + 0.002)
        sampler.turnEnded(at: t + 2 * p + 0.004)                   // 4 ms past its target
        sampler.observe(now: t + 2 * p, target: t + 3 * p)         // the link itself on time
        sampler.turnBegan(at: t + 2 * p + 0.017)                   // a turn that began after it
        sampler.turnEnded(at: t + 2 * p + 0.030)
        sampler.observe(now: t + 3 * p, target: t + 4 * p)
        let reply = sampler.reply()
        let lifetime = try XCTUnwrap(reply["lifetime"] as? [String: Int])
        XCTAssertEqual(lifetime["missed"], 0)
        XCTAssertEqual(lifetime["overruns"], 1)
        XCTAssertEqual(lifetime["late"], 1)
        let late = try XCTUnwrap((reply["late"] as? [[String: Any]])?.first)
        XCTAssertEqual(try XCTUnwrap(late["overrun"] as? Double), 4, accuracy: 0.05)
        XCTAssertTrue(session.agent(#"{"op":"logs","since":0}"#).contains("ms past the target"))

        // One turn through two callbacks, never sleeping between: the first
        // target is overrun when the second callback comes.
        let u = t + 10 * p
        sampler.observe(now: u, target: u + p, at: u + 0.0005)
        sampler.turnBegan(at: u + 0.001)
        sampler.observe(now: u + p, target: u + 2 * p, at: u + p + 0.006)
        sampler.turnEnded(at: u + p + 0.008)
        // A stop drops an overrun not yet sampled: the next segment's first
        // frames are on time.
        sampler.observe(now: u + 2 * p, target: u + 3 * p, at: u + 2 * p + 0.0005)
        sampler.turnBegan(at: u + 2 * p + 0.001)
        sampler.turnEnded(at: u + 3 * p + 0.005)
        sampler.stop()
        let v = u + 20 * p
        sampler.observe(now: v, target: v + p, at: v + 0.0005)
        sampler.observe(now: v + p, target: v + 2 * p, at: v + p + 0.0005)
        let after = try XCTUnwrap(sampler.reply()["lifetime"] as? [String: Int])
        XCTAssertEqual(after["overruns"], 2, "the turn through two callbacks, not the stopped segment's")
        let through = try XCTUnwrap((sampler.reply()["late"] as? [[String: Any]])?.last)
        XCTAssertEqual(try XCTUnwrap(through["overrun"] as? Double), 6, accuracy: 0.05)
    }

    /// LLP 1075.003.000.001 §3.1: a late frame names the hatch calls that
    /// overlapped its interval, each charged its overlap; an on-time one names none.
    func testALateFrameNamesTheHatchesThatRanInIt() throws {
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sampler = try XCTUnwrap(session.sampler, "a development build samples")
        let p = 1.0 / 60
        var t = CACurrentMediaTime()
        sampler.observe(now: t, target: t + p)
        session.hatchDiagnostics.timed("element avatar", "built", counts: false) { Thread.sleep(forTimeInterval: 0.03) }
        session.hatchDiagnostics.timed("element avatar", "changed", counts: false) { Thread.sleep(forTimeInterval: 0.005) }
        t = CACurrentMediaTime()
        sampler.observe(now: t, target: t + p)                       // 35 ms and more since the last: late
        sampler.observe(now: t + p, target: t + 2 * p)               // on time, and no call in it
        let late = try XCTUnwrap(sampler.reply()["late"] as? [[String: Any]])
        XCTAssertEqual(late.count, 1)
        let ran = try XCTUnwrap(late[0]["hatches"] as? [[String: Any]])
        XCTAssertEqual(ran.count, 1)
        XCTAssertEqual(ran[0]["hatch"] as? String, "element avatar")
        XCTAssertEqual(ran[0]["calls"] as? Int, 2)
        XCTAssertGreaterThanOrEqual(try XCTUnwrap(ran[0]["ms"] as? Double), 35)
        XCTAssertNil(late[0]["coverage"], "every call in the window is still kept")
        // A window before any call names none; one that cuts a call charges the part inside it.
        XCTAssertNil(session.hatchDiagnostics.window(from: t - 100, to: t - 99))
        let half = try XCTUnwrap(session.hatchDiagnostics.window(from: t - 0.010, to: t)?["hatches"] as? [[String: Any]])
        XCTAssertLessThanOrEqual(try XCTUnwrap(half[0]["ms"] as? Double), 10.01)
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

        // What a hatch recorded is in the trace's `hatches`, as `state` and `perf hatches` give it (LLP 1075.003.000.001 §3.3).
        _ = session.hatchDiagnostics.record(kind: 1, node: 0, scope: "module", name: "swipes", value: 3, text: Data())
        let url = try session.saveTrace().get()
        defer { try? FileManager.default.removeItem(at: url) }
        let trace = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
        for key in ["identity", "proxies", "plan", "journal", "frames", "perf", "hatches"] { XCTAssertNotNil(trace[key], key) }
        let hatches = try XCTUnwrap(trace["hatches"] as? [String: [String: Any]])
        XCTAssertEqual(((hatches["perf"]?["counters"] as? [String: Any])?["module"] as? [String: Int])?["swipes"], 3)
        XCTAssertEqual((((hatches["state"]?["scopes"] as? [String: Any])?["module"] as? [String: Any])?["counters"] as? [String: Int])?["swipes"], 3)
        XCTAssertEqual(hatches["perf"]?["plan"] as? String, (trace["perf"] as? [String: Any])?["plan"] as? String, "the hatches' read names the plan `perf` does")
        XCTAssertEqual((trace["frames"] as? [String: Any])?["records"].map { ($0 as? [Any])?.count }, 2)
        XCTAssertNotNil((trace["perf"] as? [String: Any])?["sites"], "the runner measures in a development build")
        // The last one also under the name a phone's is copied off by
        // (`agent.mjs trace --phone` reads this literal name), replaced by
        // the next.
        let latest = FileManager.default.temporaryDirectory.appendingPathComponent("trace-latest.json")
        let first = try Data(contentsOf: url)
        XCTAssertEqual(try Data(contentsOf: latest), first)
        session.log("between the traces")
        let next = try session.saveTrace().get()
        defer { try? FileManager.default.removeItem(at: next) }
        XCTAssertEqual(try Data(contentsOf: latest), try Data(contentsOf: next))
        XCTAssertNotEqual(try Data(contentsOf: latest), first)
    }
}

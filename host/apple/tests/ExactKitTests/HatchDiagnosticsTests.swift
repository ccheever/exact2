import XCTest
@testable import ExactKit

/// What hatch code records is bounded (LLP 1075.003.000.001 §3.2): each
/// bound refuses and counts past it, a call's time is its own, counts from
/// other threads sum exactly, and their other records overwrite the oldest
/// past the ring. A read changes nothing.
final class HatchDiagnosticsTests: XCTestCase {
    private func record(_ store: HatchDiagnostics, _ kind: UInt32, _ name: String = "", _ value: Double = 0, text: String = "",
                        scope: String = "module", node: UInt32 = 0) -> UInt64 {
        store.record(kind: kind, node: node, scope: scope, name: name, value: value, text: Data(text.utf8))
    }

    private func refusals(_ store: HatchDiagnostics) -> [String: Int] {
        let state = store.state(words: [:])
        return ["rejected": state["rejected"] as? Int ?? -1, "abandoned": state["abandoned"] as? Int ?? -1, "dropped": state["dropped"] as? Int ?? -1]
    }

    func testEachBoundRefusesAndCountsPastIt() throws {
        try XCTSkipUnless(HatchDiagnostics.measuring)
        let store = HatchDiagnostics()

        // Counters: 64 names; a bad name or amount is refused.
        for i in 0..<70 { _ = record(store, 1, "c\(i)", 2) }
        _ = record(store, 1, "Bad Name", 1)
        _ = record(store, 1, "c0", -1)
        _ = record(store, 1, "c0", 3)
        let counters = (store.perf(tags: [:])["counters"] as? [String: [String: UInt64]])?["module"] ?? [:]
        XCTAssertEqual(counters.count, 64)
        XCTAssertEqual(counters["c0"], 5)
        XCTAssertEqual(refusals(store)["rejected"], 6 + 2)

        // A timing: a cumulative count, sum and max, and its last 256 samples.
        for i in 1...300 { _ = record(store, 2, "frame", Double(i)) }
        let frame = try XCTUnwrap(((store.perf(tags: [:])["timings"] as? [String: [String: [String: Any]]])?["module"])?["frame"])
        XCTAssertEqual(frame["count"] as? Int, 300)
        XCTAssertEqual(frame["sum"] as? Double, 45150)
        XCTAssertEqual(frame["max"] as? Double, 300)
        XCTAssertEqual(frame["samples"] as? Int, 256)
        XCTAssertEqual(frame["dropped"] as? Int, 44)
        XCTAssertEqual(frame["measured"] as? Bool, true)

        // Spans: 32 open at once; a node's open ones end with it, abandoned; an end twice is one.
        let spans = (0..<33).map { _ in record(store, 3, "held", scope: "element dot", node: 7) }
        XCTAssertEqual(refusals(store)["rejected"], 8 + 1)
        _ = record(store, 4, "", Double(spans[0]))
        _ = record(store, 4, "", Double(spans[0]))
        store.ended(node: 7)
        XCTAssertEqual(refusals(store)["abandoned"], 31)
        let held = try XCTUnwrap(((store.perf(tags: [:])["timings"] as? [String: [String: [String: Any]]])?["element dot"])?["held"])
        XCTAssertEqual(held["count"] as? Int, 1)

        // Snapshots: 16 names of 4 KB of JSON; the latest is kept; one that does not fit is refused whole.
        for i in 0..<18 { _ = record(store, 5, "s\(i)", text: "{\"n\":\(i)}") }
        _ = record(store, 5, "s0", text: "{\"n\":99}")
        _ = record(store, 5, "s1", text: "\"" + String(repeating: "x", count: 5000) + "\"")
        _ = record(store, 5, "s2", text: "{not json")
        let scopes = try XCTUnwrap(store.state(words: [:])["scopes"] as? [String: [String: Any]])
        let published = try XCTUnwrap(scopes["module"]?["published"] as? [String: [String: Int]])
        XCTAssertEqual(published.count, 16)
        XCTAssertEqual(published["s0"], ["n": 99])
        XCTAssertEqual(published["s1"], ["n": 1])
        XCTAssertEqual(refusals(store)["rejected"], 9 + 2 + 2)

        // A read changes nothing.
        let a = try JSONSerialization.data(withJSONObject: store.perf(tags: [:]), options: .sortedKeys)
        let b = try JSONSerialization.data(withJSONObject: store.perf(tags: [:]), options: .sortedKeys)
        XCTAssertEqual(a, b)
    }

    func testACallsTimeIsItsOwn() throws {
        try XCTSkipUnless(HatchDiagnostics.measuring)
        let store = HatchDiagnostics()
        store.timed("navigation", "built") {
            Thread.sleep(forTimeInterval: 0.004)
            store.timed("element dot", "built", counts: false) { Thread.sleep(forTimeInterval: 0.02) }
        }
        let hatches = try XCTUnwrap(store.perf(tags: [:])["hatches"] as? [String: [String: Any]])
        let outer = try XCTUnwrap(hatches["navigation"]?["ms"] as? Double), inner = try XCTUnwrap(hatches["element dot"]?["ms"] as? Double)
        XCTAssertGreaterThanOrEqual(inner, 20)
        XCTAssertGreaterThanOrEqual(outer, 4)
        XCTAssertLessThan(outer, 20, "the nested call's time is not the outer call's")
        // A container's calls are counted by scope; a node's are ElementHatches'.
        let scopes = try XCTUnwrap(store.state(words: [:])["scopes"] as? [String: [String: Any]])
        XCTAssertEqual(scopes["navigation"]?["calls"] as? [String: Int], ["built": 1])
        XCTAssertNil(scopes["element dot"])
    }

    func testCountsFromFourThreadsSumExactlyAndTheRingOverwritesItsOldest() throws {
        try XCTSkipUnless(HatchDiagnostics.measuring)
        let store = HatchDiagnostics()
        let done = expectation(description: "four threads")
        done.expectedFulfillmentCount = 4
        for _ in 0..<4 {
            Thread.detachNewThread {
                for _ in 0..<10_000 { _ = store.record(kind: 1, node: 0, scope: "module", name: "ticks", value: 1, text: Data()) }
                done.fulfill()
            }
        }
        wait(for: [done], timeout: 30)
        let counters = (store.perf(tags: [:])["counters"] as? [String: [String: UInt64]])?["module"] ?? [:]
        XCTAssertEqual(counters["ticks"], 40_000)

        // 2,000 samples from another thread before the main thread drains: the last 1,024 are kept.
        let samples = expectation(description: "samples")
        Thread.detachNewThread {
            for i in 0..<2000 { _ = store.record(kind: 2, node: 0, scope: "module", name: "late", value: Double(i), text: Data()) }
            samples.fulfill()
        }
        wait(for: [samples], timeout: 30)
        let late = try XCTUnwrap(((store.perf(tags: [:])["timings"] as? [String: [String: [String: Any]]])?["module"])?["late"])
        let kept = try XCTUnwrap(late["count"] as? Int)
        XCTAssertEqual(kept + (refusals(store)["dropped"] ?? 0), 2000)
        XCTAssertEqual(late["max"] as? Double, 1999)
    }
}

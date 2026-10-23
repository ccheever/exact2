import XCTest
@testable import ExactKit
@testable import ExactDrag

/// Answers Arrange's calls as the runtime would, and records them in order:
/// the Swift half of the web/Linux sequence (`host/web/src/reorder_tests.rs`,
/// `host/linux/tests/it/arrange.rs`); `host/apple/src/arrange_tests.rs` is
/// the Rust half.
final class ReorderRecorder: ReorderCalls {
    var log: [String] = []
    var list: UInt32 = 1, wrapper: UInt32 = 2
    var admit = true
    private func state(_ phase: String, dispatched: Bool = false) -> Batch {
        wireBatch([["op": "reorder", "token": "7", "list": Int(list), "wrapper": Int(wrapper),
                    "phase": phase, "dispatched": dispatched]])
    }
    func reorderBegin(_ handle: UInt32, scrollTop: Double, now: Double) -> Batch {
        log.append("begin \(handle) top=\(scrollTop)")
        return admit ? state("active") : wireBatch([["op": "reorder", "token": "0", "list": 0, "wrapper": 0, "phase": "refused"]])
    }
    func reorderMove(_ token: UInt64, dy: Double, scrollTop: Double, inside: Bool, now: Double) -> Batch {
        log.append("move \(token) dy=\(dy)\(inside ? "" : " outside")")
        return state("active")
    }
    func reorderEnd(_ token: UInt64, drop: Bool, dy: Double, scrollTop: Double, inside: Bool, velocity: Double, now: Double) -> Batch {
        log.append(drop ? "drop \(token) dy=\(dy)" : "cancel \(token)")
        return state("settling", dispatched: drop)
    }
    static func finished() -> ReorderState? {
        ReorderState(["op": "reorder", "token": "7", "list": 0, "wrapper": 0, "phase": "finished", "dispatched": true])
    }
}

/// The grouped session's calls, recorded (LLP 1094 D5): the drop answers
/// `holding` until the test says otherwise.
final class ReorderGroupRecorder: ReorderGroupCalls {
    var log: [String] = []
    var hold = true
    func state(_ phase: String, target: Int = 1, row: Int = 2, ending: String? = nil, dispatched: Bool = false) -> Batch {
        var op: [String: Any] = ["op": "reorder", "group": true, "token": "7", "list": 1, "wrapper": 2,
            "phase": phase, "target": target, "row": row, "dispatched": dispatched]
        op["ending"] = ending ?? NSNull()
        return wireBatch([op])
    }
    func groupBegin(_ handle: UInt32, scrollTop: Double, ghost: Bool, now: Double) -> Batch {
        log.append("begin \(handle)\(ghost ? " ghost" : "")")
        return state("active")
    }
    func moveInto(_ token: UInt64, target: UInt32, contentY: Double, scrollTop: Double, inside: Bool, now: Double) -> Batch {
        log.append("into \(target)")
        return state("active", target: Int(target))
    }
    func groupStep(_ token: UInt64, step: ReorderGroupStep, now: Double) -> Batch {
        log.append("step \(step.rawValue)")
        return state("active", target: 5)
    }
    func groupEnd(_ token: UInt64, drop: Bool, now: Double) -> Batch {
        log.append(drop ? "drop" : "cancel")
        if !drop { return state("cancelling", row: 2) }
        return hold ? state("holding", target: 5, dispatched: true)
            : state("settling", target: 5, row: 6, ending: "landed", dispatched: true)
    }
    func groupFinish(_ token: UInt64, now: Double) -> Batch {
        log.append("finish")
        return state("finished", target: 0, row: 0)
    }
}

final class ReorderTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactDrag.install() } // LLP 1047.001 D4
    func testStateKeepsTheSerialExactAndRefusesAMalformedOp() {
        let state = ReorderState(["token": "18446744073709551615", "list": 3, "wrapper": 9, "phase": "settling", "dispatched": true])
        XCTAssertEqual(state?.token, UInt64.max)
        XCTAssertEqual(state?.list, 3)
        XCTAssertEqual(state?.wrapper, 9)
        XCTAssertEqual(state?.dispatched, true)
        XCTAssertNil(ReorderState(["token": 7, "list": 3, "wrapper": 9, "phase": "active"]))
        XCTAssertNil(ReorderState(["token": "7", "list": 3, "phase": "active"]))
    }

    func testEdgeScrollIsTheWebsBandsAndSpeed() {
        XCTAssertEqual(ReorderEdge.direction(offset: 10, height: 400), -1)
        XCTAssertEqual(ReorderEdge.direction(offset: -30, height: 400), -1, "past the top still scrolls up")
        XCTAssertEqual(ReorderEdge.direction(offset: 200, height: 400), 0)
        XCTAssertEqual(ReorderEdge.direction(offset: 380, height: 400), 1)
        XCTAssertEqual(ReorderEdge.direction(offset: 900, height: 400), 1)
        XCTAssertEqual(ReorderEdge.direction(offset: .nan, height: 400), 0)
        XCTAssertEqual(ReorderEdge.step(direction: 1, dt: 0.016), 720 * 0.016, accuracy: 1e-9)
        XCTAssertEqual(ReorderEdge.step(direction: -1, dt: 0.5), -720 * 0.032, accuracy: 1e-9, "at most 32 ms of catch-up")
        XCTAssertEqual(ReorderEdge.step(direction: 1, dt: -1), 0)
    }

    func testReleaseVelocityIsTheLastTwoSamples() {
        var v = ReorderVelocity()
        XCTAssertEqual(v.value, 0)
        v.record(time: 1.0, y: 0)
        v.record(time: 1.02, y: 10)
        v.record(time: 1.04, y: 30)
        XCTAssertEqual(v.value, 1000, accuracy: 1e-6)
        v.record(time: 1.03, y: 99) // backwards in time: refused
        XCTAssertEqual(v.value, 1000, accuracy: 1e-6)
        v.record(time: 1.2, y: 30) // a stationary end sample reads as rest
        XCTAssertEqual(v.value, 0)
    }
}

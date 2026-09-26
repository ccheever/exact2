import Foundation
import XCTest
@testable import ExactKit

final class CollectionTests: XCTestCase {
    func testWireIsVersionTwoLittleEndianAndPreservesLargeEpochs() {
        let facts = CollectionFacts(top: 12, portWidth: 320, portHeight: 480, rowWidth: 300,
            measurements: [.init(view: 23, epoch: 0xfedcba9876543210, height: 72)], focus: 31, interaction: nil)
        let bytes = [UInt8](facts.encode(view: 17, revision: 9, sequence: 11))
        XCTAssertEqual(bytes.count, 100)
        XCTAssertEqual(Array(bytes.prefix(8)), [2, 0, 0, 0, 17, 0, 0, 0])
        XCTAssertEqual(Array(bytes[8..<16]), [9, 0, 0, 0, 0, 0, 0, 0])
        XCTAssertEqual(Array(bytes[56..<64]), [31, 0, 0, 0, 0, 0, 0, 0])
        XCTAssertEqual(Array(bytes[72..<80]), [255, 255, 255, 255, 1, 0, 0, 0], "no limit, one row")
        XCTAssertEqual(Array(bytes[84..<92]), [0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe])
        let filled = [UInt8](facts.encode(view: 17, revision: 9, sequence: 11, velocity: -2, limit: 3))
        XCTAssertEqual(Array(filled[64..<76]), [0, 0, 0, 0, 0, 0, 0, 0xc0, 3, 0, 0, 0])
    }

    func testCorrectionIsOncePerRevisionAndCannotReplaceNewUserIntent() {
        var cursor = CollectionCursor()
        XCTAssertTrue(cursor.takeCorrection(revision: 2, sequence: 0))
        XCTAssertFalse(cursor.takeCorrection(revision: 2, sequence: 0))
        cursor.advance()
        XCTAssertFalse(cursor.takeCorrection(revision: 3, sequence: 0))
        XCTAssertTrue(cursor.takeCorrection(revision: 3, sequence: 1))
        XCTAssertFalse(cursor.takeCorrection(revision: 2, sequence: 1))
    }

    func testFeedbackDedupIncludesEpochGeometryAndBothIndependentPins() {
        let a = CollectionFacts(top: 0, portWidth: 100, portHeight: 200, rowWidth: 80,
            measurements: [.init(view: 4, epoch: 7, height: 30)], focus: 8, interaction: 9)
        var b = a
        XCTAssertEqual(a, b)
        b.measurements[0].epoch = 8
        XCTAssertNotEqual(a, b)
        b = a; b.rowWidth = 79
        XCTAssertNotEqual(a, b)
        b = a; b.focus = nil
        XCTAssertNotEqual(a, b)
        b = a; b.interaction = nil
        XCTAssertNotEqual(a, b)
    }

    func testMalformedSnapshotsDoNotBecomeGeometry() {
        let row: [String: Any] = ["view": 3, "root": 4, "index": 0, "top": 0, "height": 40, "epoch": "18446744073709551614", "measured": false]
        let value: [String: Any] = ["view": 2, "revision": 5, "scrollSequence": 0, "count": 100,
            "totalExtent": 4000, "rows": [row], "correction": NSNull()]
        XCTAssertEqual(CollectionSnapshot(value)?.rows.first?.epoch, UInt64.max - 1)
        var broken = value; broken["totalExtent"] = Double.nan
        XCTAssertNil(CollectionSnapshot(broken))
        broken = value; broken["rows"] = [row, row]
        XCTAssertNil(CollectionSnapshot(broken))
    }


    func testDecimalJSONCountersRetainAllSixtyFourBits() throws {
        let bytes = Data(#"{"view":2,"revision":"18446744073709551614","scrollSequence":"18446744073709551613","totalExtent":4000,"rows":[],"correction":{"scrollSequence":"18446744073709551613","scrollTop":90}}"#.utf8)
        let object = try XCTUnwrap(JSONSerialization.jsonObject(with: bytes) as? [String: Any])
        let snapshot = try XCTUnwrap(CollectionSnapshot(object))
        XCTAssertEqual(snapshot.revision, UInt64.max - 1)
        XCTAssertEqual(snapshot.sequence, UInt64.max - 2)
        XCTAssertEqual(snapshot.correction?.sequence, UInt64.max - 2)
    }

    func testTurnBudgetRejectsRecursionAndYieldsAfterTwoPasses() {
        var budget = CollectionTurnBudget()
        XCTAssertTrue(budget.begin())
        XCTAssertFalse(budget.begin())
        budget.end()
        XCTAssertTrue(budget.begin())
        budget.end()
        XCTAssertFalse(budget.begin())
        budget.nextTurn()
        XCTAssertTrue(budget.begin())
    }
}

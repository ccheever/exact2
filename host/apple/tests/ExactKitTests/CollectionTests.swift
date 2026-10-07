import Foundation
import XCTest
@testable import ExactKit

final class CollectionTests: XCTestCase {
    func testWireIsVersionThreeLittleEndianAndPreservesLargeEpochs() {
        let facts = CollectionFacts(offset: 12, portMain: 480, portCross: 320, cross: 300,
            measurements: [.init(view: 23, epoch: 0xfedcba9876543210, size: 72)], focus: 31, interaction: nil)
        let bytes = [UInt8](facts.encode(view: 17, revision: 9, sequence: 11))
        XCTAssertEqual(bytes.count, 104)
        XCTAssertEqual(Array(bytes.prefix(8)), [3, 0, 0, 0, 17, 0, 0, 0])
        XCTAssertEqual(Array(bytes[32..<40]), [UInt8](withUnsafeBytes(of: (480.0).bitPattern.littleEndian, Array.init)), "the main axis first")
        XCTAssertEqual(Array(bytes[8..<16]), [9, 0, 0, 0, 0, 0, 0, 0])
        XCTAssertEqual(Array(bytes[56..<64]), [31, 0, 0, 0, 0, 0, 0, 0])
        XCTAssertEqual(Array(bytes[72..<84]), [255, 255, 255, 255, 0, 0, 0, 0, 1, 0, 0, 0], "no limit, no moving ancestor, one row")
        XCTAssertEqual(Array(bytes[88..<96]), [0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe])
        let filled = [UInt8](facts.encode(view: 17, revision: 9, sequence: 11, velocity: -2, limit: 3, ancestorMoving: true))
        XCTAssertEqual(Array(filled[64..<80]), [0, 0, 0, 0, 0, 0, 0, 0xc0, 3, 0, 0, 0, 1, 0, 0, 0])
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
        let a = CollectionFacts(offset: 0, portMain: 200, portCross: 100, cross: 80,
            measurements: [.init(view: 4, epoch: 7, size: 30)], focus: 8, interaction: 9)
        var b = a
        XCTAssertEqual(a, b)
        b.measurements[0].epoch = 8
        XCTAssertNotEqual(a, b)
        b = a; b.cross = 79
        XCTAssertNotEqual(a, b)
        b = a; b.focus = nil
        XCTAssertNotEqual(a, b)
        b = a; b.interaction = nil
        XCTAssertNotEqual(a, b)
    }

    func testMalformedSnapshotsDoNotBecomeGeometry() {
        let row: [String: Any] = ["view": 3, "root": 4, "index": 0, "start": 0, "size": 40, "epoch": "18446744073709551614", "measured": false]
        let value: [String: Any] = ["view": 2, "revision": 5, "scrollSequence": 0, "count": 100,
            "totalExtent": 4000, "rows": [row], "correction": NSNull()]
        XCTAssertEqual(CollectionSnapshot(value)?.rows.first?.epoch, UInt64.max - 1)
        var broken = value; broken["totalExtent"] = Double.nan
        XCTAssertNil(CollectionSnapshot(broken))
        broken = value; broken["rows"] = [row, row]
        XCTAssertNil(CollectionSnapshot(broken))
        // A `scrollIntoView` into the padding before the first row is a
        // negative correction (LLP 1010 §6.9); a non-finite one is not.
        var padded = value; padded["correction"] = ["scrollSequence": 1, "offset": -92]
        XCTAssertEqual(CollectionSnapshot(padded)?.correction?.offset, -92)
        padded["correction"] = ["scrollSequence": 1, "offset": -Double.infinity]
        XCTAssertNil(CollectionSnapshot(padded))
    }


    func testDecimalJSONCountersRetainAllSixtyFourBits() throws {
        let bytes = Data(#"{"view":2,"revision":"18446744073709551614","scrollSequence":"18446744073709551613","totalExtent":4000,"rows":[],"correction":{"scrollSequence":"18446744073709551613","offset":90}}"#.utf8)
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

    /// A rescue is never refused for the turn's passes, only while a pass
    /// runs (a report's batch reentering); each refusal is counted by why.
    func testARescueIsNotRefusedForTheTurnsPasses() {
        var budget = CollectionTurnBudget()
        XCTAssertTrue(budget.begin()); budget.end()
        XCTAssertTrue(budget.begin()); budget.end()
        XCTAssertFalse(budget.begin())
        XCTAssertEqual(budget.refusedSpent, 1)
        XCTAssertTrue(budget.begin(rescue: true))
        XCTAssertFalse(budget.begin(rescue: true), "not inside a running pass")
        XCTAssertEqual(budget.refusedBusy, 1)
        budget.end()
        XCTAssertEqual(budget.refusedSpent, 1)
    }
}

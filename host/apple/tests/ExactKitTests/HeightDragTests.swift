import XCTest
@testable import ExactKit
@testable import ExactDrag

final class HeightDragTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactDrag.install() } // LLP 1047.001 D4
    func testBindingPreservesGenerationalKeysAndDoesNotResolveNames() {
        let binding = HeightDragBinding([
            "id": 7, "target": 3,
            "handleKey": "18446744073709551615",
            "targetKey": "9007199254740993"
        ])
        XCTAssertEqual(binding?.id, 7)
        XCTAssertEqual(binding?.target, 3)
        XCTAssertEqual(binding?.handleKey, UInt64.max)
        XCTAssertEqual(binding?.targetKey, 9007199254740993)
        XCTAssertNil(HeightDragBinding([
            "id": 7, "target": 3, "handleKey": 42, "targetKey": "43"
        ]))
    }

    func testBindingRetirementIsExplicitAndPartialTargetPairIsRefused() {
        let retired = HeightDragBinding([
            "id": 7, "target": NSNull(), "handleKey": "42", "targetKey": NSNull()
        ])
        XCTAssertNotNil(retired)
        XCTAssertNil(retired?.target)
        XCTAssertNil(retired?.targetKey)
        XCTAssertNil(HeightDragBinding([
            "id": 7, "target": 3, "handleKey": "42", "targetKey": NSNull()
        ]))
        XCTAssertNil(HeightDragBinding([
            "id": 7, "target": NSNull(), "handleKey": "42", "targetKey": "43"
        ]))
    }

    func testCaughtHeightUsesOriginAndUpwardDragGrowsWithoutAuthoredSnapping() {
        let mapping = HeightDragPosition(base: 213.125)!
        XCTAssertEqual(mapping.value(downward: 0), 213.125)
        XCTAssertEqual(mapping.value(downward: -20), 233.125)
        XCTAssertEqual(mapping.value(downward: 20), 193.125)
        XCTAssertEqual(mapping.value(downward: 300), 0)
        XCTAssertEqual(mapping.value(downward: 210), 3.125)
    }

    func testPositionAdmissionRejectsMalformedAndOverflow() {
        XCTAssertNil(HeightDragPosition(base: -1))
        XCTAssertNil(HeightDragPosition(base: .nan))
        XCTAssertNil(HeightDragPosition(base: .infinity))
        XCTAssertNil(HeightDragPosition(base: Double.greatestFiniteMagnitude))
        let mapping = HeightDragPosition(base: 0)!
        XCTAssertNil(mapping.value(downward: .nan))
        XCTAssertNil(mapping.value(downward: -.infinity))
        XCTAssertNil(mapping.value(downward: -Double.greatestFiniteMagnitude))
        XCTAssertEqual(mapping.value(downward: -Double(Float.greatestFiniteMagnitude)), Double(Float.greatestFiniteMagnitude))
    }
}

import XCTest
@testable import ExactKit

final class HeightDragTests: XCTestCase {
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

extension HeightDragTests {
    func testReleaseKeepsRecentDisplayedVelocityButAStationaryPauseExpiresIt() {
        var samples = HeightDragVelocity(height: 180, time: 0)
        XCTAssertEqual(samples.record(height: 200, time: 0.02), true)
        XCTAssertEqual(samples.velocity, 1000)
        XCTAssertEqual(samples.record(height: 200, time: 0.03, ending: true), true)
        XCTAssertEqual(samples.velocity, 1000)
        XCTAssertEqual(samples.record(height: 200, time: 0.2, ending: true), true)
        XCTAssertEqual(samples.velocity, 0)
    }
    func testVelocityUsesConstrainedHeightsAndRejectsBadOrBackwardsSamples() {
        var samples = HeightDragVelocity(height: 240, time: 0)
        XCTAssertEqual(samples.record(height: 240, time: 0.02), true)
        XCTAssertEqual(samples.velocity, 0) // pointer may move past max-height
        XCTAssertEqual(samples.record(height: 220, time: 0.04), true)
        XCTAssertEqual(samples.velocity, -1000)
        XCTAssertEqual(samples.record(height: .nan, time: 0.05), false)
        XCTAssertEqual(samples.record(height: 180, time: 0.01), false)
        XCTAssertEqual(samples.height, 220)
        XCTAssertEqual(samples.time, 0.04)
        XCTAssertEqual(samples.velocity, -1000)
    }
}

extension HeightDragTests {
    func testCoalescedDragHasDirectionWithoutInstantaneousVelocity() {
        XCTAssertTrue(HeightDragDirection.accepts(velocityX: 0, velocityY: 0, translationX: 0, translationY: -175.927875))
        XCTAssertTrue(HeightDragDirection.accepts(velocityX: 0, velocityY: 0, translationX: 1, translationY: 176))
        XCTAssertFalse(HeightDragDirection.accepts(velocityX: 0, velocityY: 0, translationX: 176, translationY: 1))
        XCTAssertFalse(HeightDragDirection.accepts(velocityX: 0, velocityY: 0, translationX: 0, translationY: 0))
        XCTAssertFalse(HeightDragDirection.accepts(velocityX: 100, velocityY: 1, translationX: 0, translationY: 176))
        XCTAssertFalse(HeightDragDirection.accepts(velocityX: .nan, velocityY: 0, translationX: 0, translationY: 176))
        XCTAssertFalse(HeightDragDirection.accepts(velocityX: 0, velocityY: 0, translationX: 0, translationY: .infinity))
    }
}

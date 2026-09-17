import XCTest
@testable import ExactKit

final class GestureHoldTests: XCTestCase {
    func testImmediateReverseCanCatchDisplacedRowButVerticalMotionWins() {
        XCTAssertTrue(SwipeRecognition.accepts(x: -12, y: 1, presentedX: 87.2))
        XCTAssertFalse(SwipeRecognition.accepts(x: -12, y: 1, presentedX: 0))
        XCTAssertFalse(SwipeRecognition.accepts(x: -12, y: 20, presentedX: 87.2))
        XCTAssertTrue(SwipeRecognition.accepts(x: 12, y: 1, presentedX: 0))
    }
    func testCatchBeyondResistanceKeepsExactOriginAndReverses() {
        let catchValue = 91.125
        let catchMap = SwipeDisplacement(base: catchValue)
        XCTAssertEqual(catchMap.value(0), catchValue)
        XCTAssertEqual(catchMap.value(10), catchValue + 2, accuracy: 1e-10)
        XCTAssertEqual(catchMap.value(-10), catchValue - 2, accuracy: 1e-10)
        XCTAssertLessThan(catchMap.value(-300), 0)
        XCTAssertEqual(catchMap.velocity(displacement: 0, fingerVelocity: -100), -20)
    }
    func testCaughtCompanionAlsoKeepsItsPresentationAtZeroDisplacement() {
        let indicator = SwipeIndicator(base: 0.8, progressAtCatch: 1)
        XCTAssertEqual(indicator.value(1), 0.8)
        XCTAssertEqual(indicator.value(0.5), 0.4)
        let partial = SwipeIndicator(base: 0.25, progressAtCatch: 0.5)
        XCTAssertEqual(partial.value(0.5), 0.25)
        XCTAssertEqual(partial.value(1), 1)
    }
    func testNativeTokenPreservesEveryBit() {
        let hold = NativeHold(["token": "18446744073709551615", "x": 91.0, "y": 2.0])
        XCTAssertEqual(hold?.token, UInt64.max)
        XCTAssertNil(NativeHold(["token": 1, "x": 0.0, "y": 0.0]))
    }
    func testFreshDragAndVelocityUseSameDisplayedDerivative() {
        let map = SwipeDisplacement(base: 0)
        XCTAssertEqual(map.value(64), 64)
        XCTAssertEqual(map.value(100), 71.2, accuracy: 1e-10)
        XCTAssertEqual(map.value(-100), -71.2, accuracy: 1e-10)
        XCTAssertEqual(map.velocity(displacement: 100, fingerVelocity: 500), 100)
        XCTAssertEqual(map.velocity(displacement: 20, fingerVelocity: -500), -500)
    }
}

#if os(macOS)
import AppKit
extension GestureHoldTests {
    func testInputEligibilityIncludesAncestorsAndAttachment() {
        _ = NSApplication.shared
        let presenter = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.viewport
        defer { window.close() }
        let parent = NodeView(id: 1, kind: "view", presenter: presenter)
        let row = NodeView(id: 2, kind: "view", presenter: presenter)
        presenter.root.addSubview(parent); parent.addSubview(row)
        XCTAssertTrue(SwipeInput.allows(row))
        row.props["disabled"] = "true"
        XCTAssertFalse(SwipeInput.allows(row))
        row.props.removeValue(forKey: "disabled")
        parent.props["disabled"] = "true"
        XCTAssertFalse(SwipeInput.allows(row))
        parent.props.removeValue(forKey: "disabled")
        parent.props["inert"] = "true"
        XCTAssertFalse(SwipeInput.allows(row))
        parent.props.removeValue(forKey: "inert")
        parent.routeInert = true
        XCTAssertFalse(SwipeInput.allows(row))
        parent.routeInert = false
        parent.isHidden = true
        XCTAssertFalse(SwipeInput.allows(row))
        parent.isHidden = false
        XCTAssertTrue(SwipeInput.allows(row))
        parent.removeFromSuperview()
        XCTAssertFalse(SwipeInput.allows(row))
    }
}
#endif

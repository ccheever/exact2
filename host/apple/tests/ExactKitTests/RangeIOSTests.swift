#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// `input type="range"` is a UISlider (LLP 1069.001 D5), whose thumb moves
/// from step to step as a browser's does.
///   bun host/apple/build.mjs --test --ios
final class RangeIOSTests: XCTestCase {
    private var window: UIWindow!

    private func slider(_ props: [String: String]) throws -> (Presenter, UISlider) {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "control", "props": ["type": "range"].merging(props) { $1 },
             "handlers": ["input", "change"], "style": [:]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 28.0],
        ]))
        return (p, try XCTUnwrap(p.controls.controls[1] as? UISlider))
    }

    /// iOS 26's ticks are the steps, and UIKit puts the thumb on one; untitled, none is drawn.
    func testEveryStepIsATickUIKitMovesTheThumbTo() throws {
        guard #available(iOS 26.0, *) else { throw XCTSkip("UISlider has ticks from iOS 26") }
        let (_, s) = try slider(["min": "65", "max": "85", "step": "1", "value": "72"])
        XCTAssertEqual(s.value, 72)
        XCTAssertEqual(s.trackConfiguration?.ticks.count, 21)
        XCTAssertEqual(s.trackConfiguration?.allowsTickValuesOnly, true)
        XCTAssertTrue(s.trackConfiguration?.ticks.allSatisfy { $0.title == nil && $0.image == nil } ?? false)
        s.value = 73.4
        XCTAssertEqual(s.value, 73, "on the nearest step")
    }

    /// A span that is no multiple of the step ends at the last step under `max`, as HTML's does.
    func testAStepThatDoesNotDivideTheSpanStopsBelowMax() throws {
        guard #available(iOS 26.0, *) else { throw XCTSkip("UISlider has ticks from iOS 26") }
        let (_, s) = try slider(["min": "0", "max": "10", "step": "3", "value": "6"])
        XCTAssertEqual(s.trackConfiguration?.ticks.map(\.position) ?? [], [0, 0.3, 0.6, 0.9].map { Float($0) }, accuracy: 1e-6)
        s.value = 9.9
        XCTAssertEqual(s.value, 9, accuracy: 1e-5)
    }

    /// `step="any"` glides; a thousand-odd steps are a continuous control
    /// whose thumb is snapped as it moves; a changed step makes its ticks again.
    func testAnyStepGlidesAndTooManyStepsSnapAsTheThumbMoves() throws {
        let (p, s) = try slider(["min": "0", "max": "100", "step": "any", "value": "40"])
        if #available(iOS 26.0, *) { XCTAssertNil(s.trackConfiguration) }
        s.value = 40.25
        p.controls.rangeMoved(s) // what UIKit sends as the thumb moves (a test bundle routes no target-actions)
        XCTAssertEqual(s.value, 40.25)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["min": "0", "max": "5000", "step": "1"]]]))
        if #available(iOS 26.0, *) { XCTAssertNil(s.trackConfiguration, "5001 values are no steps") }
        s.value = 73.6
        p.controls.rangeMoved(s)
        XCTAssertEqual(s.value, 74, "snapped as it moves")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["min": "0", "max": "10", "step": "5"]]]))
        if #available(iOS 26.0, *) { XCTAssertEqual(s.trackConfiguration?.ticks.count, 3) }
        // Back to gliding: no ticks, and a value set is the value shown
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["step": "any"]]]))
        if #available(iOS 26.0, *) { XCTAssertEqual(s.trackConfiguration?.ticks.count ?? 0, 0) }
        s.value = 2.5
        XCTAssertEqual(s.value, 2.5)
    }

    func testTinyStepsDoNotOverflowTheNativeTickCount() throws {
        let (p, s) = try slider(["min": "0", "max": "100", "step": "1e-300", "value": "40"])
        if #available(iOS 26.0, *) { XCTAssertNil(s.trackConfiguration) }
        XCTAssertEqual(s.value, 40)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["step": "1e-20"]]]))
        if #available(iOS 26.0, *) { XCTAssertNil(s.trackConfiguration) }
        XCTAssertEqual(s.value, 40)
    }
}

private func XCTAssertEqual(_ a: [Float], _ b: [Float], accuracy: Float, file: StaticString = #filePath, line: UInt = #line) {
    XCTAssertEqual(a.count, b.count, file: file, line: line)
    for (x, y) in zip(a, b) { XCTAssertEqual(x, y, accuracy: accuracy, file: file, line: line) }
}
#endif

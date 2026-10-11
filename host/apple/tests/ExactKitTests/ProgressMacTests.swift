#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1069.001, amended 2026-10-07, on AppKit: an indeterminate `progress`
/// is a spinning `NSProgressIndicator`, small in the default 20 × 20 box and
/// regular from 32 points, centred, named by `aria-label`; it turns while it
/// shows and stops when hidden or removed (the agent's clock, which holds
/// it, needs a session these presenters lack).
final class ProgressMacTests: XCTestCase {
    private var window: NSWindow!

    override func tearDown() { window?.close(); window = nil }

    private func presenter(_ ops: [[String: Any]]) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch(ops))
        return p
    }
    private func progress(_ id: Int, _ props: [String: String] = [:], side: Double = 20, y: Double = 0) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "control",
          "props": ["type": "progress", "accessibilityRole": "progressbar", "accessibilityBusy": "true"].merging(props) { $1 },
          "handlers": [], "style": ["text_color": [255, 0, 0, 255]]],
         ["op": "frame", "id": id, "x": 0.0, "y": y, "w": side, "h": side]]
    }
    private func box(_ id: Int) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "view", "props": [:], "handlers": [], "style": ["text_color": [0, 0, 0, 255]]],
         ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0]]
    }

    func testItIsAppKitsSpinnerSizedByItsBox() throws {
        let p = presenter(box(1) + progress(2, ["testId": "busy", "accessibilityLabel": "Loading"]) + progress(3, side: 40, y: 40)
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]])
        let small = try XCTUnwrap(p.controls.spinners[2])
        XCTAssertEqual(small.style, .spinning)
        XCTAssertTrue(small.isIndeterminate)
        XCTAssertEqual(small.controlSize, .small)
        XCTAssertEqual(small.frame, NSRect(x: 2, y: 2, width: 16, height: 16), "centred in its box")
        XCTAssertEqual(small.accessibilityLabel(), "Loading")
        XCTAssertEqual(small.accessibilityIdentifier(), "busy")
        XCTAssertEqual(try XCTUnwrap(p.controls.spinners[3]).controlSize, .regular)
        XCTAssertNil(p.controls.controls[2], "a spinner is not one of the form controls")
        let seen = try XCTUnwrap(p.controls.observation(try XCTUnwrap(p.views[2])))
        XCTAssertEqual(seen["view"] as? String, "NSProgressIndicator")
        if !ExactEnv.agentFreezes { XCTAssertEqual(seen["animating"] as? Bool, true) }
        // A click passes through the spinner to its node, and on to an ancestor's press.
        XCTAssertNil(small.hitTest(NSPoint(x: 8, y: 8)))
        XCTAssertTrue(p.views[1]?.hitTest(NSPoint(x: 10, y: 10)) === p.views[2])
        p.apply(wireBatch([["op": "props", "id": 2, "clear": ["accessibilityLabel"]]]))
        XCTAssertNotEqual(small.accessibilityLabel(), "Loading", "a cleared label is cleared")
    }

    /// LLP 1116 D8: with a `value`, a `progress` is AppKit's determinate bar
    /// across its content box, small in HTML's 16-point box and regular from
    /// 20, from 0 to `max` (1 unsaid), named by `aria-label`, its words
    /// `aria-valuetext`; a click passes through it. Its value taken away, it
    /// is the spinner again.
    func testAValuedProgressIsAppKitsBar() throws {
        func bar(_ id: Int, _ props: [String: String], h: Double, y: Double) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "control",
              "props": ["type": "progress", "accessibilityRole": "progressbar"].merging(props) { $1 }, "handlers": [], "style": [:]],
             ["op": "frame", "id": id, "x": 0.0, "y": y, "w": 160.0, "h": h]]
        }
        let p = presenter(box(1) + bar(2, ["value": "3", "max": "8", "testId": "water", "accessibilityLabel": "Water"], h: 16, y: 0)
                          + bar(3, ["value": "0.5"], h: 24, y: 40) + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]])
        let water = try XCTUnwrap(p.controls.bars[2])
        XCTAssertEqual(water.style, .bar)
        XCTAssertFalse(water.isIndeterminate)
        XCTAssertEqual(water.controlSize, .small)
        XCTAssertEqual([water.minValue, water.doubleValue, water.maxValue], [0, 3, 8])
        XCTAssertEqual(water.frame, NSRect(x: 0, y: 2, width: 160, height: 12), "across its box, centred")
        XCTAssertNil(p.controls.spinners[2])
        XCTAssertEqual(water.accessibilityLabel(), "Water")
        XCTAssertEqual(water.accessibilityIdentifier(), "water")
        XCTAssertEqual(water.accessibilityRole(), .progressIndicator)
        let tall = try XCTUnwrap(p.controls.bars[3])
        XCTAssertEqual(tall.controlSize, .regular)
        XCTAssertEqual([tall.doubleValue, tall.maxValue], [0.5, 1])
        XCTAssertNil(water.hitTest(NSPoint(x: 8, y: 8)))
        let seen = try XCTUnwrap(p.controls.observation(try XCTUnwrap(p.views[2])))
        XCTAssertEqual(seen["style"] as? String, "bar")
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["value": "9", "accessibilityValueText": "8 glasses"]]]))
        XCTAssertEqual(water.doubleValue, 8, "past max: max")
        XCTAssertEqual(water.accessibilityValueDescription(), "8 glasses")
        p.apply(wireBatch([["op": "props", "id": 2, "clear": ["value"]]]))
        XCTAssertNil(p.controls.bars[2])
        XCTAssertNil(water.superview)
        XCTAssertNotNil(p.controls.spinners[2], "no value: the spinner")
    }

    func testItStopsWhenHiddenOrRemoved() throws {
        try XCTSkipIf(ExactEnv.agentFreezes, "the agent's frozen clock holds every spinner")
        let p = presenter(box(1) + progress(2) + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        XCTAssertTrue(p.controls.animating.contains(2))
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["display": "none", "text_color": [0, 0, 0, 255]]]]))
        XCTAssertFalse(p.controls.animating.contains(2), "under a display: none box it stops")
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["text_color": [0, 0, 0, 255]]]]))
        XCTAssertTrue(p.controls.animating.contains(2), "shown again, it turns")
        p.apply(wireBatch([["op": "destroy", "id": 2], ["op": "children", "id": 1, "ids": []]]))
        XCTAssertNil(p.controls.spinners[2], "removed with its node")
        XCTAssertTrue(p.controls.animating.isEmpty)
    }
}
#endif

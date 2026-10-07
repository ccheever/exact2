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

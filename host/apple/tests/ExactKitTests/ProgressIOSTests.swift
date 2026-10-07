#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1069.001, amended 2026-10-07, on UIKit: an indeterminate `progress`
/// is a `UIActivityIndicatorView`, `.medium` in the default 20 × 20 box and
/// `.large` from 37 points, centred, in the node's `color`, named by
/// `aria-label`; it turns while it shows and stops when hidden or removed
/// (the agent's clock, which holds it, needs a session these presenters
/// lack). UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class ProgressIOSTests: XCTestCase {
    private var window: UIWindow!

    private func presenter(_ ops: [[String: Any]]) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
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

    func testItIsUIKitsActivityIndicatorSizedByItsBox() throws {
        let p = presenter(box(1) + progress(2, ["testId": "busy", "accessibilityLabel": "Loading"]) + progress(3, side: 40, y: 40)
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]])
        let medium = try XCTUnwrap(p.controls.spinners[2])
        XCTAssertEqual(medium.style, .medium)
        XCTAssertEqual(medium.frame.size, medium.intrinsicContentSize)
        XCTAssertEqual(medium.center, CGPoint(x: 10, y: 10), "centred in its box")
        XCTAssertFalse(medium.hidesWhenStopped, "stopped, it shows its frame")
        XCTAssertEqual(medium.accessibilityLabel, "Loading")
        XCTAssertEqual(medium.accessibilityIdentifier, "busy")
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        medium.color.getRed(&r, green: &g, blue: &b, alpha: &a)
        XCTAssertEqual([r, g, b, a], [1, 0, 0, 1], "its node's color")
        XCTAssertEqual(try XCTUnwrap(p.controls.spinners[3]).style, .large)
        XCTAssertNil(p.controls.controls[2], "an indicator is not one of the form controls")
        let seen = try XCTUnwrap(p.controls.observation(try XCTUnwrap(p.views[2])))
        XCTAssertEqual(seen["view"] as? String, "UIActivityIndicatorView")
        XCTAssertEqual(seen["style"] as? String, "medium")
        if !ExactEnv.agentFreezes { XCTAssertTrue(medium.isAnimating) }
    }

    func testItStopsWhenHiddenOrRemoved() throws {
        try XCTSkipIf(ExactEnv.agentFreezes, "the agent's frozen clock holds every indicator")
        let p = presenter(box(1) + progress(2) + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        let spinner = try XCTUnwrap(p.controls.spinners[2])
        XCTAssertTrue(spinner.isAnimating)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["display": "none", "text_color": [0, 0, 0, 255]]]]))
        XCTAssertFalse(spinner.isAnimating, "under a display: none box it stops")
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["text_color": [0, 0, 0, 255]]]]))
        XCTAssertTrue(spinner.isAnimating, "shown again, it turns")
        p.apply(wireBatch([["op": "destroy", "id": 2], ["op": "children", "id": 1, "ids": []]]))
        XCTAssertNil(p.controls.spinners[2], "removed with its node")
        XCTAssertNil(spinner.superview)
    }
}
#endif

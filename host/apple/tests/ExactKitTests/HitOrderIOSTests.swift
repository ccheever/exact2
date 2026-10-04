#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A touch reaches what paints on top (CSS): a raised `z-index` sibling (a
/// positioned box, where `z-index` applies)
/// before a later one that covers it, then later siblings before earlier.
/// UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class HitOrderIOSTests: XCTestCase {
    private var window: UIWindow!

    private func presenter(raised: Double) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        // A screen (1): a button (2) raised over its top-right corner, then a
        // list (3) after it in the tree that fills the whole screen.
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "handlers": ["press"], "props": ["testId": "menu"],
             "style": ["position_type": raised == 0 ? "static" : "absolute", "z_index": raised, "text_color": [0, 0, 0, 255]]],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["press"], "props": ["testId": "list"]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "rank", "id": 2, "rank": raised * 2],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 340.0, "y": 0.0, "w": 44.0, "h": 44.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
        ]))
        window.layoutIfNeeded()
        return p
    }

    private func hit(_ p: Presenter, _ point: CGPoint) -> UIView? {
        window.hitTest(point, with: nil)
    }

    func testARaisedSiblingTakesTheTouchFromALaterOneOverIt() throws {
        let p = presenter(raised: 2)
        let menu = try XCTUnwrap(p.views[2]), list = try XCTUnwrap(p.views[3])
        XCTAssertTrue(hit(p, CGPoint(x: 360, y: 20)) === menu, "the raised button")
        XCTAssertTrue(hit(p, CGPoint(x: 100, y: 200)) === list, "the list, where nothing is raised over it")
    }

    func testAPointerEventsNoneBoxLetsTheTouchThrough() throws {
        let p = presenter(raised: 0)
        // A header backdrop (4) raised over the list, which never takes a touch.
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "style": ["position_type": "absolute", "z_index": 3.0, "pointer_events": "none", "text_color": [0, 0, 0, 255]]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "rank", "id": 4, "rank": 6],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 400.0, "h": 120.0],
        ]))
        window.layoutIfNeeded()
        let list = try XCTUnwrap(p.views[3])
        XCTAssertTrue(hit(p, CGPoint(x: 100, y: 60)) === list, "through the backdrop to the list")
    }

    /// A native module's view inside a `pointer-events: none` box is not a
    /// target either: the touch reaches the button around it (paint F9).
    func testAPointerEventsNoneBoxsPlatformViewLetsTheTouchThrough() throws {
        let p = presenter(raised: 0)
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "style": ["pointer_events": "none"]],
            ["op": "children", "id": 2, "ids": [4]],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 44.0, "h": 44.0],
        ]))
        let thumbnail = try XCTUnwrap(p.views[4])
        thumbnail.addSubview(UIView(frame: thumbnail.bounds)) // the module's view
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [3, 2]]]))
        window.layoutIfNeeded()
        XCTAssertTrue(hit(p, CGPoint(x: 360, y: 20)) === p.views[2], "the button around the native view")
    }

    func testInFlowTheLaterSiblingIsOnTop() throws {
        let p = presenter(raised: 0)
        let list = try XCTUnwrap(p.views[3])
        XCTAssertTrue(hit(p, CGPoint(x: 360, y: 20)) === list, "tree order, as the web paints it")
    }
}
#endif

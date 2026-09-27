#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// The presence ops on UIKit (LLP 1063): `exit` keeps a leaving view where
/// it was, above its old siblings, inert and out of every lookup, while the
/// engine's values still reach it, until the host's `destroy` ends it; a
/// `layout` present places the box outermost, from its top-left corner.
///   bun host/apple/build.mjs --test --ios
final class PresenceIOSTests: XCTestCase {
    private var window: UIWindow!

    /// 1 → [2 → [3], 4], each row 50 points tall.
    private func fixture() -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "create", "id": 4, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2, 4]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 50.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 50.0, "w": 400.0, "h": 50.0],
        ]))
        return p
    }

    func testAnExitKeepsTheViewWhereItWasInertAndUnnamedUntilItsDestroy() throws {
        let p = fixture()
        let (parent, leaving, inner, sibling) = try (XCTUnwrap(p.views[1]), XCTUnwrap(p.views[2]), XCTUnwrap(p.views[3]), XCTUnwrap(p.views[4]))
        // The host's batch: the exit first, then the tree without it; its
        // destroy is withheld until the engine passes the exit's end.
        p.apply(wireBatch([
            ["op": "exit", "id": 2],
            ["op": "children", "id": 1, "ids": [4]],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 400.0, "h": 50.0],
        ]))
        XCTAssertNil(p.views[2], "out of every lookup by id")
        XCTAssertNil(p.views[3], "its subtree too")
        XCTAssertTrue(leaving.superview === parent, "still in the window")
        XCTAssertTrue(inner.superview === leaving)
        XCTAssertTrue(parent.subviews.last === leaving, "above its old siblings")
        XCTAssertEqual(leaving.frame, CGRect(x: 0, y: 0, width: 400, height: 50), "its last box")
        XCTAssertEqual(sibling.frame.minY, 0, "its sibling takes its place")
        XCTAssertFalse(leaving.isUserInteractionEnabled)
        XCTAssertTrue(leaving.accessibilityElementsHidden)
        p.apply(wireBatch([["op": "present", "id": 2, "property": "opacity", "x": 0.25, "y": 0.0]]))
        XCTAssertEqual(leaving.alpha, 0.25, accuracy: 1e-6, "the engine animates it")
        p.apply(wireBatch([["op": "destroy", "id": 2]]))
        XCTAssertNil(leaving.superview, "the exit ended")
        XCTAssertNil(inner.superview?.superview)
        XCTAssertTrue(p.views[4] === sibling)
    }

    func testALayoutPresentPlacesTheBoxFromItsTopLeftOutsideItsOwnTransforms() throws {
        let p = fixture()
        let v = try XCTUnwrap(p.views[3])
        // Grown from 25 to 50 high and moved down 10: it starts where it was.
        p.apply(wireBatch([["op": "present", "id": 3, "property": "layout", "x": 4.0, "y": 10.0, "w": 1.0, "h": 0.5]]))
        let corner = { (x: CGFloat, y: CGFloat) -> CGPoint in
            // About the center, as UIKit applies a transform.
            let c = CGPoint(x: 50, y: 25)
            let q = CGPoint(x: x - c.x, y: y - c.y).applying(v.transform)
            return CGPoint(x: q.x + c.x, y: q.y + c.y)
        }
        XCTAssertEqual(corner(0, 0).x, 4, accuracy: 1e-9)
        XCTAssertEqual(corner(0, 0).y, 10, accuracy: 1e-9)
        XCTAssertEqual(corner(100, 50).x, 104, accuracy: 1e-9)
        XCTAssertEqual(corner(100, 50).y, 35, accuracy: 1e-9)
        // An authored scale stays about the center, inside the layout box.
        p.apply(wireBatch([["op": "present", "id": 3, "property": "scale", "x": 0.5, "y": 0.0]]))
        XCTAssertEqual(corner(0, 0).x, 29, accuracy: 1e-9)
        XCTAssertEqual(corner(0, 0).y, 16.25, accuracy: 1e-9)
        p.apply(wireBatch([
            ["op": "present", "id": 3, "property": "scale", "x": 1.0, "y": 0.0],
            ["op": "present", "id": 3, "property": "layout", "x": 0.0, "y": 0.0, "w": 1.0, "h": 1.0],
        ]))
        XCTAssertTrue(v.transform.isIdentity)
    }
}
#endif

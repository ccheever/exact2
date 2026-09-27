#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// The presence ops on AppKit (LLP 1063): `exit` keeps a leaving view where
/// it was, above its old siblings, inert and out of every lookup, while the
/// engine's values still reach it, until the host's `destroy` ends it; a
/// `layout` present places the box outermost, from its top-left corner.
final class PresenceMacTests: XCTestCase {
    /// 1 → [2 → [3], 4], each row 50 points tall.
    private func fixture() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
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
        XCTAssertEqual(leaving.frame, NSRect(x: 0, y: 0, width: 400, height: 50), "its last box")
        XCTAssertEqual(sibling.frame.minY, 0, "its sibling takes its place")
        XCTAssertTrue(leaving.routeInert)
        XCTAssertTrue(leaving.isAccessibilityHidden())
        p.apply(wireBatch([["op": "present", "id": 2, "property": "opacity", "x": 0.25, "y": 0.0]]))
        XCTAssertEqual(leaving.alphaValue, 0.25, accuracy: 1e-6, "the engine animates it")
        p.apply(wireBatch([["op": "destroy", "id": 2]]))
        XCTAssertNil(leaving.superview, "the exit ended")
        XCTAssertTrue(p.views[4] === sibling)
    }

    func testALayoutPresentPlacesTheBoxFromItsTopLeftOutsideItsOwnTransforms() throws {
        let p = fixture()
        let v = try XCTUnwrap(p.views[3])
        // Grown from 25 to 50 high and moved down 10: it starts where it was.
        p.apply(wireBatch([["op": "present", "id": 3, "property": "layout", "x": 4.0, "y": 10.0, "w": 1.0, "h": 0.5]]))
        // The layer's transform is about its origin, the flipped view's top-left.
        let corner = { (x: CGFloat, y: CGFloat) in CGPoint(x: x, y: y).applying(v.layer!.affineTransform()) }
        XCTAssertEqual(corner(0, 0).x, 4, accuracy: 1e-9)
        XCTAssertEqual(corner(0, 0).y, 10, accuracy: 1e-9)
        XCTAssertEqual(corner(100, 50).x, 104, accuracy: 1e-9)
        XCTAssertEqual(corner(100, 50).y, 35, accuracy: 1e-9)
        p.apply(wireBatch([["op": "present", "id": 3, "property": "scale", "x": 0.5, "y": 0.0]]))
        XCTAssertEqual(corner(0, 0).x, 29, accuracy: 1e-9)
        XCTAssertEqual(corner(0, 0).y, 16.25, accuracy: 1e-9)
        p.apply(wireBatch([
            ["op": "present", "id": 3, "property": "scale", "x": 1.0, "y": 0.0],
            ["op": "present", "id": 3, "property": "layout", "x": 0.0, "y": 0.0, "w": 1.0, "h": 1.0],
        ]))
        XCTAssertTrue(v.layer!.affineTransform().isIdentity)
    }
}
#endif

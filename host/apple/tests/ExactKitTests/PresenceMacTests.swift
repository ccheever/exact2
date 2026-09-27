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

    func testResetEndsExitsBeforeAnIDIsReused() throws {
        let p = fixture()
        let old = try XCTUnwrap(p.views[2])
        let child = try XCTUnwrap(p.views[3])
        p.apply(wireBatch([["op": "exit", "id": 2]]))
        p.reset()
        XCTAssertTrue(p.leaving.isEmpty)
        XCTAssertNil(old.superview)
        XCTAssertNil(child.superview?.superview)
        p.apply(wireBatch([
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "roots", "ids": [2]],
        ]))
        let fresh = try XCTUnwrap(p.views[2])
        p.apply(wireBatch([["op": "destroy", "id": 2]]))
        XCTAssertNil(p.views[2])
        XCTAssertNil(fresh.superview)
    }

    func testALayoutSpringCannotGiveTheSurfaceANegativeSize() throws {
        let p = fixture()
        let v = try XCTUnwrap(p.views[3])
        p.apply(wireBatch([["op": "present", "id": 3, "property": "layout", "x": 4.0, "y": 10.0, "w": -0.5, "h": -2.0]]))
        let surface = try XCTUnwrap(v.surface)
        XCTAssertEqual(surface.bounds.size, .zero)
        XCTAssertEqual(surface.frame.origin, .zero)
        XCTAssertEqual(v.bounds.size, CGSize(width: 100, height: 50))
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

    func testALayoutPresentMovesTheBoxAndSizesOnlyItsSurface() throws {
        let p = fixture()
        let v = try XCTUnwrap(p.views[3])
        let clips = { v.clipsToBounds }
        let blue = [0, 0, 255, 255]
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["background_color": blue, "border_radius_top_left": 6, "border_radius_top_right": 6, "border_radius_bottom_left": 6, "border_radius_bottom_right": 6, "overflow_x": "hidden", "overflow_y": "hidden"]]]))
        XCTAssertTrue(clips())
        // Grown from 25 to 50 high and moved down 10: it starts where it was.
        p.apply(wireBatch([["op": "present", "id": 3, "property": "layout", "x": 4.0, "y": 10.0, "w": 1.0, "h": 0.5]]))
        // The layer's transform is about its origin, the flipped view's top-left.
        let corner = { (x: CGFloat, y: CGFloat) in CGPoint(x: x, y: y).applying(v.layer!.affineTransform()) }
        let mask = { v.layer?.mask }
        // Moved, never scaled: its content keeps its laid-out size.
        XCTAssertEqual(corner(0, 0).x, 4, accuracy: 1e-9)
        XCTAssertEqual(corner(0, 0).y, 10, accuracy: 1e-9)
        XCTAssertEqual(corner(100, 50).x, 104, accuracy: 1e-9)
        XCTAssertEqual(corner(100, 50).y, 60, accuracy: 1e-9)
        XCTAssertEqual(v.bounds.size, CGSize(width: 100, height: 50), "laid out at its final size")
        // Its surface is the shown size, from its top-left corner, and its
        // own is off; its children are clipped to the shown box.
        let surface = try XCTUnwrap(v.surface)
        XCTAssertEqual(surface.frame, CGRect(x: 0, y: 0, width: 100, height: 25))
        XCTAssertEqual(surface.backgroundColor?.components?.map { Double($0) }, [0, 0, 1, 1])
        XCTAssertEqual(surface.cornerRadius, 6)
        XCTAssertFalse(clips(), "the mask clips instead: the surface may outgrow the frame")
        XCTAssertTrue(mask() === surface.clip)
        XCTAssertEqual(surface.clip.path?.boundingBox.height ?? 0, 25, accuracy: 1e-9)
        XCTAssertEqual(surface.clip.path?.boundingBox.width ?? 0, 100, accuracy: 1e-9)
        // Shrinking past the frame: the surface is larger than the view.
        p.apply(wireBatch([["op": "present", "id": 3, "property": "layout", "x": 0.0, "y": 0.0, "w": 1.0, "h": 1.5]]))
        XCTAssertEqual(surface.frame.height, 75)
        // At rest the node paints its own surface and clip again.
        p.apply(wireBatch([["op": "present", "id": 3, "property": "layout", "x": 0.0, "y": 0.0, "w": 1.0, "h": 1.0]]))
        XCTAssertNil(v.surface)
        XCTAssertNil(surface.superlayer)
        XCTAssertTrue(clips())
        XCTAssertNil(mask())
        XCTAssertTrue(v.layer!.affineTransform().isIdentity)
    }

    func testASurfaceTheLayerCannotSayIsDrawnUprightAtTheShownSize() throws {
        let p = fixture()
        let v = try XCTUnwrap(p.views[3])
        let (red, blue) = ([255, 0, 0, 255], [0, 0, 255, 255])
        // Sides that differ: drawn, as the node's own `draw(_:)` draws them.
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["border_width": 4, "border_color_top": red, "border_color_right": red, "border_color_bottom": blue, "border_color_left": red]]]))
        p.apply(wireBatch([["op": "present", "id": 3, "property": "layout", "x": 0.0, "y": 0.0, "w": 1.0, "h": 0.5]]))
        let surface = try XCTUnwrap(v.surface)
        XCTAssertNotNil(surface.drawn)
        let (w, h) = (100, 25)
        let ctx = try XCTUnwrap(CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        // Its backing store, drawn as on screen, then composited.
        surface.displayIfNeeded()
        XCTAssertNotNil(surface.contents)
        surface.render(in: ctx)
        let bytes = try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self)
        let pixel = { (x: Int, y: Int) in (0..<4).map { Int(bytes[(y * w + x) * 4 + $0]) } }
        // The first row in memory is the top: red there, blue at the bottom.
        XCTAssertEqual(pixel(50, 1), red)
        XCTAssertEqual(pixel(50, h - 2), blue)
    }
}
#endif

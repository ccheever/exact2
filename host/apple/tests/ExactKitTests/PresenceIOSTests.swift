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

    func testALayoutPresentMovesTheBoxAndSizesOnlyItsSurface() throws {
        let p = fixture()
        let v = try XCTUnwrap(p.views[3])
        let clips = { v.clipsToBounds }
        let blue = [0, 0, 255, 255]
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["background_color": blue, "border_radius_top_left": 6, "border_radius_top_right": 6, "border_radius_bottom_left": 6, "border_radius_bottom_right": 6, "overflow_x": "hidden", "overflow_y": "hidden"]]]))
        XCTAssertTrue(clips())
        // Grown from 25 to 50 high and moved down 10: it starts where it was.
        p.apply(wireBatch([["op": "present", "id": 3, "property": "layout", "x": 4.0, "y": 10.0, "w": 1.0, "h": 0.5]]))
        // About the center, as UIKit applies a transform.
        let corner = { (x: CGFloat, y: CGFloat) -> CGPoint in
            let c = CGPoint(x: 50, y: 25)
            let q = CGPoint(x: x - c.x, y: y - c.y).applying(v.transform)
            return CGPoint(x: q.x + c.x, y: q.y + c.y)
        }
        let own = { v.layer.backgroundColor }
        let mask = { v.layer.mask }
        v.layer.displayIfNeeded()
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
        XCTAssertNil(own())
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
        XCTAssertTrue(v.transform.isIdentity)
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
        // Its backing store, drawn as on screen, then snapshot as UIKit does.
        surface.displayIfNeeded()
        XCTAssertNotNil(surface.contents)
        let format = UIGraphicsImageRendererFormat(); format.scale = 1
        let image = UIGraphicsImageRenderer(size: CGSize(width: w, height: h), format: format).image { surface.render(in: $0.cgContext) }
        let ctx = try XCTUnwrap(CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.draw(try XCTUnwrap(image.cgImage), in: CGRect(x: 0, y: 0, width: w, height: h))
        let bytes = try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self)
        let pixel = { (x: Int, y: Int) in (0..<4).map { Int(bytes[(y * w + x) * 4 + $0]) } }
        // The first row in memory is the top: red there, blue at the bottom.
        XCTAssertEqual(pixel(50, 1), red)
        XCTAssertEqual(pixel(50, h - 2), blue)
    }
}
#endif

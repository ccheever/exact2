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
            // Named, so they are views and not flat leaves (LLP 1068 §6.1).
            ["op": "create", "id": 3, "kind": "view", "props": ["testId": "inner"]],
            ["op": "create", "id": 4, "kind": "view", "props": ["testId": "sibling"]],
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

    private func tabBarFixture() -> Presenter {
        let p = fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "tablist"]],
            ["op": "create", "id": 11, "kind": "button", "props": ["accessibilityRole": "tab"], "handlers": ["press"]],
            ["op": "create", "id": 12, "kind": "button", "props": ["accessibilityRole": "tab"], "handlers": ["press"]],
            ["op": "create", "id": 13, "kind": "image", "props": ["imageSource": "symbol:sf/house", "symbolName": "house"]],
            ["op": "create", "id": 14, "kind": "text", "props": ["text": "Home"]],
            ["op": "create", "id": 15, "kind": "image", "props": ["imageSource": "symbol:sf/star", "symbolName": "star"]],
            ["op": "create", "id": 16, "kind": "text", "props": ["text": "Saved"]],
            ["op": "children", "id": 11, "ids": [13, 14]],
            ["op": "children", "id": 12, "ids": [15, 16]],
            ["op": "children", "id": 10, "ids": [11, 12]],
            ["op": "roots", "ids": [1, 10]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 400.0, "h": 20.0],
        ]))
        return p
    }

    private func drainIntrinsicSizes() {
        let delivered = expectation(description: "intrinsic size delivered after the batch")
        DispatchQueue.main.async { delivered.fulfill() }
        wait(for: [delivered], timeout: 2)
    }

    func testTabBarReportsItsHeightAndAlwaysFillsTheKernelBox() throws {
        let p = tabBarFixture()
        var reports: [CGSize?] = []
        p.onIntrinsic = { sizes in
            for (id, size) in sizes where id == 10 { reports.append(size) }
        }
        let owner = try XCTUnwrap(p.views[10])
        let bar = try XCTUnwrap(owner.subviews.first { $0 is UITabBar })
        XCTAssertEqual(bar.frame, owner.bounds, "the native bar never escapes the kernel box")
        drainIntrinsicSizes()
        let size = try XCTUnwrap(reports.last ?? nil)
        XCTAssertGreaterThan(size.height, 20)
        p.apply(wireBatch([["op": "frame", "id": 10, "x": 0.0, "y": 0.0,
                            "w": Double(size.width), "h": Double(size.height)]]))
        XCTAssertEqual(bar.frame, owner.bounds)
        drainIntrinsicSizes()
        XCTAssertEqual(reports.count, 1, "applying the measured box does not remeasure forever")
        p.apply(wireBatch([["op": "frame", "id": 10, "x": 0.0, "y": 0.0,
                            "w": 300.0, "h": Double(size.height)]]))
        drainIntrinsicSizes()
        XCTAssertEqual((reports.last ?? nil)?.width, 300)
        XCTAssertEqual(bar.frame, owner.bounds)
        p.apply(wireBatch([["op": "props", "id": 10,
                            "set": ["accessibilityRole": "tablist", "accessibilityOrientation": "vertical"]]]))
        drainIntrinsicSizes()
        XCTAssertEqual(reports.count, 3)
        XCTAssertNil(reports.last ?? nil, "leaving the projection clears its native minimum")
        XCTAssertNil(bar.superview)
        XCTAssertFalse(try XCTUnwrap(p.views[11]).isHidden)
        p.reset()
    }

    func testMissingRawSymbolsKeepTabBarsAndClearSegmentImages() throws {
        let p = tabBarFixture()
        defer { p.reset() }
        let owner = try XCTUnwrap(p.views[10])
        let icon = try XCTUnwrap(p.views[13])
        let bar = try XCTUnwrap(owner.subviews.first { $0 is UITabBar } as? UITabBar)
        p.apply(wireBatch([["op": "props", "id": 11, "set": ["accessibilitySelected": "true"]]]))
        var sizes: [CGSize?] = []
        p.onIntrinsic = { reports in for (id, size) in reports where id == 13 { sizes.append(size) } }
        for name in ["airpodsmax", "exact.nonexistent.symbol", "", "airpodsmax"] {
            p.apply(wireBatch([["op": "props", "id": 13,
                "set": ["imageSource": "symbol:sf/\(name)", "symbolName": name]]]))
            drainIntrinsicSizes()
            XCTAssertTrue(bar.superview === owner, "lookup never changes projection")
            XCTAssertEqual(bar.selectedItem?.tag, 0)
            XCTAssertEqual(bar.items?.first?.title, "Home")
            let found = name == "airpodsmax"
            XCTAssertEqual(icon.image != nil, found)
            XCTAssertEqual(bar.items?.first?.image != nil, found)
            XCTAssertNil(icon.symbolRefusal)
            if !found { XCTAssertEqual(sizes.last ?? nil, CGSize(width: 16, height: 16)) }
        }
        p.apply(wireBatch([
            ["op": "props", "id": 11, "set": ["accessibilityLabel": "Home"]],
            ["op": "children", "id": 11, "ids": [13]],
            ["op": "children", "id": 12, "ids": [15]],
            ["op": "frame", "id": 13, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0],
        ]))
        let segments = try XCTUnwrap(owner.subviews.first { $0 is UISegmentedControl } as? UISegmentedControl)
        XCTAssertNotNil(segments.imageForSegment(at: 0))
        for name in ["exact.nonexistent.symbol", "", "airpodsmax"] {
            p.apply(wireBatch([["op": "props", "id": 13,
                "set": ["imageSource": "symbol:sf/\(name)", "symbolName": name]]]))
            XCTAssertEqual(icon.image != nil, name == "airpodsmax")
            XCTAssertEqual(segments.imageForSegment(at: 0)?.accessibilityLabel, "Home")
            XCTAssertNil(segments.titleForSegment(at: 0))
            XCTAssertEqual(segments.selectedSegmentIndex, 0)
        }
    }

    func testResetRestoresTabBarMembersAndDropsTheProjection() throws {
        let p = tabBarFixture()
        let owner = try XCTUnwrap(p.views[10])
        let first = try XCTUnwrap(p.views[11])
        let second = try XCTUnwrap(p.views[12])
        let old = try XCTUnwrap(owner.subviews.first { $0 is UITabBar })
        XCTAssertTrue(first.isHidden)
        XCTAssertTrue(second.isHidden)
        p.segments.reset()
        XCTAssertNil(old.superview)
        XCTAssertFalse(first.isHidden)
        XCTAssertFalse(second.isHidden)
        p.segments.sync()
        let fresh = try XCTUnwrap(owner.subviews.first { $0 is UITabBar })
        XCTAssertFalse(fresh === old)
        p.reset()
        XCTAssertNil(fresh.superview)
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

#if os(iOS)
import UIKit
import ImageIO
import XCTest
@testable import ExactKit

/// A box Core Animation can paint keeps no bitmap: background, one radius
/// over a mask of corners, and a uniform border are layer properties; a
/// node with no paint (a collection's spacer) has no contents at all. Only
/// what the layer cannot say still draws. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class BoxLayerIOSTests: XCTestCase {
    private let white: BatchValue = [255, 255, 255, 255]
    private let blue: BatchValue = [0, 136, 255, 255]

    private func node(_ style: NodeStyle, size: CGSize = CGSize(width: 300, height: 120)) -> NodeView {
        let p = Presenter()
        let n = NodeView(id: 1, kind: "view", presenter: p)
        p.views[n.id] = n
        n.frame = CGRect(origin: .zero, size: size)
        n.applyStyle(style)
        n.layer.displayIfNeeded()
        return n
    }

    func testAnUnpaintedNodeHasNoBitmapHoweverTall() {
        let spacer = node([:], size: CGSize(width: 402, height: 865_678))
        XCTAssertNil(spacer.layer.contents)
        XCTAssertNil(spacer.layer.backgroundColor)
    }

    func testBackgroundAndMaskedRadiusAreLayerProperties() {
        let card = node(["background_color": white, "border_radius_top_left": 24, "border_radius_top_right": 24])
        XCTAssertNil(card.layer.contents)
        XCTAssertEqual(card.layer.backgroundColor?.alpha, 1)
        XCTAssertEqual(card.layer.cornerRadius, 24)
        XCTAssertEqual(card.layer.maskedCorners, [.layerMinXMinYCorner, .layerMaxXMinYCorner])
        XCTAssertFalse(card.layer.masksToBounds, "rounding clips nothing while the overflow is visible")
    }

    func testAUniformBorderStaysUnderTheChildrenUnlessTheyAreClipped() {
        let style: NodeStyle = ["border_width": 2, "border_color_top": blue, "border_radius_top_left": 11,
            "border_radius_top_right": 11, "border_radius_bottom_right": 11, "border_radius_bottom_left": 11]
        let open = node(style, size: CGSize(width: 22, height: 22))
        XCTAssertNil(open.layer.contents)
        XCTAssertEqual(open.layer.borderWidth, 0)
        let under = open.layer.sublayers?.first
        XCTAssertEqual(under?.borderWidth, 2)
        XCTAssertEqual(under?.cornerRadius, 11)
        var clipped = style; clipped["overflow_x"] = "hidden"; clipped["overflow_y"] = "hidden"
        let closed = node(clipped, size: CGSize(width: 22, height: 22))
        XCTAssertNil(closed.layer.contents)
        XCTAssertEqual(closed.layer.borderWidth, 2, "a clipping box's border covers what it clips")
        XCTAssertTrue(closed.layer.masksToBounds)
        XCTAssertNil(closed.boxBorder)
    }

    /// A row's separator (`border-bottom`) is a rectangle on a shape layer
    /// under the children: no backing store of the row's size for a hairline.
    func testSidesInOneColourAreAShapeLayer() throws {
        let row = node(["background_color": white, "border_width_bottom": 0.5, "border_width_left": 2, "border_color_top": blue])
        XCTAssertNil(row.layer.contents, "sides in one colour keep no bitmap")
        XCTAssertEqual(row.layer.backgroundColor?.alpha, 1)
        let edges = try XCTUnwrap(row.boxBorder as? CAShapeLayer)
        XCTAssertTrue(row.layer.sublayers?.first === edges, "under the children")
        XCTAssertEqual(edges.fillColor?.components, row.color("border_color_top", .clear).cgColor.components)
        let path = try XCTUnwrap(edges.path)
        XCTAssertTrue(path.contains(CGPoint(x: 150, y: 119.75)), "the bottom side")
        XCTAssertTrue(path.contains(CGPoint(x: 1, y: 60)), "the left side")
        XCTAssertFalse(path.contains(CGPoint(x: 150, y: 60)), "nothing inside")
        XCTAssertFalse(path.contains(CGPoint(x: 299, y: 60)), "no right side")
    }

    func testWhatTheLayerCannotSayStillDraws() {
        let sides = node(["background_color": white, "border_width_bottom": 1, "border_width_top": 1,
            "border_color_top": blue, "border_color_bottom": white])
        XCTAssertNotNil(sides.layer.contents, "sides in two colours draw")
        XCTAssertNil(sides.layer.backgroundColor, "and so does its background, once")
        let corners = node(["background_color": white, "border_radius_top_left": 8, "border_radius_bottom_right": 20])
        XCTAssertNotNil(corners.layer.contents, "two radii draw")
        XCTAssertEqual(corners.layer.cornerRadius, 0)
    }

    /// An image's decoded pixels are a sublayer's contents: the visible part
    /// of the fitted image (`contentsRect`), carrying the radius, with no
    /// bitmap painted for the view. An image whose clip the sublayer cannot
    /// say (padding under a radius) draws as before.
    func testAnImagesPixelsAreASublayersContents() throws {
        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("exact-image-layer-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: root) }
        let context = try XCTUnwrap(CGContext(data: nil, width: 200, height: 100, bitsPerComponent: 8, bytesPerRow: 800,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.setFillColor(red: 1, green: 0, blue: 0, alpha: 1); context.fill(CGRect(x: 0, y: 0, width: 200, height: 100))
        let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(root.appendingPathComponent("wide.png") as CFURL, "public.png" as CFString, 1, nil))
        CGImageDestinationAddImage(destination, try XCTUnwrap(context.makeImage()), nil)
        XCTAssertTrue(CGImageDestinationFinalize(destination))

        let p = Presenter(), loader = RasterLoader(), resolver = AssetResolver(root: root)
        defer { loader.shutdown(); withExtendedLifetime(resolver) {} }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300))
        p.viewport.frame = window.bounds; window.addSubview(p.viewport); window.makeKeyAndVisible()
        func image(_ id: UInt32, _ style: NodeStyle) -> NodeView {
            let n = NodeView(id: id, kind: "image", presenter: p)
            p.views[id] = n; p.viewport.addSubview(n)
            n.frame = CGRect(x: 0, y: 0, width: 40, height: 40)
            n.applyStyle(style)
            n.loadGeneration = 1
            loader.load(n, source: "wide.png", resolver: resolver)
            return n
        }
        let avatar = image(1, ["object_fit": .string("cover"), "border_radius_top_left": 20, "border_radius_top_right": 20,
            "border_radius_bottom_right": 20, "border_radius_bottom_left": 20])
        let padded = image(2, ["object_fit": .string("cover"), "padding_left": 4, "border_radius_top_left": 20,
            "border_radius_top_right": 20, "border_radius_bottom_right": 20, "border_radius_bottom_left": 20])
        let end = Date(timeIntervalSinceNow: 5)
        while (avatar.raster == nil || padded.raster == nil) && Date() < end { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01)) }
        avatar.layer.displayIfNeeded(); padded.layer.displayIfNeeded()

        let sub = try XCTUnwrap(avatar.imageLayer)
        XCTAssertTrue((sub.contents as AnyObject?) === avatar.raster?.image.image)
        XCTAssertNil(avatar.layer.contents, "no bitmap of the view's size")
        XCTAssertEqual(sub.frame, avatar.bounds)
        // A 2:1 image covering a square: the middle half of its width shows.
        XCTAssertEqual(sub.contentsRect, CGRect(x: 0.25, y: 0, width: 0.5, height: 1))
        XCTAssertEqual(sub.cornerRadius, 20); XCTAssertTrue(sub.masksToBounds)

        XCTAssertNil(padded.imageLayer)
        XCTAssertNotNil(padded.layer.contents, "a radius over a padded content box draws")

        avatar.raster = nil
        XCTAssertNil(avatar.imageLayer, "no pixels outlive the lease")
        XCTAssertNil(sub.superlayer)
        padded.raster = nil
    }
}
#endif

#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1035.001, the zoom's alignment: UIKit's zoom lands the source on the
/// presented route's element with the source's id; for an image, on the
/// image as drawn, so a thumbnail morphs into the photo rather than into the
/// whole route with two photos cross-fading.
final class ZoomAlignmentIOSTests: XCTestCase {
    private func png(_ root: URL, _ name: String, width: Int, height: Int) throws {
        let context = try XCTUnwrap(CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.setFillColor(red: 0.9, green: 0.88, blue: 0.8, alpha: 1); context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(root.appendingPathComponent(name) as CFURL, "public.png" as CFString, 1, nil))
        CGImageDestinationAddImage(destination, try XCTUnwrap(context.makeImage()), nil)
        XCTAssertTrue(CGImageDestinationFinalize(destination))
    }

    func testAZoomLandsOnTheImageAsDrawn() throws {
        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("exact-zoom-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: root) }
        try png(root, "photo.png", width: 400, height: 300)
        let p = Presenter(), loader = RasterLoader(), resolver = AssetResolver(root: root)
        defer { loader.shutdown(); withExtendedLifetime(resolver) {} }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        let zoomed = UIView(frame: window.bounds)
        window.addSubview(zoomed); window.makeKeyAndVisible()
        let image = NodeView(id: 1, kind: "image", presenter: p)
        p.views[1] = image; zoomed.addSubview(image)
        image.frame = zoomed.bounds
        image.applyStyle(["object_fit": .string("contain")])
        // Before its pixels: the box, or the source's natural size when given.
        XCTAssertEqual(ModalHost.zoomAlignment(target: image, fallbackNatural: nil, in: zoomed), zoomed.bounds)
        let early = ModalHost.zoomAlignment(target: image, fallbackNatural: CGSize(width: 1000, height: 750), in: zoomed)
        XCTAssertEqual(early.width, 402, accuracy: 0.01); XCTAssertEqual(early.height, 301.5, accuracy: 0.01)
        XCTAssertEqual(early.midY, 437, accuracy: 0.01, "contained, centred")
        // With its own pixels (4:3 again): the same rectangle.
        image.loadGeneration = 1
        loader.load(image, source: "photo.png", resolver: resolver)
        let end = Date(timeIntervalSinceNow: 5)
        while image.raster == nil && Date() < end { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01)) }
        XCTAssertNotNil(image.raster)
        let drawn = ModalHost.zoomAlignment(target: image, fallbackNatural: nil, in: zoomed)
        XCTAssertEqual(drawn.height, 301.5, accuracy: 0.01)
        // A target that is a box holding the image lands on the image inside it.
        let holder = NodeView(id: 2, kind: "view", presenter: p)
        p.views[2] = holder; zoomed.addSubview(holder)
        holder.frame = CGRect(x: 0, y: 100, width: 402, height: 600)
        image.removeFromSuperview(); holder.addSubview(image); image.frame = holder.bounds
        let nested = ModalHost.zoomAlignment(target: holder, fallbackNatural: nil, in: zoomed)
        XCTAssertEqual(nested.midY, 400, accuracy: 0.01, "the image within its holder")
        XCTAssertEqual(nested.height, 301.5, accuracy: 0.01)
        // Padding and a border: the image is fitted, and clipped, in its
        // content box, as it is drawn.
        image.removeFromSuperview(); zoomed.addSubview(image); image.frame = zoomed.bounds
        image.applyStyle(["object_fit": .string("contain"), "padding_left": 20, "padding_right": 20, "border_width": 1])
        let padded = ModalHost.zoomAlignment(target: image, fallbackNatural: nil, in: zoomed)
        XCTAssertEqual(padded.minX, 21, accuracy: 0.01, "inside the border and the padding")
        XCTAssertEqual(padded.width, 360, accuracy: 0.01)
        XCTAssertEqual(padded.height, 270, accuracy: 0.01, "contained in 360 wide")
        // An image deeper in: a holder's child's child.
        image.applyStyle(["object_fit": .string("contain")])
        let outer = NodeView(id: 4, kind: "view", presenter: p), inner = NodeView(id: 5, kind: "view", presenter: p)
        p.views[4] = outer; p.views[5] = inner
        zoomed.addSubview(outer); outer.frame = zoomed.bounds
        outer.container.addSubview(inner); inner.frame = outer.bounds
        image.removeFromSuperview(); inner.container.addSubview(image); image.frame = inner.bounds
        XCTAssertTrue(ModalHost.zoomImage(outer) === image, "found among the descendants")
        XCTAssertEqual(ModalHost.zoomAlignment(target: outer, fallbackNatural: nil, in: zoomed).height, 301.5, accuracy: 0.01)
        // A box with no image: its own box.
        let plain = NodeView(id: 3, kind: "view", presenter: p)
        p.views[3] = plain; zoomed.addSubview(plain); plain.frame = CGRect(x: 10, y: 20, width: 30, height: 40)
        XCTAssertEqual(ModalHost.zoomAlignment(target: plain, fallbackNatural: nil, in: zoomed), CGRect(x: 10, y: 20, width: 30, height: 40))
    }

    /// The interactive dismissal lands unaligned: given a rect, UIKit re-bases
    /// the route that followed the finger onto it at lift-off (a held frame,
    /// then a jump). The rect resolved meanwhile is kept for the next aligned
    /// zoom, and an unresolved target still answers the last one.
    func testAnInteractiveDismissalLandsUnaligned() {
        let landing = ZoomLanding()
        let open = CGRect(x: 0, y: 286.25, width: 402, height: 301.5), moved = CGRect(x: 0, y: 300, width: 402, height: 301.5)
        landing.appeared()
        XCTAssertEqual(landing.answer(open), open)
        XCTAssertEqual(landing.answer(nil), open, "a target gone: the last answer")
        // A refused dismissal changes nothing.
        XCTAssertFalse(landing.dismissal(begins: false))
        XCTAssertEqual(landing.answer(nil), open)
        // Accepted: unaligned, the new rect kept.
        XCTAssertTrue(landing.dismissal(begins: true))
        XCTAssertNil(landing.answer(moved))
        XCTAssertNil(landing.answer(nil))
        // Cancelled (the route appears again), then a Close: aligned on the newest rect.
        landing.appeared()
        XCTAssertEqual(landing.answer(nil), moved)
        // A second drag is unaligned again.
        XCTAssertTrue(landing.dismissal(begins: true))
        XCTAssertNil(landing.answer(open))
    }
}
#endif

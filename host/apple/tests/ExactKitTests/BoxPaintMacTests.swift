#if os(macOS)
import AppKit
import ImageIO
import XCTest
@testable import ExactKit

/// A node's box as the Mac paints it, read back as pixels: a rounded box's
/// border in the colour the host sends for its sides, and each corner at
/// its own radius, as iOS and Linux draw them.
final class BoxPaintMacTests: XCTestCase {
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil }
    /// A presenter in a window, as the agent captures one.
    private func presenter() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 100, height: 100)
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
        return p
    }
    private func paint(_ style: NodeStyle) -> NSBitmapImageRep {
        let p = presenter()
        let node = NodeView(id: 1, kind: "view", presenter: p)
        node.frame = NSRect(x: 0, y: 0, width: 100, height: 40)
        p.root.addSubview(node); p.views[1] = node
        node.applyStyle(style)
        return capture(node)
    }
    /// As the agent's screenshot captures it.
    private func capture(_ view: NSView) -> NSBitmapImageRep { Capture.picture(of: view)! }
    /// sRGB components of the pixel under a point in the view's own (flipped) space.
    private func rgba(_ rep: NSBitmapImageRep, _ x: CGFloat, _ y: CGFloat) -> [CGFloat] {
        let scale = CGFloat(rep.pixelsWide) / rep.size.width
        let c = rep.colorAt(x: Int(x * scale), y: Int(y * scale))!.usingColorSpace(.sRGB)!
        return [c.redComponent, c.greenComponent, c.blueComponent, c.alphaComponent]
    }
    private func box(radii: [BatchValue]) -> NodeStyle {
        var style: NodeStyle = ["background_color": [255, 255, 255, 255]]
        for (corner, radius) in zip(["top_left", "top_right", "bottom_right", "bottom_left"], radii) { style["border_radius_" + corner] = radius }
        for side in ["top", "right", "bottom", "left"] { style["border_width_" + side] = 2; style["border_color_" + side] = [255, 0, 0, 255] }
        return style
    }

    func testRoundedBorderStrokesTheColourTheHostSends() {
        // Caltrain's station search: equal widths, every corner rounded.
        let rep = paint(box(radii: [8, 8, 8, 8]))
        for (x, y) in [(50.0, 0.5), (50.0, 39.5), (0.5, 20.0), (99.5, 20.0)] {
            let c = rgba(rep, x, y)
            XCTAssert(c[0] > 0.9 && c[1] < 0.2 && c[2] < 0.2 && c[3] > 0.9, "border at \(x),\(y) is \(c)")
        }
    }

    func testEachCornerKeepsItsOwnRadius() {
        let rep = paint(box(radii: [0, 16, 16, 0]))
        XCTAssertGreaterThan(rgba(rep, 0.5, 0.5)[3], 0.9, "the square top-left corner is painted")
        XCTAssertGreaterThan(rgba(rep, 0.5, 39.5)[3], 0.9, "the square bottom-left corner is painted")
        XCTAssertLessThan(rgba(rep, 99.5, 0.5)[3], 0.1, "the rounded top-right corner is empty")
        XCTAssertLessThan(rgba(rep, 99.5, 39.5)[3], 0.1, "the rounded bottom-right corner is empty")
    }

    /// A translucent fill is painted once into a capture, not by the
    /// layer and again by `draw(_:)` (spreadsheet F18: 12 % came out 20 %).
    func testATranslucentFillIsCapturedOnce() throws {
        let p = presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": [255.0, 255.0, 255.0, 255.0]]],
            ["op": "create", "id": 2, "kind": "view", "style": ["background_color": [0.0, 0.0, 0.0, 128.0]]],
            ["op": "create", "id": 3, "kind": "view", "style": ["background_color": [0.0, 0.0, 0.0, 128.0], "border_radius": 8.0,
                "border_width": 1.0, "border_color_top": [0.0, 0.0, 0.0, 128.0]]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 50.0, "w": 100.0, "h": 50.0],
        ]))
        let rep = capture(try XCTUnwrap(p.views[1]))
        // Half black over white, once: not the quarter a second coat leaves.
        XCTAssertEqual(rgba(rep, 50, 25)[0], 0.5, accuracy: 0.1, "a fill the layer paints")
        XCTAssertEqual(rgba(rep, 50, 75)[0], 0.5, accuracy: 0.1, "a rounded, bordered fill")
    }

    /// An image whose pixels are a sublayer is captured once: `draw(_:)`
    /// paints the bitmap for the shot, and the sublayer is hidden for it,
    /// so half-transparent red over white stays pink (Grok's batch 2 review).
    func testATranslucentLayerImageIsCapturedOnce() throws {
        let (page, image) = try imageOnPage(red: 0.5, style: ["object_fit": .string("fill")])
        let sub = try XCTUnwrap(image.imageLayer, "the image took the layer path")
        let rep = capture(page)
        let c = rgba(rep, 20, 20)
        XCTAssertEqual(c[0], 1, accuracy: 0.05); XCTAssertEqual(c[1], 0.5, accuracy: 0.1, "pink, not the red of a second coat: \(c)")
        XCTAssertFalse(sub.isHidden, "the sublayer shows again after the shot")
    }

    /// An image's own background is captured behind its picture, as CSS
    /// paints it and the window shows it: with a radius the fill is a
    /// sublayer, which the shot drew over the picture (podcast F7).
    func testARoundedImageBackgroundIsCapturedUnderThePicture() throws {
        let (page, image) = try imageOnPage(red: 1, style: [
            "object_fit": .string("cover"), "background_color": [0, 0, 255, 255], "border_radius": 8])
        XCTAssertNotNil(image.imageLayer, "the image took the layer path")
        let fill = try XCTUnwrap(image.boxFill, "the rounded fill is a sublayer")
        let c = rgba(capture(page), 20, 20)
        XCTAssert(c[0] > 0.9 && c[2] < 0.1, "the red picture, not its blue background: \(c)")
        XCTAssertFalse(fill.isHidden, "the fill shows again after the shot")
    }

    /// A 40-point image of opaque or translucent red, on a white page, its
    /// pixels loaded.
    private func imageOnPage(red alpha: CGFloat, style: NodeStyle) throws -> (NodeView, NodeView) {
        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("exact-image-capture-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: root) }
        let context = try XCTUnwrap(CGContext(data: nil, width: 4, height: 4, bitsPerComponent: 8, bytesPerRow: 16,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.setFillColor(red: 1, green: 0, blue: 0, alpha: alpha); context.fill(CGRect(x: 0, y: 0, width: 4, height: 4))
        let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(root.appendingPathComponent("red.png") as CFURL, "public.png" as CFString, 1, nil))
        CGImageDestinationAddImage(destination, try XCTUnwrap(context.makeImage()), nil)
        XCTAssertTrue(CGImageDestinationFinalize(destination))

        let p = presenter(), loader = RasterLoader(), resolver = AssetResolver(root: root)
        defer { loader.shutdown(); withExtendedLifetime(resolver) {} }
        let page = NodeView(id: 1, kind: "view", presenter: p)
        page.frame = NSRect(x: 0, y: 0, width: 100, height: 100)
        p.root.addSubview(page); p.views[1] = page
        page.applyStyle(["background_color": [255, 255, 255, 255]])
        let image = NodeView(id: 2, kind: "image", presenter: p)
        image.frame = NSRect(x: 0, y: 0, width: 40, height: 40)
        page.addSubview(image); p.views[2] = image
        image.applyStyle(style)
        image.loadGeneration = 1
        loader.load(image, source: "red.png", resolver: resolver)
        let end = Date(timeIntervalSinceNow: 5)
        while image.raster == nil && Date() < end { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01)) }
        image.layer?.displayIfNeeded()
        return (page, image)
    }

    /// A capture shows what the window does: siblings in `z-index` order
    /// (`zPosition`), not the order `cacheDisplay` draws subviews in — a
    /// sticky header over the rows under it (spreadsheet F13), a raised
    /// dropdown over what follows it (shop F18) — and a lowered animation at
    /// what it presents, not its model value (shop F25).
    func testACaptureShowsTheZOrderAndTheAnimationsTheWindowShows() throws {
        let p = presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["display": "flex", "background_color": [255.0, 255.0, 255.0, 255.0]]],
            ["op": "create", "id": 2, "kind": "view", "style": ["background_color": [255.0, 0.0, 0.0, 255.0]]],
            // `z-index: 3`, as the kernel sends it: the rank, doubled (LLP 1083.000 D4).
            ["op": "rank", "id": 2, "rank": 6],
            ["op": "create", "id": 3, "kind": "view", "style": ["background_color": [0.0, 0.0, 255.0, 255.0]]],
            ["op": "create", "id": 4, "kind": "view", "style": ["background_color": [0.0, 0.0, 0.0, 255.0]]],
            // LLP 1083: the kernel sends paint rank separately from style.
            ["op": "rank", "id": 2, "rank": 6],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 20.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 60.0, "w": 100.0, "h": 40.0],
            // Held at its middle, as the agent's clock holds it: 50 % black over white.
            ["op": "animations", "id": 4, "specs": [["id": "fade#0#opacity", "k": "opacity", "s": 0.0, "dl": 0.0, "d": 1.0, "n": 1.0,
                                                     "t": [0.0, 1.0], "v": [1.0, 0.0], "c": [], "fill": 3.0, "h": 0.5]]],
        ]))
        let root = try XCTUnwrap(p.views[1])
        let order = root.subviews
        let rep = capture(root)
        let raised = rgba(rep, 50, 30)
        XCTAssert(raised[0] > 0.9 && raised[2] < 0.1, "the raised box over the later one: \(raised)")
        XCTAssertEqual(rgba(rep, 50, 80)[0], 0.5, accuracy: 0.1, "the opacity the animation holds")
        XCTAssertEqual(root.subviews, order, "the subviews are put back")
        XCTAssertEqual(p.views[4]?.layer?.opacity, 1, "the model is put back")
    }
}
#endif

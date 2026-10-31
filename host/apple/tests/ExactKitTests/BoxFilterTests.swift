import XCTest
import ImageIO
import CoreText
#if os(macOS)
import AppKit
#else
import UIKit
#endif
@testable import ExactKit

final class BoxFilterTests: XCTestCase {
    private var p: Presenter!
    #if os(macOS)
    private var window: NSWindow!
    #else
    private var window: UIWindow!
    #endif

    override func setUp() {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        p = Presenter()
        #if os(macOS)
        window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 200, height: 200), styleMask: [], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.viewport.wantsLayer = true
        p.root.wantsLayer = true
        p.root.frame = CGRect(x: 0, y: 0, width: 200, height: 200)
        window.orderFront(nil)
        window.displayIfNeeded()
        #else
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 200, height: 200))
        let controller = UIViewController()
        controller.view = p.viewport
        window.rootViewController = controller
        window.isHidden = false
        #endif
    }

    override func tearDown() {
        #if os(macOS)
        window.close()
        #else
        window.isHidden = true
        window.rootViewController = nil
        #endif
        window = nil; p = nil
    }

    private func node(_ id: UInt32, kind: String = "view", in parent: NodeView? = nil) -> NodeView {
        let node = NodeView(id: id, kind: kind, presenter: p)
        p.views[id] = node
        node.frame = CGRect(x: 0, y: 0, width: 40, height: 40)
        if let parent { parent.addSubview(node) } else { p.root.addSubview(node) }
        return node
    }

    private func filter() throws -> BatchValue {
        // Identity colour matrix, with a subregion covering the whole box.
        let matrix: [Double] = [1,0,0,0,0, 0,1,0,0,0, 0,0,1,0,0, 0,0,0,1,0]
        let program: [Double] = [0,0,0,0,1, 5,-1,-3,0,0,10000,10000,0] + matrix
        guard SvgFilterGPU.runs(program.map(Float.init)) else { throw missing("metal", "no GPU filter path") }
        return ["p": .array(program.map(BatchValue.number))]
    }

    /// `drop-shadow(dx dy σ rgba)` as `box_filter` encodes it: the region
    /// how far past the box it reaches, the subregion the same.
    private func dropShadow(dx: Double, dy: Double, sigma: Double, rgba: [Double], darkRGBA: [Double]? = nil) -> BatchValue {
        func program(_ c: [Double]) -> [Double] {
            let reach = 3 * sigma
            let region = [-reach + min(dx, 0), -reach + min(dy, 0), 2 * reach + abs(dx), 2 * reach + abs(dy)]
            return region + [1, 6, -1, -3] + region + [0, sigma, sigma, dx, dy] + c
        }
        var o: [String: BatchValue] = ["p": .array(program(rgba).map(BatchValue.number))]
        if let darkRGBA { o["pd"] = .array(program(darkRGBA).map(BatchValue.number)) }
        return .object(o)
    }

    #if os(iOS) || os(tvOS)
    /// A lone `drop-shadow()` is the box's own layer shadow: no picture,
    /// no mask hiding the box, nothing to draw again after a batch. A box
    /// that clips, which would clip its layer's shadow, and a chain of more
    /// than the shadow are pictured.
    func testALoneDropShadowIsTheLayersShadow() throws {
        let box = node(1)
        box.applyStyle(["filter": dropShadow(dx: 0, dy: 2, sigma: 2, rgba: [0, 0, 0, 0.6])])
        p.boxFilters.render()
        XCTAssertNotNil(box.boxFilter)
        XCTAssertNil(box.boxFilter?.picture.superlayer, "no picture stands in for the box")
        XCTAssertNil(box.layer.mask, "the box itself shows")
        XCTAssertEqual(box.layer.shadowRadius, 2, "Core Animation's radius is the σ")
        XCTAssertEqual(box.layer.shadowOffset, CGSize(width: 0, height: 2))
        XCTAssertEqual(box.layer.shadowOpacity, 0.6, accuracy: 0.001)
        XCTAssertEqual(box.layer.shadowColor?.components?.prefix(3).map { Double($0) }, [0, 0, 0])

        box.overrideUserInterfaceStyle = .dark
        box.updateTraitsIfNeeded()
        XCTAssertTrue(box.drawsDark)
        box.applyStyle(["filter": dropShadow(dx: 0, dy: 0, sigma: 1, rgba: [0, 0, 0, 1], darkRGBA: [1, 1, 1, 1])])
        XCTAssertEqual(box.layer.shadowColor?.components?.prefix(3).map { Double($0) }, [1, 1, 1], "the dark appearance's colour")
        box.overrideUserInterfaceStyle = .unspecified
        box.updateTraitsIfNeeded()

        box.applyStyle(["filter": try filter()])
        XCTAssertNotNil(box.boxFilter?.picture.superlayer, "a chain of more than the shadow is pictured")
        XCTAssertEqual(box.layer.shadowOpacity, 0, "and the layer casts none")
        XCTAssertTrue(box.layer.mask === box.boxFilter?.hide)

        let shadow = dropShadow(dx: 0, dy: 2, sigma: 2, rgba: [0, 0, 0, 0.6])
        box.applyStyle(["filter": shadow, "overflow_x": "hidden", "overflow_y": "hidden"])
        p.boxFilters.render()
        XCTAssertTrue(box.layer.masksToBounds)
        XCTAssertNotNil(box.boxFilter?.picture.superlayer, "a box that clips is pictured, its shadow past its bounds")
        XCTAssertEqual(box.layer.shadowOpacity, 0)

        box.applyStyle(["filter": shadow])
        p.boxFilters.render()
        XCTAssertNil(box.boxFilter?.picture.superlayer)
        XCTAssertEqual(box.layer.shadowOpacity, 0.6, accuracy: 0.001)
        box.applyStyle([:])
        XCTAssertTrue(p.boxFilters.isEmpty)
        XCTAssertEqual(box.layer.shadowOpacity, 0, "an unfiltered box casts nothing")
        XCTAssertNil(box.layer.mask)
    }
    #endif

    private func picture(_ node: NodeView) throws -> CGImage {
        try XCTUnwrap(node.boxFilter?.picture.sublayers?.first?.contents) as! CGImage
    }

    private func pixel(_ image: CGImage, x: Int, y: Int) throws -> [UInt8] {
        let ctx = try XCTUnwrap(CGContext(data: nil, width: 40, height: 40, bitsPerComponent: 8,
            bytesPerRow: 160, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: 40, height: 40))
        let bytes = try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self)
        return Array(UnsafeBufferPointer(start: bytes + (y * 40 + x) * 4, count: 4))
    }

    #if os(macOS)
    func testAFilteredGradientKeepsSquareCornersAsItsRadiiChange() throws {
        let box = node(1), effect = try filter()
        let corners = ["top_left", "top_right", "bottom_right", "bottom_left"]
        let gradient: BatchValue = ["linear": 90, "stops": [0,255,0,0,255, 1,0,0,255,255]]
        for selected in Array(1..<16) + [0] {
            var style: NodeStyle = ["background_image": gradient, "filter": effect]
            for (i, corner) in corners.enumerated() { style["border_radius_" + corner] = .number(selected & (1 << i) == 0 ? 0 : 16) }
            box.applyStyle(style)
            window.displayIfNeeded()
            box.renderFilter(); settle()
            let image = try picture(box)
            let alpha = try [(1,1), (38,1), (38,38), (1,38)].map { try pixel(image, x: $0.0, y: $0.1)[3] }
            XCTAssertEqual(alpha, (0..<4).map { selected & (1 << $0) == 0 ? 255 : 0 }, "corners \(selected)")
            XCTAssertEqual(try pixel(image, x: 20, y: 20)[3], 255)
        }
    }

    func testOversizedBorderRadiiReduceBeforeTheInnerEdgeIsInset() throws {
        let box = node(1), effect = try filter()
        for filtered in [false, true] {
            func render(radius: Double) throws -> CGImage {
                var style: NodeStyle = ["background_color": [255,0,0,255], "border_width": 4,
                    "border_color_top": [0,0,255,255], "border_radius_top_left": .number(radius),
                    "border_radius_top_right": .number(radius)]
                if filtered { style["filter"] = effect }
                box.applyStyle(style)
                window.displayIfNeeded()
                if filtered { box.renderFilter(); settle(); return try picture(box) }
                return try XCTUnwrap(Capture.picture(of: XCTUnwrap(box.layer), size: box.bounds.size, scale: 1))
            }
            let reduced = try render(radius: 20), oversized = try render(radius: 999)
            XCTAssertEqual(try pixel(reduced, x: 20, y: 20), [255,0,0,255])
            XCTAssertEqual(try pixel(reduced, x: 20, y: 1), [0,0,255,255])
            var mismatches = 0
            for y in [1,5,10,20,30,35,38] {
                for x in [1,5,10,20,30,35,38] {
                    if try pixel(reduced, x: x, y: y) != pixel(oversized, x: x, y: y) { mismatches += 1 }
                }
            }
            XCTAssertEqual(mismatches, 0, "999-point top corners must paint like 20-point corners; filtered: \(filtered)")
        }
    }

    func testAFilteredBoxKeepsEachRoundedAndSquareCorner() throws {
        let box = node(1), effect = try filter()
        let corners = ["top_left", "top_right", "bottom_right", "bottom_left"]
        for bordered in [false, true] {
            for clipped in [false, true] {
                for selected in 1..<16 {
                    var style: NodeStyle = ["background_color": [255,0,0,255], "filter": effect]
                    if bordered { style["border_width"] = 4; style["border_color_top"] = [0,0,255,255] }
                    if clipped { style["overflow_x"] = "hidden"; style["overflow_y"] = "hidden" }
                    for (i, corner) in corners.enumerated() { style["border_radius_" + corner] = .number(selected & (1 << i) == 0 ? 0 : 16) }
                    box.applyStyle(style)
                    #if os(macOS)
                    window.displayIfNeeded()
                    #endif
                    box.renderFilter(); settle()
                    let image = try picture(box)
                    let alpha = try [(1,1), (38,1), (38,38), (1,38)].map { try pixel(image, x: $0.0, y: $0.1)[3] }
                    XCTAssertEqual(alpha, (0..<4).map { selected & (1 << $0) == 0 ? 255 : 0 }, "corners \(selected), border \(bordered), clip \(clipped)")
                    XCTAssertEqual(try pixel(image, x: 20, y: 20), [255,0,0,255])
                    if bordered { XCTAssertEqual(try pixel(image, x: 20, y: 1), [0,0,255,255]) }
                }
            }
        }
    }

    #endif

    /// A batch draws a box's picture again only when it touched the box or
    /// something in it, or the box's size changed; otherwise the picture
    /// follows the box. A picture after the first is drawn off the main
    /// thread, the old one showing until it lands.
    func testABoxIsDrawnAgainOnlyWhenWhatItShowsChanged() throws {
        let box = node(1)
        _ = node(2, in: box); _ = node(3)
        box.applyStyle(["background_color": [255,0,0,255], "filter": try filter()])
        #if os(macOS)
        window.displayIfNeeded()
        box.renderFilter()
        #endif
        settle()
        let first = try picture(box)
        p.touched(3)
        p.boxFilters.render()
        XCTAssertFalse(p.boxFilters.drawing, "a batch that touched nothing in it draws nothing")
        box.frame.origin = CGPoint(x: 10, y: 20)
        p.boxFilters.render()
        XCTAssertFalse(p.boxFilters.drawing)
        XCTAssertTrue(try picture(box) === first)
        let moved: CALayer? = box.layer
        XCTAssertEqual(box.boxFilter?.picture.position, moved?.position, "a box that moved is followed")
        p.touched(2)
        p.boxFilters.render()
        XCTAssertTrue(p.boxFilters.drawing, "a change inside it draws it again, off the main thread")
        XCTAssertTrue(try picture(box) === first, "the old picture shows until the new one lands")
        settle()
        let second = try picture(box)
        XCTAssertFalse(second === first)
        XCTAssertEqual(try pixel(second, x: 20, y: 20), [255,0,0,255])
        box.frame.size = CGSize(width: 50, height: 40)
        p.boxFilters.render()
        settle()
        XCTAssertFalse(try picture(box) === second, "so does a new size")
    }

    func testAnImageDecodeUpdatesItsFilteredAncestorWithoutABatch() throws {
        let outer = node(3), box = node(1, in: outer), image = node(2, kind: "image", in: box)
        outer.applyStyle(["filter": try filter()])
        box.applyStyle(["filter": try filter()])
        #if os(macOS)
        window.displayIfNeeded()
        #endif
        p.boxFilters.render()
        let before = try picture(box)
        let ctx = try XCTUnwrap(CGContext(data: nil, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.setFillColor(CGColor(srgbRed: 1, green: 0, blue: 0, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: 1, height: 1))
        let data = NSMutableData()
        let destination = try XCTUnwrap(CGImageDestinationCreateWithData(data, "public.png" as CFString, 1, nil))
        CGImageDestinationAddImage(destination, try XCTUnwrap(ctx.makeImage()), nil)
        XCTAssertTrue(CGImageDestinationFinalize(destination))
        let source = "data:image/png;base64," + (data as Data).base64EncodedString()
        let loader = RasterLoader(), resolver = AssetResolver(root: FileManager.default.temporaryDirectory)
        defer { loader.shutdown(); image.raster = nil }
        image.loadGeneration = 1
        XCTAssertTrue(loader.load(image, source: source, resolver: resolver))
        let end = Date(timeIntervalSinceNow: 20)
        while image.raster == nil && Date() < end { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05)) }
        XCTAssertNotNil(image.raster)
        settle()
        let after = try picture(box)
        XCTAssertFalse(before === after)
        XCTAssertEqual(try pixel(after, x: 20, y: 20), [255,0,0,255])
        XCTAssertEqual(try pixel(picture(outer), x: 20, y: 20), [255,0,0,255], "inner pictures publish before their ancestors")
    }
    /// Every picture drawn and landed: a box's first on screen is drawn
    /// in place, any later one off the main thread.
    private func settle() {
        let end = Date(timeIntervalSinceNow: 20)
        repeat { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01)) } while p.boxFilters.drawing && Date() < end
        XCTAssertFalse(p.boxFilters.drawing)
    }

    func testALateTextRasterUpdatesItsFilteredAncestorWithoutABatch() throws {
        let box = node(1), text = node(2, kind: "text", in: box)
        box.frame.size = CGSize(width: 160, height: 120); text.frame = box.bounds
        #if os(macOS)
        text.wantsLayer = true
        window.displayIfNeeded()
        let font = NSFont.systemFont(ofSize: 24), ink = NSColor.red
        #else
        let font = UIFont.systemFont(ofSize: 24), ink = UIColor.red
        #endif
        box.applyStyle(["filter": try filter()])
        box.renderFilter()
        let before = try picture(box)
        let key = TextRasterKey(spec: text.paragraphSpec(), size: text.bounds.size, box: text.bounds, scale: 2)
        let source = NSAttributedString(string: "Late", attributes: [.font: font, .foregroundColor: ink])
        let job = TextRasterJob(source: source, ranges: [CFRange(location: 0, length: 4)], baselines: [28],
            flush: 0, box: text.bounds, size: text.bounds.size, scale: 2)
        let result = try XCTUnwrap(job.render())
        text.textRasterKey = key
        #if os(macOS)
        text.textRasterReady = false
        text.showTextRaster(result.surface, for: key, frame: result.frame)
        #else
        text.showTextRaster(result, for: key)
        #endif
        settle()
        let after = try picture(box)
        XCTAssertFalse(before === after)
        let bytes = try XCTUnwrap(after.dataProvider?.data)
        let data = try XCTUnwrap(CFDataGetBytePtr(bytes))
        XCTAssertTrue((0..<after.height).contains { y in
            (0..<after.width).contains { x in data[y * after.bytesPerRow + x * 4 + 3] > 0 }
        }, "the published glyphs are in the filtered picture")
    }

}

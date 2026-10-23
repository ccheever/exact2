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
                    box.renderFilter()
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
        drainPublication()
        let after = try picture(box)
        XCTAssertFalse(before === after)
        XCTAssertEqual(try pixel(after, x: 20, y: 20), [255,0,0,255])
        XCTAssertEqual(try pixel(picture(outer), x: 20, y: 20), [255,0,0,255], "inner pictures publish before their ancestors")
    }
    private func drainPublication() {
        let published = expectation(description: "queued paint publication")
        DispatchQueue.main.async { published.fulfill() }
        wait(for: [published], timeout: 20)
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
        text.textRasterKey = key; text.textRasterReady = false
        #if os(macOS)
        text.showTextRaster(result.surface, for: key, frame: result.frame)
        #else
        text.showTextRaster(result, for: key)
        #endif
        drainPublication()
        let after = try picture(box)
        XCTAssertFalse(before === after)
        let bytes = try XCTUnwrap(after.dataProvider?.data)
        let data = try XCTUnwrap(CFDataGetBytePtr(bytes))
        XCTAssertTrue((0..<after.height).contains { y in
            (0..<after.width).contains { x in data[y * after.bytesPerRow + x * 4 + 3] > 0 }
        }, "the published glyphs are in the filtered picture")
    }

}

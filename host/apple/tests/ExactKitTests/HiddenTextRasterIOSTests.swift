import XCTest
import CoreGraphics
@testable import ExactKit
#if os(macOS)
import AppKit
import IOSurface
#else
import UIKit
#endif

/// A paragraph set to `visibility: hidden` must drop the text bitmap it was
/// already showing. A run that still computes `visible` is drawn by
/// TextEngine, which skips hidden runs, rather than by that bitmap.
/// The suffix selects this class on the simulator lane.
final class HiddenTextRasterIOSTests: XCTestCase {
    private var session: ExactSession?
    #if os(macOS)
    private var window: NSWindow?
    override func tearDown() {
        window?.close(); window = nil
        session?.destroy(); session = nil
        super.tearDown()
    }
    #else
    private var window: UIWindow?
    override func tearDown() {
        window?.isHidden = true; window = nil
        session?.destroy(); session = nil
        super.tearDown()
    }
    #endif

    private func presenter(_ label: String) -> Presenter {
        let session = ExactApp.shared.makeSession(label: label)
        self.session = session
        let p = session.presenter
        #if os(macOS)
        _ = NSApplication.shared
        p.viewport.frame = NSRect(x: 0, y: 0, width: 500, height: 200)
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
        #else
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 500, height: 200))
        window.frame = CGRect(x: 0, y: 0, width: 500, height: 200)
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        self.window = window
        #endif
        return p
    }

    /// The host writes `op` first. `wireBatch`'s dictionary order does not,
    /// and only that first-key path used to drop a run's `visibility`.
    private func wire(_ json: String) -> Batch {
        let batch = Batch.decode(Data(json.utf8))
        precondition(batch.error == nil, batch.error ?? json)
        return batch
    }

    /// Opaque pixels `draw` puts in a bitmap. The layer's old surface is not in it.
    private func ink(_ node: NodeView) -> Int {
        let w = max(1, Int(node.bounds.width)), h = max(1, Int(node.bounds.height))
        guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                  space: CGColorSpaceCreateDeviceRGB(),
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return 0 }
        #if os(macOS)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: true)
        node.draw(node.bounds)
        NSGraphicsContext.restoreGraphicsState()
        #else
        UIGraphicsPushContext(ctx)
        node.draw(node.bounds)
        UIGraphicsPopContext()
        #endif
        guard let raw = ctx.data else { return 0 }
        let bytes = raw.bindMemory(to: UInt8.self, capacity: w * h * 4)
        var count = 0
        for i in 0..<(w * h) where bytes[i * 4 + 3] > 16 { count += 1 }
        return count
    }

    /// The object the layer is showing. A later blank raster is a different object.
    private func plant(_ node: NodeView) throws -> AnyObject {
        #if os(macOS)
        let surface = try XCTUnwrap(IOSurface(properties: [.width: 8, .height: 8, .bytesPerElement: 4, .pixelFormat: 0x4247_5241]))
        node.textRaster = surface
        node.textRasterFrame = CGRect(origin: .zero, size: node.bounds.size)
        node.textRasterScale = 2
        node.textRasterReady = true
        node.presentTextRaster()
        XCTAssertTrue((node.layer?.contents as AnyObject?) === surface, "the planted surface is what the layer shows")
        return surface
        #else
        guard let ctx = CGContext(data: nil, width: 4, height: 4, bitsPerComponent: 8, bytesPerRow: 16,
                                  space: CGColorSpaceCreateDeviceRGB(),
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
            XCTFail("no raster context")
            return NSObject()
        }
        ctx.setFillColor(CGColor(srgbRed: 1, green: 0, blue: 0, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: 4, height: 4))
        let image = try XCTUnwrap(ctx.makeImage())
        node.textRaster = image
        node.textRasterReady = true
        node.textRasterFrame = node.bounds
        let ink = CALayer()
        ink.frame = node.bounds
        ink.contents = image
        node.textRasterLayer = ink
        node.layer.addSublayer(ink)
        XCTAssertNotNil(node.textRasterLayer?.superlayer)
        return image
        #endif
    }

    private func stillShows(_ node: NodeView, _ planted: AnyObject) -> Bool {
        #if os(macOS)
        if (node.layer?.contents as AnyObject?) === planted { return true }
        if (node.textRaster as AnyObject?) === planted { return true }
        return (node.textRasterOverflowLayer?.contents as AnyObject?) === planted
        #else
        if node.textRaster === (planted as! CGImage) { return true }
        return (node.textRasterLayer?.contents as AnyObject?) === planted
        #endif
    }

    func testAHiddenParagraphDropsTheRasterItWasShowing() throws {
        let p = presenter("hidden-raster")
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "text", "props": ["text": "Hidden glyphs"], "style": ["font_size": 28]],
            ["op": "roots", "ids": [1]],
            // Outside the scrollport and the raster reach, so the batch's text
            // refresh does not paint a replacement over the planted surface.
            ["op": "frame", "id": 1, "x": 0.0, "y": 4000.0, "w": 400.0, "h": 80.0],
        ]))
        let node = try XCTUnwrap(p.views[1])
        XCTAssertFalse(node.isHidden)
        XCTAssertFalse(p.textIsVisible(node), "the paragraph sits outside the scrollport")
        let planted = try plant(node)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["font_size": 28, "visibility": "hidden"]]]))
        XCTAssertFalse(stillShows(node, planted), "visibility: hidden leaves the previous text bitmap up")
        #if os(macOS)
        node.presentTextRaster()
        XCTAssertFalse(stillShows(node, planted), "presenting again must not put the old surface back")
        #endif
        XCTAssertEqual(ink(node), 0, "a fully hidden paragraph draws no glyphs")
    }

    func testAVisibleRunIsDrawnInsteadOfThePreviousBitmap() throws {
        let p = presenter("hidden-raster-run")
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "text", "style": ["font_size": 32]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 420.0, "h": 80.0],
        ]))
        p.apply(wire(#"{"ops":[{"op":"paragraph","id":1,"runs":[{"id":2,"parent":1,"paint":true,"props":{"text":"Secret"},"style":{"font_size":32,"visibility":"hidden"}},{"id":3,"parent":1,"paint":true,"props":{"text":"Shown"},"style":{"font_size":32,"visibility":"visible"}}]}]}"#))
        let node = try XCTUnwrap(p.views[1])
        XCTAssertEqual(node.paragraphSpec().runs.map(\.hidden), [true, false], "the host's run visibility reaches the paragraph")
        let planted = try plant(node)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["font_size": 32, "visibility": "hidden"]]]))
        XCTAssertFalse(stillShows(node, planted), "a hidden paragraph with a visible run still shows the old bitmap")
        #if os(macOS)
        XCTAssertFalse(node.rastersText, "the visible run draws; it is not a new raster of the old glyphs")
        #else
        XCTAssertFalse(node.canRasterText, "the visible run draws; it is not a new raster of the old glyphs")
        #endif
        XCTAssertGreaterThan(ink(node), 0, "the visible run still paints")
    }
}

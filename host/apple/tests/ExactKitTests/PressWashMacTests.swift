#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit
@testable import ExactSurfaces

/// The pressed look's wash (`PressLookMac.swift`) is masked by a picture of
/// the node. AppKit's backing stores belong to the views that drew them: a
/// committed layer that shares one leaves the view showing nothing once the
/// share is gone and the view moves (T3 Code's "Check account" lost its
/// label after a press that put a spinner before it).
@MainActor
final class PressWashMacTests: XCTestCase {
    private var window: NSWindow!
    private var session: ExactSession?

    override func tearDown() { session?.destroy(); session = nil; window?.orderOut(nil); window = nil; super.tearDown() }

    /// A pressable button (1): rounded, bordered, filled, with a label (2)
    /// in black on white, 18 pt in a 20 pt line so its ink overflows the
    /// line box and AppKit draws it on a content layer of its own.
    private func fixture(zoom: Double = 1) -> Presenter {
        _ = NSApplication.shared
        // A session's presenter, whose text engine lays out and draws the label.
        let session = ExactApp.shared.makeSession(label: "press-wash")
        self.session = session
        let p = session.presenter
        if zoom != 1 { session.pageZoom = zoom }
        window = NSWindow(contentRect: NSRect(x: 40, y: 40, width: 240, height: 80), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.backgroundColor = .white
        window.appearance = NSAppearance(named: .aqua)
        window.contentView = p.viewport
        var button: [String: Any] = ["background_color": [255.0, 255.0, 255.0, 255.0], "border_width": 1.0, "border_color_top": [212.0, 212.0, 216.0, 255.0]]
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] { button["border_radius_" + corner] = 8.0 }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["testId": "check"], "style": button],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Check account"],
             "style": ["font_size": 18.0, "line_height": "20px", "text_color": [0.0, 0.0, 0.0, 255.0], "white_space": "nowrap"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 20.0, "y": 20.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 150.0, "h": 20.0],
        ]))
        p.views[1]?.handlers = ["press"]
        return p
    }

    private func settle() {
        window.displayIfNeeded()
        CATransaction.flush()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.2))
    }

    /// The window server's picture of the window; nil where none answers
    /// (a process may read its own windows without permission).
    private func onScreen() -> CGImage? {
        typealias Create = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?
        guard let sym = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "CGWindowListCreateImage"),
              let image = unsafeBitCast(sym, to: Create.self)(.null, 1 << 3, UInt32(window.windowNumber), 1 << 0)?.takeRetainedValue(),
              !Agent.emptyPicture(image) else { return nil }
        return image
    }

    /// How many pixels of a window rectangle (points, top-left origin) are
    /// near black: the label's ink.
    private func ink(_ image: CGImage, in rect: CGRect) -> Int {
        let scale = CGFloat(image.width) / window.frame.width
        let box = CGRect(x: rect.minX * scale, y: rect.minY * scale, width: rect.width * scale, height: rect.height * scale).integral
        guard let crop = image.cropping(to: box) else { return 0 }
        let w = crop.width, h = crop.height
        var pixels = [UInt8](repeating: 0, count: w * h * 4)
        let ctx = CGContext(data: &pixels, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.draw(crop, in: CGRect(x: 0, y: 0, width: w, height: h))
        return stride(from: 0, to: pixels.count, by: 4).filter { CapturePixels.painted(pixels, at: $0) && pixels[$0] < 96 && pixels[$0 + 1] < 96 && pixels[$0 + 2] < 96 }.count
    }

    /// Every `contents` object of a layer tree.
    private func contents(_ layer: CALayer) -> [ObjectIdentifier] {
        (layer.contents.map { [ObjectIdentifier($0 as AnyObject)] } ?? []) + (layer.mask.map(contents) ?? []) + (layer.sublayers ?? []).flatMap(contents)
    }

    /// The mask is a picture of its own: it holds none of the backing stores
    /// the node's views show.
    func testTheWashSharesNoBackingStoreWithTheNode() throws {
        let p = fixture()
        window.orderFrontRegardless()
        settle()
        let button = try XCTUnwrap(p.views[1]), layer = try XCTUnwrap(button.layer)
        let live = Set(contents(layer))
        XCTAssertFalse(live.isEmpty, "the label's view has drawn")
        button.pressed = true
        let wash = try XCTUnwrap(layer.sublayers?.first { $0.name == "exact.pressWash" })
        let mask = try XCTUnwrap(wash.mask)
        XCTAssertTrue(Set(contents(mask)).isDisjoint(with: live), "the mask shares a view's backing store")
        button.pressed = false
    }

    func testRepeatedCanvasCapturesKeepTheLiveLabelAfterItMoves() throws {
        ExactSurfaces.install()
        let p = fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 3, "kind": "canvas"],
            ["op": "children", "id": 3, "ids": [1]],
            ["op": "roots", "ids": [3]],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 240, "h": 80],
        ]))
        window.orderFrontRegardless()
        settle()
        let button = try XCTUnwrap(p.views[1])
        let text = try XCTUnwrap(p.views[2])
        XCTAssertNotNil(text.canvasAbove)
        XCTAssertNotNil(text.textRaster, "even small canvas labels need independently owned pixels for capture")
        let label = CGRect(x: 30, y: 30, width: 150, height: 20)
        guard let before = onScreen() else { throw XCTSkip("no window server picture") }
        let originalInk = ink(before, in: label)
        XCTAssertGreaterThan(originalInk, 40)
        for step in 0..<6 {
            autoreleasepool {
                for _ in 0..<3 {
                    XCTAssertNotNil(Capture.bitmap(of: button, scale: window.backingScaleFactor))
                }
                settle()
            }
            let dx = CGFloat(step.isMultiple(of: 2) ? 18 : 0)
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 10 + dx, "y": 10.0, "w": 150.0, "h": 20.0]]))
            settle()
            let shown = try XCTUnwrap(onScreen())
            XCTAssertGreaterThan(ink(shown, in: label.offsetBy(dx: dx, dy: 0)), originalInk / 2,
                                 "capture \(step) must not take the live label's backing store")
        }
    }

    func testOffscreenDrawingRestoresOwnedCanvasTextWithoutAnotherEdit() throws {
        ExactSurfaces.install()
        let p = fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 3, "kind": "canvas"],
            ["op": "children", "id": 3, "ids": [1]],
            ["op": "roots", "ids": [3]],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 240, "h": 80],
            ["op": "style", "id": 2, "style": ["font_size": 52.0, "line_height": 0.6, "text_color": [0, 0, 0, 255]]],
            ["op": "frame", "id": 2, "x": 10, "y": 10, "w": 150, "h": 31.2],
        ]))
        window.orderFrontRegardless()
        settle()
        let text = try XCTUnwrap(p.views[2])
        XCTAssertNotNil(text.textRasterOverflowLayer)
        XCTAssertFalse(Capture.capturing)
        let bitmap = try XCTUnwrap(text.bitmapImageRepForCachingDisplay(in: text.bounds))
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
        text.draw(text.bounds)
        NSGraphicsContext.restoreGraphicsState()
        XCTAssertNil(text.textRaster, "offscreen drawing temporarily uses direct paint")
        let deadline = Date(timeIntervalSinceNow: 2)
        while text.textRaster == nil, Date() < deadline {
            RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01))
        }
        XCTAssertNotNil(text.textRaster, "the publication pump restores pixels without another edit")
        XCTAssertNotNil(text.textRasterOverflowLayer?.contents)
    }

    /// The red channel at a window point (points, top-left origin).
    private func red(_ image: CGImage, _ x: CGFloat, _ y: CGFloat) -> UInt8 {
        let scale = CGFloat(image.width) / window.frame.width
        guard let crop = image.cropping(to: CGRect(x: (x * scale).rounded(), y: (y * scale).rounded(), width: 1, height: 1)) else { return 0 }
        var pixel = [UInt8](repeating: 0, count: 4)
        let ctx = CGContext(data: &pixel, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
                            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.draw(crop, in: CGRect(x: 0, y: 0, width: 1, height: 1))
        XCTAssertTrue(CapturePixels.painted(pixel, at: 0), "the sampled colour is painted")
        return pixel[0]
    }

    /// On screen the wash tints what the node paints, the right way up: an
    /// unfilled pressable whose top half is a grey child darkens there and
    /// not below it.
    func testTheWashTintsWhatTheNodePaintsUpright() throws {
        let p = fixture()
        p.apply(wireBatch([
            ["op": "style", "id": 1, "style": [:]],
            ["op": "create", "id": 3, "kind": "view", "style": ["background_color": [128.0, 128.0, 128.0, 255.0]]],
            ["op": "children", "id": 1, "ids": [3]],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 200.0, "h": 20.0],
        ]))
        window.orderFrontRegardless()
        settle()
        let button = try XCTUnwrap(p.views[1])
        button.pressed = true
        settle()
        guard let pressed = onScreen() else { throw XCTSkip("no window server picture") }
        button.pressed = false
        // The child's middle, (20 + 100, 20 + 10); below it, (120, 50).
        XCTAssertLessThan(red(pressed, 120, 30), 120, "the grey child is tinted")
        XCTAssertGreaterThan(red(pressed, 120, 50), 245, "below it the node paints nothing, and nothing is tinted")
    }

    /// Pressed, released, then a child put before the label, which moves
    /// it: the window still shows the label.
    func testTheLabelStaysDrawnAfterAPressAndAMove() throws {
        let p = fixture()
        window.orderFrontRegardless()
        settle()
        let button = try XCTUnwrap(p.views[1])
        // The label's line, in the window: (20 + 10, 20 + 10), 150 × 20.
        let label = CGRect(x: 30, y: 30, width: 150, height: 20), moved = label.offsetBy(dx: 18, dy: 0)
        guard let before = onScreen() else { throw XCTSkip("no window server picture") }
        XCTAssertGreaterThan(ink(before, in: label), 40, "the label shows before the press")
        button.pressed = true
        settle()
        button.pressed = false
        settle()
        p.apply(wireBatch([
            ["op": "create", "id": 3, "kind": "view", "style": ["background_color": [200.0, 200.0, 200.0, 255.0]]],
            ["op": "children", "id": 1, "ids": [3, 2]],
            ["op": "frame", "id": 3, "x": 10.0, "y": 14.0, "w": 12.0, "h": 12.0],
            ["op": "frame", "id": 2, "x": 28.0, "y": 10.0, "w": 150.0, "h": 20.0],
        ]))
        settle()
        let after = try XCTUnwrap(onScreen())
        XCTAssertGreaterThan(ink(after, in: moved), 40, "the label shows after the press and the move")
    }

    /// The copy the wash's picture is drawn from shares none of AppKit's
    /// backing stores either, and draws the label's ink where the window
    /// shows it, at Actual Size and under a page zoom (a 2.4 device scale).
    /// At 120% a sidebar row's project name went blank after a press once a
    /// copy shared its backing store, until the row was laid out again.
    func testTheWashCopyDrawsWhatAppKitDrewAgain() throws {
        for zoom in [1, 1.2] {
            session?.destroy(); window?.orderOut(nil)
            let p = fixture(zoom: zoom)
            // Small enough that AppKit draws it at both scales, not a raster (`textIsSmall`).
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 100.0, "h": 20.0]]))
            window.orderFrontRegardless()
            settle()
            let button = try XCTUnwrap(p.views[1]), layer = try XCTUnwrap(button.layer)
            let text = try XCTUnwrap(p.views[2]), textLayer = try XCTUnwrap(text.layer)
            XCTAssertTrue(Capture.borrowsBackingStore(textLayer) || textLayer.sublayers?.contains(where: Capture.borrowsBackingStore) == true,
                          "AppKit draws the label: the case this covers")
            let scale = try XCTUnwrap(button.pixelScale)
            let tree = Capture.paintOrderLayer(of: button, root: layer, redraw: scale)
            defer { Capture.releasePaintCopy(tree) }
            XCTAssertTrue(Set(contents(tree)).isDisjoint(with: Set(stores(layer))), "at \(zoom), the copy shares a view's backing store")
            // The label's ink rows in the copy's picture and on screen, in device pixels of the button's box.
            let picture = try XCTUnwrap(Capture.picture(of: tree, size: layer.bounds.size, scale: scale))
            guard let shown = onScreen() else { throw XCTSkip("no window server picture") }
            let box = button.convert(button.bounds, to: nil)
            let onScreenBox = CGRect(x: box.minX, y: window.frame.height - box.maxY, width: box.width, height: box.height)
            let copiedInk = try XCTUnwrap(inkRows(picture, in: CGRect(origin: .zero, size: layer.bounds.size), scale: scale), "the copy draws the label")
            let shownInk = try XCTUnwrap(inkRows(shown, in: onScreenBox, scale: CGFloat(shown.width) / window.frame.width), "the window draws the label")
            XCTAssertEqual(copiedInk, shownInk, "at \(zoom)")
        }
    }

    /// Every backing store AppKit drew into in a layer tree.
    private func stores(_ layer: CALayer) -> [ObjectIdentifier] {
        (Capture.borrowsBackingStore(layer) ? [ObjectIdentifier(layer.contents! as AnyObject)] : []) + (layer.sublayers ?? []).flatMap(stores)
    }

    /// The first and last rows of near-black ink in a rectangle of an image
    /// (points, top-left origin), counted from the rectangle's top.
    private func inkRows(_ image: CGImage, in rect: CGRect, scale: CGFloat) -> ClosedRange<Int>? {
        let box = CGRect(x: rect.minX * scale, y: rect.minY * scale, width: rect.width * scale, height: rect.height * scale).integral
        guard let crop = image.cropping(to: box) else { return nil }
        let w = crop.width, h = crop.height
        var pixels = [UInt8](repeating: 0, count: w * h * 4)
        let ctx = CGContext(data: &pixels, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.draw(crop, in: CGRect(x: 0, y: 0, width: w, height: h))
        let rows = (0..<h).filter { y in (0..<w).contains { x in let i = (y * w + x) * 4; return pixels[i + 3] > 200 && pixels[i] < 96 && pixels[i + 1] < 96 && pixels[i + 2] < 96 } }
        guard let first = rows.first, let last = rows.last else { return nil }
        return first...last
    }
}
#endif

#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

private var capturedPaintPixels: [[UInt8]] = []

final class PaintCaptureMacTests: XCTestCase {
    func testAgentCaptureKeepsPlacementsOnTheirUploadedChildren() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "capture-placements")
        defer { session.destroy() }
        let p = session.presenter
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 100, height: 100),
                              styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        defer { window.close() }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "canvas"],
            ["op": "create", "id": 3, "kind": "view", "props": ["testId": "hud"],
             "style": ["background_color": [0, 0, 255, 255]]],
            ["op": "create", "id": 4, "kind": "view", "props": ["testId": "placed"],
             "style": ["background_color": [255, 0, 0, 255]]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3, 4]], ["op": "roots", "ids": [1]],
            ["op": "rank", "id": 3, "rank": 2],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 100],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 100, "h": 100],
            ["op": "frame", "id": 3, "x": 25, "y": 25, "w": 50, "h": 50],
            ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 20, "h": 20],
        ]))
        let canvas = try XCTUnwrap(p.views[2]), hud = try XCTUnwrap(p.views[3]), placed = try XCTUnwrap(p.views[4])
        let module = GpuModule(create: { _, _, _, _, _ in 7 }, bind: { _, _, _ in 0 },
            render: { _, _, _, _, _ in 1 }, dirty: { _ in 0 }, destroy: { _ in },
            texture: { _, _, _, _, _ in 0 }, textureMetal: nil, sync: nil,
            childrenMode: { _ in 2 }, readback: { _, _, _, _, _, bytes, length in
                if let bytes { for i in 0..<length { bytes[i] = i % 4 == 1 || i % 4 == 3 ? 255 : 0 } }
                return 0
            }, child: { _, _, _, _, _, _, _, _, _, _, _, _ in 0 }, childrenCount: { _, _ in 0 },
            placement: { _, index, out, _ in
                guard index == 1, let out else { return 0 }
                for (i, value) in [Float(1), 0, 0, 0, 1, 0, 0, 0, 1, 0].enumerated() { out[i] = value }
                return 1
            }, shader: nil, validateShader: nil, clearShaders: nil, errorLen: { 0 }, errorPtr: { nil },
            wantsInput: nil, input: nil, messages: nil, published: nil, agent: nil, outPtr: nil)
        let entry = Canvases.Entry(view: canvas, name: "paint", values: [])
        entry.id = 7; entry.module = module; entry.each = true; entry.through = true
        session.canvases.entries[2] = entry
        window.orderFront(nil)
        canvas.needsCapture = true
        session.canvases.captureIfNeeded()
        _ = session.canvases.readback(view: canvas)
        XCTAssertNil(hud.placement); XCTAssertNotNil(placed.placement)
        window.displayIfNeeded(); CATransaction.flush()
        for _ in 0..<2 {
            // The HUD's higher paint rank makes cacheDisplay temporarily
            // reorder these live views; readback still reports upload indices.
            let rep = try XCTUnwrap(Capture.picture(of: p.viewport))
            let color = try XCTUnwrap(rep.colorAt(x: rep.pixelsWide / 2, y: rep.pixelsHigh / 2))
            XCTAssertGreaterThan(color.blueComponent, 0.99, "ordinary HUD still covers the GPU picture")
            XCTAssertLessThan(color.greenComponent, 0.01)
            XCTAssertNil(hud.placement); XCTAssertEqual(hud.alphaValue, 1)
            XCTAssertNotNil(placed.placement); XCTAssertEqual(placed.alphaValue, 0)
            XCTAssertEqual(canvas.overlay?.subviews.compactMap { ($0 as? NodeView)?.id }, [3, 4])
        }
    }

    func testCanvasTextCaptureKeepsAppKitGlyphOrientation() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "capture-orientation")
        defer { session.destroy() }
        let p = session.presenter
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 320, height: 80),
                              styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        defer { window.close() }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "canvas"],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Friendly F · player 42"],
             "style": ["font_size": 22, "text_color": [255, 255, 255, 255]]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 320, "h": 80],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 320, "h": 80],
            ["op": "frame", "id": 3, "x": 12, "y": 17, "w": 280, "h": 30],
        ]))
        let overlay = try XCTUnwrap(p.views[2]?.overlay)
        let label = NSTextField(labelWithString: "Native field Fpq")
        label.frame = NSRect(x: 12, y: 50, width: 280, height: 26)
        label.font = .systemFont(ofSize: 19); label.textColor = .white
        overlay.addSubview(label)
        let image = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 2, pixelsHigh: 2,
            bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
            colorSpaceName: .deviceRGB, bytesPerRow: 8, bitsPerPixel: 32))
        for x in 0..<2 { image.setColor(.red, atX: x, y: 0); image.setColor(.blue, atX: x, y: 1) }
        let imageView = NSImageView(frame: NSRect(x: 260, y: 50, width: 26, height: 26))
        imageView.image = NSImage(cgImage: try XCTUnwrap(image.cgImage), size: NSSize(width: 26, height: 26))
        overlay.addSubview(imageView)
        let decoratedImage = CALayer()
        decoratedImage.frame = CGRect(x: 220, y: 50, width: 26, height: 26)
        decoratedImage.contents = image.cgImage
        decoratedImage.borderColor = NSColor.green.cgColor; decoratedImage.borderWidth = 2
        overlay.layer?.addSublayer(decoratedImage)
        window.orderFront(nil)
        window.displayIfNeeded(); CATransaction.flush()
        // Both a whole HUD texture and one placed child use this capture.
        for view in [overlay, try XCTUnwrap(p.views[3]), label] as [NSView] {
            let reference = try XCTUnwrap(Capture.picture(of: view))
            let captured = try XCTUnwrap(Capture.bitmap(of: view, scale: window.backingScaleFactor))
            XCTAssertEqual(reference.pixelsWide, captured.pixelsWide)
            XCTAssertEqual(reference.pixelsHigh, captured.pixelsHigh)
            let width = reference.pixelsWide, height = reference.pixelsHigh
            func mask(_ rep: NSBitmapImageRep) -> [Bool] {
                (0..<(width * height)).map { (rep.colorAt(x: $0 % width, y: $0 / width)?.alphaComponent ?? 0) > 0.5 }
            }
            let expected = mask(reference), actual = mask(captured)
            // CoreText's direct and cached glyph edges can differ by one
            // physical pixel. Match ink in both directions within that edge;
            // a vertically inverted letter cannot pass this comparison.
            func missed(_ from: [Bool], _ to: [Bool]) -> Int {
                from.indices.filter { from[$0] && !to[$0] }.filter { index in
                    let x = index % width, y = index / width
                    return !(max(0, y - 1)...min(height - 1, y + 1)).contains { yy in
                        (max(0, x - 1)...min(width - 1, x + 1)).contains { to[yy * width + $0] }
                    }
                }.count
            }
            let ink = expected.filter { $0 }.count + actual.filter { $0 }.count
            let missing = missed(expected, actual) + missed(actual, expected)
            XCTAssertGreaterThan(ink, 200, "the comparison contains glyphs")
            XCTAssertLessThan(Double(missing) / Double(max(1, ink)), 0.01,
                              "\(type(of: view)): glyph orientation and position match AppKit within one physical pixel")
            if view === overlay {
                for point in [CGPoint(x: 270, y: 54), CGPoint(x: 270, y: 72),
                              CGPoint(x: 233, y: 54), CGPoint(x: 233, y: 72), CGPoint(x: 221, y: 63)] {
                    let x = Int(point.x * window.backingScaleFactor), yy = Int(point.y * window.backingScaleFactor)
                    let a = try XCTUnwrap(reference.colorAt(x: x, y: yy)?.usingColorSpace(.sRGB))
                    let b = try XCTUnwrap(captured.colorAt(x: x, y: yy)?.usingColorSpace(.sRGB))
                    XCTAssertEqual(a.redComponent, b.redComponent, accuracy: 0.01)
                    XCTAssertEqual(a.greenComponent, b.greenComponent, accuracy: 0.01)
                    XCTAssertEqual(a.blueComponent, b.blueComponent, accuracy: 0.01)
                }
            }
        }
    }

    func testCanvasCapturesRanksFromTheCurrentBatch() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "paint-capture")
        defer { session.destroy() }
        let p = session.presenter
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 100, height: 100),
                              styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        defer { window.close() }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "canvas"],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "create", "id": 4, "kind": "view", "style": ["background_color": [255, 0, 0, 255]]],
            ["op": "create", "id": 5, "kind": "view", "style": ["background_color": [0, 0, 255, 255]]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 3, "ids": [4, 5]], ["op": "roots", "ids": [1]],
        ] + (1...5).map { ["op": "frame", "id": $0, "x": 0, "y": 0, "w": 100, "h": 100] }))
        let canvas = try XCTUnwrap(p.views[2])
        let module = GpuModule(create: { _, _, _, _, _ in 7 }, bind: { _, _, _ in 0 },
            render: { _, _, _, _, _ in 1 }, dirty: { _ in 0 }, destroy: { _ in },
            texture: { _, width, height, bytes, _ in
                if let bytes {
                    let index = (Int(height / 2) * Int(width) + Int(width / 2)) * 4
                    capturedPaintPixels.append(Array(UnsafeBufferPointer(start: bytes + index, count: 4)))
                }
                return 0
            }, textureMetal: nil, sync: nil,
            childrenMode: { _ in 1 }, readback: { _, _, _, _, _, _, _ in 0 },
            child: { _, _, _, _, _, _, _, _, _, _, _, _ in 0 }, childrenCount: { _, _ in 0 }, placement: { _, _, _, _ in 0 },
            shader: nil, validateShader: nil, clearShaders: nil, errorLen: { 0 }, errorPtr: { nil },
            wantsInput: nil, input: nil, messages: nil, published: nil, agent: nil, outPtr: nil)
        let entry = Canvases.Entry(view: canvas, name: "paint", values: [])
        entry.id = 7; entry.module = module; entry.through = true
        session.canvases.entries[2] = entry
        capturedPaintPixels = []
        canvas.needsCapture = true
        session.canvases.captureIfNeeded()
        XCTAssertEqual(capturedPaintPixels, [[0, 0, 255, 255]])
        // Capture must see pending ranks even inside a nested transaction.
        PaintOrder.begin()
        defer { PaintOrder.end() }
        p.apply(wireBatch([["op": "rank", "id": 4, "rank": 1]]))
        XCTAssertEqual(capturedPaintPixels.count, 2)
        XCTAssertEqual(capturedPaintPixels.last, [255, 0, 0, 255], "the same batch uploads the new front sibling")
        XCTAssertFalse(canvas.needsCapture, "no later recapture is owed")
        p.apply(wireBatch([["op": "rank", "id": 4, "rank": -2]]))
        XCTAssertEqual(capturedPaintPixels.last, [0, 0, 255, 255])
        p.apply(wireBatch([["op": "rank", "id": 4, "rank": 10]]))
        XCTAssertEqual(capturedPaintPixels.last, [255, 0, 0, 255])
        XCTAssertEqual(capturedPaintPixels.count, 4)
    }

    func testCaptureSamplesAnimatedShapeOpacityTransformAndMask() throws {
        _ = NSApplication.shared
        let root = NSView(frame: NSRect(x: 0, y: 0, width: 100, height: 100))
        root.wantsLayer = true
        let window = NSWindow(contentRect: root.bounds, styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = root
        defer { window.close() }
        let backing = try XCTUnwrap(root.layer)
        backing.backgroundColor = NSColor.white.cgColor
        func frozen(_ layer: CALayer, _ key: String, from: Any, to: Any) {
            let animation = CABasicAnimation(keyPath: key)
            animation.fromValue = from; animation.toValue = to
            animation.duration = 10; animation.speed = 0; animation.timeOffset = 5
            animation.fillMode = .both; animation.isRemovedOnCompletion = false
            layer.add(animation, forKey: key)
        }
        let stroke = CAShapeLayer()
        let path = CGMutablePath(); path.move(to: CGPoint(x: 10, y: 20)); path.addLine(to: CGPoint(x: 90, y: 20))
        stroke.path = path; stroke.strokeColor = NSColor.red.cgColor; stroke.lineWidth = 10
        backing.addSublayer(stroke)
        frozen(stroke, "strokeEnd", from: 0, to: 1)
        let faded = CALayer()
        faded.frame = CGRect(x: 10, y: 40, width: 20, height: 20); faded.backgroundColor = NSColor.blue.cgColor
        backing.addSublayer(faded)
        frozen(faded, "opacity", from: 0, to: 1)
        let moved = CALayer()
        moved.frame = CGRect(x: 10, y: 70, width: 20, height: 20); moved.backgroundColor = NSColor.red.cgColor
        backing.addSublayer(moved)
        frozen(moved, "transform", from: NSValue(caTransform3D: CATransform3DIdentity),
               to: NSValue(caTransform3D: CATransform3DMakeTranslation(80, 0, 0)))
        let masked = CALayer()
        masked.frame = CGRect(x: 70, y: 40, width: 20, height: 20); masked.backgroundColor = NSColor.red.cgColor
        let mask = CALayer(); mask.frame = masked.bounds; mask.backgroundColor = NSColor.black.cgColor
        masked.mask = mask; backing.addSublayer(masked)
        frozen(mask, "opacity", from: 0, to: 1)
        window.orderFront(nil)
        root.displayIfNeeded(); CATransaction.flush()
        let deadline = Date().addingTimeInterval(1)
        while stroke.presentation() == nil && Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.01)) }
        XCTAssertEqual(try XCTUnwrap(stroke.presentation()).strokeEnd, 0.5, accuracy: 0.01)
        XCTAssertEqual(try XCTUnwrap(faded.presentation()).opacity, 0.5, accuracy: 0.01)
        XCTAssertEqual(try XCTUnwrap(moved.presentation()).transform.m41, 40, accuracy: 0.01)
        XCTAssertEqual(try XCTUnwrap(mask.presentation()).opacity, 0.5, accuracy: 0.01)
        let rep = try XCTUnwrap(Capture.paintOrderBitmap(of: root))
        func pixel(_ x: Int, _ y: Int) throws -> NSColor {
            try XCTUnwrap(rep.colorAt(x: x * rep.pixelsWide / 100, y: y * rep.pixelsHigh / 100))
        }
        XCTAssertLessThan(try pixel(20, 20).greenComponent, 0.05)
        XCTAssertGreaterThan(try pixel(80, 20).greenComponent, 0.95, "the stroke is only half drawn")
        XCTAssertEqual(try pixel(20, 50).redComponent, 0.5, accuracy: 0.02, "presented opacity")
        XCTAssertGreaterThan(try pixel(20, 80).greenComponent, 0.95, "the model transform is not drawn")
        XCTAssertLessThan(try pixel(60, 80).greenComponent, 0.05, "presented transform")
        XCTAssertEqual(try pixel(80, 50).greenComponent, 0.5, accuracy: 0.02, "presented mask")
        XCTAssertEqual(stroke.strokeEnd, 1); XCTAssertEqual(faded.opacity, 1)
        XCTAssertEqual(moved.transform.m41, 0); XCTAssertEqual(mask.opacity, 1)
        XCTAssertTrue(stroke.superlayer === backing)
    }

    func testRanksNegativeChildrenAndOpacityAreComposedOnce() throws {
        _ = NSApplication.shared
        let p = Presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": [255, 255, 255, 255]]],
            ["op": "create", "id": 2, "kind": "view", "style": ["background_color": [255, 0, 0, 255]]],
            ["op": "create", "id": 3, "kind": "view", "style": ["background_color": [0, 0, 255, 255]]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 80, "h": 80],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 80, "h": 80],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 80, "h": 80],
            ["op": "rank", "id": 2, "rank": 1],
        ]))
        let root = try XCTUnwrap(p.views[1])
        let window = NSWindow(contentRect: root.bounds, styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = root
        root.displayIfNeeded()
        let liveParent = p.views[2]?.layer?.superlayer
        func color() throws -> NSColor {
            let rep = try XCTUnwrap(Capture.paintOrderBitmap(of: root))
            return try XCTUnwrap(rep.colorAt(x: rep.pixelsWide / 2, y: rep.pixelsHigh / 2))
        }
        XCTAssertGreaterThan(try color().redComponent, 0.95)
        p.views[2]?.frame = CGRect(x: 0, y: 0, width: 80, height: 20)
        let oriented = try XCTUnwrap(Capture.paintOrderBitmap(of: root))
        XCTAssertGreaterThan(try XCTUnwrap(oriented.colorAt(x: 10, y: 10)).redComponent, 0.95)
        XCTAssertGreaterThan(try XCTUnwrap(oriented.colorAt(x: 10, y: oriented.pixelsHigh - 10)).blueComponent, 0.95)
        p.views[2]?.frame = root.bounds
        p.views[2]?.alphaValue = 0.5
        let mixed = try color()
        XCTAssertEqual(mixed.redComponent, 0.5, accuracy: 0.02)
        XCTAssertEqual(mixed.blueComponent, 0.5, accuracy: 0.02)
        p.views[2]?.setRank(-2)
        XCTAssertGreaterThan(try color().blueComponent, 0.95)
        p.views[3]?.isHidden = true
        let negative = try color()
        XCTAssertEqual(negative.redComponent, 1, accuracy: 0.02)
        XCTAssertEqual(negative.greenComponent, 0.5, accuracy: 0.02, "negative child still paints above its parent's background")
        XCTAssertEqual(root.subviews.compactMap { ($0 as? NodeView)?.id }, [2, 3])
        XCTAssertTrue(p.views[2]?.layer?.superlayer === liveParent, "capture leaves live layers in place")
    }

    func testCaptureOverlaysOnlyAnimatedPropertiesOnSameTurnModelWrites() throws {
        _ = NSApplication.shared
        let root = NSView(frame: NSRect(x: 0, y: 0, width: 100, height: 100))
        root.wantsLayer = true
        let window = NSWindow(contentRect: root.bounds, styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = root
        defer { window.close() }
        let backing = try XCTUnwrap(root.layer)
        backing.backgroundColor = NSColor.white.cgColor
        func animate(_ layer: CALayer, _ keyPath: String, grouped: Bool = false) {
            let animation = CABasicAnimation(keyPath: keyPath)
            animation.fromValue = 0; animation.toValue = 1; animation.duration = 10
            let frozen: CAAnimation
            if grouped {
                let group = CAAnimationGroup(); group.animations = [animation]; frozen = group
            } else { frozen = animation }
            frozen.duration = 10; frozen.speed = 0; frozen.timeOffset = 5
            frozen.fillMode = .both; frozen.isRemovedOnCompletion = false
            // An animation's storage key need not be its property's key path.
            layer.add(frozen, forKey: "capture-test")
        }
        func line(at y: CGFloat) -> CGPath {
            let path = CGMutablePath()
            path.move(to: CGPoint(x: 10, y: y)); path.addLine(to: CGPoint(x: 90, y: y))
            return path
        }
        let stroke = CAShapeLayer()
        stroke.path = line(at: 10); stroke.strokeColor = NSColor.red.cgColor; stroke.lineWidth = 10
        backing.addSublayer(stroke); animate(stroke, "strokeEnd")
        let faded = CALayer()
        faded.anchorPoint = .zero; faded.position = CGPoint(x: 10, y: 40)
        faded.bounds = CGRect(x: 0, y: 0, width: 10, height: 20)
        faded.backgroundColor = NSColor.red.cgColor
        backing.addSublayer(faded); animate(faded, "opacity", grouped: true)
        let masked = CALayer()
        masked.frame = CGRect(x: 10, y: 70, width: 80, height: 20)
        masked.backgroundColor = NSColor.blue.cgColor
        let mask = CAShapeLayer()
        mask.path = CGPath(rect: CGRect(x: 0, y: 0, width: 10, height: 20), transform: nil)
        masked.mask = mask; backing.addSublayer(masked); animate(mask, "opacity", grouped: true)
        window.orderFront(nil)
        root.displayIfNeeded(); CATransaction.flush()
        let deadline = Date().addingTimeInterval(1)
        while stroke.presentation() == nil && Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.01)) }
        XCTAssertEqual(try XCTUnwrap(stroke.presentation()).strokeEnd, 0.5, accuracy: 0.01)
        XCTAssertEqual(try XCTUnwrap(faded.presentation()).opacity, 0.5, accuracy: 0.01)
        XCTAssertEqual(try XCTUnwrap(mask.presentation()).opacity, 0.5, accuracy: 0.01)

        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        stroke.path = line(at: 25)
        faded.backgroundColor = NSColor.blue.cgColor
        faded.bounds.size.width = 80
        mask.path = CGPath(rect: masked.bounds, transform: nil)
        // Deliberately capture before committing: presentation still has the
        // old path, red fill and narrow bounds, but the active opacity/stroke
        // values must still come from it (also for a mask and animation group).
        let rep = try XCTUnwrap(Capture.paintOrderBitmap(of: root, scale: 1))
        func pixel(_ x: Int, _ y: Int) throws -> NSColor { try XCTUnwrap(rep.colorAt(x: x, y: y)) }
        XCTAssertGreaterThan(try pixel(20, 10).greenComponent, 0.95, "the old path is gone")
        XCTAssertLessThan(try pixel(20, 25).greenComponent, 0.05, "the fresh path is drawn")
        XCTAssertGreaterThan(try pixel(80, 25).greenComponent, 0.95, "strokeEnd stays half drawn")
        for y in [50, 80] {
            let color = try pixel(70, y)
            XCTAssertEqual(color.redComponent, 0.5, accuracy: 0.02, "fresh geometry with presented opacity at y=\(y)")
            XCTAssertEqual(color.greenComponent, 0.5, accuracy: 0.02)
            XCTAssertGreaterThan(color.blueComponent, 0.95, "fresh blue fill")
        }
        XCTAssertEqual(stroke.strokeEnd, 1); XCTAssertEqual(faded.opacity, 1); XCTAssertEqual(mask.opacity, 1)
        XCTAssertTrue(stroke.superlayer === backing); XCTAssertTrue(masked.mask === mask)
    }
}
#endif

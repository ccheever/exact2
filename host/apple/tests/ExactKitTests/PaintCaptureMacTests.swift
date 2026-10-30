#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit
@testable import ExactSurfaces

private var capturedPaintPixels: [[UInt8]] = []
private var capturedPaintRows: [(top: [UInt8], bottom: [UInt8])] = []

/// Red over blue, drawn into its backing store, not a layer's colour.
private final class DrawnHalves: NSView {
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        NSColor.red.setFill(); NSRect(x: 0, y: 0, width: bounds.width, height: bounds.height / 2).fill()
        NSColor.blue.setFill(); NSRect(x: 0, y: bounds.height / 2, width: bounds.width, height: bounds.height / 2).fill()
    }
}

final class PaintCaptureMacTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactSurfaces.install() } // LLP 1047.001 D4
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
        let entry = CanvasesHost.Entry(view: canvas, name: "paint", values: [])
        entry.id = 7; entry.module = module; entry.through = true
        session.surfaceHost.entries[2] = entry
        capturedPaintPixels = []
        canvas.needsCapture = true
        session.surfaceHost.captureIfNeeded()
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

    /// A canvas's children reach its surface top row first, as they show
    /// (Caltrain's sky, LLP 1014). In an app's window AppKit flips the page
    /// scroller's layer, and the layer copy the capture draws has no
    /// ancestors to carry that: what a view drew came out upside down.
    func testCanvasChildrenUploadTopRowFirst() throws { try canvasChildrenUpload(firstEach: false) }
    func testCanvasChildrenFirstUploadPerChild() throws { try canvasChildrenUpload(firstEach: true) }

    private func canvasChildrenUpload(firstEach: Bool) throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "paint-orientation")
        defer { session.destroy() }
        let p = session.presenter
        let window = CaptureDisplayWindow(contentRect: NSRect(x: 0, y: 0, width: 100, height: 100), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        // As an app's window holds it: in an unflipped, layer-backed view (`ExactView`).
        let content = NSView(frame: NSRect(x: 0, y: 0, width: 100, height: 100))
        content.wantsLayer = true
        window.contentView = content
        p.viewport.frame = content.bounds
        content.addSubview(p.viewport)
        for v: NSView? in [p.viewport, p.viewport.contentView, p.viewport.documentView] { v?.wantsLayer = true }
        defer { window.close() }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "canvas"],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3]], ["op": "roots", "ids": [1]],
        ] + (1...3).map { ["op": "frame", "id": $0, "x": 0, "y": 0, "w": 100, "h": 100] }))
        let canvas = try XCTUnwrap(p.views[2])
        if firstEach {
            window.orderFrontRegardless(); window.displayIfNeeded(); CATransaction.flush()
        }
        let drawn = DrawnHalves(frame: NSRect(x: 0, y: 0, width: 100, height: 100))
        drawn.wantsLayer = true
        let card = try XCTUnwrap(p.views[3])
        card.addSubview(drawn)
        let field = NSTextField(frame: NSRect(x: 0, y: 30, width: 80, height: 24))
        field.placeholderString = "Search"; card.addSubview(field)
        capturedPaintRows = []
        let module = GpuModule(create: { _, _, _, _, _ in 7 }, bind: { _, _, _ in 0 },
            render: { _, _, _, _, _ in 1 }, dirty: { _ in 0 }, destroy: { _ in },
            texture: { _, width, height, bytes, _ in
                if let bytes {
                    let pixel = { (y: Int) in Array(UnsafeBufferPointer(start: bytes + (y * Int(width) + Int(width / 2)) * 4, count: 4)) }
                    capturedPaintRows.append((pixel(Int(height) / 10), pixel(Int(height) * 9 / 10)))
                }
                return 0
            }, textureMetal: nil, sync: nil,
            childrenMode: { _ in 1 }, readback: { _, _, _, _, _, _, _ in 0 },
            child: { _, _, _, _, _, _, _, _, width, height, bytes, _ in
                if let bytes {
                    let pixel = { (y: Int) in Array(UnsafeBufferPointer(start: bytes + (y * Int(width) + Int(width / 2)) * 4, count: 4)) }
                    capturedPaintRows.append((pixel(Int(height) / 10), pixel(Int(height) * 9 / 10)))
                }
                return 0
            }, childrenCount: { _, _ in 0 }, placement: { _, _, _, _ in 0 },
            shader: nil, validateShader: nil, clearShaders: nil, errorLen: { 0 }, errorPtr: { nil },
            wantsInput: nil, input: nil, messages: nil, published: nil, agent: nil, outPtr: nil)
        let entry = CanvasesHost.Entry(view: canvas, name: "paint", values: [])
        entry.id = 7; entry.module = module; entry.through = true
        session.surfaceHost.entries[2] = entry
        if firstEach {
            entry.each = true
            XCTAssertNil(drawn.layer?.superlayer, "the first per-child upload starts with a detached backing layer")
            XCTAssertNil(field.layer, "the field has not inherited backing before the first per-child upload")
        } else {
            window.orderFrontRegardless(); content.displayIfNeeded(); window.display(); CATransaction.flush()
        }
        XCTAssertEqual(p.viewport.layer?.isGeometryFlipped, true, "AppKit flips the page scroller's layer in an unflipped window")
        window.displays = 0
        canvas.needsCapture = true
        session.surfaceHost.captureIfNeeded()
        XCTAssertEqual(window.displays, firstEach ? 1 : 0, "prepare the whole upload once, only for new layers")
        let uploaded = try XCTUnwrap(capturedPaintRows.last)
        XCTAssertEqual(uploaded.top, [255, 0, 0, 255], "the first row is the top half's")
        XCTAssertEqual(uploaded.bottom, [0, 0, 255, 255])
        entry.each = true; canvas.needsCapture = true
        session.surfaceHost.captureIfNeeded()
        XCTAssertEqual(capturedPaintRows.count, 2)
        XCTAssertEqual(window.displays, firstEach ? 1 : 0, "the second upload needs no window display")
        let child = try XCTUnwrap(capturedPaintRows.last)
        XCTAssertEqual(child.top, [255, 0, 0, 255], "per-child uploads have the same orientation")
        XCTAssertEqual(child.bottom, [0, 0, 255, 255])
    }

    func testBranchLookupWorkIsLinear() throws {
        _ = NSApplication.shared
        let root = NSView(frame: NSRect(x: 0, y: 0, width: 10, height: 10))
        root.wantsLayer = true
        for i in 0..<1_000 {
            let child = NSView(frame: NSRect(x: i, y: 0, width: 10, height: 10))
            child.wantsLayer = true
            root.addSubview(child)
        }
        var probes = 0
        let copy = Capture.paintOrderLayer(of: root, root: try XCTUnwrap(root.layer), branchProbe: { probes += 1 })
        XCTAssertEqual(copy.sublayers?.map { $0.position.x }, (0..<1_000).map { CGFloat($0) })
        XCTAssertLessThanOrEqual(probes, 1_000, "branch membership work must be linear in children")
    }

    func testIntermediateLayersKeepGeometryAndDrawOnlyOnce() throws {
        _ = NSApplication.shared
        let root = NSView(frame: NSRect(x: 0, y: 0, width: 100, height: 100))
        root.wantsLayer = true
        let window = NSWindow(contentRect: root.bounds, styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = root
        defer { window.close() }
        let presenter = Presenter()
        let drawn = NodeView(id: 1, kind: "view", presenter: presenter)
        drawn.frame = NSRect(x: 10, y: 20, width: 40, height: 40)
        drawn.wantsLayer = true
        root.addSubview(drawn)
        window.orderFront(nil); root.displayIfNeeded(); CATransaction.flush()
        let backing = try XCTUnwrap(root.layer), ink = try XCTUnwrap(drawn.layer)
        for (y, color) in [(0, NSColor.red), (20, NSColor.blue)] {
            let half = CALayer(); half.frame = CGRect(x: 0, y: y, width: 40, height: 20)
            half.backgroundColor = color.cgColor; ink.addSublayer(half)
        }
        let bridge = CALayer()
        bridge.frame = root.bounds; bridge.bounds.origin = CGPoint(x: 3, y: 7)
        bridge.transform = CATransform3DMakeTranslation(9, 11, 0)
        bridge.isGeometryFlipped = true; bridge.opacity = 0.5
        backing.addSublayer(bridge); bridge.addSublayer(ink)
        for filtered in [false, true] {
            if filtered {
                let shadow: [Double] = [0,0,0,0, 1,6,-1,-3, 0,0,0,0, 0,0,0,0,0, 1,0,0,1]
                drawn.applyStyle(["filter": ["p": .array(shadow.map(BatchValue.number))]])
                let filter = try XCTUnwrap(drawn.boxFilter)
                // Stand in for the GPU's result; this test checks composition,
                // independently of how BoxFilter produces its pixels.
                filter.picture.frame = ink.frame
                filter.picture.backgroundColor = NSColor.red.cgColor
                filter.picture.opacity = 0.5
                ink.mask = filter.hide
                bridge.insertSublayer(filter.picture, above: ink)
                bridge.opacity = 1
            }
            let original = NSBitmapImageRep(cgImage: try XCTUnwrap(Capture.picture(of: backing, size: root.bounds.size, scale: 1)))
            let copy = try XCTUnwrap(Capture.paintOrderBitmap(of: root, scale: 1))
            var colored = 0, difference: CGFloat = 0
            for y in 0..<100 {
                for x in 0..<100 {
                    let a = try XCTUnwrap(original.colorAt(x: x, y: y)), b = try XCTUnwrap(copy.colorAt(x: x, y: y))
                    difference = max(difference, abs(b.alphaComponent - a.alphaComponent))
                    if a.alphaComponent > 0.1 {
                        colored += 1
                        difference = max(difference, abs(b.redComponent - a.redComponent), abs(b.blueComponent - a.blueComponent))
                    }
                }
            }
            XCTAssertLessThan(difference, 0.01, "the copy keeps the bridge's flip, bounds, transform and opacity")
            XCTAssertEqual(colored, 1600, "the reference paints the whole 40-by-40 child")
            XCTAssertTrue(ink.superlayer === bridge, "capture leaves the live geometry in place")
            if filtered {
                let peak = (0..<100).flatMap { y in (0..<100).compactMap { x in copy.colorAt(x: x, y: y)?.alphaComponent } }.max()!
                XCTAssertEqual(peak, 0.5, accuracy: 0.01, "a filtered translucent child paints once")
            }
        }
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
        while stroke.presentation() == nil && Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
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
        window.displayIfNeeded(); root.displayIfNeeded()
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
        while stroke.presentation() == nil && Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
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

#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

private var capturedPaintPixels: [[UInt8]] = []

final class PaintCaptureMacTests: XCTestCase {
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
}
#endif

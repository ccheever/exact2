#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactSurfaces

private var capturedPaintPixels: [[UInt8]] = []

final class PaintCaptureIOSTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactSurfaces.install() } // LLP 1047.001 D4
    func testCanvasCapturesRanksFromTheCurrentBatch() throws {
        let session = ExactApp.shared.makeSession(label: "paint-capture")
        defer { session.destroy() }
        let p = session.presenter
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 100, height: 100))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport); window.makeKeyAndVisible()
        defer { window.isHidden = true }
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
        XCTAssertEqual(p.views[3]?.subviews.compactMap { ($0 as? NodeView)?.id }, [4, 5],
                       "capture leaves logical view order intact")
    }

    func testCPUComposesFlatLeavesNegativeRanksAndGroupOpacityOnce() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 100, height: 100))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport); window.makeKeyAndVisible()
        defer { window.isHidden = true }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": [255, 255, 255, 255]]],
            ["op": "create", "id": 2, "kind": "view", "props": ["testId": "red"],
             "style": ["background_color": [255, 0, 0, 255]]],
            ["op": "create", "id": 3, "kind": "view",
             "style": ["width": 100, "height": 100, "background_color": [0, 0, 255, 255]]],
            ["op": "children", "id": 1, "ids": [3, 2]], ["op": "roots", "ids": [1]],
            ["op": "rank", "id": 2, "rank": -2],
        ] + (1...3).map { ["op": "frame", "id": $0, "x": 0, "y": 0, "w": 100, "h": 100] }))
        let root = try XCTUnwrap(p.views[1])
        let red = try XCTUnwrap(p.views[2])
        red.alpha = 0.5
        XCTAssertNil(p.views[3], "the blue sibling is a flat leaf")
        let flat = try XCTUnwrap(root.layer.sublayers?.first { $0.flatHit != nil })
        let logicalLayers = root.layer.sublayers
        func pixel() throws -> [UInt8] {
            let bitmap = try XCTUnwrap(Bitmap.blank(width: 100, height: 100))
            Capture.draw(root, scale: 1, into: bitmap)
            let bytes = try XCTUnwrap(bitmap.bytes).assumingMemoryBound(to: UInt8.self)
            return Array(UnsafeBufferPointer(start: bytes + (50 * 100 + 50) * 4, count: 4))
        }
        XCTAssertEqual(try pixel(), [0, 0, 255, 255], "flat rank zero covers a later negative sibling")
        flat.isHidden = true
        let negative = try pixel()
        XCTAssertEqual(negative[0], 255)
        XCTAssertEqual(Int(negative[1]), 128, accuracy: 1, "negative child stays above its parent's fill")
        flat.isHidden = false
        red.setRank(1)
        let mixed = try pixel()
        XCTAssertEqual(Int(mixed[0]), 128, accuracy: 1)
        XCTAssertEqual(mixed[1], 0)
        XCTAssertEqual(Int(mixed[2]), 128, accuracy: 1, "group opacity is applied once")
        XCTAssertEqual(root.layer.sublayers, logicalLayers, "capture never rearranges live layers")
    }

}
#endif

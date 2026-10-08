#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactSurfaces

private var seenIds: Set<UInt32> = []

/// A canvas's `CAMetalLayer` is lent from the process's spare ones and given
/// back when its view goes (`MetalCanvasView`, `MetalLayerPool`; LLP 1068 §4.5):
/// the next canvas draws into the same layer's drawables instead of making
/// three more. The layer still holds the picture last presented to it, so it
/// stays hidden until the module says the new canvas's first frame is with
/// the compositor — the first pixels after a take are the new content's or
/// the box's background, never the old row's. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class CanvasLayerPoolIOSTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactSurfaces.install() } // LLP 1047.001 D4
    override func setUp() {
        MetalLayerPool.drain()
        seenIds = []
    }

    func testAViewGoneLendsItsLayerToTheNextWhichWaitsHidden() {
        var first: MetalCanvasView? = MetalCanvasView(frame: CGRect(x: 0, y: 0, width: 100, height: 60))
        let layer = first!.metalLayer
        XCTAssertFalse(layer.isHidden, "a new layer has no picture to hide")
        XCTAssertFalse(first!.awaitingFirstFrame)
        XCTAssertTrue(layer.superlayer === first!.layer)
        first = nil
        XCTAssertNil(layer.superlayer, "given back, out of the tree")
        XCTAssertEqual(MetalLayerPool.spares, 1)

        let second = MetalCanvasView(frame: CGRect(x: 0, y: 0, width: 100, height: 60))
        XCTAssertTrue(second.metalLayer === layer, "the same layer, its drawables with it")
        XCTAssertEqual(MetalLayerPool.spares, 0)
        XCTAssertTrue(layer.isHidden && second.awaitingFirstFrame, "hidden: it still holds the first canvas's picture")
        second.reveal()
        XCTAssertFalse(layer.isHidden || second.awaitingFirstFrame)
        second.reveal()
        XCTAssertFalse(layer.isHidden)
    }

    func testTheLayerFollowsItsViewsBoundsAndScale() {
        let view = MetalCanvasView(frame: CGRect(x: 0, y: 0, width: 100, height: 60))
        view.frame = CGRect(x: 0, y: 0, width: 240, height: 135)
        view.layoutIfNeeded()
        XCTAssertEqual(view.metalLayer.frame, view.bounds)
        XCTAssertNil(view.metalLayer.animation(forKey: "bounds"), "never animated: a resize is not a stretch")
    }

    func testNoMoreThanAFewSparesAreKept() {
        var views = (0..<8).map { _ in MetalCanvasView(frame: .zero) }
        let made = Set(views.map { ObjectIdentifier($0.metalLayer) })
        XCTAssertEqual(made.count, 8, "eight canvases alive are eight layers")
        views.removeAll()
        XCTAssertEqual(MetalLayerPool.spares, MetalLayerPool.keep)
    }

    /// Through the presenter's own canvases: the reused layer is revealed by
    /// the module's word about this canvas (`gpu_seen`), not by a render.
    func testAReusedLayerIsRevealedWhenTheModuleSaysItsFirstFrameIsShown() throws {
        var old: MetalCanvasView? = MetalCanvasView(frame: .zero)
        let layer = old!.metalLayer
        old = nil

        let s = ExactApp.shared.makeSession(label: "canvas-pool-test")
        defer { s.destroy() }
        let m = GpuModule(create: { _, _, _, _, _ in 7 }, bind: { _, _, _ in 0 },
            render: { _, _, _, _, _ in 1 }, dirty: { _ in 0 }, destroy: { _ in },
            texture: { _, _, _, _, _ in 0 }, textureMetal: nil, sync: nil,
            childrenMode: { _ in 0 }, readback: { _, _, _, _, _, _, _ in 0 },
            child: { _, _, _, _, _, _, _, _, _, _, _, _ in 0 }, childrenCount: { _, _ in 0 }, placement: { _, _, _, _ in 0 },
            shader: nil, validateShader: nil, clearShaders: nil, errorLen: { 0 }, errorPtr: { nil },
            wantsInput: nil, input: nil, messages: nil, published: nil, agent: nil, outPtr: nil)
        m.seen = { id in seenIds.contains(id) ? 1 : 0 }
        s.surfaceHost.modules[""] = m; s.surfaceHost.attempted = [""]
        m.canvases.add(s.surfaceHost)
        let canvas = NodeView(id: 100, kind: "canvas", presenter: s.presenter)
        canvas.frame = CGRect(x: 0, y: 0, width: 200, height: 120)
        s.presenter.root.addSubview(canvas); s.presenter.views[100] = canvas
        let metal = try XCTUnwrap(canvas.canvasMetal)
        XCTAssertTrue(metal.metalLayer === layer && layer.isHidden, "the row's canvas took the spare layer, hidden")
        let e = CanvasesHost.Entry(view: canvas, name: "wave", values: []); e.id = 7; e.module = m
        s.surfaceHost.entries[100] = e

        // Frames recorded, submitted, even presented for other canvases: not this one's word.
        s.surfaceHost.revealPresented()
        XCTAssertTrue(layer.isHidden, "still the old picture under it")
        seenIds = [7]
        s.surfaceHost.revealPresented()
        XCTAssertFalse(layer.isHidden, "its first frame is with the compositor")
        XCTAssertFalse(metal.awaitingFirstFrame)
    }
}
#endif

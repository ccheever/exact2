#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A `path` node on AppKit (LLP 1065): shape layers fitted by the view box,
/// stroked between the engine's `stroke-start` and `stroke-end`, painted by
/// SVG's rows — a moving paint value over them — and a dashed stroke the trim
/// reveals. `bun host/apple/build.mjs --test` runs it.
final class PathViewMacTests: XCTestCase {
    private let black: BatchValue = [0, 0, 0, 255]

    private func path(_ props: [String: String], _ style: NodeStyle, size: CGSize = CGSize(width: 200, height: 100)) -> (NodeView, PathView) {
        _ = NSApplication.shared
        let p = Presenter()
        let n = NodeView(id: 1, kind: "path", presenter: p)
        p.root.addSubview(n); p.views[n.id] = n
        n.frame = CGRect(origin: .zero, size: size)
        n.applyStyle(style)
        n.applyProps(set: props, clear: [])
        PathView.sync(n)
        let view = PathView.of(n)!
        view.layoutSubtreeIfNeeded()
        return (n, view)
    }

    /// Whether the pixel at (x, y) of the view's layer, rendered, is ink.
    private func ink(_ view: PathView, _ x: Int, _ y: Int) -> Bool {
        let (w, h) = (Int(view.bounds.width), Int(view.bounds.height))
        let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        view.layer!.render(in: ctx)
        let data = ctx.data!.assumingMemoryBound(to: UInt8.self)
        return data[(y * w + x) * 4 + 3] > 128
    }

    func testTheViewBoxFitsAndTheEngineTrimsTheStroke() {
        let (_, view) = path(["pathData": "M 0 5 L 20 5", "viewBox": "0 0 20 10", "preserveAspectRatio": "xMidYMid meet"],
                             ["stroke": black, "stroke_width": 2, "fill": "none"])
        let t = view.layers.shape.affineTransform()
        XCTAssertEqual(t, CGAffineTransform(scaleX: 10, y: 10), "the view box's units are the box's tenths")
        XCTAssertEqual(view.layers.shape.lineWidth, 2, "in path units: the fit scales it")
        XCTAssertNil(view.layers.shape.fillColor)
        XCTAssertTrue(ink(view, 150, 50))
        view.present("stroke-end", 0.25)
        XCTAssertEqual(view.layers.shape.strokeEnd, 0.25)
        XCTAssertTrue(ink(view, 40, 50))
        XCTAssertFalse(ink(view, 60, 50), "a quarter of the line, not more")
    }

    func testAspectRatioAndANonScalingStroke() {
        let (_, stretched) = path(["pathData": "M 0 5 L 20 5", "viewBox": "0 0 20 10", "preserveAspectRatio": "none"],
                                  ["stroke": black], size: CGSize(width: 200, height: 50))
        XCTAssertEqual(stretched.layers.shape.affineTransform(), CGAffineTransform(scaleX: 10, y: 5))
        let (_, sliced) = path(["pathData": "M 0 5 L 20 5", "viewBox": "0 0 20 10", "preserveAspectRatio": "xMinYMin slice"],
                               ["stroke": black], size: CGSize(width: 100, height: 100))
        XCTAssertEqual(sliced.layers.shape.affineTransform(), CGAffineTransform(scaleX: 10, y: 10))
        let (_, fixed) = path(["pathData": "M 0 5 L 20 5", "viewBox": "0 0 20 10", "preserveAspectRatio": "xMidYMid meet"],
                              ["stroke": black, "stroke_width": 2, "vector_effect": "non-scaling-stroke"])
        XCTAssertTrue(fixed.layers.shape.affineTransform().isIdentity, "the path is mapped, not the layer")
        XCTAssertEqual(fixed.layers.shape.path?.boundingBoxOfPath, CGRect(x: 0, y: 50, width: 200, height: 0))
        XCTAssertEqual(fixed.layers.shape.lineWidth, 2, "two points, whatever the fit")
    }

    func testSVGPaintingRowsAndMovingPaint() {
        let (n, view) = path(["pathData": "M 0 0 L 20 0 L 20 20 Z"],
                             ["fill": "currentcolor", "text_color": [255, 0, 0, 255], "fill_rule": "evenodd",
                              "stroke": black, "stroke_miterlimit": 10, "stroke_linejoin": "bevel"])
        let shape = view.layers.shape
        XCTAssertEqual(shape.fillColor?.components?.map { Int(($0 * 255).rounded()) }, [255, 0, 0, 255])
        XCTAssertEqual(shape.fillRule, .evenOdd)
        XCTAssertEqual(shape.miterLimit, 10)
        XCTAssertEqual(shape.lineJoin, .bevel)
        // Paint motion's value over the row, then the row again (LLP 1062).
        n.present(paint: "fill", [0, 0, 255, 255])
        XCTAssertEqual(shape.fillColor?.components?.map { Int(($0 * 255).rounded()) }, [0, 0, 255, 255])
        n.present(paint: "fill", nil)
        XCTAssertEqual(shape.fillColor?.components?.map { Int(($0 * 255).rounded()) }, [255, 0, 0, 255])
        let (_, none) = path(["pathData": "M 0 0 L 20 0"], ["fill": "none"])
        XCTAssertNil(none.layers.shape.fillColor)
        XCTAssertNil(none.layers.shape.strokeColor, "SVG's stroke starts at none")
    }

    func testTheTrimRevealsDashesWithoutMovingThem() {
        let (_, view) = path(["pathData": "M 0 50 L 200 50"],
                             ["stroke": black, "stroke_width": 10, "stroke_dasharray": [20, 10, 5], "stroke_dashoffset": 3])
        let layers = view.layers
        XCTAssertNil(layers.shape.strokeColor, "the dashes stroke instead")
        XCTAssertNotNil(layers.dashes.strokeColor)
        XCTAssertEqual(layers.dashes.lineDashPattern, [20, 10, 5, 20, 10, 5], "an odd list repeats")
        XCTAssertEqual(layers.dashes.lineDashPhase, 3)
        XCTAssertTrue(layers.dashes.mask === layers.reveal)
        view.present("stroke-start", 0.5)
        XCTAssertEqual(layers.reveal.strokeStart, 0.5)
        XCTAssertEqual(layers.dashes.strokeStart, 0, "the dashes stay where SVG lays them")
    }

    func testAZeroLengthSubpathIsADot() {
        let (_, view) = path(["pathData": "M 50 50 Z M 120 50 L 190 50 M 10 10"],
                             ["stroke": black, "stroke_width": 20, "stroke_linecap": "round", "fill": "none"])
        XCTAssertTrue(ink(view, 50, 50), "a round cap on a zero-length subpath")
        XCTAssertFalse(ink(view, 10, 10), "a lone moveto draws nothing")
        view.present("stroke-end", 0)
        XCTAssertFalse(ink(view, 50, 50), "not before the pen starts")
    }

    func testAnEvenOddClipPath() {
        _ = NSApplication.shared
        let p = Presenter()
        let n = NodeView(id: 1, kind: "view", presenter: p)
        p.root.addSubview(n); p.views[n.id] = n
        n.frame = CGRect(x: 0, y: 0, width: 40, height: 40)
        n.applyStyle(["clip_path": .object(["rule": "evenodd", "commands": [["M", [0, 0]], ["L", [40, 0]], ["L", [40, 40]], ["L", [0, 40]], ["Z", []],
                                                                                   ["M", [10, 10]], ["L", [30, 10]], ["L", [30, 30]], ["L", [10, 30]], ["Z", []]]])])
        XCTAssertEqual((n.layer?.mask as? CAShapeLayer)?.fillRule, .evenOdd)
        XCTAssertFalse(n.clipPath!.contains(CGPoint(x: 20, y: 20), using: n.clipRule), "the hole is outside")
        XCTAssertTrue(n.clipPath!.contains(CGPoint(x: 5, y: 5), using: n.clipRule))
    }
}
#endif

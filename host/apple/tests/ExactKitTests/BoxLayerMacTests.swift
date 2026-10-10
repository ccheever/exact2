#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A box Core Animation can paint keeps no bitmap on AppKit, as on UIKit
/// (`BoxLayerIOSTests`): background, one radius over a mask of corners, a
/// uniform border, a gradient and an image's pixels are layer properties
/// and sublayers, and the view answers `wantsUpdateLayer`, so AppKit never
/// gives it a backing store. Only what the layer cannot say still draws.
final class BoxLayerMacTests: XCTestCase {
    func testPercentageRadiusClipsMaterialAndOverflowAfterResize() throws {
        var style: NodeStyle = ["overflow_x": "hidden", "overflow_y": "hidden"]
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
            style["border_radius_" + corner] = ["pct": 50]
        }
        let n = node(style, size: CGSize(width: 160, height: 80))
        n.props["backgroundMaterial"] = "ultra-thin"
        n.updateMaterial()
        n.applyBoxLayer()
        let material = try XCTUnwrap(n.materialView)
        material.wantsLayer = true
        let materialLayer = try XCTUnwrap(material.layer)
        let nodeLayer = try XCTUnwrap(n.layer)
        let materialMask = try XCTUnwrap(materialLayer.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(materialMask.path).contains(CGPoint(x: 20, y: 5)))
        XCTAssertTrue(try XCTUnwrap(materialMask.path).contains(CGPoint(x: 80, y: 5)))
        let clip = try XCTUnwrap(nodeLayer.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(clip.path).contains(CGPoint(x: 20, y: 5)))
        n.frame.size = CGSize(width: 80, height: 160)
        n.layoutSubtreeIfNeeded()
        let resized = try XCTUnwrap(materialLayer.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(resized.path).contains(CGPoint(x: 5, y: 20)))
        XCTAssertTrue(try XCTUnwrap(resized.path).contains(CGPoint(x: 5, y: 80)))
        n.applyStyle([:])
        n.updateMaterial()
        n.applyBoxLayer()
        XCTAssertNil(materialLayer.mask, "a later square style removes the elliptical clip")
        XCTAssertNil(nodeLayer.mask)
    }

    func testVideoPercentageRadiusUsesTheContentEdgeAndClears() throws {
        var style: NodeStyle = ["padding_left": 10, "padding_top": 10, "padding_right": 10, "padding_bottom": 10]
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
            style["border_radius_" + corner] = ["pct": 50]
        }
        let n = node(style, size: CGSize(width: 160, height: 80))
        let media = MediaPlatformView()
        VideoView.layout(media, in: n)
        let mediaLayer = try XCTUnwrap(media.layer)
        XCTAssertEqual(media.frame, CGRect(x: 10, y: 10, width: 140, height: 60))
        let mask = try XCTUnwrap(mediaLayer.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(mask.path).contains(CGPoint(x: 10, y: 5)))
        XCTAssertTrue(try XCTUnwrap(mask.path).contains(CGPoint(x: 70, y: 5)))
        n.frame.size = CGSize(width: 80, height: 160)
        VideoView.layout(media, in: n)
        XCTAssertEqual(media.frame, CGRect(x: 10, y: 10, width: 60, height: 140))
        let resized = try XCTUnwrap(mediaLayer.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(resized.path).contains(CGPoint(x: 5, y: 10)))
        n.applyStyle([:])
        VideoView.layout(media, in: n)
        XCTAssertNil(mediaLayer.mask)
    }

    func testPercentageOverflowClipComposesWithAuthoredClipPath() throws {
        var style: NodeStyle = ["overflow_x": "hidden", "overflow_y": "hidden",
            "clip_path": ["commands": [["M", [0, 0]], ["L", [80, 0]], ["L", [80, 80]], ["L", [0, 80]], ["Z", []]]]]
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
            style["border_radius_" + corner] = ["pct": 50]
        }
        let n = node(style, size: CGSize(width: 160, height: 80))
        n.applyBoxLayer()
        let mask = try XCTUnwrap(n.layer!.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(mask.path).contains(CGPoint(x: 20, y: 5)))
        let authored = try XCTUnwrap(mask.mask as? CAShapeLayer)
        XCTAssertTrue(try XCTUnwrap(authored.path).contains(CGPoint(x: 60, y: 40)))
        XCTAssertFalse(try XCTUnwrap(authored.path).contains(CGPoint(x: 100, y: 40)))
        let layer = try XCTUnwrap(n.layer)
        func alpha(_ x: Int, _ y: Int) -> UInt8 {
            let width = Int(n.bounds.width), height = Int(n.bounds.height)
            let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
            // An opaque descendant proves both masks clip real rendered content.
            let child = CALayer()
            child.frame = n.bounds
            child.backgroundColor = CGColor(gray: 1, alpha: 1)
            layer.addSublayer(child)
            defer { child.removeFromSuperlayer() }
            layer.render(in: context)
            let row = layer.isGeometryFlipped ? height - 1 - y : y
            return context.data!.assumingMemoryBound(to: UInt8.self)[(row * width + x) * 4 + 3]
        }
        XCTAssertEqual(alpha(20, 5), 0, "outside ellipse")
        XCTAssertEqual(alpha(60, 40), 255, "inside both masks")
        XCTAssertEqual(alpha(100, 40), 0, "outside authored clip")
        n.frame.size = CGSize(width: 80, height: 160)
        n.applyBoxLayer()
        XCTAssertEqual(alpha(5, 20), 0, "resized ellipse")
        XCTAssertEqual(alpha(40, 40), 255)
        XCTAssertEqual(alpha(40, 120), 0, "authored clip survives resize")
        n.applyStyle([:])
        n.applyBoxLayer()
        XCTAssertNil(layer.mask)
        XCTAssertEqual(alpha(5, 5), 255, "removing the style clears both masks")
    }

    func testPercentageOverflowClipSurvivesSurfaceRoundTrip() throws {
        for shadow in [false, true] {
            var style: NodeStyle = ["overflow_x": "hidden", "overflow_y": "hidden"]
            for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
                style["border_radius_" + corner] = ["pct": 50]
            }
            if shadow {
                style["box_shadow"] = [["o": [0, 0], "b": 4, "s": 0, "c": [0, 0, 0, 128]]]
            }
            let n = node(style, size: CGSize(width: 160, height: 80))
            n.applyBoxLayer()
            if shadow { XCTAssertNotNil(n.clipBox) }
            let target = try XCTUnwrap(n.clipBox?.layer ?? n.layer)
            n.layoutScale = CGPoint(x: 0.5, y: 0.5)
            n.applySurface()
            XCTAssertTrue(target.mask === n.surface?.clip)
            n.layoutScale = CGPoint(x: 1, y: 1)
            n.applySurface()
            n.display()
            XCTAssertNil(n.surface)
            let mask = try XCTUnwrap(target.mask as? CAShapeLayer, "ellipse restored, shadow=\(shadow)")
            XCTAssertFalse(try XCTUnwrap(mask.path).contains(CGPoint(x: 20, y: 5)))
            XCTAssertTrue(try XCTUnwrap(mask.path).contains(CGPoint(x: 80, y: 5)))
        }
    }

    func testPercentageClipOnlyKindsUseAnEllipseWithoutACircularClip() throws {
        for kind in ["text", "canvas", "iframe"] {
            var style: NodeStyle = ["overflow_x": "hidden", "overflow_y": "hidden"]
            for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
                style["border_radius_" + corner] = ["pct": 50]
            }
            let n = node(style, kind: kind, size: CGSize(width: 160, height: 80))
            n.applyBoxLayer()
            XCTAssertEqual(n.layer?.cornerRadius, 0, kind)
            let mask = try XCTUnwrap(n.layer?.mask as? CAShapeLayer)
            XCTAssertFalse(try XCTUnwrap(mask.path).contains(CGPoint(x: 20, y: 5)))
            XCTAssertTrue(try XCTUnwrap(mask.path).contains(CGPoint(x: 80, y: 5)))
        }
    }

    func testVideoStyleChangesRefreshGeometryWithoutPropChanges() throws {
        let n = node([:], size: CGSize(width: 160, height: 80))
        let video = VideoView(owner: n)
        n.video = video
        guard let media = n.subviews.first else {
            throw XCTSkip("the optional video module is not beside this test runner")
        }
        video.update()
        var style: NodeStyle = ["padding_left": 10, "padding_top": 10, "padding_right": 10, "padding_bottom": 10]
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
            style["border_radius_" + corner] = ["pct": 50]
        }
        n.applyStyle(style)
        XCTAssertEqual(media.frame, CGRect(x: 10, y: 10, width: 140, height: 60))
        let mask = try XCTUnwrap(media.layer?.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(mask.path).contains(CGPoint(x: 10, y: 5)))
        n.applyStyle([:])
        XCTAssertEqual(media.frame, n.bounds)
        XCTAssertNil(media.layer?.mask)
        n.applyStyle(style)
        XCTAssertNotNil(media.layer?.mask)
    }

    private let white: BatchValue = [255, 255, 255, 255]
    private let blue: BatchValue = [0, 136, 255, 255]

    func testPercentageRadiusUsesBothAxesAndFollowsResize() {
        var style: NodeStyle = ["background_color": [36, 104, 172, 255]]
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
            style["border_radius_" + corner] = ["pct": 50]
        }
        let n = node(style, size: CGSize(width: 160, height: 80))
        XCTAssertEqual(n.cornerSizes(in: n.bounds), Array(repeating: CGSize(width: 80, height: 40), count: 4))
        XCTAssertFalse(n.roundedPath(in: n.bounds).cgPath.contains(CGPoint(x: 20, y: 5)))
        XCTAssertTrue(n.roundedPath(in: n.bounds).cgPath.contains(CGPoint(x: 80, y: 5)))
        n.frame.size = CGSize(width: 80, height: 160)
        XCTAssertEqual(n.cornerSizes(in: n.bounds), Array(repeating: CGSize(width: 40, height: 80), count: 4))
        XCTAssertNil(FlatPaint(style), "percentage geometry cannot use a fixed-radius flat leaf")
    }

    private func node(_ style: NodeStyle, kind: String = "view", size: CGSize = CGSize(width: 300, height: 120)) -> NodeView {
        _ = NSApplication.shared
        let p = Presenter()
        let n = NodeView(id: 1, kind: kind, presenter: p)
        n.frame = NSRect(origin: .zero, size: size)
        p.root.addSubview(n); p.views[1] = n
        n.applyStyle(style)
        n.display()
        return n
    }
    private func round(_ r: Double, _ corners: [String] = ["top_left", "top_right", "bottom_right", "bottom_left"]) -> NodeStyle {
        var s: NodeStyle = [:]
        for c in corners { s["border_radius_" + c] = .number(r) }
        return s
    }

    func testBackgroundAndMaskedRadiusAreLayerProperties() {
        let square = node(["background_color": white])
        XCTAssertTrue(square.wantsUpdateLayer)
        XCTAssertNil(square.layer?.contents)
        XCTAssertEqual(square.layer?.backgroundColor?.alpha, 1)
        var style = round(24, ["top_left", "top_right"]); style["background_color"] = white
        let card = node(style)
        XCTAssertTrue(card.wantsUpdateLayer)
        XCTAssertNil(card.layer?.contents)
        // AppKit makes a backing layer's radius clip, so a rounded box that
        // does not clip fills a sublayer under its children.
        XCTAssertNil(card.layer?.backgroundColor)
        XCTAssertEqual(card.layer?.cornerRadius, 0)
        XCTAssertEqual(card.layer?.masksToBounds, false, "rounding clips nothing while the overflow is visible")
        XCTAssertEqual(card.boxFill?.backgroundColor?.alpha, 1)
        XCTAssertEqual(card.boxFill?.cornerRadius ?? 0, 24, accuracy: 0.001)
        // The layer is flipped as the view is: minimum y is the top.
        XCTAssertEqual(card.boxFill?.maskedCorners, [.layerMinXMinYCorner, .layerMaxXMinYCorner])
        style["overflow_x"] = "hidden"; style["overflow_y"] = "hidden"
        let clipped = node(style)
        XCTAssertEqual(clipped.layer?.backgroundColor?.alpha, 1, "a clipping box is rounded by its own mask")
        XCTAssertEqual(clipped.layer?.cornerRadius ?? 0, 24, accuracy: 0.001)
        XCTAssertNil(clipped.boxFill)
    }

    /// The live row's waveform bar: a 3 × h pill of one colour.
    func testAPillPastHalfItsWidthIsReducedNotDrawn() {
        var style = round(1.5); style["background_color"] = blue
        let bar = node(style, size: CGSize(width: 3, height: 20))
        XCTAssertTrue(bar.wantsUpdateLayer)
        XCTAssertEqual(bar.boxFill?.cornerRadius ?? 0, 1.5, accuracy: 0.001)
        var wide = round(100); wide["background_color"] = blue
        XCTAssertEqual(node(wide, size: CGSize(width: 56, height: 28)).boxFill?.cornerRadius ?? 0, 14, accuracy: 0.001)
    }

    func testAUniformBorderStaysUnderTheChildrenUnlessTheyAreClipped() {
        var style = round(11); style["border_width"] = 2; style["border_color_top"] = blue
        let open = node(style, size: CGSize(width: 22, height: 22))
        XCTAssertTrue(open.wantsUpdateLayer)
        XCTAssertEqual(open.layer?.borderWidth, 0)
        XCTAssertEqual(open.boxBorder?.borderWidth, 2)
        XCTAssertEqual(open.boxBorder?.cornerRadius ?? 0, 11, accuracy: 0.001)
        style["overflow_x"] = "hidden"; style["overflow_y"] = "hidden"
        let closed = node(style, size: CGSize(width: 22, height: 22))
        XCTAssertTrue(closed.wantsUpdateLayer)
        XCTAssertEqual(closed.layer?.borderWidth, 2, "a clipping box's border covers what it clips")
        XCTAssertEqual(closed.layer?.masksToBounds, true)
        XCTAssertNil(closed.boxBorder)
    }

    /// A row's separator (`border-bottom`): a rectangle on a shape layer.
    func testSidesInOneColourAreAShapeLayer() {
        let row = node(["border_width_bottom": 0.5, "border_color_bottom": white, "border_color_top": white])
        XCTAssertTrue(row.wantsUpdateLayer)
        let shape = row.boxBorder as? CAShapeLayer
        XCTAssertNotNil(shape)
        XCTAssertEqual(shape?.path?.boundingBox.maxY, 120, "the bottom side, y down")
        XCTAssertGreaterThanOrEqual(shape?.path?.boundingBox.height ?? 0, 0.5, "at least one device pixel")
    }

    func testGroupedSeparatorUsesTheExistingEdgeLayerAndFollowsTheLastRow() throws {
        let owner = node([:])
        let first = node(["padding_left": 16, "border_color_bottom": white])
        let last = node([:])
        first.props["groupedRowSeparator"] = "true"
        last.props["groupedRowSeparator"] = "true"
        owner.addSubview(first); owner.addSubview(last)
        first.applyBoxLayer(); last.applyBoxLayer()
        let shape = try XCTUnwrap(first.boxBorder as? CAShapeLayer)
        XCTAssertEqual(shape.path?.boundingBox, CGRect(x: 16, y: 119, width: 284, height: 1))
        XCTAssertEqual(first.layer?.sublayers?.count, 1, "the existing border layer carries the separator")
        XCTAssertNil(first.layer?.contents, "a separator does not allocate a row-sized bitmap")
        XCTAssertTrue(first.wantsUpdateLayer)
        XCTAssertNil(last.boxBorder, "the last row has no system separator")
        last.applyStyle(["display": "none"])
        first.applyBoxLayer()
        XCTAssertNil(first.boxBorder, "a hidden trailing row leaves no separator")
        last.applyStyle([:]); first.applyBoxLayer()
        XCTAssertNotNil(first.boxBorder)
        last.removeFromSuperview(); first.applyBoxLayer()
        XCTAssertNil(first.boxBorder, "removing a retained row clears the parent's last-row cache")
        owner.addSubview(last); first.applyBoxLayer()
        XCTAssertNotNil(first.boxBorder, "inserting a row restores the previous row's separator")
        first.frame.size.width = 200
        first.applyStyle(["padding_left": 56, "border_color_bottom": white])
        first.applyBoxLayer()
        XCTAssertEqual((first.boxBorder as? CAShapeLayer)?.path?.boundingBox,
                       CGRect(x: 56, y: 119, width: 144, height: 1))
        first.applyStyle(["padding_left": 56, "border_width_left": 2, "border_width_right": 3,
                          "border_width_bottom": 4, "border_color_bottom": white])
        XCTAssertEqual(first.groupedSeparatorRect, CGRect(x: 58, y: 115, width: 139, height: 1),
                       "the separator stays in the padding box above authored borders")
        first.applyStyle([:])
        XCTAssertEqual(first.groupedSeparatorRect?.minX, 16, "a native button has the system inset without UA padding")
        first.applyStyle(["padding_left": 0])
        XCTAssertEqual(first.groupedSeparatorRect?.minX, 0, "an authored zero inset stays zero")
        first.applyStyle(["padding_left": ["pct": 10, "px": 2], "border_width_left": 2, "border_width_right": 3])
        XCTAssertEqual(first.groupedSeparatorRect?.minX, 23.5, "a percent/calc paint offset follows the web padding-box basis")
    }

    func testShowingManyGroupedRowsInOneBatchPublishesTheFinalSeparators() throws {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 300, height: 300)
        let ids = Array(2...33)
        var inspectedColdBatch = false
        p.onViewportFit = { [weak p] in
            guard let p else { return }
            inspectedColdBatch = true
            XCTAssertEqual(p.views[100]?.groupedSeparatorInvalidationPending, false,
                           "an ordinary parent queues no grouped paint work")
            XCTAssertEqual(p.views[1]?.groupedSeparatorInvalidationPending, true,
                           "a marked cold group defers its one repaint until the batch ends")
            XCTAssertNil(p.views[1]?.groupedLastVisibleRow, "cold mounting does not resolve intermediate last rows")
        }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "view", "props": ["viewportFit": "cover"], "style": [:]],
            ["op": "create", "id": 100, "kind": "view", "style": [:]],
            ["op": "create", "id": 101, "kind": "view", "style": [:]],
            ["op": "children", "id": 100, "ids": [101]],
        ]
        for id in ids {
            ops.append(["op": "create", "id": id, "kind": "view",
                "props": ["groupedRowSeparator": "true"],
                "style": ["display": id == 2 ? "block" : "none", "padding_left": 16,
                          "border_color_bottom": [255, 255, 255, 255]]])
            ops.append(["op": "frame", "id": id, "x": 0.0, "y": Double(id - 2) * 52, "w": 300.0, "h": 52.0])
        }
        ops += [["op": "children", "id": 1, "ids": ids], ["op": "roots", "ids": [1, 100]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 1664.0]]
        p.apply(wireBatch(ops))
        XCTAssertTrue(inspectedColdBatch, "the viewport callback observes the real batch before deferred work")
        p.onViewportFit = nil
        let owner = try XCTUnwrap(p.views[1])
        let first = try XCTUnwrap(p.views[2])
        XCTAssertFalse(owner.groupedSeparatorInvalidationPending, "cold mounting completes its deferred sweep")
        XCTAssertNil(first.groupedSeparatorRect, "only one row is initially visible")
        p.apply(wireBatch(ids.dropFirst().map { id in
            ["op": "style", "id": id, "style": ["display": "block", "padding_left": 16,
                                               "border_color_bottom": [255, 255, 255, 255]]]
        }))
        XCTAssertFalse(owner.groupedSeparatorInvalidationPending, "bulk show completes its deferred sweep")
        first.applyBoxLayer()
        XCTAssertNotNil(first.boxBorder)
        let last = try XCTUnwrap(p.views[33])
        last.applyBoxLayer()
        XCTAssertNil(last.boxBorder)
        XCTAssertTrue(owner.groupedLastVisibleRow === last, "the cache describes the completed batch")
    }

    func testWhatTheLayerCannotSayStillDraws() {
        var corners = round(8, ["top_left"]); corners["background_color"] = white
        corners["border_radius_top_right"] = 4
        XCTAssertFalse(node(corners).wantsUpdateLayer, "radii that differ by corner draw")
        XCTAssertFalse(node(["background_color": white], kind: "canvas").wantsUpdateLayer)
        XCTAssertFalse(node(["background_color": white], kind: "iframe").wantsUpdateLayer)
    }

    func testAGradientIsASublayerWithTheBoxRadius() {
        let gradient: BatchValue = .object(["linear": .number(90), "stops": .array([0, 255, 0, 0, 255, 1, 0, 0, 255, 255].map { .number($0) })])
        var style = round(12); style["background_image"] = gradient
        let n = node(style)
        guard n.boxGradient != nil else { return XCTFail("no gradient layer for \(n.style)") }
        XCTAssertTrue(n.wantsUpdateLayer)
        XCTAssertEqual(n.boxGradient?.cornerRadius ?? 0, 12, accuracy: 0.001)
        XCTAssertEqual(n.boxGradient?.frame, n.bounds)
    }
}
#endif

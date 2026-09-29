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
    private let white: BatchValue = [255, 255, 255, 255]
    private let blue: BatchValue = [0, 136, 255, 255]

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

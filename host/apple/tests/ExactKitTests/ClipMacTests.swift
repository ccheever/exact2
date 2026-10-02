#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// What an AppKit node clips (LLP 1054 P2/P3): `overflow: hidden` clips its
/// subviews to the rounded border box, as UIKit does, and a clamped
/// paragraph clips its own drawing, as CSS's line-clamp implies overflow.
final class ClipMacTests: XCTestCase {
    private func node(_ kind: String, _ style: NodeStyle) -> NodeView {
        _ = NSApplication.shared
        let p = Presenter()
        let node = NodeView(id: 1, kind: kind, presenter: p)
        node.frame = NSRect(x: 0, y: 0, width: 48, height: 48)
        p.root.addSubview(node); p.views[1] = node
        node.applyStyle(style)
        return node
    }

    func testHiddenOverflowClipsToTheRoundedBox() {
        // The port's avatar: an image in a clipped circle.
        let n = node("view", ["overflow_x": "hidden", "overflow_y": "hidden", "border_radius": 24])
        XCTAssertTrue(n.clipsToBounds)
        XCTAssertEqual(n.layer?.masksToBounds, true)
        XCTAssertEqual(n.layer?.cornerRadius, 24)
    }

    func testVisibleOverflowDoesNotClip() {
        let n = node("view", ["border_radius": 24])
        XCTAssertFalse(n.clipsToBounds)
        XCTAssertEqual(n.layer?.masksToBounds, false)
        // Clipping off again drops the mask's radius with it.
        n.applyStyle(["overflow_x": "hidden", "overflow_y": "hidden", "border_radius": 24])
        n.applyStyle(["border_radius": 24])
        XCTAssertEqual(n.layer?.cornerRadius, 0)
    }

    /// CSS reduces a radius larger than the box (`border-radius: 100px` on a
    /// 56 × 28 pill is 14); Core Animation given the authored 100 draws
    /// nothing at all (the xheavy glass row's rating capsule vanished,
    /// text and material included).
    func testAClipRadiusPastTheBoxIsReducedAsCSSReducesIt() {
        let radii: NodeStyle = ["border_radius_top_left": 100, "border_radius_top_right": 100,
                                "border_radius_bottom_right": 100, "border_radius_bottom_left": 100]
        var style = radii; style["overflow_x"] = "hidden"; style["overflow_y"] = "hidden"
        let n = node("view", style)
        XCTAssertEqual(n.layer?.cornerRadius ?? 0, 24, accuracy: 0.001, "48 × 48: half the side")
        n.frame = NSRect(x: 0, y: 0, width: 56, height: 28)
        XCTAssertEqual(n.layer?.cornerRadius ?? 0, 14, accuracy: 0.001, "the kernel's new size reduces it again")
    }

    func testPercentageBackdropClipsAnEllipseAndFollowsResize() throws {
        var style: NodeStyle = ["backdrop_blur": 4]
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] {
            style["border_radius_" + corner] = ["pct": 50]
        }
        let n = node("view", style)
        n.frame.size = CGSize(width: 160, height: 80)
        let mask = try XCTUnwrap(n.layer?.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(mask.path).contains(CGPoint(x: 20, y: 5)))
        XCTAssertTrue(try XCTUnwrap(mask.path).contains(CGPoint(x: 80, y: 5)))
        n.frame.size = CGSize(width: 80, height: 160)
        let resized = try XCTUnwrap(n.layer?.mask as? CAShapeLayer)
        XCTAssertFalse(try XCTUnwrap(resized.path).contains(CGPoint(x: 5, y: 20)))
        n.applyStyle([:])
        XCTAssertNil(n.layer?.mask)
    }

    func testAMaterialsRadiusIsReducedToo() {
        let n = node("view", ["border_radius_top_left": 100, "border_radius_top_right": 100,
                              "border_radius_bottom_right": 100, "border_radius_bottom_left": 100])
        n.frame = NSRect(x: 0, y: 0, width: 56, height: 28)
        n.props["backgroundMaterial"] = "ultra-thin"
        n.updateMaterial()
        XCTAssertEqual(n.materialView?.layer?.cornerRadius ?? 0, 14, accuracy: 0.001)
    }

    func testAClampedParagraphClipsItsDrawing() {
        XCTAssertTrue(node("text", ["line_clamp": 2]).clipsToBounds)
        XCTAssertFalse(node("text", [:]).clipsToBounds)
    }
}
#endif

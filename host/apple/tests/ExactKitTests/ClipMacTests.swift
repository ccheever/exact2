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

    func testAClampedParagraphClipsItsDrawing() {
        XCTAssertTrue(node("text", ["line_clamp": 2]).clipsToBounds)
        XCTAssertFalse(node("text", [:]).clipsToBounds)
    }
}
#endif

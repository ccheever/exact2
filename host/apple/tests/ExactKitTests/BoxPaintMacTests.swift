#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A node's box as the Mac paints it, read back as pixels: a rounded box's
/// border in the colour the host sends for its sides, and each corner at
/// its own radius, as iOS and Linux draw them.
final class BoxPaintMacTests: XCTestCase {
    private func paint(_ style: NodeStyle) -> NSBitmapImageRep {
        _ = NSApplication.shared
        let p = Presenter()
        let node = NodeView(id: 1, kind: "view", presenter: p)
        node.frame = NSRect(x: 0, y: 0, width: 100, height: 40)
        p.root.addSubview(node); p.views[1] = node
        node.applyStyle(style)
        let rep = node.bitmapImageRepForCachingDisplay(in: node.bounds)!
        // As the agent's screenshot and a canvas's surface capture it.
        Capture.capturing = true
        node.cacheDisplay(in: node.bounds, to: rep)
        Capture.capturing = false
        return rep
    }
    /// sRGB components of the pixel under a point in the view's own (flipped) space.
    private func rgba(_ rep: NSBitmapImageRep, _ x: CGFloat, _ y: CGFloat) -> [CGFloat] {
        let scale = CGFloat(rep.pixelsWide) / rep.size.width
        let c = rep.colorAt(x: Int(x * scale), y: Int(y * scale))!.usingColorSpace(.sRGB)!
        return [c.redComponent, c.greenComponent, c.blueComponent, c.alphaComponent]
    }
    private func box(radii: [BatchValue]) -> NodeStyle {
        var style: NodeStyle = ["background_color": [255, 255, 255, 255]]
        for (corner, radius) in zip(["top_left", "top_right", "bottom_right", "bottom_left"], radii) { style["border_radius_" + corner] = radius }
        for side in ["top", "right", "bottom", "left"] { style["border_width_" + side] = 2; style["border_color_" + side] = [255, 0, 0, 255] }
        return style
    }

    func testRoundedBorderStrokesTheColourTheHostSends() {
        // Caltrain's station search: equal widths, every corner rounded.
        let rep = paint(box(radii: [8, 8, 8, 8]))
        for (x, y) in [(50.0, 0.5), (50.0, 39.5), (0.5, 20.0), (99.5, 20.0)] {
            let c = rgba(rep, x, y)
            XCTAssert(c[0] > 0.9 && c[1] < 0.2 && c[2] < 0.2 && c[3] > 0.9, "border at \(x),\(y) is \(c)")
        }
    }

    func testEachCornerKeepsItsOwnRadius() {
        let rep = paint(box(radii: [0, 16, 16, 0]))
        XCTAssertGreaterThan(rgba(rep, 0.5, 0.5)[3], 0.9, "the square top-left corner is painted")
        XCTAssertGreaterThan(rgba(rep, 0.5, 39.5)[3], 0.9, "the square bottom-left corner is painted")
        XCTAssertLessThan(rgba(rep, 99.5, 0.5)[3], 0.1, "the rounded top-right corner is empty")
        XCTAssertLessThan(rgba(rep, 99.5, 39.5)[3], 0.1, "the rounded bottom-right corner is empty")
    }
}
#endif

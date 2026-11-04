#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// Every box drawn on the device's pixel grid, its edges rounded half up at
/// their absolute positions, as Chromium paints them
/// (`PhysicalRect::PixelSnappedIntRect`) and as the iOS presenter draws them
/// (`PixelGridIOSTests`). Drawn where layout puts it, a box off the grid is
/// blended across two device rows.
final class PixelGridMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil; page = nil }

    private func presenter() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        XCTAssertEqual(window.backingScaleFactor, 2, "the rounding this checks is a 2x display's")
        page = p.root
        return p
    }

    private var page: NSView!
    /// `v`'s edges in the page's device pixels, down from its top.
    private func pixels(_ v: NSView) -> NSRect {
        let r = page.convert(v.bounds, from: v), k = page.pixelScale ?? 0
        return NSRect(x: r.minX * k, y: r.minY * k, width: r.width * k, height: r.height * k)
    }
    private func assertOnGrid(_ v: NSView, _ message: String, file: StaticString = #filePath, line: UInt = #line) {
        let r = pixels(v)
        for edge in [r.minX, r.minY, r.maxX, r.maxY] {
            XCTAssertEqual(edge, edge.rounded(), accuracy: 1e-6, "\(message): \(r)", file: file, line: line)
        }
    }

    func testABoxOffTheGridIsDrawnOnIt() throws {
        let p = presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "style": ["background_color": [0, 0, 0, 255]]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "frame", "id": 2, "x": 10.3, "y": 446.6875 - 400, "w": 100.1, "h": 1],
        ]))
        let box = try XCTUnwrap(p.views[2])
        // Each edge on its nearest device pixel, a half up: y 46.6875 is
        // pixel 93.375, so 93; its bottom, 47.6875, is 95.375, so 95.
        XCTAssertEqual(box.frame, NSRect(x: 10.5, y: 46.5, width: 100, height: 1))
        assertOnGrid(box, "a 1 pt border's box")
        XCTAssertEqual(box.layoutBounds.size, NSSize(width: 100.1, height: 1), "its content is laid out at layout's size")
    }

    func testEdgesThatMeetInLayoutMeetWhenDrawnAtAnyDepth() throws {
        let p = presenter()
        // A child 0.4 into a parent at 10.3 is at 10.7: pixel 21.4, so 21,
        // where the sibling that meets it in layout begins too. Rounded from
        // its parent's drawn edge (10.5 + 0.4 = 10.9, pixel 22) it would not.
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "create", "id": 4, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2, 4]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 3, "x": 0, "y": 0.4, "w": 50, "h": 5],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "frame", "id": 2, "x": 0, "y": 10.3, "w": 100, "h": 20],
            ["op": "frame", "id": 4, "x": 0, "y": 15.7, "w": 100, "h": 5],
        ]))
        let child = try XCTUnwrap(p.views[3]), sibling = try XCTUnwrap(p.views[4])
        assertOnGrid(child, "a child framed before its parent")
        XCTAssertEqual(pixels(child).minY, 21, "10.7 rounds to pixel 21, from the absolute position")
        XCTAssertEqual(pixels(child).maxY, pixels(sibling).minY, "the edges that meet in layout meet")

        // The parent moves; its child moves with it, on the grid.
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 0, "y": 10.1, "w": 100, "h": 20]]))
        assertOnGrid(child, "after its parent moved")
        XCTAssertEqual(pixels(child).minY, 21, "10.5 is pixel 21, a half up")
    }

    func testABoxInAScrollersDocumentIsOnTheGrid() throws {
        let p = presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_y": "auto", "overflow_x": "hidden"]],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "frame", "id": 2, "x": 10, "y": 23.63, "w": 300, "h": 200.2],
            ["op": "frame", "id": 3, "x": 0, "y": 477.69 - 23.63, "w": 300, "h": 22.75],
            ["op": "content", "id": 2, "w": 300.0, "h": 600.35],
        ]))
        p.viewport.layoutSubtreeIfNeeded()
        let scroller = try XCTUnwrap(p.views[2]), row = try XCTUnwrap(p.views[3])
        assertOnGrid(scroller, "the scroller")
        assertOnGrid(row, "a row in its document")
        let document = try XCTUnwrap(scroller.scroll?.documentView)
        // Its extent ends at its last edge's device pixel: 23.63 + 600.35
        // is 623.98, pixel 1247.96, so 1248, 1201 pixels below the
        // scroller's top at pixel 47.
        XCTAssertEqual(document.frame.height, 600.5)
    }

    func testAPageZoomRoundsToItsDevicePixels() throws {
        let p = presenter()
        p.viewport.magnification = 1.25
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 200],
            ["op": "frame", "id": 2, "x": 0, "y": 10.3, "w": 100, "h": 20],
        ]))
        let box = try XCTUnwrap(p.views[2])
        XCTAssertEqual(box.pixelScale, 2.5, "a CSS px is 2.5 device pixels at 125% on a 2x display")
        // 10.3 is pixel 25.75, so 26: 10.4 CSS px.
        XCTAssertEqual(box.frame.minY, 10.4, accuracy: 1e-9)
        assertOnGrid(box, "at 125%")

        // Back to Actual Size: every box again on the new grid.
        p.viewport.magnification = 1
        p.regridAll()
        XCTAssertEqual(box.frame.minY, 10.5, accuracy: 1e-9)
        assertOnGrid(box, "at 100%")
    }

    /// A code block's lines, as the T3 Code clone's: a `pre` at 477.6875
    /// with 12.8 px of padding and 17.875 px lines. Chromium rounds each
    /// baseline at its place on the page; rounded in its box, which the
    /// padding leaves off the grid, a line sat a device pixel low.
    func testABaselineRoundsAtItsPlaceOnThePage() throws {
        let p = presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "export function outline("]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "frame", "id": 2, "x": 1, "y": 177.6875, "w": 300, "h": 79.21875],
            ["op": "fieldContent", "id": 2, "rect": [12.8, 12.8, 274.4, 53.625]],
        ]))
        let text = try XCTUnwrap(p.views[2])
        let top = text.paragraphBox().minY, drawnTop = page.convert(NSPoint.zero, from: text).y
        for line in 0..<3 {
            for ascent in stride(from: CGFloat(10), through: 14, by: 0.125) {
                let baseline = CGFloat(line) * 17.875 + ascent
                let painted = drawnTop + TextEngine.paintedLine(baseline, top: top, scale: 2, lineScale: 2)
                XCTAssertEqual(painted, PixelGrid.round(177.6875 + 12.8 + baseline, scale: 2), accuracy: 1e-9, "line \(line), ascent \(ascent)")
            }
        }
    }

    func testAStickyBoxStaysOnTheGrid() {
        let place = PixelGrid.place(NSRect(x: 0, y: 10.3, width: 10, height: 10), in: .zero, scale: 2)
        // A displacement of 3.3 takes the box's top to 13.6: pixel 27.2, so
        // 27, 6 pixels below its drawn 21.
        XCTAssertEqual(PixelGrid.shift(place, by: CGPoint(x: 0, y: 3.3), scale: 2).y, 3)
        XCTAssertEqual(PixelGrid.round(-10.25, scale: 2), -10, "a half rounds up, below zero too")
    }
}
#endif

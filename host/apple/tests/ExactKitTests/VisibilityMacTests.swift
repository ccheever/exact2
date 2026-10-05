#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// CSS `visibility` on macOS: a hidden box keeps its geometry and paints
/// nothing of its own, and it is not a hit. A descendant whose own style is
/// `visible` paints and is hit. One that is `hidden` does not. Wire batches
/// send each node's style as the host will, so the inheriting child carries
/// `visibility: hidden` itself.
final class VisibilityMacTests: XCTestCase {
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil }

    private func presenter() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 200, height: 200)
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
        return p
    }

    private func rgba(_ rep: NSBitmapImageRep, _ x: CGFloat, _ y: CGFloat) -> [CGFloat] {
        let scale = CGFloat(rep.pixelsWide) / rep.size.width
        let c = rep.colorAt(x: Int(x * scale), y: Int(y * scale))!.usingColorSpace(.sRGB)!
        return [c.redComponent, c.greenComponent, c.blueComponent, c.alphaComponent]
    }

    func testAVisibleDescendantOfAHiddenBoxPaintsAndIsHit() throws {
        let p = presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": [255.0, 255.0, 255.0, 255.0]]],
            ["op": "create", "id": 2, "kind": "view", "style": ["background_color": [255.0, 0.0, 0.0, 255.0], "visibility": "hidden"]],
            ["op": "create", "id": 3, "kind": "view", "style": ["background_color": [0.0, 255.0, 0.0, 255.0], "visibility": "visible"]],
            ["op": "create", "id": 4, "kind": "view", "style": ["background_color": [0.0, 0.0, 255.0, 255.0], "visibility": "hidden"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 120.0, "h": 80.0],
            ["op": "frame", "id": 3, "x": 10.0, "y": 10.0, "w": 40.0, "h": 30.0],
            ["op": "frame", "id": 4, "x": 70.0, "y": 10.0, "w": 40.0, "h": 30.0],
        ]))
        let page = try XCTUnwrap(p.views[1])
        let parent = try XCTUnwrap(p.views[2])
        let shown = try XCTUnwrap(p.views[3])
        let kept = try XCTUnwrap(p.views[4])
        XCTAssertFalse(parent.isHidden, "visibility keeps the box in the tree")
        XCTAssertGreaterThan(parent.frame.width, 0)
        XCTAssertGreaterThan(parent.frame.height, 0)
        XCTAssertGreaterThan(shown.frame.width, 0)
        XCTAssertGreaterThan(kept.frame.width, 0)
        let rep = try XCTUnwrap(Capture.picture(of: page))
        func onPage(_ view: NodeView, _ point: NSPoint) -> NSPoint { page.convert(point, from: view) }
        let shownAt = onPage(shown, NSPoint(x: shown.bounds.midX, y: shown.bounds.midY))
        let keptAt = onPage(kept, NSPoint(x: kept.bounds.midX, y: kept.bounds.midY))
        let ownAt = onPage(parent, NSPoint(x: 20, y: 60))
        let green = rgba(rep, shownAt.x, shownAt.y)
        XCTAssert(green[1] > 0.9 && green[0] < 0.1 && green[2] < 0.1, "the visible child paints: \(green)")
        let blue = rgba(rep, keptAt.x, keptAt.y)
        XCTAssertFalse(blue[2] > 0.8 && blue[0] < 0.2 && blue[1] < 0.2, "the hidden child paints nothing: \(blue)")
        let red = rgba(rep, ownAt.x, ownAt.y)
        XCTAssertFalse(red[0] > 0.8 && red[1] < 0.2 && red[2] < 0.2, "the hidden box paints nothing of its own: \(red)")
        func hit(_ view: NodeView, _ point: NSPoint) -> NSView? {
            page.hitTest(page.superview!.convert(point, from: view))
        }
        XCTAssertTrue(hit(shown, NSPoint(x: shown.bounds.midX, y: shown.bounds.midY)) === shown)
        let keptHit = hit(kept, NSPoint(x: kept.bounds.midX, y: kept.bounds.midY))
        XCTAssertFalse(keptHit === kept)
        XCTAssertFalse(keptHit === parent)
        let ownHit = hit(parent, NSPoint(x: 20, y: 60))
        XCTAssertFalse(ownHit === parent)
    }
}
#endif

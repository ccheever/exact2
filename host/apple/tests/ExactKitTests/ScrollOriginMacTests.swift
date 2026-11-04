#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A box's scroll position stays inside its scroll range (CSSOM View §
/// "scroll an element": the position is clamped to the scrolling area), as
/// Chrome's does. AppKit's clip view aligns its origin to the backing
/// store's pixels as the document's size changes; laid out off the device
/// grid at that moment, a box with nothing to scroll was left scrolled by a
/// fraction of a point, and its content drew that much lower once the box
/// came to rest on the grid.
final class ScrollOriginMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    /// A 45.5 pt `overflow-y: auto` box inside a parent at `y`, holding
    /// `content` points of content.
    private func fixture(y: Double, content: Double) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view", "style": ["overflow_y": "auto", "overflow_x": "hidden"]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Ask anything"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "frame", "id": 2, "x": 10, "y": y, "w": 340, "h": 69.5],
            ["op": "frame", "id": 3, "x": 16, "y": 16, "w": 300, "h": 45.5],
            ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 300, "h": content],
            ["op": "content", "id": 3, "w": 300.0, "h": content],
        ]))
        p.viewport.layoutSubtreeIfNeeded()
        return p
    }
    /// The box comes to rest at `y`, its content `content` tall.
    private func settle(_ p: Presenter, y: Double, content: Double) {
        p.apply(wireBatch([
            ["op": "frame", "id": 2, "x": 10, "y": y, "w": 340, "h": 69.5],
            ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 300, "h": content],
            ["op": "content", "id": 3, "w": 300.0, "h": content],
        ]))
        p.viewport.layoutSubtreeIfNeeded()
    }

    func testABoxWithNothingToScrollKeepsItsContentAtItsTop() throws {
        let p = fixture(y: 100.25, content: 22.75)
        XCTAssertEqual(window.backingScaleFactor, 2, "the alignment this guards is a 2x display's")
        let clip = try XCTUnwrap(p.views[3]?.scroll?.contentView)
        // The content grows to fill the box while it is laid out off the grid.
        settle(p, y: 100.25, content: 45.5)
        XCTAssertEqual(clip.bounds.origin, .zero, "nothing to scroll: no offset, off the grid")
        settle(p, y: 100.5, content: 45.5)
        XCTAssertEqual(clip.bounds.origin, .zero, "at rest on the grid")
    }

    func testAScrollerAtItsTopStaysAtItsTop() throws {
        let p = fixture(y: 100.25, content: 45.5)
        let clip = try XCTUnwrap(p.views[3]?.scroll?.contentView)
        settle(p, y: 100.25, content: 120)
        XCTAssertEqual(clip.bounds.origin.y, 0, "at the top of its range")
        let end = clip.constrainBoundsRect(NSRect(origin: NSPoint(x: 0, y: 1000), size: clip.bounds.size))
        XCTAssertEqual(end.maxY, 120, accuracy: 0.5, "and at its end, past which nothing goes")
    }
}
#endif

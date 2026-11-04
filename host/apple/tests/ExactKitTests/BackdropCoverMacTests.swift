#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A focused button under a later `backdrop-filter` node (LLP 1115.002 D7,
/// the user's ruling of 2026-11-05): AppKit's ring is not blurred by the
/// cover, so the button draws none while the cover is over its ring's
/// bounds, keeps the focus, and is ringed again when the cover goes.
final class BackdropCoverMacTests: XCTestCase {
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil }

    private let cover: [String: Any] = ["backdrop_filter": [["blur": 4.0]], "background_color": [0, 0, 0, 163]]

    /// A page (id 1) with a button (id 2) at 10, 10, 40 × 30, focused as the
    /// keyboard focuses it.
    private func page() throws -> (Presenter, NodeView) {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 200, height: 200)
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 200, "h": 200],
            ["op": "create", "id": 2, "kind": "button", "props": ["id": "toggle"], "handlers": ["press"]],
            ["op": "frame", "id": 2, "x": 10, "y": 10, "w": 40, "h": 30],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
        ]))
        let button = try XCTUnwrap(p.views[2])
        XCTAssertTrue(window.makeFirstResponder(button))
        XCTAssertTrue(button.focusVisible)
        return (p, button)
    }

    private func addCover(_ p: Presenter, x: Int = 0, y: Int = 0, w: Int = 200, h: Int = 200, before: Bool = false) {
        p.apply(wireBatch([
            ["op": "create", "id": 3, "kind": "view", "style": cover],
            ["op": "frame", "id": 3, "x": x, "y": y, "w": w, "h": h],
            ["op": "children", "id": 1, "ids": before ? [3, 2] : [2, 3]],
        ]))
    }

    func testALaterBackdropOverTheRingTakesTheRingAndLeavesTheFocus() throws {
        let (p, button) = try page()
        XCTAssertEqual(button.focusRingMaskBounds, button.bounds)
        addCover(p)
        XCTAssertEqual(button.focusRingMaskBounds, .zero, "no sharp ring through the blur")
        XCTAssertTrue(window?.firstResponder === button, "the focus stays")
        XCTAssertTrue(button.focusVisible)
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2]], ["op": "destroy", "id": 3]]))
        XCTAssertEqual(button.focusRingMaskBounds, button.bounds, "the ring comes back when the cover goes")
    }

    func testTheRingStaysWhereNoCoverIsOverIt() throws {
        let (p, button) = try page()
        addCover(p, before: true)
        XCTAssertEqual(button.focusRingMaskBounds, button.bounds, "an earlier backdrop paints under the button")
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 3]], ["op": "frame", "id": 3, "x": 30, "y": 0, "w": 170, "h": 200]]))
        XCTAssertEqual(button.focusRingMaskBounds, button.bounds, "a cover over part of the ring leaves it")
        p.apply(wireBatch([["op": "frame", "id": 3, "x": 0, "y": 0, "w": 200, "h": 200]]))
        XCTAssertEqual(button.focusRingMaskBounds, .zero)
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["background_color": [0, 0, 0, 163]]]]))
        XCTAssertEqual(button.focusRingMaskBounds, button.bounds, "a box with no backdrop is no cover")
        p.apply(wireBatch([["op": "style", "id": 3, "style": cover.merging(["visibility": "hidden"]) { $1 }]]))
        XCTAssertEqual(button.focusRingMaskBounds, button.bounds, "nor a hidden backdrop")
    }
}
#endif

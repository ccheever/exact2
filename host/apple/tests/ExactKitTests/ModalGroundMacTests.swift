// @ref LLP 1115.002 D7.1 — a modal route's surface on the Mac: the page's
// ground where the route paints none, so the route it covers does not show
// through it (Shelf's library under the New Book form's header).
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class ModalGroundMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    private let white = [255.0, 255.0, 255.0, 255.0], red = [255.0, 0.0, 0.0, 255.0], blue = [0.0, 0.0, 255.0, 255.0]

    /// A navigation root (1) on a white page whose stack is a red route
    /// (2, `a`) and a modal one (3, `b`) holding a blue box (4) at its top
    /// left; the root names `selected`.
    private func fixture(selected: String) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 200, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["navigationBack": "back", "navigationKey": selected], "style": ["background_color": white]],
            ["op": "create", "id": 2, "kind": "view", "props": ["navigationKey": "a"], "style": ["background_color": red]],
            ["op": "create", "id": 3, "kind": "view", "props": ["navigationKey": "b", "navigationPresentation": "modal"]],
            ["op": "create", "id": 4, "kind": "view", "style": ["background_color": blue]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 200.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 200.0, "h": 200.0],
            ["op": "frame", "id": 4, "x": 20.0, "y": 20.0, "w": 40.0, "h": 40.0],
        ]))
        return p
    }

    /// The page's pixel at a point, by name: a capture comes back in the
    /// display's colour, so a pure channel reads near its value, not at it.
    private func pixel(_ p: Presenter, _ x: CGFloat, _ y: CGFloat) throws -> String {
        let page = try XCTUnwrap(p.views[1])
        let rep = try XCTUnwrap(Capture.picture(of: page))
        let scale = CGFloat(rep.pixelsWide) / rep.size.width
        let c = try XCTUnwrap(rep.colorAt(x: Int(x * scale), y: Int(y * scale))?.usingColorSpace(.sRGB))
        switch [c.redComponent, c.greenComponent, c.blueComponent].map({ $0 > 0.6 }) {
        case [true, true, true]: return "white"
        case [true, false, false]: return "red"
        case [false, true, false]: return "green"
        case [false, false, true]: return "blue"
        default: return "\(c)"
        }
    }

    func testAModalRouteStandsOnThePagesGroundAndAnAuthoredBackgroundIsItsOwn() throws {
        let p = fixture(selected: "b")
        let beneath = try XCTUnwrap(p.views[2]), modal = try XCTUnwrap(p.views[3])
        // The page stays under a sheet: the covered route is still shown.
        XCTAssertFalse(beneath.isHidden)
        XCTAssertFalse(modal.isHidden)
        XCTAssertEqual(try pixel(p, 150, 150), "white", "the page's ground, not the red route beneath")
        XCTAssertEqual(try pixel(p, 40, 40), "blue", "what the route holds paints over its ground")
        // The ground is the route's size, and stays so.
        let ground = try XCTUnwrap(modal.subviews.first as? ModalGround)
        XCTAssertEqual(ground.frame, modal.bounds)
        p.apply(wireBatch([["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 120.0, "h": 200.0]]))
        XCTAssertEqual(ground.frame, modal.bounds)
        XCTAssertEqual(try pixel(p, 100, 150), "white")
        XCTAssertEqual(try pixel(p, 150, 150), "red", "beside a route narrower than the page, the page beneath shows")
        // A click on the ground is a click on the route, not on a view of the host's.
        XCTAssertFalse(p.root.hitTest(NSPoint(x: 100, y: 150)) is ModalGround)
        // The page's ground changed: the route's follows.
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["background_color": [0.0, 255.0, 0.0, 255.0]]]]))
        XCTAssertEqual(try pixel(p, 100, 150), "green")
        // What the author wrote is the surface: no ground of the host's under it.
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["background_color": blue]]]))
        XCTAssertFalse(modal.subviews.contains { $0 is ModalGround })
        XCTAssertEqual(try pixel(p, 100, 150), "blue")
        p.apply(wireBatch([["op": "style", "id": 3, "style": [:]]]))
        XCTAssertTrue(modal.subviews.first is ModalGround, "unsaid again, the page's again")
        // The sheet closed: the route beneath is the page.
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["navigationKey": "a"]]]))
        XCTAssertTrue(modal.isHidden)
        XCTAssertEqual(try pixel(p, 100, 150), "red")
        // Only a modal route covers a shown one: any other gets no ground.
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["navigationPresentation": "fullscreen"]],
                           ["op": "props", "id": 1, "set": ["navigationKey": "b"]]]))
        XCTAssertTrue(beneath.isHidden)
        for route in [beneath, modal] { XCTAssertFalse(route.subviews.contains { $0 is ModalGround }) }
    }
}
#endif

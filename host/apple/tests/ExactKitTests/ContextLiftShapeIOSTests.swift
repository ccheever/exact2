#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// The shape UIKit lifts a context menu's node in (`ContextMenuHost.targeted`)
/// when the node paints no ground of its own: the grounds the nodes inside it
/// paint, as a chat balloon's body and tail, so long as its text lies on them.
final class ContextLiftShapeIOSTests: XCTestCase {
    private var window: UIWindow!
    private var session: ExactSession?

    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; super.tearDown() }

    /// A balloon at (100, 100), 120 by 40: a body clipped to a rectangle and
    /// a tail clipped to a triangle 8 points below it, both blue, and its
    /// text. `textAt` moves the text's frame.
    private func balloon(textAt text: CGRect = CGRect(x: 14, y: 8, width: 92, height: 24)) throws -> Presenter {
        let session = ExactApp.shared.makeSession(label: "lift-shape")
        self.session = session
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        let blue: [Double] = [0, 136, 255, 255]
        let body: [String: Any] = ["commands": [["M", [0, 0]], ["L", [120, 0]], ["L", [120, 40]], ["L", [0, 40]], ["Z", []]]]
        let tail: [String: Any] = ["commands": [["M", [104, 40]], ["L", [120, 40]], ["L", [120, 48]], ["Z", []]]]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "props": ["contextPopover": "message-menu"], "handlers": ["contextmenu"]],
            ["op": "create", "id": 3, "kind": "view", "style": ["background_color": blue, "clip_path": tail]],
            ["op": "create", "id": 4, "kind": "view", "style": ["background_color": blue, "clip_path": body]],
            ["op": "create", "id": 5, "kind": "text", "style": ["font_size": 17.0]],
            ["op": "create", "id": 6, "kind": "view", "props": ["popover": "auto", "id": "message-menu"]],
            ["op": "create", "id": 7, "kind": "button", "handlers": ["press"]],
            ["op": "children", "id": 6, "ids": [7]],
            ["op": "children", "id": 2, "ids": [3, 4, 5]],
            ["op": "children", "id": 1, "ids": [2, 6]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 800],
            ["op": "frame", "id": 2, "x": 100, "y": 100, "w": 120, "h": 40],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 120, "h": 48],
            ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 120, "h": 48],
            ["op": "frame", "id": 5, "x": text.minX, "y": text.minY, "w": text.width, "h": text.height],
        ]))
        p.buttonFace = { _ in var face = ButtonFace(); face.title = "Copy"; return face }
        p.menus.context.sync()
        return p
    }

    private func lifted(_ p: Presenter) throws -> UITargetedPreview {
        let node = try XCTUnwrap(p.views[2])
        let interaction = try XCTUnwrap(node.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let config = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: 60, y: 20)))
        return try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, previewForHighlightingMenuWithConfiguration: config))
    }

    func testABalloonLiftsInItsBodyAndItsTail() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        let p = try balloon()
        defer { p.menus.reset() }
        let path = try XCTUnwrap(try lifted(p).parameters.visiblePath)
        XCTAssertEqual(path.bounds, CGRect(x: 0, y: 0, width: 120, height: 48), "the body and the tail below it")
        XCTAssertTrue(path.contains(CGPoint(x: 60, y: 20)), "the body")
        XCTAssertTrue(path.contains(CGPoint(x: 118, y: 44)), "the tail")
        XCTAssertFalse(path.contains(CGPoint(x: 4, y: 44)), "beside the tail, under the body")
    }

    func testANodeWhoseTextLiesOffItsGroundsLiftsWhole() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        // The text beside the shapes, as a name beside an avatar
        let p = try balloon(textAt: CGRect(x: 14, y: 60, width: 92, height: 24))
        defer { p.menus.reset() }
        XCTAssertNil(try lifted(p).parameters.visiblePath)
    }
}
#endif

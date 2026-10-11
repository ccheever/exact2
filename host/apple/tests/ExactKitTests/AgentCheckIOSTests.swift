#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// `check` on UIKit (LLP 1116.003 D8): the observations the driver judges —
/// a paragraph's ink and whether it fits its box, a control covered by
/// another node, a target the author sized small, a view past the viewport's
/// side, a card's fill without an edge, two controls overlapping — read from
/// the presented view tree.
final class AgentCheckIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    private func mounted(_ ops: [[String: Any]]) -> Agent {
        let session = ExactApp.shared.makeSession(label: "agent-check")
        self.session = session
        let p = session.presenter
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        self.window = window
        p.viewport.frame = window.bounds
        window.rootViewController?.view.addSubview(p.viewport)
        p.apply(wireBatch(ops))
        p.viewport.layoutIfNeeded()
        return Agent(session: session)
    }

    func testCheckObservesInkFitCoverTargetsOverflowCardsAndOverlap() throws {
        let white: [Double] = [255, 255, 255, 255]
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": white]],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Faint", "testId": "faint"], "style": ["text_color": [187, 187, 187, 255], "font_size": 17]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "A long label that will not fit in its box"], "style": ["overflow_x": "hidden", "overflow_y": "hidden", "font_size": 17]],
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"], "props": ["testId": "under"]],
            ["op": "create", "id": 5, "kind": "view", "props": ["testId": "cover"], "style": ["background_color": [200, 200, 200, 255]]],
            ["op": "create", "id": 6, "kind": "view", "handlers": ["press"], "props": ["testId": "tiny"]],
            ["op": "create", "id": 7, "kind": "view", "props": ["testId": "wide"]],
            ["op": "create", "id": 8, "kind": "view", "props": ["testId": "card"], "style": ["background_color": white, "border_radius_top_left": 12, "border_radius_top_right": 12, "border_radius_bottom_right": 12, "border_radius_bottom_left": 12]],
            ["op": "create", "id": 9, "kind": "view", "handlers": ["press"], "props": ["testId": "a"]],
            ["op": "create", "id": 10, "kind": "view", "handlers": ["press"], "props": ["testId": "b"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5, 6, 7, 8, 9, 10]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 16.0, "y": 20.0, "w": 60.0, "h": 22.0],
            ["op": "frame", "id": 3, "x": 16.0, "y": 60.0, "w": 60.0, "h": 22.0],
            ["op": "frame", "id": 4, "x": 16.0, "y": 100.0, "w": 120.0, "h": 44.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 90.0, "w": 400.0, "h": 70.0],
            ["op": "frame", "id": 6, "x": 16.0, "y": 180.0, "w": 20.0, "h": 20.0],
            ["op": "frame", "id": 7, "x": 16.0, "y": 220.0, "w": 600.0, "h": 20.0],
            ["op": "frame", "id": 8, "x": 16.0, "y": 260.0, "w": 368.0, "h": 80.0],
            ["op": "frame", "id": 9, "x": 16.0, "y": 360.0, "w": 100.0, "h": 44.0],
            ["op": "frame", "id": 10, "x": 96.0, "y": 360.0, "w": 100.0, "h": 44.0],
        ])
        let reply = agent.check([:])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["target"] as? Int, 44)
        XCTAssertEqual(reply["scheme"] as? String, "light")
        let items = try XCTUnwrap(reply["items"] as? [[String: Any]])
        func item(_ id: Int) throws -> [String: Any] { try XCTUnwrap(items.first { $0["id"] as? Int == id }, "#\(id) in \(items)") }
        let faint = try item(2)
        XCTAssertEqual(faint["text"] as? String, "Faint")
        XCTAssertEqual((faint["fg"] as? [Double])?.prefix(3).map { $0 }, [187, 187, 187], "the paragraph's ink: \(faint)")
        XCTAssertEqual(faint["size"] as? Double, 17)
        XCTAssertNil(faint["over"], "it fits")
        let clipped = try item(3)
        let over = try XCTUnwrap(clipped["over"] as? [Double], "a wrapped label taller than its box: \(clipped)")
        XCTAssertGreaterThan(over[1], 2)
        XCTAssertEqual(clipped["clips"] as? Bool, true)
        let under = try item(4)
        let cover = try XCTUnwrap(under["cover"] as? [String: Any], "the cover over its middle: \(under)")
        XCTAssertEqual(cover["by"] as? Int, 5)
        XCTAssertEqual(cover["kind"] as? String, "node")
        let tiny = try item(6)
        XCTAssertEqual(tiny["control"] as? String, "press")
        XCTAssertEqual(tiny["sized"] as? String, "author")
        XCTAssertEqual(tiny["w"] as? Double, 20)
        XCTAssertEqual(try item(7)["outX"] as? Double, 216, "600 wide from 16, 216 past a 400-wide viewport")
        let card = try item(8)
        XCTAssertEqual(card["fill"] as? [Double], [255, 255, 255, 1])
        XCTAssertEqual(card["radius"] as? Double, 12)
        XCTAssertNil(card["edge"], "no border, no shadow")
        XCTAssertEqual(try item(9)["control"] as? String, "press")
        XCTAssertEqual(try item(10)["x"] as? Double, 96, "b starts inside a: the driver finds them overlapping")
    }
}
#endif

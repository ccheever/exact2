#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1066 D7: `background-attachment: fixed` sizes a gradient on the
/// viewport. Every node with it shows its part of one gradient (a
/// messenger's outgoing bubbles, dark at the top of the screen and light at
/// the bottom), re-aimed as its scroller moves; a bubble's corners of
/// different radii, which `draw(_:)` paints, still take the layer, masked.
final class FixedGradientIOSTests: XCTestCase {
    private var window: UIWindow!
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }

    private func presenter() -> Presenter {
        let p = Presenter()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300))
        window.frame = CGRect(x: 0, y: 0, width: 300, height: 300)
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        return p
    }

    private let gradient: [String: Any] = ["linear": 180, "stops": [0, 5, 82, 240, 255, 1, 44, 107, 237, 255]]

    func testAFixedGradientIsAimedAtTheViewportAndFollowsTheScroll() throws {
        let p = presenter()
        var bubble: [String: Any] = ["background_image": gradient, "background_attachment": "fixed"]
        var joined = bubble
        joined["border_radius_top_right"] = 4.0
        for corner in ["top_left", "bottom_right", "bottom_left"] { joined["border_radius_" + corner] = 18.0 }
        bubble["border_radius_top_left"] = 18.0
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view", "style": bubble],
            ["op": "create", "id": 3, "kind": "view", "style": joined],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 200.0, "w": 200.0, "h": 100.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 1000.0],
        ]))
        let first = try XCTUnwrap(p.views[2]), second = try XCTUnwrap(p.views[3])
        for n in [first, second] { n.layer.displayIfNeeded() }
        let top = try XCTUnwrap(first.boxGradient, "a fixed gradient is a layer")
        let joinedLayer = try XCTUnwrap(second.boxGradient, "over a drawn box too")
        XCTAssertTrue(second.drawsPaint, "corners of different radii draw")
        XCTAssertNotNil(joinedLayer.mask as? CAShapeLayer, "masked to the drawn outline")
        // One gradient over the 300-point viewport: the first bubble (y 0,
        // 100 tall) shows its top third, the second (y 200) its bottom third.
        XCTAssertEqual(top.startPoint.y, 0, accuracy: 0.01)
        XCTAssertEqual(top.endPoint.y, 3, accuracy: 0.01)
        XCTAssertEqual(joinedLayer.startPoint.y, -2, accuracy: 0.01)
        XCTAssertEqual(joinedLayer.endPoint.y, 1, accuracy: 0.01)
        // Scrolled 100: the gradient stays, so each bubble shows a lower part.
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        scroll.contentOffset.y = 100
        XCTAssertEqual(top.startPoint.y, 1, accuracy: 0.01)
        XCTAssertEqual(joinedLayer.startPoint.y, -1, accuracy: 0.01)
        XCTAssertEqual(joinedLayer.endPoint.y, 2, accuracy: 0.01)
    }

    func testAnUnfixedGradientKeepsItsPaddingBox() throws {
        let p = presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_image": gradient, "border_radius_top_left": 12.0, "border_radius_top_right": 12.0, "border_radius_bottom_right": 12.0, "border_radius_bottom_left": 12.0]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 150.0, "w": 200.0, "h": 100.0],
        ]))
        let n = try XCTUnwrap(p.views[1])
        n.layer.displayIfNeeded()
        let g = try XCTUnwrap(n.boxGradient)
        XCTAssertEqual(g.startPoint.y, 0, accuracy: 0.01)
        XCTAssertEqual(g.endPoint.y, 1, accuracy: 0.01)
        XCTAssertNil(p.fixedGradients.allObjects.first)
    }
}
#endif

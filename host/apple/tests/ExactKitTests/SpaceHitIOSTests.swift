#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactSurfaces

/// A box turned in space (LLP 1077 D8) takes touches where it is drawn, as
/// CSS hit-tests through the 3D projection, and a hidden back face takes
/// none. UIKit carries a point through the plane itself (measured here);
/// the host refuses the hidden back face. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class SpaceHitIOSTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactSurfaces.install() } // LLP 1047.001 D4
    private var window: UIWindow!

    /// A 200×200 parent with `perspective: 200px` holding a 100×100 button
    /// at (50, 50) turned about y by `degrees`.
    private func fixture(degrees: Double, axis: [Double] = [0, 1, 0], child: [String: Any] = [:]) throws -> (Presenter, NodeView, NodeView) {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        var style: [String: Any] = ["rotate_axis": axis]
        for (k, v) in child { style[k] = v }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["perspective": 200.0]],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"], "style": style],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 50.0, "y": 50.0, "w": 100.0, "h": 100.0],
            ["op": "present", "id": 2, "property": "rotate", "x": degrees],
        ]))
        window.layoutIfNeeded() // the perspective's origin follows the laid-out box
        return (p, try XCTUnwrap(p.views[1]), try XCTUnwrap(p.views[2]))
    }

    // Turned 60° about y under a 200 px perspective, the near (left) edge
    // stands at x ≈ 68 and runs y ≈ 36…164; the far (right) edge at x ≈ 121.
    func testATurnedBoxIsHitWhereItIsDrawn() throws {
        let (_, parent, child) = try fixture(degrees: 60)
        XCTAssertTrue(parent.hitTest(CGPoint(x: 75, y: 45), with: nil) === child, "inside the drawn quad, outside the flat frame")
        XCTAssertTrue(parent.hitTest(CGPoint(x: 100, y: 100), with: nil) === child, "the centre")
        XCTAssertFalse(parent.hitTest(CGPoint(x: 140, y: 100), with: nil) === child, "inside the flat frame, beyond the far edge")
        XCTAssertFalse(parent.hitTest(CGPoint(x: 55, y: 100), with: nil) === child, "inside the flat frame, before the near edge")
        // A press inside resolves through the same projection.
        let window = try XCTUnwrap(child.window)
        let inChild = child.local(parent.convert(CGPoint(x: 75, y: 45), to: window))
        XCTAssertTrue(child.bounds.contains(inChild), "\(inChild)")
        // UIKit's conversion carries the plane; the host's own plane (what
        // macOS maps through, and the back-face test) is the same geometry.
        let h = try XCTUnwrap(child.plane)
        for q in [CGPoint(x: 0, y: 0), CGPoint(x: 100, y: 0), CGPoint(x: 30, y: 80), CGPoint(x: 100, y: 100)] {
            let ours = NodeView.map(h, q), uikit = child.convert(q, to: parent)
            XCTAssertEqual(ours.x, uikit.x, accuracy: 1e-3, "\(q)")
            XCTAssertEqual(ours.y, uikit.y, accuracy: 1e-3, "\(q)")
        }
    }

    // Moved 100 px toward the viewer under a 300 px perspective from the
    // top left, a 60 px box at (100, 100) is drawn half again as large at
    // (150, 150)…(240, 240), clear of its frame.
    func testABoxMovedAlongZIsHitWhereItIsDrawn() throws {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["perspective": 300.0, "perspective_origin": [0.0, 0.0]]],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"], "style": ["translate_z": 100.0]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 100.0, "y": 100.0, "w": 60.0, "h": 60.0],
        ]))
        window.layoutIfNeeded()
        let parent = try XCTUnwrap(p.views[1]), child = try XCTUnwrap(p.views[2])
        XCTAssertTrue(parent.hitTest(CGPoint(x: 200, y: 200), with: nil) === child)
        XCTAssertTrue(parent.hitTest(CGPoint(x: 235, y: 235), with: nil) === child)
        XCTAssertFalse(parent.hitTest(CGPoint(x: 110, y: 110), with: nil) === child, "its flat frame, where nothing of it is drawn")
        // The perspective about an origin off the holder's anchor, too.
        let h = try XCTUnwrap(child.plane)
        for q in [CGPoint(x: 0, y: 0), CGPoint(x: 60, y: 60)] {
            let ours = NodeView.map(h, q), uikit = child.convert(q, to: parent)
            XCTAssertEqual(ours.x, uikit.x, accuracy: 1e-3, "\(q)")
            XCTAssertEqual(ours.y, uikit.y, accuracy: 1e-3, "\(q)")
        }
        XCTAssertEqual(NodeView.map(h, .zero).x, 150, accuracy: 1e-3)
    }

    // A canvas whose surface placed a 60 pt button 50 pt in, inside a box
    // translated 100 pt: the button is drawn at (150, 50)…(210, 110). UIKit
    // carries the ancestor's transform; the placement composes with it for
    // the hit, a point in the button and the agent's box.
    func testACanvasPlacedChildInsideATranslatedBoxResolvesWhereDrawn() throws {
        let session = ExactApp.shared.makeSession(label: "placed-in-translated")
        defer { session.destroy() }
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "canvas"],
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 200.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 60.0, "h": 60.0],
            ["op": "present", "id": 1, "property": "translate", "x": 100.0, "y": 0.0],
        ]))
        window.layoutIfNeeded()
        let box = try XCTUnwrap(p.views[1]), button = try XCTUnwrap(p.views[3])
        button.placement = [1, 0, 50, 0, 1, 50, 0, 0, 1, 0]
        button.alpha = 0
        let sup = try XCTUnwrap(box.superview)
        XCTAssertTrue(box.hitTest(sup.convert(CGPoint(x: 180, y: 80), to: box), with: nil) === button, "where it is drawn")
        XCTAssertFalse(box.hitTest(sup.convert(CGPoint(x: 80, y: 80), to: box), with: nil) === button, "placed, but not translated")
        // A point that misses every placed child is the canvas's, unless the
        // canvas lets the touch through (Grok's batch 2 review).
        let canvas = try XCTUnwrap(p.views[2])
        XCTAssertTrue(box.hitTest(sup.convert(CGPoint(x: 180, y: 150), to: box), with: nil) === canvas)
        canvas.applyStyle(["pointer_events": "none"])
        XCTAssertFalse(box.hitTest(sup.convert(CGPoint(x: 180, y: 150), to: box), with: nil) === canvas, "pointer-events: none")
        XCTAssertTrue(box.hitTest(sup.convert(CGPoint(x: 180, y: 80), to: box), with: nil) === button, "its placed child still takes it")
        canvas.applyStyle([:])
        let inButton = button.local(sup.convert(CGPoint(x: 180, y: 80), to: nil))
        XCTAssertEqual(inButton.x, 30, accuracy: 1e-6)
        XCTAssertEqual(inButton.y, 30, accuracy: 1e-6)
        let seen = Agent(session: session).box(button)
        let expected = sup.convert(CGPoint(x: 150, y: 50), to: p.viewport)
        XCTAssertEqual(seen.minX, expected.x - p.viewport.contentOffset.x, accuracy: 1e-6)
        XCTAssertEqual(seen.minY, expected.y - p.viewport.contentOffset.y, accuracy: 1e-6)
        XCTAssertEqual(seen.width, 60, accuracy: 1e-6)
        XCTAssertEqual(seen.height, 60, accuracy: 1e-6)
    }

    func testAHiddenBackFaceTakesNoHit() throws {
        let (_, parent, child) = try fixture(degrees: 150, child: ["backface_visibility": "hidden"])
        XCTAssertFalse(parent.hitTest(CGPoint(x: 100, y: 100), with: nil) === child)
        let (_, shown, back) = try fixture(degrees: 150)
        XCTAssertTrue(shown.hitTest(CGPoint(x: 100, y: 100), with: nil) === back, "a visible back face is hit")
        let (_, front, face) = try fixture(degrees: 30, child: ["backface_visibility": "hidden"])
        XCTAssertTrue(front.hitTest(CGPoint(x: 100, y: 100), with: nil) === face, "a hidden back face turned toward the viewer is hit")
    }

    func testAnUnturnedBoxIsUnchanged() throws {
        let (_, parent, child) = try fixture(degrees: 0)
        XCTAssertTrue(parent.hitTest(CGPoint(x: 55, y: 55), with: nil) === child)
        XCTAssertTrue(parent.hitTest(CGPoint(x: 145, y: 145), with: nil) === child)
        XCTAssertFalse(parent.hitTest(CGPoint(x: 45, y: 100), with: nil) === child)
        // A 2D turn stays UIKit's.
        let (_, flat, turned) = try fixture(degrees: 45, axis: [0, 0, 1])
        XCTAssertTrue(flat.hitTest(CGPoint(x: 100, y: 35), with: nil) === turned, "a 45° diamond's top corner")
        XCTAssertFalse(flat.hitTest(CGPoint(x: 55, y: 55), with: nil) === turned, "the flat frame's corner")
    }
}
#endif

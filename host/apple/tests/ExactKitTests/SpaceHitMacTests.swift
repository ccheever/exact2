#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit
@testable import ExactSurfaces

/// A transformed box takes the mouse where it is drawn, as CSS hit-tests
/// through the transform — 2D, or in space (LLP 1077 D8), where a hidden back
/// face takes none.
/// AppKit's own geometry places every box at its frame, so the host maps the
/// point through the box's plane (`SpaceTransform.swift`).
final class SpaceHitMacTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactSurfaces.install() } // LLP 1047.001 D4
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil }

    /// A parent with `perspective` holding a button, turned or moved in z.
    private func fixture(parent: [String: Any] = ["perspective": 200.0], size: Double = 200, child: [String: Any], frame: [Double] = [50, 50, 100, 100], degrees: Double = 0, present: [[String: Any]] = []) -> (Presenter, NodeView, NodeView) {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 400)
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": parent],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"], "style": child],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": size, "h": size],
            ["op": "frame", "id": 2, "x": frame[0], "y": frame[1], "w": frame[2], "h": frame[3]],
            ["op": "present", "id": 2, "property": "rotate", "x": degrees],
        ] + present))
        // A laid-out window: AppKit rewrites layer geometry as it lays out,
        // and the host puts the transform and perspective back (`layout()`).
        p.viewport.layoutSubtreeIfNeeded()
        return (p, p.views[1]!, p.views[2]!)
    }

    /// What the mouse finds at a point in the parent's coordinates.
    private func hit(_ parent: NodeView, _ x: CGFloat, _ y: CGFloat) -> NSView? {
        parent.hitTest(parent.superview!.convert(NSPoint(x: x, y: y), from: parent))
    }

    // Turned 60° about y under a 200 px perspective, the near (left) edge
    // stands at x ≈ 68 and runs y ≈ 36…164; the far (right) edge at x ≈ 121.
    func testATurnedBoxIsHitWhereItIsDrawn() {
        let (_, parent, child) = fixture(child: ["rotate_axis": [0, 1, 0]], degrees: 60)
        XCTAssertTrue(hit(parent, 75, 45) === child, "inside the drawn quad, outside the flat frame")
        XCTAssertTrue(hit(parent, 100, 100) === child, "the centre")
        XCTAssertFalse(hit(parent, 140, 100) === child, "inside the flat frame, beyond the far edge")
        XCTAssertFalse(hit(parent, 55, 100) === child, "inside the flat frame, before the near edge")
        // A click inside resolves through the same plane, and is inside.
        let inWindow = parent.convert(NSPoint(x: 75, y: 45), to: nil)
        XCTAssertTrue(child.bounds.contains(child.local(inWindow)), "\(child.local(inWindow))")
        XCTAssertTrue(child.pressInside(inWindow))
        XCTAssertFalse(child.pressInside(parent.convert(NSPoint(x: 140, y: 100), to: nil)))
    }

    // Moved 100 px toward the viewer under a 300 px perspective from the
    // top left, a 60 px box at (100, 100) is drawn half again as large at
    // (150, 150)…(240, 240), clear of its frame.
    func testABoxMovedAlongZIsHitWhereItIsDrawn() {
        let (_, parent, child) = fixture(parent: ["perspective": 300.0, "perspective_origin": [0.0, 0.0]], size: 300,
                                         child: ["translate_z": 100.0], frame: [100, 100, 60, 60])
        XCTAssertTrue(hit(parent, 200, 200) === child)
        XCTAssertTrue(hit(parent, 235, 235) === child)
        XCTAssertFalse(hit(parent, 110, 110) === child, "its flat frame, where nothing of it is drawn")
    }

    func testAHiddenBackFaceTakesNoHit() {
        let (_, parent, child) = fixture(child: ["rotate_axis": [0, 1, 0], "backface_visibility": "hidden"], degrees: 150)
        XCTAssertFalse(hit(parent, 100, 100) === child)
        let (_, shown, back) = fixture(child: ["rotate_axis": [0, 1, 0]], degrees: 150)
        XCTAssertTrue(hit(shown, 100, 100) === back, "a visible back face is hit")
        let (_, front, face) = fixture(child: ["rotate_axis": [0, 1, 0], "backface_visibility": "hidden"], degrees: 30)
        XCTAssertTrue(hit(front, 100, 100) === face, "a hidden back face turned toward the viewer is hit")
    }

    // 2D transforms: AppKit places a box at its frame whatever its layer's
    // transform, so these go through the same plane.
    func testATranslatedBoxIsHitWhereItIsDrawn() {
        let (_, parent, child) = fixture(parent: [:], size: 300, child: [:], frame: [50, 50, 60, 60],
                                         present: [["op": "present", "id": 2, "property": "translate", "x": 100.0, "y": 0.0]])
        XCTAssertTrue(hit(parent, 180, 80) === child, "where it is drawn, 150…210")
        XCTAssertFalse(hit(parent, 80, 80) === child, "its flat frame")
        let inWindow = parent.convert(NSPoint(x: 180, y: 80), to: nil)
        XCTAssertEqual(child.local(inWindow).x, 30, accuracy: 1e-6)
        XCTAssertEqual(child.local(inWindow).y, 30, accuracy: 1e-6)
        XCTAssertTrue(child.pressInside(inWindow))
    }

    func testARotatedBoxIsHitWhereItIsDrawn() {
        // Turned 45° about its centre (150, 150): a diamond whose top corner
        // is at y ≈ 79 and whose sides cut off the frame's corners.
        let (_, parent, child) = fixture(parent: [:], size: 300, child: [:], frame: [100, 100, 100, 100], degrees: 45)
        XCTAssertTrue(hit(parent, 150, 85) === child, "the diamond's top corner, above the frame")
        XCTAssertFalse(hit(parent, 105, 105) === child, "the frame's corner, outside the diamond")
    }

    func testAPressIsUndoneOnce() {
        // Pressed to half its size about its centre (100, 100): drawn at
        // 75…125. The hit is where it is drawn; whether a held press is still
        // inside is the box unpressed, its plane's press undone once.
        let (_, parent, child) = fixture(parent: [:], child: ["press_scale": 0.5])
        child.press = PressFeedback(from: 0.5, to: 0.5, start: 0)
        child.applyTransform()
        XCTAssertEqual(child.pressFactor, 0.5, accuracy: 1e-9)
        XCTAssertTrue(hit(parent, 100, 100) === child)
        XCTAssertFalse(hit(parent, 60, 100) === child, "outside the pressed box as drawn")
        XCTAssertTrue(child.pressInside(parent.convert(NSPoint(x: 60, y: 100), to: nil)), "inside the box unpressed")
        XCTAssertFalse(child.pressInside(parent.convert(NSPoint(x: 40, y: 100), to: nil)), "outside it")
    }

    // A canvas whose surface placed a 60 pt button 50 pt in, inside a box
    // translated 100 pt: the button is drawn at (150, 50)…(210, 110) of the
    // box's untranslated space. The hit, a point in the button (the release's
    // inside test, inline and SVG targets) and the agent's box all compose
    // the ancestor's transform, the placement and the button's own space.
    func testACanvasPlacedChildInsideATranslatedBoxResolvesWhereDrawn() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "placed-in-translated")
        defer { session.destroy() }
        let p = session.presenter
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 400)
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
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
        p.viewport.layoutSubtreeIfNeeded()
        let box = try XCTUnwrap(p.views[1]), button = try XCTUnwrap(p.views[3])
        button.placement = [1, 0, 50, 0, 1, 50, 0, 0, 1, 0]
        button.alphaValue = 0
        // Points in the box's untranslated space, which AppKit's own
        // conversion uses (it ignores the layer's transform).
        let sup = try XCTUnwrap(box.superview)
        func inWindow(_ x: CGFloat, _ y: CGFloat) -> NSPoint { box.convert(NSPoint(x: x, y: y), to: nil) }
        XCTAssertTrue(box.hitTest(sup.convert(NSPoint(x: 180, y: 80), from: box)) === button, "where it is drawn")
        XCTAssertFalse(box.hitTest(sup.convert(NSPoint(x: 80, y: 80), from: box)) === button, "placed, but not translated")
        // A point that misses every placed child is the canvas's, unless the
        // canvas lets the pointer through (Grok's batch 2 review).
        let canvas = try XCTUnwrap(p.views[2])
        XCTAssertTrue(box.hitTest(sup.convert(NSPoint(x: 180, y: 150), from: box)) === canvas)
        canvas.applyStyle(["pointer_events": "none"])
        XCTAssertFalse(box.hitTest(sup.convert(NSPoint(x: 180, y: 150), from: box)) === canvas, "pointer-events: none")
        XCTAssertTrue(box.hitTest(sup.convert(NSPoint(x: 180, y: 80), from: box)) === button, "its placed child still takes it")
        canvas.applyStyle([:])
        XCTAssertEqual(button.local(inWindow(180, 80)).x, 30, accuracy: 1e-6)
        XCTAssertEqual(button.local(inWindow(180, 80)).y, 30, accuracy: 1e-6)
        XCTAssertTrue(button.pressInside(inWindow(180, 80)), "a release where it is drawn is inside")
        XCTAssertFalse(button.pressInside(inWindow(80, 80)))
        let seen = Agent(session: session).box(button)
        let clip = p.viewport.contentView
        let expected = clip.convert(NSPoint(x: 150, y: 50), from: box)
        XCTAssertEqual(seen.minX, expected.x - clip.bounds.origin.x, accuracy: 1e-6)
        XCTAssertEqual(seen.minY, expected.y - clip.bounds.origin.y, accuracy: 1e-6)
        XCTAssertEqual(seen.width, 60, accuracy: 1e-6)
        XCTAssertEqual(seen.height, 60, accuracy: 1e-6)
    }

    func testAnUnturnedBoxIsUnchanged() {
        let (_, parent, child) = fixture(child: ["rotate_axis": [0, 1, 0]], degrees: 0)
        XCTAssertTrue(hit(parent, 55, 55) === child)
        XCTAssertTrue(hit(parent, 145, 145) === child)
        XCTAssertFalse(hit(parent, 45, 100) === child)
        let inWindow = parent.convert(NSPoint(x: 60, y: 70), to: nil)
        XCTAssertEqual(child.local(inWindow).x, 10, accuracy: 1e-6)
        XCTAssertEqual(child.local(inWindow).y, 20, accuracy: 1e-6)
        let (_, flat, plain) = fixture(child: [:])
        XCTAssertNil(plain.plane, "an untransformed box keeps AppKit's own geometry")
        XCTAssertTrue(hit(flat, 55, 55) === plain)
    }
}
#endif

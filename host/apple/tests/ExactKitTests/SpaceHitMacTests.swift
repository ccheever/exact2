#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A box in space (LLP 1077 D8) takes the mouse where it is drawn, as CSS
/// hit-tests through the 3D projection, and a hidden back face takes none.
/// AppKit's own geometry places every box at its frame, so the host maps the
/// point through the box's plane (`SpaceTransform.swift`).
final class SpaceHitMacTests: XCTestCase {
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil }

    /// A parent with `perspective` holding a button, turned or moved in z.
    private func fixture(parent: [String: Any] = ["perspective": 200.0], size: Double = 200, child: [String: Any], frame: [Double] = [50, 50, 100, 100], degrees: Double = 0) -> (Presenter, NodeView, NodeView) {
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
        ]))
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

    func testAnUnturnedBoxIsUnchanged() {
        let (_, parent, child) = fixture(child: ["rotate_axis": [0, 1, 0]], degrees: 0)
        XCTAssertTrue(hit(parent, 55, 55) === child)
        XCTAssertTrue(hit(parent, 145, 145) === child)
        XCTAssertFalse(hit(parent, 45, 100) === child)
        let inWindow = parent.convert(NSPoint(x: 60, y: 70), to: nil)
        XCTAssertEqual(child.local(inWindow).x, 10, accuracy: 1e-6)
        XCTAssertEqual(child.local(inWindow).y, 20, accuracy: 1e-6)
        let (_, flat, plain) = fixture(child: [:])
        XCTAssertNil(plain.plane, "a flat box keeps AppKit's own geometry")
        XCTAssertTrue(hit(flat, 55, 55) === plain)
    }
}
#endif

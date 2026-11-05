#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A box with a backdrop blur and a `box-shadow` on macOS. AppKit filters a
/// layer's backdrop only while the layer masks to its bounds, which would
/// clip the outer shadow CSS draws past the border box: it casts from a
/// layer standing beside the node's, under it, and shows in the window and
/// in the agent's capture.
final class BackdropShadowMacTests: XCTestCase {
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil }

    private let glass: [String: Any] = [
        "backdrop_filter": [["blur": 6.0]], "background_color": [255, 255, 255, 128],
        "box_shadow": [["o": [0, 12], "b": 8, "s": 0, "c": [0, 0, 0, 255]]],
        "border_radius": 8, "border_radius_top_left": 8, "border_radius_top_right": 8,
        "border_radius_bottom_right": 8, "border_radius_bottom_left": 8,
    ]

    /// A white page with the glass box (id 2) at 50, 40, 100 × 60, in a
    /// borderless window on screen.
    private func page(_ siblings: [[String: Any]] = []) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 200, height: 200)
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        window.orderFrontRegardless()
        self.window = window
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": [255, 255, 255, 255]]],
            ["op": "create", "id": 2, "kind": "view", "style": glass],
        ] + siblings + [
            ["op": "children", "id": 1, "ids": [2] + siblings.compactMap { $0["id"] }],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 200, "h": 200],
            ["op": "frame", "id": 2, "x": 50, "y": 40, "w": 100, "h": 60],
        ]))
        settle()
        return p
    }

    /// AppKit attaches new views' layers at the window's display; the
    /// window server shows a committed frame a refresh or two later.
    private func settle() {
        window?.displayIfNeeded()
        CATransaction.flush()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.1))
    }

    private func red(_ rep: NSBitmapImageRep, _ x: CGFloat, _ y: CGFloat) throws -> CGFloat {
        let scale = CGFloat(rep.pixelsWide) / 200
        return try XCTUnwrap(rep.paintedColorAt(x: Int(x * scale), y: Int(y * scale))?.usingColorSpace(.sRGB)).redComponent
    }

    func testABackdropBoxCastsItsShadowOutsideItsBox() throws {
        let p = page()
        let node = try XCTUnwrap(p.views[2]), page = try XCTUnwrap(p.views[1])
        let layer = try XCTUnwrap(node.layer), caster = try XCTUnwrap(node.shadowCaster)
        // The blur is as it was: the node's own layer, masking to its bounds.
        XCTAssertFalse(layer.backgroundFilters?.isEmpty ?? true)
        XCTAssertTrue(layer.masksToBounds)
        // Nothing between the caster and the page's layer clips it.
        var above = caster.superlayer
        while let l = above, l !== page.layer {
            XCTAssertFalse(l.masksToBounds, "\(l) clips the shadow")
            XCTAssertNil(l.mask, "\(l) masks the shadow")
            above = l.superlayer
        }
        XCTAssertTrue(above === page.layer, "the caster stands in the page's layer")

        // The agent's capture: dark under the box, white beside it, and
        // nothing of the shadow inside the translucent box.
        let shot = try XCTUnwrap(Capture.picture(of: p.viewport))
        XCTAssertLessThan(try red(shot, 100, 108), 0.75, "the shadow below the box")
        XCTAssertGreaterThan(try red(shot, 20, 108), 0.98, "the page beside it")
        XCTAssertGreaterThan(try red(shot, 100, 70), 0.95, "the box's inside")

        // The window as the window server composites it.
        guard let window, let live = Agent.ownWindowImage(window.windowNumber) else {
            throw XCTSkip("the window server gave no picture (a display asleep or a locked screen)")
        }
        let rep = NSBitmapImageRep(cgImage: live)
        XCTAssertLessThan(try red(rep, 100, 108), 0.75, "the shadow below the box, in the window")
        XCTAssertGreaterThan(try red(rep, 20, 108), 0.98, "the page beside it, in the window")
    }

    /// The stand is directly under the node's layer and shows as it does.
    private func assertStands(_ node: NodeView, file: StaticString = #filePath, line: UInt = #line) throws {
        let layer = try XCTUnwrap(node.layer, file: file, line: line)
        let stand = try XCTUnwrap(node.shadowStand, file: file, line: line)
        XCTAssertTrue(node.shadowCaster?.superlayer === stand, "the caster is the stand's", file: file, line: line)
        let siblings = try XCTUnwrap(layer.superlayer?.sublayers, file: file, line: line)
        let at = try XCTUnwrap(siblings.firstIndex { $0 === layer }, file: file, line: line)
        XCTAssertTrue(at > 0 && siblings[at - 1] === stand, "directly under the node's layer", file: file, line: line)
        XCTAssertEqual(stand.frame, layer.frame, file: file, line: line)
        XCTAssertTrue(CATransform3DEqualToTransform(stand.transform, layer.transform), file: file, line: line)
        XCTAssertEqual(stand.opacity, layer.opacity, file: file, line: line)
        XCTAssertEqual(stand.isHidden, layer.isHidden, file: file, line: line)
        XCTAssertEqual(stand.zPosition, layer.zPosition, file: file, line: line)
    }

    func testTheShadowFollowsTheBoxAndLeavesWithTheBlur() throws {
        let sibling: [String: Any] = ["op": "create", "id": 3, "kind": "view", "style": ["background_color": [0, 0, 255, 255]]]
        let p = page([sibling])
        let node = try XCTUnwrap(p.views[2]), layer = try XCTUnwrap(node.layer)
        try assertStands(node)
        // An enter or exit animation's frames: opacity and a translation.
        p.apply(wireBatch([["op": "present", "id": 2, "property": "opacity", "x": 0.4],
                           ["op": "present", "id": 2, "property": "translate", "x": 10.0, "y": -6.0]]))
        try assertStands(node)
        XCTAssertEqual(node.shadowStand?.opacity ?? 0, 0.4, accuracy: 0.001)
        XCTAssertNotEqual(layer.transform.m41, 0, "the translation is the layer's")
        // Moved, resized and raised.
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 30, "y": 50, "w": 120, "h": 70]]))
        node.setPaintPosition(2)
        try assertStands(node)
        // A keyframe animation Core Animation plays on the layer plays on the stand.
        p.apply(wireBatch([["op": "animations", "id": 2, "specs": [["id": "fade#0#opacity", "k": "opacity", "s": 0.0, "dl": 0.0,
            "d": 1.0, "n": 1.0, "t": [0.0, 1.0], "v": [1.0, 0.0], "c": [], "fill": 3.0]]]]))
        XCTAssertNotNil(node.shadowStand?.animation(forKey: "fade#0#opacity"))
        p.apply(wireBatch([["op": "animations", "id": 2, "specs": []]]))
        XCTAssertNil(node.shadowStand?.animation(forKey: "fade#0#opacity"))
        // Hidden with the node.
        node.isHidden = true
        try assertStands(node)
        node.isHidden = false
        // The siblings reordered: under the node again once AppKit has moved the layers.
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [3, 2]]]))
        settle()
        try assertStands(node)
        // The blur gone, the node's own layer casts again and the stand leaves.
        let stand = try XCTUnwrap(node.shadowStand)
        var flat = glass
        flat["backdrop_filter"] = nil
        p.apply(wireBatch([["op": "style", "id": 2, "style": flat]]))
        XCTAssertNil(node.shadowStand)
        XCTAssertNil(stand.superlayer)
        XCTAssertTrue(node.shadowCaster?.superlayer === layer)
        XCTAssertFalse(layer.masksToBounds)
        // Back, then unmounted: the stand leaves with the node.
        p.apply(wireBatch([["op": "style", "id": 2, "style": glass]]))
        settle()
        try assertStands(node)
        let again = try XCTUnwrap(node.shadowStand)
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [3]]]))
        XCTAssertNil(again.superlayer)
    }
}
#endif

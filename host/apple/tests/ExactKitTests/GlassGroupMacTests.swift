#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1053.000.000 on AppKit: a `glassGroup` node's container view is its
/// innermost view and holds its children (D2); a material wins over it
/// (D6); a grouped glass whose path fades or clips moves into a container
/// of its own and back (D4), keeping the first responder; the group view
/// is never a hit target.
final class GlassGroupMacTests: XCTestCase {
    private var window: NSWindow!

    override func setUpWithError() throws {
        guard #available(macOS 26.0, *) else { throw XCTSkip("Liquid Glass is macOS 26") }
    }
    override func tearDown() { window?.close(); window = nil }

    private func presenter(_ ops: [[String: Any]]) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch(ops))
        return p
    }
    private func box(_ id: Int, _ props: [String: String] = [:], kind: String = "view", x: Double = 0, w: Double = 40, handlers: [String] = []) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": kind, "props": props, "handlers": handlers,
          "style": ["border_radius": 20.0, "text_color": [0, 0, 0, 255]]],
         ["op": "frame", "id": id, "x": x, "y": 0.0, "w": w, "h": 40.0]]
    }
    /// A group (1) of a glass (2) and, under a plain box (3), a glass (4).
    private func cluster(_ group: [String: String] = ["glassGroup": "12"]) -> [[String: Any]] {
        box(1, group, w: 200, handlers: ["press"]) + box(2, ["backgroundMaterial": "glass"])
            + box(3, x: 48, w: 48) + box(4, ["backgroundMaterial": "glass"])
            + [["op": "children", "id": 3, "ids": [4]], ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]]
    }
    private func spacing(_ node: NodeView) -> CGFloat? {
        guard #available(macOS 26.0, *) else { return nil }
        return (node.glassGroupView as? NSGlassEffectContainerView)?.spacing
    }
    private func present(_ p: Presenter, _ id: Int, _ opacity: Double) {
        p.apply(wireBatch([["op": "present", "id": id, "property": "opacity", "x": opacity, "y": 0.0, "w": 0.0, "h": 0.0]]))
    }

    /// LLP 1053.000.000.000 D2: an auto group's spacing is the points the
    /// Rust host resolved — the props' at creation, then the style key a
    /// gap change sends — and a number again when the prop is one.
    func testAnAutoGroupTakesItsSpacingFromWhatTheHostResolved() throws {
        let p = presenter(cluster(["glassGroup": "8", "glassGroupAuto": "true"]))
        let group = try XCTUnwrap(p.views[1])
        XCTAssertEqual(spacing(group), 8)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["text_color": [0, 0, 0, 255], "glass_group_spacing": 20.0]]]))
        XCTAssertEqual(spacing(group), 20, "a gap change sends style, not props")
        var native: [String: Any] = [:]
        group.glassAgentFields(&native)
        XCTAssertEqual((native["glassGroup"] as? [String: Any])?["auto"] as? Bool, true)
        // The flag cleared alone: the prop rules, though a stale key remains.
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["glassGroup": "12"], "clear": ["glassGroupAuto"]]]))
        XCTAssertEqual(spacing(group), 12, "a number again")
        native = [:]
        group.glassAgentFields(&native)
        XCTAssertNil((native["glassGroup"] as? [String: Any])?["auto"], "no longer auto")
        // The key dropped alone, the flag set: the prop it came with.
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["glassGroup": "6", "glassGroupAuto": "true"], "clear": []],
                           ["op": "style", "id": 1, "style": ["text_color": [0, 0, 0, 255]]]]))
        XCTAssertEqual(spacing(group), 6)
    }

    func testAGroupHoldsItsChildrenInnermostAndGivesThemBack() throws {
        let p = presenter(cluster())
        let group = try XCTUnwrap(p.views[1])
        let view = try XCTUnwrap(group.glassGroupView)
        XCTAssertTrue(view.superview === group)
        XCTAssertTrue(group.container === group.glassGroupContent)
        XCTAssertTrue(p.views[2]?.superview === group.container && p.views[3]?.superview === group.container)
        XCTAssertEqual(spacing(group), 12)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["glassGroup": "4"], "clear": []]]))
        XCTAssertTrue(group.glassGroupView === view)
        XCTAssertEqual(spacing(group), 4)
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["glassGroup"]]]))
        XCTAssertNil(group.glassGroupView)
        XCTAssertNil(view.superview)
        XCTAssertEqual(group.subviews.compactMap { ($0 as? NodeView)?.id }, [2, 3], "back in order")
    }

    func testAMaterialOnTheNodeWinsAndTheGroupComesBack() throws {
        let p = presenter(cluster())
        let group = try XCTUnwrap(p.views[1])
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["backgroundMaterial": "thin"], "clear": []]]))
        XCTAssertNil(group.glassGroupView)
        XCTAssertTrue(p.views[2]?.superview === group.container)
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["backgroundMaterial"]]]))
        XCTAssertNotNil(group.glassGroupView)
        XCTAssertTrue(p.views[2]?.superview === group.glassGroupContent)
    }

    func testAGlassMovesIntoItsOwnContainerWhileItsPathFadesOrClips() throws {
        let p = presenter(cluster())
        let inner = try XCTUnwrap(p.views[4]), material = try XCTUnwrap(inner.materialView)
        XCTAssertNil(inner.glassIsolation)
        present(p, 3, 0.5)
        let isolation = try XCTUnwrap(inner.glassIsolation)
        XCTAssertTrue(material.superview?.superview === isolation)
        XCTAssertTrue(isolation.superview === inner)
        var native: [String: Any] = [:]
        inner.glassAgentFields(&native)
        XCTAssertEqual(native["glassGroupOf"] as? String, "#1")
        XCTAssertEqual(native["isolated"] as? [String], ["opacity"])
        XCTAssertEqual(inner.appliedMaterial, "NSGlassEffectView(.regular)", "seen through its isolation")
        present(p, 3, 1)
        XCTAssertNil(inner.glassIsolation)
        XCTAssertTrue(material.superview === inner)
        XCTAssertNil(isolation.superview)
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["overflow_x": "hidden", "overflow_y": "hidden", "text_color": [0, 0, 0, 255]]]]))
        XCTAssertNotNil(inner.glassIsolation, "an ancestor clips")
        // The material changing takes the glass out of its isolation first.
        p.apply(wireBatch([["op": "props", "id": 4, "set": ["backgroundMaterial": "thin"], "clear": []]]))
        XCTAssertNil(inner.glassIsolation)
        XCTAssertTrue(inner.materialView?.superview === inner)
    }

    func testAFieldInAGlassKeepsFocusThroughItsIsolation() throws {
        let p = presenter(box(1, ["glassGroup": "12"], w: 200) + box(2, ["backgroundMaterial": "glass"], w: 160)
            + box(3, kind: "input", w: 140) + [["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        let field = try XCTUnwrap(p.views[3]?.field)
        XCTAssertTrue(window.makeFirstResponder(field))
        func editing() -> Bool { (window.firstResponder as? NSTextView)?.delegate as? NSTextField === field }
        XCTAssertTrue(editing())
        present(p, 2, 0.5)
        XCTAssertNotNil(p.views[2]?.glassIsolation)
        XCTAssertTrue(editing(), "isolated")
        present(p, 2, 1)
        XCTAssertNil(p.views[2]?.glassIsolation)
        XCTAssertTrue(editing(), "joined")
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["glassGroup"]]]))
        XCTAssertTrue(editing(), "the group gone")
    }

    /// Review fix: a clip box coming and going under a group keeps the field
    /// being edited.
    func testAFieldKeepsFocusThroughAClipBox() throws {
        let p = presenter(box(1, ["glassGroup": "8"], w: 200) + box(2, kind: "input", w: 160) + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        let group = try XCTUnwrap(p.views[1]), field = try XCTUnwrap(p.views[2]?.field)
        XCTAssertTrue(window.makeFirstResponder(field))
        func editing() -> Bool { (window.firstResponder as? NSTextView)?.delegate as? NSTextField === field }
        let shadowed: [String: Any] = ["border_radius": 20.0, "text_color": [0, 0, 0, 255], "overflow_x": "hidden", "overflow_y": "hidden",
                                       "shadow_color": [0, 0, 0, 255], "shadow_opacity": 0.5, "shadow_offset": [0.0, 2.0], "shadow_radius": 4.0]
        p.apply(wireBatch([["op": "style", "id": 1, "style": shadowed]]))
        XCTAssertNotNil(group.clipBox)
        XCTAssertTrue(editing(), "into the clip box")
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["border_radius": 20.0, "text_color": [0, 0, 0, 255]]]]))
        XCTAssertNil(group.clipBox)
        XCTAssertTrue(editing(), "out of it")
    }

    /// Review fix: a glass inside an isolated glass answers to that glass's
    /// isolation container, so a clip between them isolates it too.
    func testAGlassInsideAnIsolatedGlassIsIsolatedByItsOwnPath() throws {
        let p = presenter(box(1, ["glassGroup": "12"], w: 200) + box(2, w: 120) + box(3, ["backgroundMaterial": "glass"], w: 120)
            + box(4, w: 40) + box(5, ["backgroundMaterial": "glass"], w: 80)
            + [["op": "children", "id": 4, "ids": [5]], ["op": "children", "id": 3, "ids": [4]], ["op": "children", "id": 2, "ids": [3]],
               ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
               ["op": "present", "id": 2, "property": "opacity", "x": 0.5, "y": 0.0, "w": 0.0, "h": 0.0],
               ["op": "style", "id": 4, "style": ["overflow_x": "hidden", "overflow_y": "hidden", "text_color": [0, 0, 0, 255]]]])
        let outer = try XCTUnwrap(p.views[3]), inner = try XCTUnwrap(p.views[5])
        XCTAssertNotNil(outer.glassIsolation, "the outer glass's path fades")
        XCTAssertNotNil(inner.glassIsolation, "the inner glass's path to the outer's container clips")
        var native: [String: Any] = [:]
        inner.glassAgentFields(&native)
        XCTAssertEqual(native["glassGroupOf"] as? String, "#1", "its group, past the isolation")
    }

    func testTheGroupViewIsNeverAHitTarget() throws {
        let p = presenter(cluster())
        let group = try XCTUnwrap(p.views[1]), parent = try XCTUnwrap(group.superview)
        let empty = group.convert(NSPoint(x: 44, y: 20), to: parent)
        XCTAssertTrue(group.hitTest(empty) === group)
        let hit = try XCTUnwrap(group.hitTest(group.convert(NSPoint(x: 10, y: 20), to: parent))), glass = try XCTUnwrap(p.views[2])
        XCTAssertTrue(hit === glass || hit.isDescendant(of: glass), "\(hit)")
    }
}
#endif

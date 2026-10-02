#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1053.000.000 on UIKit: a `glassGroup` node's container view is its
/// innermost view and holds its children (D2); a material, a scroll or a
/// canvas wins over it (D6); a grouped glass whose path fades, masks or
/// clips is isolated in its slot, and joins again when the path clears
/// (D4); the group view is never a hit target; focus survives the moves.
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class GlassGroupIOSTests: XCTestCase {
    private var window: UIWindow!

    override func setUpWithError() throws {
        guard #available(iOS 26.0, *) else { throw XCTSkip("Liquid Glass is iOS 26") }
    }

    private func presenter(_ ops: [[String: Any]]) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
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
        guard #available(iOS 26.0, *) else { return nil }
        return (node.glassGroupView?.effect as? UIGlassContainerEffect)?.spacing
    }

    func testAGroupHoldsItsChildrenInnermostAndGivesThemBack() throws {
        let p = presenter(cluster())
        let group = try XCTUnwrap(p.views[1]), glass = try XCTUnwrap(p.views[2]), plain = try XCTUnwrap(p.views[3])
        let view = try XCTUnwrap(group.glassGroupView)
        XCTAssertTrue(view.superview === group)
        XCTAssertTrue(group.container === view.contentView)
        XCTAssertTrue(glass.superview === view.contentView && plain.superview === view.contentView)
        XCTAssertEqual(view.frame, group.bounds)
        XCTAssertEqual(spacing(group), 12)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["glassGroup": "4"], "clear": []]]))
        XCTAssertTrue(group.glassGroupView === view, "a spacing change keeps the view")
        XCTAssertEqual(spacing(group), 4)
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["glassGroup"]]]))
        XCTAssertNil(group.glassGroupView)
        XCTAssertNil(view.superview)
        XCTAssertEqual(group.subviews.compactMap { ($0 as? NodeView)?.id }, [2, 3], "back in order")
        // A spacing is clamped; one that is not a number draws no group.
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["glassGroup": "-3"], "clear": []]]))
        XCTAssertEqual(spacing(group), 0)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["glassGroup": "inf"], "clear": []]]))
        XCTAssertNil(group.glassGroupView)
        var native: [String: Any] = [:]
        group.glassAgentFields(&native)
        XCTAssertEqual((native["glassGroup"] as? [String: Any])?["reason"] as? String, "spacing")
    }

    /// LLP 1053.000.000.000 D2: an auto group's spacing is the points the
    /// Rust host resolved — the props' at creation, then the style key a
    /// gap change sends — and a number again when the prop is one.
    func testAnAutoGroupTakesItsSpacingFromWhatTheHostResolved() throws {
        let p = presenter(cluster(["glassGroup": "8", "glassGroupAuto": "true"]))
        let group = try XCTUnwrap(p.views[1])
        XCTAssertEqual(spacing(group), 8)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["border_radius": 20.0, "text_color": [0, 0, 0, 255], "glass_group_spacing": 20.0]]]))
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
                           ["op": "style", "id": 1, "style": ["border_radius": 20.0, "text_color": [0, 0, 0, 255]]]]))
        XCTAssertEqual(spacing(group), 6)
    }

    func testAClipBoxTakesTheGroupViewWithIt() throws {
        let p = presenter(cluster())
        let group = try XCTUnwrap(p.views[1]), view = try XCTUnwrap(group.glassGroupView)
        // A shadow on a clipping box puts the children in a clip box.
        let shadowed: [String: Any] = ["border_radius": 20.0, "text_color": [0, 0, 0, 255], "overflow_x": "hidden", "overflow_y": "hidden",
                                       "shadow_color": [0, 0, 0, 255], "shadow_opacity": 0.5, "shadow_offset": [0.0, 2.0], "shadow_radius": 4.0]
        p.apply(wireBatch([["op": "style", "id": 1, "style": shadowed]]))
        let clip = try XCTUnwrap(group.clipBox)
        XCTAssertTrue(view.superview === clip, "innermost, inside the clip box")
        XCTAssertTrue(p.views[2]?.superview === view.contentView)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["border_radius": 20.0, "text_color": [0, 0, 0, 255]]]]))
        XCTAssertNil(group.clipBox)
        XCTAssertTrue(view.superview === group && p.views[2]?.superview === view.contentView)
    }

    func testAMaterialOrAScrollOnTheNodeWinsAndTheGroupComesBack() throws {
        let p = presenter(cluster())
        let group = try XCTUnwrap(p.views[1])
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["backgroundMaterial": "thin"], "clear": []]]))
        XCTAssertNil(group.glassGroupView)
        XCTAssertNotNil(group.materialView)
        XCTAssertTrue(p.views[2]?.superview === group.container)
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["backgroundMaterial"]]]))
        let view = try XCTUnwrap(group.glassGroupView)
        XCTAssertTrue(p.views[2]?.superview === view.contentView)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_y": "scroll", "text_color": [0, 0, 0, 255]]]]))
        XCTAssertNil(group.glassGroupView)
        XCTAssertTrue(p.views[2]?.superview === group.scroll)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["text_color": [0, 0, 0, 255]]]]))
        XCTAssertNotNil(group.glassGroupView)
    }

    func testAGlassIsIsolatedWhileItsPathFadesMasksOrClips() throws {
        let p = presenter(cluster())
        let inner = try XCTUnwrap(p.views[4]), outer = try XCTUnwrap(p.views[2])
        let slot = try XCTUnwrap(inner.glassSlot, "a glass found in a group gets a slot")
        XCTAssertTrue(inner.materialView?.superview === slot.contentView)
        XCTAssertNil(slot.effect, "joined")
        XCTAssertNotNil(outer.glassSlot)
        func isolated(_ node: NodeView) -> [String] {
            var native: [String: Any] = [:]
            node.glassAgentFields(&native)
            XCTAssertEqual(native["glassGroupOf"] as? String, "#1")
            return native["isolated"] as? [String] ?? []
        }
        func present(_ id: Int, _ opacity: Double) {
            p.apply(wireBatch([["op": "present", "id": id, "property": "opacity", "x": opacity, "y": 0.0, "w": 0.0, "h": 0.0]]))
        }
        present(3, 0.5)
        XCTAssertTrue(slot.effect is UIGlassContainerEffect, "an ancestor fades")
        XCTAssertEqual(isolated(inner), ["opacity"])
        XCTAssertNil(outer.glassSlot?.effect, "its neighbour stays joined")
        present(3, 1)
        XCTAssertNil(slot.effect)
        present(4, 0.3)
        XCTAssertNotNil(slot.effect, "its own opacity")
        present(4, 1)
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["overflow_x": "hidden", "overflow_y": "hidden", "text_color": [0, 0, 0, 255]]]]))
        XCTAssertEqual(isolated(inner), ["clip"])
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["text_color": [0, 0, 0, 255]]]]))
        XCTAssertNil(slot.effect)
        // Its own clip does not isolate it: its bounds hold its glass.
        p.apply(wireBatch([["op": "style", "id": 4, "style": ["border_radius": 20.0, "overflow_x": "hidden", "overflow_y": "hidden", "text_color": [0, 0, 0, 255]]]]))
        XCTAssertEqual(isolated(inner), [])
        // Out of every group, a slot holds no container.
        present(3, 0.5)
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["glassGroup"]]]))
        XCTAssertNil(slot.effect)
        // The batch's reorder keeps the slot, not the glass in it, at the back.
        XCTAssertTrue(inner.subviews.first === slot && inner.materialView?.superview === slot.contentView)
    }

    func testTheGroupViewIsNeverAHitTarget() throws {
        let p = presenter(cluster() + box(5, x: 230, w: 30) + [["op": "children", "id": 1, "ids": [2, 3, 5]]])
        let group = try XCTUnwrap(p.views[1])
        // Empty group space, the bridge between merged shapes included.
        XCTAssertTrue(group.hitTest(CGPoint(x: 44, y: 20), with: nil) === group)
        let hit = try XCTUnwrap(group.hitTest(CGPoint(x: 10, y: 20), with: nil)), glass = try XCTUnwrap(p.views[2])
        XCTAssertTrue(hit === glass || hit.isDescendant(of: glass), "\(hit)")
        // A child in visible overflow, past the group's 200 points.
        let over = try XCTUnwrap(group.hitTest(CGPoint(x: 240, y: 20), with: nil)), outside = try XCTUnwrap(p.views[5])
        XCTAssertTrue(over === outside || over.isDescendant(of: outside), "\(over)")
    }

    func testFocusSurvivesTheGroupComingAndGoing() throws {
        let p = presenter(box(1, w: 200) + box(2, kind: "input", w: 160) + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        let group = try XCTUnwrap(p.views[1]), field = try XCTUnwrap(p.views[2]?.field)
        XCTAssertTrue(field.becomeFirstResponder())
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["glassGroup": "8"], "clear": []]]))
        XCTAssertNotNil(group.glassGroupView)
        XCTAssertTrue(field.isFirstResponder, "added")
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["glassGroup"]]]))
        XCTAssertTrue(field.isFirstResponder, "removed")
    }

    /// Review fix: a clip box coming and going under a group moves the
    /// children through the focus helper too.
    func testFocusSurvivesAClipBoxComingAndGoing() throws {
        let p = presenter(box(1, ["glassGroup": "8"], w: 200) + box(2, kind: "input", w: 160) + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        let group = try XCTUnwrap(p.views[1]), field = try XCTUnwrap(p.views[2]?.field)
        XCTAssertTrue(field.becomeFirstResponder())
        let shadowed: [String: Any] = ["border_radius": 20.0, "text_color": [0, 0, 0, 255], "overflow_x": "hidden", "overflow_y": "hidden",
                                       "shadow_color": [0, 0, 0, 255], "shadow_opacity": 0.5, "shadow_offset": [0.0, 2.0], "shadow_radius": 4.0]
        p.apply(wireBatch([["op": "style", "id": 1, "style": shadowed]]))
        XCTAssertNotNil(group.clipBox)
        XCTAssertTrue(field.isFirstResponder, "into the clip box")
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["border_radius": 20.0, "text_color": [0, 0, 0, 255]]]]))
        XCTAssertNil(group.clipBox)
        XCTAssertTrue(field.isFirstResponder, "out of it")
    }

    /// Review fix: a glass inside an isolated glass answers to that glass's
    /// container, so a clip between them isolates it too.
    func testAGlassInsideAnIsolatedGlassIsIsolatedByItsOwnPath() throws {
        let p = presenter(box(1, ["glassGroup": "12"], w: 200) + box(2, w: 120) + box(3, ["backgroundMaterial": "glass"], w: 120)
            + box(4, w: 40) + box(5, ["backgroundMaterial": "glass"], w: 80)
            + [["op": "children", "id": 4, "ids": [5]], ["op": "children", "id": 3, "ids": [4]], ["op": "children", "id": 2, "ids": [3]],
               ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
               ["op": "present", "id": 2, "property": "opacity", "x": 0.5, "y": 0.0, "w": 0.0, "h": 0.0],
               ["op": "style", "id": 4, "style": ["overflow_x": "hidden", "overflow_y": "hidden", "text_color": [0, 0, 0, 255]]]])
        let outer = try XCTUnwrap(p.views[3]), inner = try XCTUnwrap(p.views[5])
        XCTAssertNotNil(outer.glassSlot?.effect, "the outer glass's path fades")
        XCTAssertNotNil(inner.glassSlot?.effect, "the inner glass's path to the outer's container clips")
        var native: [String: Any] = [:]
        inner.glassAgentFields(&native)
        XCTAssertEqual(native["glassGroupOf"] as? String, "#1", "its group, past the isolation")
        XCTAssertEqual(native["isolated"] as? [String], ["clip"])
    }

    func testAFlatLeafFollowsItsParentIntoTheGroup() throws {
        let leaf: [[String: Any]] = [["op": "create", "id": 6, "kind": "view",
                                      "style": ["width": 3.0, "height": 20.0, "background_color": [0, 122, 255, 255], "text_color": [0, 0, 0, 255]]],
                                     ["op": "frame", "id": 6, "x": 100.0, "y": 0.0, "w": 3.0, "h": 20.0]]
        let p = presenter(box(1, w: 200) + box(2, ["backgroundMaterial": "glass"]) + leaf
            + [["op": "children", "id": 1, "ids": [2, 6]], ["op": "roots", "ids": [1]]])
        XCTAssertNil(p.views[6], "flat")
        let group = try XCTUnwrap(p.views[1])
        func leafLayer(in view: UIView) -> Bool { view.layer.sublayers?.contains { $0.frame == CGRect(x: 100, y: 0, width: 3, height: 20) } ?? false }
        XCTAssertTrue(leafLayer(in: group))
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["glassGroup": "8"], "clear": []]]))
        let view = try XCTUnwrap(group.glassGroupView)
        XCTAssertTrue(leafLayer(in: view.contentView), "into the group's content")
        XCTAssertFalse(leafLayer(in: group))
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["glassGroup"]]]))
        XCTAssertTrue(leafLayer(in: group), "and back")
    }
}
#endif

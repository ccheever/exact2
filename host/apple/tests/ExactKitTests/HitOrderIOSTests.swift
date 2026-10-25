#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A touch reaches what paints on top (CSS): a raised `z-index` sibling (a
/// positioned box, where `z-index` applies)
/// before a later one that covers it, then later siblings before earlier.
/// UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class HitOrderIOSTests: XCTestCase {
    private var window: UIWindow!

    private func presenter(raised: Double) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        // A screen (1): a button (2) raised over its top-right corner, then a
        // list (3) after it in the tree that fills the whole screen.
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "handlers": ["press"], "props": ["testId": "menu"],
             "style": ["position_type": raised == 0 ? "static" : "absolute", "z_index": raised, "text_color": [0, 0, 0, 255]]],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["press"], "props": ["testId": "list"]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "rank", "id": 2, "rank": raised * 2],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 340.0, "y": 0.0, "w": 44.0, "h": 44.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
        ]))
        window.layoutIfNeeded()
        return p
    }

    private func hit(_ p: Presenter, _ point: CGPoint) -> UIView? {
        window.hitTest(point, with: nil)
    }

    func testAPlainLeafTakesOnlyPointsInsideItsLocalBounds() throws {
        let p = presenter(raised: 0)
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "view", "props": ["testId": "leaf"]],
            ["op": "children", "id": 1, "ids": [20]],
            ["op": "frame", "id": 20, "x": 50.0, "y": 50.0, "w": 40.0, "h": 40.0],
        ]))
        let leaf = try XCTUnwrap(p.views[20])
        XCTAssertTrue(leaf.subviews.isEmpty)
        XCTAssertTrue(leaf.hitTest(CGPoint(x: 20, y: 20), with: nil) === leaf)
        for point in [CGPoint(x: -1, y: 20), CGPoint(x: 41, y: 20),
                      CGPoint(x: 20, y: -1), CGPoint(x: 20, y: 41), CGPoint(x: 40, y: 20)] {
            XCTAssertNil(leaf.hitTest(point, with: nil), "outside local bounds: \(point)")
        }
    }

    func testOverflowingFlatChildrenRemainHittableAsTheyPromoteDemoteAndLeave() throws {
        let p = presenter(raised: 0)
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "view", "handlers": ["press"]],
            ["op": "create", "id": 21, "kind": "view", "style": ["background_color": [0, 122, 255, 255],
                "text_color": [0, 0, 0, 255], "written": ["background_color"]]],
            ["op": "children", "id": 20, "ids": [21]],
            ["op": "children", "id": 1, "ids": [20]],
            ["op": "frame", "id": 20, "x": 0.0, "y": 0.0, "w": 100.0, "h": 100.0],
            ["op": "frame", "id": 21, "x": 120.0, "y": 10.0, "w": 20.0, "h": 20.0],
        ]))
        let holder = try XCTUnwrap(p.views[20]), point = CGPoint(x: 125, y: 15)
        XCTAssertTrue(holder.subviews.isEmpty, "the child is drawn as a layer")
        XCTAssertTrue(p.flats.holdsLeaves(holder.id))
        XCTAssertTrue(holder.hitTest(point, with: nil) === holder, "flat overflow hits its owning view")
        p.apply(wireBatch([["op": "rank", "id": 21, "rank": 1]]))
        let promoted = try XCTUnwrap(p.views[21])
        XCTAssertFalse(p.flats.holdsLeaves(holder.id))
        XCTAssertTrue(holder.hitTest(point, with: nil) === promoted, "promoted overflow hits its own view")
        p.apply(wireBatch([["op": "rank", "id": 21, "rank": 0]]))
        XCTAssertNil(p.views[21])
        XCTAssertTrue(holder.subviews.isEmpty)
        XCTAssertTrue(p.flats.holdsLeaves(holder.id))
        XCTAssertTrue(holder.hitTest(point, with: nil) === holder, "demoted overflow hits its owning view again")
        p.apply(wireBatch([["op": "children", "id": 20, "ids": []], ["op": "destroy", "id": 21]]))
        XCTAssertTrue(holder.subviews.isEmpty)
        XCTAssertFalse(p.flats.holdsLeaves(holder.id))
        XCTAssertNil(holder.hitTest(point, with: nil), "removing the last child restores the ordinary leaf bounds")
    }

    func testARaisedSiblingTakesTheTouchFromALaterOneOverIt() throws {
        let p = presenter(raised: 2)
        let menu = try XCTUnwrap(p.views[2]), list = try XCTUnwrap(p.views[3])
        XCTAssertTrue(hit(p, CGPoint(x: 360, y: 20)) === menu, "the raised button")
        XCTAssertTrue(hit(p, CGPoint(x: 100, y: 200)) === list, "the list, where nothing is raised over it")
    }

    func testAPointerEventsNoneBoxLetsTheTouchThrough() throws {
        let p = presenter(raised: 0)
        // A header backdrop (4) raised over the list, which never takes a touch.
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "style": ["position_type": "absolute", "z_index": 3.0, "pointer_events": "none", "text_color": [0, 0, 0, 255]]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "rank", "id": 4, "rank": 6],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 400.0, "h": 120.0],
        ]))
        window.layoutIfNeeded()
        let list = try XCTUnwrap(p.views[3])
        XCTAssertTrue(hit(p, CGPoint(x: 100, y: 60)) === list, "through the backdrop to the list")
    }

    /// `pointer-events` is inherited: a box carrying its parent's `none`
    /// passes the touch through where it paints outside the parent's box
    /// (x2apps repro pointer-events-inherit-translate); `auto` again is hit.
    func testAnInheritedPointerEventsNoneInVisibleOverflowLetsTheTouchThrough() throws {
        let p = presenter(raised: 0)
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "style": ["position_type": "absolute", "pointer_events": "none"]],
            ["op": "create", "id": 5, "kind": "view", "style": ["pointer_events": "none"]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [3, 2, 4]],
            ["op": "frame", "id": 4, "x": 0.0, "y": 300.0, "w": 400.0, "h": 60.0],
            // Painted 300 above the row's box, over the raised button.
            ["op": "frame", "id": 5, "x": 300.0, "y": -300.0, "w": 100.0, "h": 60.0],
        ]))
        window.layoutIfNeeded()
        let menu = try XCTUnwrap(p.views[2]), toast = try XCTUnwrap(p.views[5])
        XCTAssertTrue(hit(p, CGPoint(x: 360, y: 20)) === menu, "through the inheriting toast to the button")
        toast.applyStyle(["pointer_events": "auto"])
        XCTAssertTrue(hit(p, CGPoint(x: 360, y: 20)) === toast, "a toast that sets auto again takes the touch")
    }

    /// A placement or projection that hides and restores a `display: none`
    /// box restores the host's word, so it shows once displayed (review B1).
    func testADisplayNoneBoxHiddenAndRestoredByTheHostShowsOnceDisplayed() throws {
        let p = presenter(raised: 0)
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "style": ["display": "none"]],
            ["op": "children", "id": 1, "ids": [3, 2, 4]],
        ]))
        let n = try XCTUnwrap(p.views[4])
        XCTAssertTrue(n.isHidden)
        n.placementHidden = true
        n.placementHidden = false
        let saved = n.hiddenByHost
        n.isHidden = true
        n.isHidden = saved
        n.applyStyle(["display": "block"])
        XCTAssertFalse(n.isHidden)
    }

    /// An iPad keyboard's Enter, Tab, Escape, Backspace and Delete arrive as
    /// one character; each is named back to ARIA's key before it is matched,
    /// so the button that declares it is pressed (review B2: only longer
    /// inputs were, and the command, taken over the system's, was dropped).
    func testANamedOneCharacterShortcutPressesItsButton() throws {
        let p = presenter(raised: 0)
        p.apply(wireBatch([
            ["op": "create", "id": 5, "kind": "button", "handlers": ["press"], "props": ["accessibilityKeyShortcuts": "Escape"]],
            ["op": "children", "id": 1, "ids": [3, 2, 5]],
            ["op": "frame", "id": 5, "x": 10.0, "y": 300.0, "w": 80.0, "h": 40.0],
        ]))
        window.layoutIfNeeded()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let cases: [(String, String, UIKeyModifierFlags)] = [
            ("Escape", UIKeyCommand.inputEscape, []), ("Meta+Enter", "\r", .command), ("Tab", "\t", []),
            ("Backspace", "\u{8}", []), ("Delete", UIKeyCommand.inputDelete, []), ("Meta+k", "k", .command),
        ]
        for (chord, input, flags) in cases {
            p.apply(wireBatch([["op": "props", "id": 5, "set": ["accessibilityKeyShortcuts": chord], "clear": []]]))
            pressed = []
            p.performShortcut(UIKeyCommand(input: input, modifierFlags: flags, action: #selector(UIResponder.becomeFirstResponder)))
            XCTAssertEqual(pressed, [5], chord)
        }
    }

    /// A native module's view inside a `pointer-events: none` box is not a
    /// target either: the touch reaches the button around it (paint F9).
    func testAPointerEventsNoneBoxsPlatformViewLetsTheTouchThrough() throws {
        let p = presenter(raised: 0)
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "style": ["pointer_events": "none"]],
            ["op": "children", "id": 2, "ids": [4]],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 44.0, "h": 44.0],
        ]))
        let thumbnail = try XCTUnwrap(p.views[4])
        thumbnail.addSubview(UIView(frame: thumbnail.bounds)) // the module's view
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [3, 2]]]))
        window.layoutIfNeeded()
        XCTAssertTrue(hit(p, CGPoint(x: 360, y: 20)) === p.views[2], "the button around the native view")
    }

    /// A button floating above a zero-height box (#721): a dock (4) of
    /// `pointer-events: none` boxes over the list, its zero-height box (5)
    /// holding a row (6) that overflows it upward, and in the row a capsule
    /// (7) whose clip leaves its shadow outside, so its children are in a
    /// clip box. The button (8) sets `auto` again. A finger reaches it, and
    /// so does UIKit's accessibility, which leaves out a view with empty
    /// bounds and everything under it.
    func testAButtonFloatingAboveAZeroHeightBoxIsTouchedAndExposed() throws {
        let p = presenter(raised: 0)
        let none: [String: Any] = ["pointer_events": "none"]
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "style": ["position_type": "absolute", "pointer_events": "none"]],
            ["op": "create", "id": 5, "kind": "view", "style": none],
            ["op": "create", "id": 6, "kind": "view", "style": ["position_type": "absolute", "pointer_events": "none"]],
            ["op": "create", "id": 7, "kind": "view", "style": ["pointer_events": "none", "overflow_x": "hidden", "overflow_y": "hidden", "border_radius": 19.0,
                                                               "box_shadow": [["o": [0.0, 4.0], "b": 6.0, "s": 0, "c": [0, 0, 0, 26]]]]],
            ["op": "create", "id": 8, "kind": "view", "handlers": ["press"], "props": ["testId": "floating", "accessibilityRole": "button", "accessibilityLabel": "Floating"]],
            ["op": "create", "id": 9, "kind": "view", "style": none],
            ["op": "children", "id": 1, "ids": [3, 2, 4]], ["op": "children", "id": 4, "ids": [5, 9]], ["op": "children", "id": 5, "ids": [6]],
            ["op": "children", "id": 6, "ids": [7]], ["op": "children", "id": 7, "ids": [8]],
            ["op": "frame", "id": 4, "x": 0.0, "y": 300.0, "w": 400.0, "h": 60.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 400.0, "h": 0.0],
            ["op": "frame", "id": 6, "x": 0.0, "y": -40.0, "w": 400.0, "h": 38.0],
            ["op": "frame", "id": 7, "x": 150.0, "y": 0.0, "w": 100.0, "h": 38.0],
            ["op": "frame", "id": 8, "x": 0.0, "y": 0.0, "w": 38.0, "h": 38.0],
            ["op": "frame", "id": 9, "x": 10.0, "y": 0.0, "w": 390.0, "h": 60.0],
        ]))
        p.syncAccessibility()
        window.layoutIfNeeded()
        let button = try XCTUnwrap(p.views[8]), capsule = try XCTUnwrap(p.views[7]), zero = try XCTUnwrap(p.views[5])
        XCTAssertNotNil(capsule.clipBox, "the capsule's children are in its clip box")
        XCTAssertTrue(hit(p, CGPoint(x: 170, y: 279)) === button, "through the clip box to the button")
        XCTAssertTrue(hit(p, CGPoint(x: 220, y: 279)) === p.views[3], "the rest of the capsule lets the touch through to the list")

        // A model of UIKit's walk, as XCTest's snapshot showed it: a
        // container's own list, else its subviews with area; an element ends it.
        func exposed(_ view: UIView) -> [UIView] {
            if view.isAccessibilityElement { return [view] }
            let kids = view.accessibilityElements?.compactMap { $0 as? UIView } ?? view.subviews.filter { !$0.bounds.isEmpty }
            return kids.flatMap(exposed)
        }
        XCTAssertTrue(exposed(try XCTUnwrap(p.views[1])).contains { $0 === button }, "the floating button is exposed")
        let dock = try XCTUnwrap(p.views[4]), row = try XCTUnwrap(p.views[6]), composer = try XCTUnwrap(p.views[9])
        // Its row in the empty box's place, which is on the composer's line and ahead of it.
        XCTAssertEqual(dock.accessibilityElements?.map { ($0 as? NodeView)?.id }, [row.id, composer.id])
        zero.accessibilityElements = []
        XCTAssertEqual(dock.accessibilityElements?.map { ($0 as? NodeView)?.id }, [composer.id], "a box's own list is kept, an empty one too")
        zero.accessibilityElements = nil
        // UIKit's order, not the tree's: tops 8 apart are read top first, and
        // closer ones as one line, by leading edge.
        let order = { dock.accessibilityElements?.map { ($0 as? NodeView)?.id } }
        p.apply(wireBatch([["op": "frame", "id": 5, "x": 20.0, "y": 8.0, "w": 380.0, "h": 0.0]]))
        XCTAssertEqual(order(), [composer.id, row.id], "the composer's top is 8 higher")
        p.apply(wireBatch([["op": "frame", "id": 5, "x": 20.0, "y": -8.0, "w": 380.0, "h": 0.0]]))
        XCTAssertEqual(order(), [row.id, composer.id], "the box's top is 8 higher")
        p.apply(wireBatch([["op": "frame", "id": 5, "x": 20.0, "y": -7.0, "w": 380.0, "h": 0.0]]))
        XCTAssertEqual(order(), [composer.id, row.id], "one line: the composer's leading edge is first")
        p.apply(wireBatch([["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 400.0, "h": 0.0]]))
        if #available(iOS 26.0, *) {
            // A glass group holds the dock's children in UIKit's effect view.
            p.apply(wireBatch([["op": "props", "id": 4, "set": ["glassGroup": "8"], "clear": []]]))
            XCTAssertNotNil(dock.glassGroupView)
            XCTAssertTrue(hit(p, CGPoint(x: 170, y: 279)) === button, "through the glass group to the button")
            XCTAssertEqual(dock.accessibilityElements?.map { ($0 as? NodeView)?.id }, [row.id, composer.id], "and its children listed from there")
        }
        // A zero-height box that clips its children hides them, and is left to UIKit.
        p.apply(wireBatch([["op": "style", "id": 5, "style": ["pointer_events": "none", "overflow_x": "hidden", "overflow_y": "hidden"]]]))
        XCTAssertTrue(zero.clipsToBounds)
        XCTAssertNil(dock.accessibilityElements, "a clipped empty box is not passed through")
    }

    /// CSS computes `overflow-y: auto` beside `overflow-x: hidden`, so a
    /// truncating title is a scroll container with nothing to scroll. On the
    /// web a click on it reaches its button; a `UIScrollView` keeps a touch
    /// from the responder chain, so one that cannot scroll hands the hit to
    /// its node, whose touch bubbles to the button.
    func testATitleThatCannotScrollPassesTheTouchToItsButton() throws {
        let p = presenter(raised: 2)
        p.apply(wireBatch([
            ["op": "create", "id": 6, "kind": "view", "props": ["testId": "title"], "style": ["overflow_x": "hidden", "overflow_y": "auto", "text_color": [0, 0, 0, 255]]],
            ["op": "children", "id": 2, "ids": [6]],
            ["op": "frame", "id": 6, "x": 0.0, "y": 0.0, "w": 44.0, "h": 20.0],
            ["op": "content", "id": 6, "w": 44.0, "h": 20.0],
        ]))
        window.layoutIfNeeded()
        let title = try XCTUnwrap(p.views[6])
        let scroller = try XCTUnwrap(title.scrollView)
        XCTAssertTrue(hit(p, CGPoint(x: 360, y: 10)) === title, "the title's node, not its scroll view")
        scroller.contentSize = CGSize(width: 44, height: 200)
        XCTAssertTrue(hit(p, CGPoint(x: 360, y: 10)) === scroller, "a scroller with somewhere to go keeps the touch for its pan")
    }

    func testInFlowTheLaterSiblingIsOnTop() throws {
        let p = presenter(raised: 0)
        let list = try XCTUnwrap(p.views[3])
        XCTAssertTrue(hit(p, CGPoint(x: 360, y: 20)) === list, "tree order, as the web paints it")
    }
}
#endif

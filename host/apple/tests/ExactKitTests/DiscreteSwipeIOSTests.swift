#if os(iOS)
import UIKit
import UIKit.UIGestureRecognizerSubclass
import XCTest
@testable import ExactKit

/// LLP 1057.001 §8 on UIKit: `swipe` is a `UISwipeGestureRecognizer` per
/// direction the node names, taking its `touch-count`; it waits for a scroll
/// view's pan only where that pan would begin, and an outer swipe for an
/// inner one; a finger's flick passes a text editor, a pointer's does not,
/// and a selection moved during the contact refuses it. UIKit synthesizes no
/// touches for a unit test, so the tests call the recognizers' delegate
/// methods with touches of their own.
final class DiscreteSwipeIOSTests: XCTestCase {
    private var window: UIWindow!

    private func host(_ ops: [[String: Any]]) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        p.apply(wireBatch(ops))
        window.makeKeyAndVisible()
        return p
    }
    private func swipes(_ node: NodeView) -> [DiscreteSwipe] { node.gestureRecognizers?.compactMap { $0 as? DiscreteSwipe } ?? [] }

    /// A touch of the kind, on the view and at the window point the test names.
    private final class Touch: UITouch {
        let on: UIView, kind: UITouch.TouchType, at: TimeInterval, point: CGPoint
        init(on: UIView, kind: UITouch.TouchType = .direct, at: TimeInterval = 10, point: CGPoint = CGPoint(x: 150, y: 50)) {
            self.on = on; self.kind = kind; self.at = at; self.point = point; super.init()
        }
        override var view: UIView? { on }
        override var type: UITouch.TouchType { kind }
        override var timestamp: TimeInterval { at }
        override func location(in view: UIView?) -> CGPoint { view.map { on.window?.convert(point, to: $0) ?? point } ?? point }
    }

    func testASwipeIsOneRecognizerPerDirectionAndTakesItsCount() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "down up", "touchCount": "2"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
        ])
        let node = try XCTUnwrap(p.views[1])
        XCTAssertEqual(swipes(node).map(\.direction), [.down, .up])
        XCTAssertEqual(swipes(node).map(\.numberOfTouchesRequired), [2, 2])
        let before = swipes(node)
        p.apply(wireBatch([["op": "props", "id": 1, "set": [:], "clear": ["touchCount"]]]))
        XCTAssertEqual(swipes(node).map(\.direction), [.down, .up])
        XCTAssertEqual(swipes(node).map(\.numberOfTouchesRequired), [1, 1], "omitted: UIKit's own, one finger")
        XCTAssertTrue(before.allSatisfy { $0.view == nil }, "a count that goes unset makes new recognizers")
        let one = swipes(node)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["swipeDirection": "left"], "clear": []]]))
        XCTAssertEqual(swipes(node).map(\.direction), [.left])
        XCTAssertTrue(one.allSatisfy { $0.view == nil }, "so does a new direction")

        // Only the node's current recognizers speak for it (`swiped`)
        XCTAssertTrue(node.discreteSwipes.contains { $0 === swipes(node)[0] })
        XCTAssertFalse(node.discreteSwipes.contains { $0 === before[0] || $0 === one[0] })
    }

    func testASwipeWaitsForAScrollViewsPanOnlyWhereItWouldScroll() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "down right"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
        ])
        let node = try XCTUnwrap(p.views[1])
        let scroll = UIScrollView(frame: CGRect(x: 0, y: 0, width: 300, height: 100))
        node.addSubview(scroll)
        let down = try XCTUnwrap(swipes(node).first { $0.word == "down" }), right = try XCTUnwrap(swipes(node).first { $0.word == "right" })
        XCTAssertTrue(node.admitsSwipe(down, Touch(on: scroll)))
        XCTAssertTrue(node.admitsSwipe(right, Touch(on: scroll)))
        scroll.contentSize = CGSize(width: 300, height: 400)
        XCTAssertTrue(node.swipeRequiresFailure(down, of: scroll.panGestureRecognizer), "it scrolls down: scroll wins")
        XCTAssertFalse(node.swipeRequiresFailure(right, of: scroll.panGestureRecognizer), "it does not scroll sideways")
        scroll.contentSize = CGSize(width: 300, height: 100)
        XCTAssertFalse(node.swipeRequiresFailure(down, of: scroll.panGestureRecognizer), "its content fits")
        scroll.alwaysBounceVertical = true
        XCTAssertTrue(node.swipeRequiresFailure(down, of: scroll.panGestureRecognizer), "it bounces")
    }

    func testAFingerPassesAnEditorAndASelectionMovedSinceItWentDownRefusesTheSwipe() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "down"]],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["value": "Hello"]],
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 120.0, "w": 60.0, "h": 40.0],
        ])
        let bar = try XCTUnwrap(p.views[1]), field = try XCTUnwrap(p.views[2]), button = try XCTUnwrap(p.views[3])
        let text = try XCTUnwrap(field.textArea)
        let down = try XCTUnwrap(swipes(bar).first)
        XCTAssertTrue(bar.admitsSwipe(down, Touch(on: text)), "a finger's flick on the field is the bar's")
        XCTAssertFalse(bar.admitsSwipe(down, Touch(on: text, kind: .indirectPointer)), "a pointer's drag selects")
        XCTAssertFalse(bar.admitsSwipe(down, Touch(on: text, kind: .pencil)))
        XCTAssertFalse(bar.admitsSwipe(down, Touch(on: button)), "a press between keeps the contact")
        down.reset()
        XCTAssertTrue(bar.admitsSwipe(down, Touch(on: text, at: 10)))
        XCTAssertTrue(down.editor === field)
        field.selectionMovedAt = 9
        XCTAssertTrue(bar.swipeMayBegin(down), "moved before the contact")
        field.selectionMovedAt = 11
        XCTAssertFalse(bar.swipeMayBegin(down), "moved during the contact: the editor's")
    }

    /// A swipe on the editor's own node is guarded as one on an ancestor is
    func testASwipeOnTheEditorsOwnNodeIsRefusedWhenItsSelectionMoved() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "textarea", "handlers": ["swipe"], "props": ["value": "Hello", "swipeDirection": "down"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0],
        ])
        let field = try XCTUnwrap(p.views[1]), text = try XCTUnwrap(field.textArea)
        let down = try XCTUnwrap(swipes(field).first)
        XCTAssertTrue(field.admitsSwipe(down, Touch(on: text, at: 10)))
        XCTAssertTrue(down.editor === field)
        field.selectionMovedAt = 11
        XCTAssertFalse(field.swipeMayBegin(down))
    }

    /// A textarea's pan holds the swipe only while its text overflows: text
    /// that fits does not scroll (`TextArea.fits`), so the flick is the swipe's.
    func testATextareaHoldsTheSwipeOnlyWhileItsTextOverflows() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "down"]],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["value": "Hello"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 60.0],
        ])
        let bar = try XCTUnwrap(p.views[1]), text = try XCTUnwrap(p.views[2]?.textArea as? TextArea)
        let down = try XCTUnwrap(swipes(bar).first)
        XCTAssertTrue(bar.admitsSwipe(down, Touch(on: text)))
        text.layoutIfNeeded()
        XCTAssertTrue(text.fits, "the premise: one line fits")
        XCTAssertFalse(bar.swipeRequiresFailure(down, of: text.panGestureRecognizer), "text that fits does not scroll")
        text.text = (1...12).map { "Line \($0)" }.joined(separator: "\n")
        text.layoutIfNeeded()
        XCTAssertFalse(text.fits, "the premise: twelve lines overflow")
        XCTAssertTrue(bar.swipeRequiresFailure(down, of: text.panGestureRecognizer), "text that overflows scrolls first")
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["overflow_y": "hidden"]]]))
        XCTAssertFalse(bar.swipeRequiresFailure(down, of: text.panGestureRecognizer), "`overflow-y: hidden` does not scroll")
    }

    /// An Exact scroller's pan holds the swipe only in a direction it would
    /// begin in: its `touch-action` refusing that direction leaves the swipe free.
    func testAScrollerHoldsTheSwipeOnlyWhereItsTouchActionLetsItPan() throws {
        for (action, holds) in [("auto", true), ("pan-x", false), ("pan-y", true), ("none", false)] {
            let p = host([
                ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "down"]],
                ["op": "create", "id": 2, "kind": "view", "style": ["overflow_x": "hidden", "overflow_y": "scroll", "touch_action": action]],
                ["op": "create", "id": 3, "kind": "view"],
                ["op": "children", "id": 1, "ids": [2]],
                ["op": "children", "id": 2, "ids": [3]],
                ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
                ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
                ["op": "content", "id": 2, "w": 300.0, "h": 600.0],
                ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 300.0, "h": 600.0],
            ])
            let bar = try XCTUnwrap(p.views[1]), scroll = try XCTUnwrap(p.views[2]?.scroll)
            let down = try XCTUnwrap(swipes(bar).first)
            XCTAssertTrue(bar.admitsSwipe(down, Touch(on: scroll)))
            XCTAssertEqual(bar.swipeRequiresFailure(down, of: scroll.panGestureRecognizer), holds, "touch-action: \(action)")
        }
    }

    /// Scroll chaining (LLP 1070 G1): a flick down from an inner scroller at
    /// its top belongs to an outer one that can scroll up, so the swipe waits
    /// for the outer pan and not the inner.
    func testAScrollerThatChainsTheDirectionLeavesTheSwipeToTheOuterOne() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "down"]],
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_x": "hidden", "overflow_y": "scroll"]],
            ["op": "create", "id": 3, "kind": "view", "style": ["overflow_x": "hidden", "overflow_y": "scroll"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "content", "id": 2, "w": 300.0, "h": 900.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 300.0, "h": 150.0],
            ["op": "content", "id": 3, "w": 300.0, "h": 600.0],
        ])
        let bar = try XCTUnwrap(p.views[1]), outer = try XCTUnwrap(p.views[2]?.scroll), inner = try XCTUnwrap(p.views[3]?.scroll)
        outer.contentOffset.y = 50
        let down = try XCTUnwrap(swipes(bar).first)
        XCTAssertTrue(bar.admitsSwipe(down, Touch(on: inner)))
        XCTAssertFalse(bar.swipeRequiresFailure(down, of: inner.panGestureRecognizer), "at its top, the inner scroller chains the flick")
        XCTAssertTrue(bar.swipeRequiresFailure(down, of: outer.panGestureRecognizer), "the outer one scrolls up")
    }

    /// LLP 1057.001 §8.4: a swipe waits for the navigation's screen-edge pop,
    /// and not for the content pop, whose own gate leaves a right swipe's
    /// touch to it (§8.4), nor another edge pan.
    func testASwipeWaitsForTheEdgePopOnly() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "right"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0],
        ])
        let row = try XCTUnwrap(p.views[1]), swipe = try XCTUnwrap(swipes(row).first)
        let nav = UINavigationController(rootViewController: UIViewController())
        nav.loadViewIfNeeded()
        let edge = try XCTUnwrap(nav.interactivePopGestureRecognizer)
        XCTAssertTrue(row.gestureRecognizer(swipe, shouldRequireFailureOf: edge), "from the edge, back wins")
        if #available(iOS 26.0, *), let content = nav.interactiveContentPopGestureRecognizer {
            XCTAssertFalse(row.gestureRecognizer(swipe, shouldRequireFailureOf: content), "not the content pop")
        }
        let stray = UIScreenEdgePanGestureRecognizer()
        UIView().addGestureRecognizer(stray)
        XCTAssertFalse(row.gestureRecognizer(swipe, shouldRequireFailureOf: stray), "nor another edge pan")
    }

    /// Rule 1: a swipe to the right does not begin within the left 20 points,
    /// where Back begins; other directions may begin there.
    func testASwipeToTheRightYieldsWhereBackBegins() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "right down"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0],
        ])
        let row = try XCTUnwrap(p.views[1])
        let right = try XCTUnwrap(swipes(row).first { $0.word == "right" }), down = try XCTUnwrap(swipes(row).first { $0.word == "down" })
        XCTAssertTrue(row.admitsSwipe(right, Touch(on: row, point: CGPoint(x: 10, y: 50))))
        XCTAssertFalse(row.swipeMayBegin(right), "where Back begins")
        right.reset()
        XCTAssertTrue(row.admitsSwipe(right, Touch(on: row, point: CGPoint(x: 40, y: 50))))
        XCTAssertTrue(row.swipeMayBegin(right))
        XCTAssertTrue(row.admitsSwipe(down, Touch(on: row, point: CGPoint(x: 10, y: 50))))
        XCTAssertTrue(row.swipeMayBegin(down), "another direction begins there")
    }

    /// The content pop leaves a touch to a node's one-finger swipe to the
    /// right, as to a `swiperight` row, unless a `press` node between
    /// keeps the contact from it (rule 3); a swipe of another count or
    /// direction, or a disabled node's, leaves the touch to the pop.
    func testTheContentPopLeavesATouchToARightSwipeWithNoPressBetween() throws {
        func gate(_ props: [String: Any], press: Bool = false) throws -> (onRow: Bool, onButton: Bool) {
            let p = host([
                ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": props],
                ["op": "create", "id": 2, "kind": "button", "handlers": press ? ["press"] : []],
                ["op": "children", "id": 1, "ids": [2]],
                ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0],
                ["op": "frame", "id": 2, "x": 200.0, "y": 20.0, "w": 60.0, "h": 40.0],
            ])
            let row = try XCTUnwrap(p.views[1]), button = try XCTUnwrap(p.views[2])
            return (NavigationHost.overRightSwipe(row, in: p.viewport), NavigationHost.overRightSwipe(button, in: p.viewport))
        }
        let right = try gate(["swipeDirection": "right"])
        XCTAssertTrue(right.onRow && right.onButton, "a button without press passes the touch on")
        XCTAssertEqual(try gate(["swipeDirection": "right"], press: true).onButton, false, "a press between keeps it")
        XCTAssertEqual(try gate(["swipeDirection": "right", "touchCount": "2"]).onRow, false, "two fingers: not the pop's swipe")
        XCTAssertEqual(try gate(["swipeDirection": "left down"]).onRow, false)
        XCTAssertEqual(try gate(["swipeDirection": "right", "disabled": "true"]).onRow, false)
    }

    /// A `swiperight` row keeps the content pop off a touch anywhere on it, a
    /// `press` node between included (§7); only a `swipe`'s protection stops
    /// at a press (§8.4).
    func testTheContentPopStaysOffASwiperightRowPastAPress() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["swiperight"]],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 200.0, "y": 20.0, "w": 60.0, "h": 40.0],
        ])
        XCTAssertTrue(NavigationHost.overRightSwipe(try XCTUnwrap(p.views[2]), in: p.viewport))
    }

    /// A disabled node takes no swipe and the contact passes to an ancestor's:
    /// UIKit asks the hit view about the ancestor's recognizer too, and a
    /// disabled one answers only for its own.
    func testADisabledInnerNodeLeavesTheContactToAnAncestorsSwipe() throws {
        for ownSwipe in [false, true] {
            let p = host([
                ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "down"]],
                ["op": "create", "id": 2, "kind": "view", "handlers": ownSwipe ? ["swipe"] : [], "props": ownSwipe ? ["swipeDirection": "down", "disabled": "true"] : ["disabled": "true"]],
                ["op": "children", "id": 1, "ids": [2]],
                ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
                ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0],
            ])
            let outer = try XCTUnwrap(p.views[1]), inner = try XCTUnwrap(p.views[2])
            let outerSwipe = try XCTUnwrap(swipes(outer).first)
            XCTAssertTrue(inner.gestureRecognizerShouldBegin(outerSwipe), "the disabled node lets the ancestor's swipe begin (own swipe: \(ownSwipe))")
            if ownSwipe { XCTAssertFalse(inner.gestureRecognizerShouldBegin(try XCTUnwrap(swipes(inner).first)), "its own swipe does not") }
        }
        // Inside a disabled node, or inert, a node takes no swipe (`SwipeInput.allows`, which a pan and the drags ask too)
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "down"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["disabled": "true"]],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "right"]],
            ["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 300.0, "h": 50.0],
        ])
        let outer = try XCTUnwrap(p.views[1]), inner = try XCTUnwrap(p.views[3])
        let innerSwipe = try XCTUnwrap(swipes(inner).first)
        XCTAssertFalse(inner.gestureRecognizerShouldBegin(innerSwipe), "inside a disabled node")
        XCTAssertFalse(inner.takesRightSwipe, "nor does it keep the content pop off its touch")
        let outerSwipe = try XCTUnwrap(swipes(outer).first)
        XCTAssertTrue(inner.gestureRecognizerShouldBegin(outerSwipe), "the inner node leaves the contact to an ancestor's swipe")
        XCTAssertTrue(outer.gestureRecognizerShouldBegin(outerSwipe), "and the ancestor above the disabled node may begin it")
        p.apply(wireBatch([["op": "props", "id": 2, "clear": ["disabled"]], ["op": "props", "id": 3, "set": ["inert": "true"]]]))
        XCTAssertFalse(inner.gestureRecognizerShouldBegin(innerSwipe), "inert")
        XCTAssertFalse(inner.takesRightSwipe, "inert")
        p.apply(wireBatch([["op": "props", "id": 3, "clear": ["inert"]]]))
        XCTAssertTrue(inner.gestureRecognizerShouldBegin(innerSwipe))
        XCTAssertTrue(inner.takesRightSwipe)
    }

    /// Rule 3 between nested nodes: an outer swipe waits for an inner one of
    /// the same direction and count, an omitted count counting as one; an
    /// inner one of another direction or count leaves the outer one free, and
    /// the inner one waits for no ancestor.
    func testAnOuterSwipeWaitsForAnInnerOneOfItsDirectionAndCount() throws {
        func waits(outer: String?, inner: String?, innerDirection: String = "down") throws -> Bool {
            var outerProps: [String: Any] = ["swipeDirection": "down"], innerProps: [String: Any] = ["swipeDirection": innerDirection]
            outerProps["touchCount"] = outer
            innerProps["touchCount"] = inner
            let p = host([
                ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": outerProps],
                ["op": "create", "id": 2, "kind": "view", "handlers": ["swipe"], "props": innerProps],
                ["op": "children", "id": 1, "ids": [2]],
                ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
                ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0],
            ])
            let outerView = try XCTUnwrap(p.views[1]), innerView = try XCTUnwrap(p.views[2])
            let outerSwipe = try XCTUnwrap(swipes(outerView).first), innerSwipe = try XCTUnwrap(swipes(innerView).first)
            XCTAssertFalse(innerView.swipeRequiresFailure(innerSwipe, of: outerSwipe), "the inner one waits for no ancestor")
            return outerView.swipeRequiresFailure(outerSwipe, of: innerSwipe)
        }
        XCTAssertTrue(try waits(outer: nil, inner: nil))
        XCTAssertFalse(try waits(outer: nil, inner: nil, innerDirection: "left"), "another direction")
        XCTAssertTrue(try waits(outer: nil, inner: "1"), "an omitted count is one")
        XCTAssertTrue(try waits(outer: "2", inner: "2"))
        XCTAssertFalse(try waits(outer: nil, inner: "2"))
        XCTAssertFalse(try waits(outer: "2", inner: nil))
    }

    /// Only the current recognizers of the node's current view speak for it
    /// (§8.5): one replaced by a new direction or count, or left on a view
    /// the node no longer has, sends nothing.
    func testARecognizerTheNodeNoLongerHasSendsNothing() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "down"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0],
        ])
        var sent: [String] = []
        p.onSwipe = { id, direction in sent.append("\(id) \(direction)") }
        let node = try XCTUnwrap(p.views[1]), first = try XCTUnwrap(swipes(node).first)
        node.swiped(first)
        XCTAssertEqual(sent, ["1 down"])
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["touchCount": "2"], "clear": []]]))
        node.swiped(first)
        XCTAssertEqual(sent, ["1 down"], "a recognizer replaced by a new count")
        let second = try XCTUnwrap(swipes(node).first)
        p.apply(wireBatch([
            ["op": "destroy", "id": 1],
            ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "down", "touchCount": "2"]],
            ["op": "roots", "ids": [1]],
        ]))
        XCTAssertFalse(p.views[1] === node, "the premise: a new view for node 1")
        node.swiped(second)
        XCTAssertEqual(sent, ["1 down"], "a recognizer on a view the node no longer has")
    }
}
#endif

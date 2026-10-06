#if os(iOS)
import UIKit
import UIKit.UIGestureRecognizerSubclass
import XCTest
@testable import ExactKit

/// LLP 1057.001 on UIKit: the web's `dblclick` order. Recognizer phases are set
/// by the test (UIKit synthesizes no touches for a unit test), as elsewhere.
final class GesturePrecedenceIOSTests: XCTestCase {
    private var window: UIWindow!

    /// A pan whose movement the test sets.
    private final class Pan: UIPanGestureRecognizer {
        var moved = CGPoint.zero, speed = CGPoint.zero, at = CGPoint(x: 150, y: 100)
        override func location(in view: UIView?) -> CGPoint { at }
        override func translation(in view: UIView?) -> CGPoint { moved }
        override func velocity(in view: UIView?) -> CGPoint { speed }
    }

    /// A pinch whose phase the test sets.
    private final class Pinch: UIPinchGestureRecognizer {
        var phase = UIGestureRecognizer.State.possible
        override var state: UIGestureRecognizer.State { get { phase } set { phase = newValue } }
    }

    private final class Taps: UITapGestureRecognizer {
        private var phase = UIGestureRecognizer.State.possible
        override var state: UIGestureRecognizer.State { get { phase } set { phase = newValue } }
    }

    private func host(_ ops: [[String: Any]]) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        p.apply(wireBatch(ops))
        window.makeKeyAndVisible()
        return p
    }
    private func drain() {
        let turn = expectation(description: "a main-queue turn")
        DispatchQueue.main.async { turn.fulfill() }
        wait(for: [turn], timeout: 1)
    }

    /// LLP 1005 §3: a node hearing `pointerdown`/`pointerup` carries an
    /// observer that never recognizes, so it takes nothing from a press or a
    /// scroll; its touch down and up are the two events.
    /// An iPad pointer's buttons as DOM counts them (review b5-b 1): the
    /// secondary is 2 and the middle 4, a touch's or an empty mask 1.
    func testAnIndirectPointerReportsItsButtonsAsTheWebDoes() {
        XCTAssertEqual(PointerRecognizer.domButtons(.primary), 1)
        XCTAssertEqual(PointerRecognizer.domButtons(.secondary), 2)
        XCTAssertEqual(PointerRecognizer.domButtons(.button(3)), 4)
        XCTAssertEqual(PointerRecognizer.domButtons([.primary, .secondary]), 3)
        XCTAssertEqual(PointerRecognizer.domButtons([]), 1)
    }

    func testAPointerNodeObservesItsTouchWithoutPreventingAnything() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "button", "handlers": ["press", "pointerdown", "pointerup"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0]
        ])
        var log: [String] = []
        p.onPointer = { id, kind, _ in log.append("\(kind == .down ? "down" : kind == .up ? "up" : "move") \(id)") }
        let node = try XCTUnwrap(p.views[1])
        let g = try XCTUnwrap(node.gestureRecognizers?.compactMap { $0 as? PointerRecognizer }.first)
        XCTAssertFalse(g.cancelsTouchesInView)
        XCTAssertFalse(g.canPrevent(UIPanGestureRecognizer()))
        XCTAssertFalse(g.canBePrevented(by: UIPanGestureRecognizer()))
        let touch = UITouch()
        g.touchesBegan([touch], with: UIEvent())
        XCTAssertEqual(log, ["down 1"])
        g.touchesMoved([touch], with: UIEvent())
        g.touchesCancelled([touch], with: UIEvent())
        XCTAssertEqual(log, ["down 1", "up 1"], "a cancel is an up")
        // Without the handlers the observer goes.
        node.handlers = ["press"]
        node.updateContextGestures()
        XCTAssertNil(node.gestureRecognizers?.first { $0 is PointerRecognizer })
    }

    /// Nested pointer nodes: the innermost enabled one takes the touch, as
    /// on the web and macOS; a disabled inner one passes it out.
    func testTheInnermostEnabledPointerNodeTakesTheTouch() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown", "pointerup"]],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["pointerdown"]],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["hover"]],
            ["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 80.0, "h": 60.0],
            ["op": "frame", "id": 3, "x": 5.0, "y": 5.0, "w": 20.0, "h": 20.0]
        ])
        let recognizer = { (id: UInt32) in p.views[id]?.gestureRecognizers?.compactMap { $0 as? PointerRecognizer }.first }
        let outer = try XCTUnwrap(recognizer(1)), inner = try XCTUnwrap(recognizer(2))
        let touched = try XCTUnwrap(p.views[3])
        XCTAssertTrue(outer.nearer(touched), "the inner pointer node is nearer")
        XCTAssertFalse(inner.nearer(touched))
        XCTAssertFalse(outer.nearer(p.views[1]), "a touch on the outer node itself is its own")
        p.views[2]?.props["disabled"] = "true"
        XCTAssertFalse(outer.nearer(touched), "a disabled inner node passes it out")
    }

    func testTheSecondTapStillPressesAndDblclickComesAfterIt() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press", "dblclick"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0]
        ])
        var log: [String] = []
        p.onPress = { log.append("press \($0)") }
        p.onDblclick = { log.append("dblclick \($0)") }
        let node = try XCTUnwrap(p.views[1])
        let recognizer = try XCTUnwrap(node.doubleRecognizer)
        XCTAssertFalse(recognizer.cancelsTouchesInView, "the second tap's touches reach the press")
        XCTAssertFalse(recognizer.delaysTouchesEnded)
        let touch: Set<UITouch> = [UITouch()]
        node.touchesBegan(touch, with: nil); node.touchesEnded(touch, with: nil)
        // UIKit may run the recognizer's action before the view's touchesEnded.
        let taps = Taps(); taps.state = .ended
        node.touchesBegan(touch, with: nil)
        node.doubleClicked(taps)
        node.touchesEnded(touch, with: nil)
        drain()
        XCTAssertEqual(log, ["press 1", "press 1", "dblclick 1"], "the web's click, click, dblclick")
    }

    /// A clip (1) holding the photo (2) and its handle (3), with `touchAction`
    /// on the handle and the clip above it.
    private func photo(handle: String, clip: String = "auto") -> (Presenter, NodeView) {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "style": ["touch_action": clip]],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view", "style": ["touch_action": handle]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "transform-drag", "id": 3, "runtime": "1", "handleKey": "2", "target": 2, "targetKey": "4", "clip": 1, "clipKey": "6"]
        ])
        return (p, p.views[3]!)
    }

    /// LLP 1057.001 §4: the binding's pan takes two fingers and a pinch rides
    /// with it — the one simultaneity, and only that pair's.
    func testThePhotoHandleHasATwoFingerPanAndASimultaneousPinch() throws {
        let (p, handle) = photo(handle: "none")
        let pan = try XCTUnwrap(handle.transformRecognizer)
        let pinch = try XCTUnwrap(handle.transformContact?.pinch)
        XCTAssertEqual(pan.maximumNumberOfTouches, 2)
        XCTAssertTrue(handle.gestureRecognizers?.contains(pinch) == true)
        XCTAssertTrue(handle.gestureRecognizer(pan, shouldRecognizeSimultaneouslyWith: pinch))
        XCTAssertTrue(handle.gestureRecognizer(pinch, shouldRecognizeSimultaneouslyWith: pan))
        XCTAssertFalse(handle.gestureRecognizer(pinch, shouldRecognizeSimultaneouslyWith: UIPanGestureRecognizer()))
        XCTAssertEqual(handle.transformShouldBegin(pinch), true, "touch-action none: the app pinches")
        XCTAssertNil(handle.transformShouldBegin(UIPinchGestureRecognizer()), "not the binding's")
        withExtendedLifetime(p) {
            p.apply(wireBatch([["op": "transform-drag", "id": 3, "runtime": "1", "handleKey": "2",
                "target": NSNull(), "targetKey": NSNull(), "clip": NSNull(), "clipKey": NSNull()]]))
        }
        XCTAssertNil(handle.transformRecognizer)
        XCTAssertNil(handle.transformContact)
        XCTAssertFalse(handle.gestureRecognizers?.contains(pinch) == true)
    }

    /// `pinch-zoom` is the platform's zoom (§2): where every node up allows it,
    /// the app's pinch stands aside, as a browser takes the pinch.
    func testTheAppPinchNeedsTouchActionToExcludePinchZoom() throws {
        for (handleAction, clipAction, app) in [("none", "auto", true), ("pan-x pan-y", "auto", true),
                                                ("auto", "auto", false), ("pinch-zoom", "manipulation", false),
                                                ("pan-y pinch-zoom", "auto", false), ("auto", "pan-y", true)] {
            let (p, handle) = photo(handle: handleAction, clip: clipAction)
            let pinch = try XCTUnwrap(handle.transformContact?.pinch)
            XCTAssertEqual(handle.transformShouldBegin(pinch), app, "\(handleAction) under \(clipAction)")
            XCTAssertEqual(handle.transformShouldBegin(try XCTUnwrap(handle.transformRecognizer)), true, "the pan is unaffected")
            withExtendedLifetime(p) {}
        }
    }

    /// The photo (clip 1, photo 2, handle 3; 300 by 150) as a page of a
    /// sideways pager (4, 300 by 200) twice its width, `touchAction` on the handle, the clip and the
    /// pager; `room` false makes the pager exactly one page wide.
    private func pagedPhoto(handle: String, clip: String = "auto", pager: String = "auto", room: Bool = true,
                            under: String? = nil, outer: String? = nil) -> (Presenter, NodeView) {
        var ops: [[String: Any]] = [
            ["op": "create", "id": 4, "kind": "view", "style": ["overflow_x": "scroll", "overflow_y": "hidden", "touch_action": pager]],
            ["op": "create", "id": 1, "kind": "view", "style": ["touch_action": clip]],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view", "style": ["touch_action": handle]],
            ["op": "children", "id": 4, "ids": [1]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [4]],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "content", "id": 4, "w": room ? 600.0 : 300.0, "h": 200.0],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 150.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 150.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 300.0, "h": 150.0],
            ["op": "transform-drag", "id": 3, "runtime": "1", "handleKey": "2", "target": 2, "targetKey": "4", "clip": 1, "clipKey": "6"]
        ]
        // A node inside the handle with its own touch-action, observing the
        // pointer so hit testing finds it (a node that takes nothing is
        // passed over; a press would keep the drag from the photo outright).
        // It hangs below the handle, inside the pager, under the finger at
        // (150, 180).
        if let under {
            ops.insert(["op": "create", "id": 5, "kind": "view", "handlers": ["pointerdown"], "style": ["touch_action": under]], at: 4)
            ops.append(["op": "children", "id": 3, "ids": [5]])
            ops.append(["op": "frame", "id": 5, "x": 0.0, "y": 160.0, "w": 300.0, "h": 40.0])
        }
        // The pager as a page of an outer sideways pager (6) three pages wide.
        if let outer {
            ops.insert(["op": "create", "id": 6, "kind": "view", "style": ["overflow_x": "scroll", "overflow_y": "hidden", "touch_action": outer]], at: 0)
            ops = ops.map { $0["op"] as? String == "roots" ? ["op": "roots", "ids": [6]] : $0 }
            ops.append(["op": "children", "id": 6, "ids": [4]])
            ops.append(["op": "frame", "id": 6, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0])
            ops.append(["op": "content", "id": 6, "w": 900.0, "h": 200.0])
        }
        let p = host(ops)
        return (p, p.views[3]!)
    }

    /// Rule 2 for the photo's pan: an axis the handle's `touch-action` names
    /// is the platform's when a scroller would take it (a pager's photo at
    /// fit, `pan-x`, pages sideways and drags to dismiss up and down); `none`,
    /// `auto` and `manipulation` keep every drag, and a pan with no direction
    /// yet is not refused.
    func testThePhotoPanLeavesThePagerTheAxesItsTouchActionNames() throws {
        let left = CGPoint(x: -300, y: 20), right = CGPoint(x: 300, y: 20), down = CGPoint(x: 20, y: 300)
        for (action, speed, begins) in [("pan-x", left, false), ("pan-x", right, false), ("pan-x", down, true),
                                        ("pan-x pinch-zoom", left, false), ("pan-left", left, true),
                                        ("pan-right", left, false), ("pan-x", .zero, true),
                                        ("pan-y", down, true), ("none", left, true), ("auto", left, true),
                                        ("manipulation", left, true)] {
            let (p, handle) = pagedPhoto(handle: action)
            let pan = Pan()
            pan.speed = speed
            handle.transformRecognizer = pan
            XCTAssertEqual(handle.transformShouldBegin(pan), begins, "\(action) at \(speed)")
            withExtendedLifetime(p) {}
        }
        // Before UIKit has a velocity, the movement that crossed the slop decides.
        let (p, handle) = pagedPhoto(handle: "pan-x")
        let pan = Pan()
        handle.transformRecognizer = pan
        pan.moved = CGPoint(x: 12, y: 2)
        XCTAssertEqual(handle.transformShouldBegin(pan), false)
        pan.moved = CGPoint(x: 2, y: 12)
        XCTAssertEqual(handle.transformShouldBegin(pan), true)
        withExtendedLifetime(p) {}
    }

    /// The yield is CSS's intersection up to a scroller that can move: an
    /// ancestor or a node under the finger that refuses the axis, a pager
    /// with nowhere to go or no pager at all leaves the drag the photo's,
    /// since nothing else would take it.
    func testThePhotoKeepsADragNoScrollerWouldTake() throws {
        let left = CGPoint(x: -300, y: 0), down = CGPoint(x: 0, y: 300)
        for (label, make, speed) in [
            ("a pan-y clip", { self.pagedPhoto(handle: "pan-x", clip: "pan-y") }, left),
            ("a none clip", { self.pagedPhoto(handle: "pan-x", clip: "none") }, left),
            ("a pan-y pager", { self.pagedPhoto(handle: "pan-x", pager: "pan-y") }, left),
            ("a none node under the finger, outside the handle", { self.pagedPhoto(handle: "pan-x", under: "none") }, left),
            ("a one-page pager", { self.pagedPhoto(handle: "pan-x", room: false) }, left),
            ("a pager that does not scroll down", { self.pagedPhoto(handle: "pan-y") }, down),
            ("no pager", { self.photo(handle: "pan-x") }, left),
        ] as [(String, () -> (Presenter, NodeView), CGPoint)] {
            let (p, handle) = make()
            let pan = Pan()
            pan.speed = speed
            pan.at = CGPoint(x: 150, y: 180)
            handle.transformRecognizer = pan
            XCTAssertEqual(handle.transformShouldBegin(pan), true, label)
            withExtendedLifetime(p) {}
        }
    }

    /// The scroller's range counts its insets, as `handsOff` does: a pager
    /// one page wide with a leading inset still scrolls sideways, so the
    /// photo yields. A pointer-observing node under the finger that allows
    /// the axis does not stop the yield.
    func testThePhotoYieldsToRangeTheInsetsMake() throws {
        let (p, handle) = pagedPhoto(handle: "pan-x", room: false)
        let pan = Pan()
        pan.speed = CGPoint(x: -300, y: 0)
        handle.transformRecognizer = pan
        XCTAssertEqual(handle.transformShouldBegin(pan), true, "no range: the photo's")
        try XCTUnwrap(p.views[4]?.scroll).contentInset = UIEdgeInsets(top: 0, left: 40, bottom: 0, right: 0)
        XCTAssertEqual(handle.transformShouldBegin(pan), false, "an inset's range: the pager's")
        let (q, under) = pagedPhoto(handle: "pan-x", under: "pan-x")
        let pan2 = Pan()
        pan2.speed = CGPoint(x: -300, y: 0)
        pan2.at = CGPoint(x: 150, y: 180)
        under.transformRecognizer = pan2
        let pager = try XCTUnwrap(q.views[4]?.scroll)
        XCTAssertTrue(pager.hitTest(CGPoint(x: 150, y: 180), with: nil) === q.views[5], "the finger reaches the child")
        XCTAssertEqual(under.transformShouldBegin(pan2), false)
        withExtendedLifetime((p, q)) {}
    }

    /// Scroll chaining: a pager at its trailing edge hands a drag on to an
    /// outer pager that can take it, so the intersection runs on to that
    /// one's owner — the photo yields when it allows the axis and keeps the
    /// drag when it refuses, since then neither scroller would begin.
    func testThePhotoFollowsAPagerAtItsEdgeToTheOneItChainsTo() throws {
        for (outer, begins) in [("auto", false), ("none", true), ("pan-y", true)] {
            let (p, handle) = pagedPhoto(handle: "pan-x", outer: outer)
            // The photo is the inner pager's second page, the one in view.
            p.apply(wireBatch([["op": "frame", "id": 1, "x": 300.0, "y": 0.0, "w": 300.0, "h": 150.0]]))
            try XCTUnwrap(p.views[4]?.scroll).contentOffset.x = 300
            let pan = Pan()
            pan.speed = CGPoint(x: -300, y: 0)
            handle.transformRecognizer = pan
            XCTAssertEqual(handle.transformShouldBegin(pan), begins, "outer \(outer)")
            // Not at the edge, the inner pager takes it whatever the outer says.
            try XCTUnwrap(p.views[4]?.scroll).contentOffset.x = 100
            XCTAssertEqual(handle.transformShouldBegin(pan), false, "outer \(outer), mid-pager")
            withExtendedLifetime(p) {}
        }
    }

    /// A disabled scroller takes nothing (an app's hook can turn
    /// `isScrollEnabled` off): the photo keeps the drag, or yields past it to
    /// an enabled scroller that can take it.
    func testADisabledPagerTakesNothing() throws {
        for (outer, begins) in [(nil, true), ("auto", false)] as [(String?, Bool)] {
            let (p, handle) = pagedPhoto(handle: "pan-x", outer: outer)
            try XCTUnwrap(p.views[4]?.scroll).isScrollEnabled = false
            let pan = Pan()
            pan.speed = CGPoint(x: -300, y: 0)
            handle.transformRecognizer = pan
            XCTAssertEqual(handle.transformShouldBegin(pan), begins, "outer \(outer ?? "none")")
            withExtendedLifetime(p) {}
        }
    }

    /// One direction on both sides of the yield: while UIKit has no velocity
    /// yet, the pager reads the movement that crossed the slop, as the photo
    /// does, so the sideways drag the photo steps aside from is the pager's.
    func testThePagerTakesTheDragThePhotoYieldsBeforeAVelocity() throws {
        let (p, handle) = pagedPhoto(handle: "pan-x")
        let pan = Pan()
        pan.moved = CGPoint(x: -12, y: 2)
        pan.at = CGPoint(x: 138, y: 102)
        handle.transformRecognizer = pan
        XCTAssertEqual(handle.transformShouldBegin(pan), false, "the photo yields")
        let pager = try XCTUnwrap(p.views[4]?.scroll)
        XCTAssertTrue(pager.admitsPan(velocity: .zero, translation: CGPoint(x: -12, y: 2), start: CGPoint(x: 150, y: 100)), "the pager takes it")
        XCTAssertFalse(pager.admitsPan(velocity: .zero, translation: CGPoint(x: 2, y: 12), start: CGPoint(x: 150, y: 100)), "down is the photo's")
        withExtendedLifetime(p) {}
    }

    /// Rule 6: a pan joining the binding's pinch is admitted whatever its
    /// direction, so the pair's continuation after one finger lifts (§4)
    /// survives a sideways pinch on a pager's photo.
    func testAPanJoiningThePinchIsAdmitted() throws {
        let (p, handle) = pagedPhoto(handle: "pan-x")
        let pan = Pan()
        pan.speed = CGPoint(x: -300, y: 0)
        handle.transformRecognizer = pan
        let pinch = Pinch()
        handle.transformContact = TransformContact(pinch)
        XCTAssertEqual(handle.transformShouldBegin(pan), false, "alone, the pager takes it")
        pinch.phase = .began
        XCTAssertEqual(handle.transformShouldBegin(pan), true, "with the pinch begun")
        pinch.phase = .changed
        XCTAssertEqual(handle.transformShouldBegin(pan), true, "and while it changes")
        withExtendedLifetime(p) {}
    }

    /// Rule 3 on UIKit: an ancestor's pan waits for a descendant's swipe to
    /// fail; never the other way, and never for a pinch.
    func testAnAncestorDragRequiresADescendantDragToFail() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pan"]],
            ["op": "create", "id": 2, "kind": "view", "handlers": ["swiperight"], "style": ["touch_action": "pan-y"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0]
        ])
        let outer = try XCTUnwrap(p.views[1]), inner = try XCTUnwrap(p.views[2])
        let pan = try XCTUnwrap(outer.layoutPanRecognizer), swipe = try XCTUnwrap(inner.swipeRecognizer)
        XCTAssertTrue(outer.gestureRecognizer(pan, shouldRequireFailureOf: swipe))
        XCTAssertFalse(inner.gestureRecognizer(swipe, shouldRequireFailureOf: pan))
        XCTAssertFalse(outer.gestureRecognizer(pan, shouldRequireFailureOf: UIPinchGestureRecognizer()))
        XCTAssertTrue(inner.stopsAtPress(swipe) && outer.stopsAtPress(pan))
        XCTAssertFalse(outer.stopsAtPress(UITapGestureRecognizer()))
    }

    /// LLP 1057.001 §7: a swipe waits for the navigation's screen-edge pop,
    /// and for nothing else of its class: iOS 26's content pop waits on the
    /// swipe, so waiting on it too would leave neither able to begin.
    func testASwipeWaitsForTheEdgePopOnly() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["swiperight"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0]
        ])
        let row = try XCTUnwrap(p.views[1]), swipe = try XCTUnwrap(row.swipeRecognizer)
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
}
#endif

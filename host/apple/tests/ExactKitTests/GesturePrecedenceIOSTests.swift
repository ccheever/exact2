#if os(iOS)
import UIKit
import UIKit.UIGestureRecognizerSubclass
import XCTest
@testable import ExactKit

/// LLP 1057.001 on UIKit: the web's `dblclick` order. Recognizer phases are set
/// by the test (UIKit synthesizes no touches for a unit test), as elsewhere.
final class GesturePrecedenceIOSTests: XCTestCase {
    private var window: UIWindow!

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

#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A press and the focus it takes on UIKit, found by the Bluesky clone's
/// composer on device: a custom button takes the focus on its touch
/// (PointerIOS), which WebKit's never does. So (1) the field it takes the
/// focus from lowers the keyboard, and under `resizes-content` the button
/// moves at once, yet the press is the finger's where it went down and came
/// up; and (2) a button holding the focus its touch gave it (the FAB that
/// opened a sheet) is no focus a node took for the autofocus pass, which
/// still gives the sheet's field its autofocus, while a field, another
/// focusable, or a button focused any other way (Tab, `focus(id)`) keeps its own.
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class PressFocusIOSTests: XCTestCase {
    private var window: UIWindow!
    private var session: ExactSession?
    private var observers: [NSObjectProtocol] = []

    override func tearDown() {
        session?.destroy(); session = nil
        observers.forEach(NotificationCenter.default.removeObserver)
        observers = []
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    private func presenter(_ ops: [[String: Any]], useSession: Bool = false) -> Presenter {
        if useSession { session = ExactApp.shared.makeSession(label: "press-inline") }
        let p = session?.presenter ?? Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch(ops))
        return p
    }
    private func node(_ id: Int, _ kind: String, _ props: [String: String] = [:], handlers: [String] = [], y: Double = 0, w: Double = 120, h: Double = 44) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": kind, "props": props, "handlers": handlers, "style": ["text_color": [0, 0, 0, 255]]],
         ["op": "frame", "id": id, "x": 0.0, "y": y, "w": w, "h": h]]
    }
    /// The page (1) with a field (2) and a custom button (3) under it.
    private func composer() -> Presenter {
        presenter(node(1, "view", w: 400, h: 400) + node(2, "input", w: 300, h: 34) + node(3, "button", handlers: ["press"], y: 200)
                  + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]])
    }
    /// A touch resting at one window point, wherever its view goes.
    private final class PointTouch: UITouch {
        var point: CGPoint
        let target: UIView
        init(_ point: CGPoint, on target: UIView) { self.point = point; self.target = target; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint { view.map { $0.convert(point, from: nil) } ?? point }
    }

    private func scrollingPress(ownerKind: String = "button", handlers: [String] = ["press"], useSession: Bool = false) -> Presenter {
        presenter(node(1, "view", w: 400, h: 400)
            + node(2, ownerKind, handlers: handlers, y: 100, w: 320)
            + node(3, "text", ["text": "0"], w: 44)
            + [["op": "create", "id": 4, "kind": "view", "style": ["overflow_x": "auto", "overflow_y": "hidden"]],
               ["op": "frame", "id": 4, "x": 44.0, "y": 0.0, "w": 276.0, "h": 44.0],
               ["op": "content", "id": 4, "w": 640.0, "h": 44.0]]
            + node(5, "text", ["text": "Plain text behind a horizontal scroller; a tap should press once."], w: 640)
            + [["op": "children", "id": 4, "ids": [5]],
               ["op": "children", "id": 2, "ids": [3, 4]],
               ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]], useSession: useSession)
    }
    private func textTouch(_ p: Presenter) throws -> PointTouch {
        let text = try XCTUnwrap(p.views[5])
        let scroll = try XCTUnwrap(p.views[4]?.scroll)
        window.layoutIfNeeded()
        XCTAssertTrue(text.superview === scroll, "the authored text is beneath Exact's inserted UIScrollView")
        let point = scroll.convert(CGPoint(x: scroll.bounds.minX + 20, y: scroll.bounds.midY), to: nil)
        let hit = try XCTUnwrap(window.hitTest(window.convert(point, from: nil), with: nil))
        guard hit === text else {
            XCTFail("the real hit must be the plain text, got \(type(of: hit))")
            throw CocoaError(.coderInvalidValue)
        }
        return PointTouch(point, on: hit)
    }
    /// Issue 408: a plain text hit under Exact's inserted scroller keeps the
    /// ancestor's complete press sequence, as the same authored row does on web.
    func testAPlainTextTouchReachesItsAncestorThroughAHorizontalScroller() throws {
        let p = scrollingPress()
        let button = try XCTUnwrap(p.views[2]), text = try XCTUnwrap(p.views[5])
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let touch = try textTouch(p)
        text.touchesBegan([touch], with: nil)
        XCTAssertTrue(button.pressed, "text down must arm the nearest ancestor through the scroller")
        XCTAssertEqual(pressed, [], "down alone does not activate")
        text.touchesEnded([touch], with: nil)
        XCTAssertEqual(pressed, [2], "text up must invoke the ancestor once")
        XCTAssertFalse(button.pressed, "up must release its feedback")
    }

    func testAForwardedCancelAndLateCallbacksCannotEndTheNextContact() throws {
        let p = scrollingPress(), text = try XCTUnwrap(p.views[5]), button = try XCTUnwrap(p.views[2])
        var pressed: [UInt32] = []; p.onPress = { pressed.append($0) }
        let old = try textTouch(p)
        text.touchesBegan([old], with: nil)
        XCTAssertTrue(button.pressed)
        text.touchesCancelled([old], with: nil)
        XCTAssertFalse(button.pressed)
        text.touchesEnded([old], with: nil)
        XCTAssertEqual(pressed, [])
        let fresh = try textTouch(p)
        text.touchesBegan([fresh], with: nil)
        text.touchesCancelled([old], with: nil)
        text.touchesEnded([old], with: nil)
        XCTAssertTrue(button.pressed, "the old contact cannot clear new feedback")
        text.touchesEnded([fresh], with: nil)
        XCTAssertEqual(pressed, [2])
    }

    func testAForwardedMoveFollowsTheFingerBeforeLifting() throws {
        let p = scrollingPress(), text = try XCTUnwrap(p.views[5]), button = try XCTUnwrap(p.views[2])
        var pressed: [UInt32] = []; p.onPress = { pressed.append($0) }
        let touch = try textTouch(p), inside = touch.point
        text.touchesBegan([touch], with: nil)
        touch.point = button.convert(CGPoint(x: 20, y: button.bounds.maxY + 60), to: nil)
        text.touchesMoved([touch], with: nil)
        XCTAssertFalse(button.pressInside(touch))
        touch.point = inside
        text.touchesMoved([touch], with: nil)
        text.touchesEnded([touch], with: nil)
        XCTAssertEqual(pressed, [2])
        let outside = try textTouch(p)
        text.touchesBegan([outside], with: nil)
        outside.point = button.convert(CGPoint(x: 20, y: button.bounds.maxY + 60), to: nil)
        text.touchesMoved([outside], with: nil)
        text.touchesEnded([outside], with: nil)
        XCTAssertEqual(pressed, [2], "lifting outside does not activate")
        XCTAssertFalse(button.pressed)
    }

    func testANearerAuthoredHandlerTakesTheScrolledTextOnce() throws {
        let p = scrollingPress(), text = try XCTUnwrap(p.views[5]), inner = try XCTUnwrap(p.views[4]), outer = try XCTUnwrap(p.views[2])
        inner.handlers = ["press"]
        var pressed: [UInt32] = []; p.onPress = { pressed.append($0) }
        let touch = try textTouch(p)
        text.touchesBegan([touch], with: nil)
        XCTAssertTrue(inner.pressed); XCTAssertFalse(outer.pressed)
        text.touchesEnded([touch], with: nil)
        XCTAssertEqual(pressed, [4])
    }

    func testAnInlineHandlerOrLinkTakesTheTextBeforeItsAncestor() throws {
        let p = scrollingPress(useSession: true), text = try XCTUnwrap(p.views[5]), outer = try XCTUnwrap(p.views[2])
        p.apply(wireBatch([["op": "props", "id": 5, "clear": ["text"]], ["op": "paragraph", "id": 5, "runs": [
            ["id": 100, "parent": 5, "paint": true, "props": ["text": "Read "]],
            ["id": 101, "parent": 5, "paint": true, "props": ["text": "guide"], "handlers": ["press"]],
            ["id": 102, "parent": 5, "paint": true, "props": ["text": " link", "href": "/guide"]],
        ]]]))
        var pressed: [UInt32] = []; p.onPress = { pressed.append($0) }
        for id in [UInt32(101), 102] {
            let run = try XCTUnwrap(p.inlineText(id))
            let rect = try XCTUnwrap(text.inlineRects(run).first)
            let point = text.convert(CGPoint(x: rect.midX, y: rect.midY), to: nil)
            XCTAssertTrue(window.hitTest(window.convert(point, from: nil), with: nil) === text)
            let touch = PointTouch(point, on: text)
            text.touchesBegan([touch], with: nil)
            XCTAssertEqual(text.inlinePressed, id)
            XCTAssertFalse(outer.pressed)
            text.touchesEnded([touch], with: nil)
            XCTAssertNil(text.inlinePressed)
        }
        XCTAssertEqual(pressed, [101], "neither inline run presses the ancestor")
    }

    func testTwoOverflowWrappersAndAScrolledOffsetKeepTheAncestor() throws {
        let p = scrollingPress(), text = try XCTUnwrap(p.views[5])
        p.apply(wireBatch([["op": "create", "id": 6, "kind": "view", "style": ["overflow_x": "auto", "overflow_y": "hidden"]],
                          ["op": "frame", "id": 6, "x": 0.0, "y": 0.0, "w": 640.0, "h": 44.0],
                          ["op": "content", "id": 6, "w": 900.0, "h": 44.0],
                          ["op": "children", "id": 6, "ids": [5]], ["op": "children", "id": 4, "ids": [6]]]))
        let inner = try XCTUnwrap(p.views[6]?.scroll), outer = try XCTUnwrap(p.views[4]?.scroll)
        XCTAssertTrue(text.superview === inner)
        XCTAssertTrue(p.views[6]?.superview === outer)
        outer.contentOffset.x = 100; inner.contentOffset.x = 20
        var pressed: [UInt32] = []; p.onPress = { pressed.append($0) }
        let point = outer.convert(CGPoint(x: outer.bounds.minX + 20, y: outer.bounds.midY), to: nil)
        XCTAssertTrue(window.hitTest(window.convert(point, from: nil), with: nil) === text)
        let touch = PointTouch(point, on: text)
        text.touchesBegan([touch], with: nil)
        XCTAssertTrue(p.views[2]?.pressed == true)
        text.touchesEnded([touch], with: nil)
        XCTAssertEqual(pressed, [2])
    }

    func testDisablingTheSourceOrInertingItsContainerCancelsArmedFeedback() throws {
        for (id, property) in [(5, "disabled"), (4, "inert"), (2, "disabled")] {
            let p = scrollingPress(), text = try XCTUnwrap(p.views[5]), button = try XCTUnwrap(p.views[2])
            var pressed: [UInt32] = []; p.onPress = { pressed.append($0) }
            let touch = try textTouch(p)
            text.touchesBegan([touch], with: nil)
            XCTAssertTrue(button.pressed)
            p.apply(wireBatch([["op": "props", "id": id, "set": [property: "true"]]]))
            XCTAssertFalse(button.pressed, "\(property) cancels the original owner immediately")
            text.touchesEnded([touch], with: nil)
            XCTAssertEqual(pressed, [])
        }
    }

    func testRetiringTheHitOrReparentingAnIntermediateCancelsTheOldOwner() throws {
        for retire in [true, false] {
            let p = scrollingPress(), text = try XCTUnwrap(p.views[5]), button = try XCTUnwrap(p.views[2])
            p.apply(wireBatch(node(6, "button", handlers: ["press"], y: 200, w: 320)
                              + [["op": "children", "id": 1, "ids": [2, 6]]]))
            var pressed: [UInt32] = []; p.onPress = { pressed.append($0) }
            let touch = try textTouch(p)
            text.touchesBegan([touch], with: nil)
            XCTAssertTrue(button.pressed)
            p.apply(wireBatch(retire ? [["op": "destroy", "id": 5]] : [["op": "children", "id": 6, "ids": [4]]]))
            XCTAssertFalse(button.pressed)
            text.touchesEnded([touch], with: nil)
            text.touchesCancelled([touch], with: nil)
            XCTAssertEqual(pressed, [], "the terminal phase cannot choose the new ancestor")
            XCTAssertFalse(p.views[6]?.pressed == true)
        }
    }

    func testAReboundOwnerKeepsItsNewContactWhenOldTerminalsArrive() throws {
        let p = scrollingPress(), text = try XCTUnwrap(p.views[5]), button = try XCTUnwrap(p.views[2])
        var pressed: [UInt32] = []; p.onPress = { pressed.append($0) }
        let old = try textTouch(p)
        text.touchesBegan([old], with: nil)
        let incarnation = button.incarnation
        button.rebind(6); p.views.removeValue(forKey: 2); p.views[6] = button
        XCTAssertNotEqual(button.incarnation, incarnation)
        XCTAssertFalse(button.pressed)
        let fresh = PointTouch(button.convert(CGPoint(x: 20, y: 20), to: nil), on: button)
        button.touchesBegan([fresh], with: nil)
        text.touchesEnded([old], with: nil)
        text.touchesCancelled([old], with: nil)
        XCTAssertTrue(button.pressed)
        button.touchesEnded([fresh], with: nil)
        XCTAssertEqual(pressed, [6])
    }

    func testAReplacementContactSurvivesItsSourcesOldRoute() throws {
        for phase in ["move", "end", "cancel", "retire"] {
            let p = scrollingPress(), text = try XCTUnwrap(p.views[5]), button = try XCTUnwrap(p.views[2])
            var pressed: [UInt32] = []; p.onPress = { pressed.append($0) }
            let touch = try textTouch(p), inside = touch.point
            text.touchesBegan([touch], with: nil)
            let original = try XCTUnwrap(button.pressContact)
            // Reuse the same UITouch deliberately: address equality cannot
            // distinguish two contact records, without relying on an allocator.
            button.touchesBegan([touch], with: nil)
            let replacement = try XCTUnwrap(button.pressContact)
            XCTAssertFalse(replacement === original)
            XCTAssertTrue(original.cancelled)
            let dimmed = button.press.dimmedAt
            switch phase {
            case "move":
                touch.point = button.convert(CGPoint(x: 20, y: button.bounds.maxY + 60), to: nil)
                text.touchesMoved([touch], with: nil)
                XCTAssertEqual(button.press.dimmedAt, dimmed, "the old route cannot change replacement feedback")
            case "end": text.touchesEnded([touch], with: nil)
            case "cancel": text.touchesCancelled([touch], with: nil)
            default: p.apply(wireBatch([["op": "destroy", "id": 5]]))
            }
            XCTAssertTrue(button.pressContact === replacement)
            XCTAssertFalse(replacement.cancelled)
            XCTAssertTrue(button.pressed)
            XCTAssertEqual(pressed, [])
            touch.point = inside
            button.touchesEnded([touch], with: nil)
            XCTAssertEqual(pressed, [2], "only the replacement owner's terminal activates")
        }
    }

    func testAForwardedPressResolvesBeforeFocusMovesItsOwner() throws {
        let p = scrollingPress(), text = try XCTUnwrap(p.views[5]), button = try XCTUnwrap(p.views[2])
        p.apply(wireBatch(node(6, "input", w: 300, h: 34) + [["op": "children", "id": 1, "ids": [6, 2]]]))
        let field = try XCTUnwrap(p.views[6]?.field)
        XCTAssertTrue(field.becomeFirstResponder())
        observers.append(NotificationCenter.default.addObserver(forName: UITextField.textDidEndEditingNotification, object: field, queue: nil) { _ in button.frame.origin.y += 150 })
        var pressed: [UInt32] = []; p.onPress = { pressed.append($0) }
        let touch = try textTouch(p)
        text.touchesBegan([touch], with: nil)
        text.touchesEnded([touch], with: nil)
        XCTAssertFalse(button.pressInside(touch))
        XCTAssertEqual(pressed, [2])
        XCTAssertTrue(button.isFirstResponder)
    }

    func testRetainFocusAndUnhandledGroundThroughOverflowKeepTheirRules() throws {
        for retained in [true, false] {
            let p = scrollingPress(ownerKind: retained ? "button" : "view", handlers: retained ? ["press"] : [])
            p.apply(wireBatch(node(6, "input", w: 300, h: 34) + [["op": "children", "id": 1, "ids": [6, 2]],
                              ["op": "props", "id": 2, "set": ["retainFocus": retained ? "true" : "false"]]]))
            let field = try XCTUnwrap(p.views[6]?.field), text = try XCTUnwrap(p.views[5])
            XCTAssertTrue(field.becomeFirstResponder())
            var pressed: [UInt32] = []; p.onPress = { pressed.append($0) }
            let touch = try textTouch(p)
            text.touchesBegan([touch], with: nil); text.touchesEnded([touch], with: nil)
            XCTAssertEqual(field.isFirstResponder, retained)
            XCTAssertEqual(pressed, retained ? [2] : [])
        }
    }

    // MARK: A press survives the button moving as it takes the focus

    /// The field resigning moves the button off the finger, as the keyboard
    /// lowering under `resizes-content` does; the touch still presses it.
    func testAPressSurvivesTheButtonMovingAsItTakesTheFocus() throws {
        let p = composer()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let field = try XCTUnwrap(p.views[2]?.field), button = try XCTUnwrap(p.views[3])
        XCTAssertTrue(field.becomeFirstResponder())
        var moved = false
        observers.append(NotificationCenter.default.addObserver(forName: UITextField.textDidEndEditingNotification, object: field, queue: nil) { _ in
            // The keyboard's relayout: the toolbar drops 150 pt, at once.
            button.frame.origin.y += 150; moved = true
        })
        let finger = button.convert(CGPoint(x: button.bounds.midX, y: button.bounds.midY), to: nil)
        let touch = PointTouch(finger, on: button)
        button.touchesBegan([touch], with: nil)
        XCTAssertTrue(button.pressed)
        button.touchesEnded([touch], with: nil)
        XCTAssertTrue(moved, "the field resigned as the button took the focus, moving the button")
        XCTAssertFalse(button.pressInside(touch), "the finger is outside the button where it now stands")
        XCTAssertTrue(button.isFirstResponder, "the button took the focus")
        XCTAssertFalse(field.isFirstResponder)
        XCTAssertEqual(pressed, [3], "the press is resolved where the finger went down and came up, before the focus moved")
        XCTAssertFalse(button.pressed)
    }

    /// The contrast: a finger that is outside when it lifts, with no focus
    /// change moving anything, presses nothing.
    func testAFingerLiftedOutsideStillPressesNothing() throws {
        let p = composer()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let button = try XCTUnwrap(p.views[3])
        let down = PointTouch(button.convert(CGPoint(x: button.bounds.midX, y: button.bounds.midY), to: nil), on: button)
        button.touchesBegan([down], with: nil)
        down.point = button.convert(CGPoint(x: button.bounds.midX, y: button.bounds.maxY + 60), to: nil)
        button.touchesEnded([down], with: nil)
        XCTAssertEqual(pressed, [], "lifted outside: no press")
        XCTAssertFalse(button.pressed)
    }

    // MARK: A button holding the focus its touch gave it does not stop a later autofocus

    /// A tap on the button, as a finger's: down and up at its centre.
    private func tap(_ button: NodeView) {
        let touch = PointTouch(button.convert(CGPoint(x: button.bounds.midX, y: button.bounds.midY), to: nil), on: button)
        button.touchesBegan([touch], with: nil)
        button.touchesEnded([touch], with: nil)
    }

    /// The FAB took the focus on its touch and opened a sheet whose
    /// textarea autofocuses: the textarea gets the focus.
    func testAButtonHoldingTheFocusDoesNotStopALaterTextareasAutofocus() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "button", handlers: ["press"])
                          + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        let fab = try XCTUnwrap(p.views[2])
        tap(fab)
        XCTAssertTrue(fab.isFirstResponder && fab.focusedByTouch, "the FAB took the focus on its touch")
        p.apply(wireBatch(node(3, "textarea", ["autofocus": "true"], y: 100, w: 300, h: 120)
                          + [["op": "children", "id": 1, "ids": [2, 3]]]))
        p.syncAccessibility()
        let area = try XCTUnwrap(p.views[3]?.textArea)
        XCTAssertTrue(area.isFirstResponder, "the sheet's textarea takes its autofocus")
        XCTAssertFalse(fab.isFirstResponder)
    }

    /// The same with a focusable non-text node (an explicit tabindex).
    func testAButtonHoldingTheFocusDoesNotStopALaterFocusablesAutofocus() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "button", handlers: ["press"])
                          + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        let fab = try XCTUnwrap(p.views[2])
        tap(fab)
        XCTAssertTrue(fab.isFirstResponder && fab.focusedByTouch, "the FAB took the focus on its touch")
        p.apply(wireBatch(node(3, "view", ["tabIndex": "0", "autofocus": "true"], y: 100)
                          + [["op": "children", "id": 1, "ids": [2, 3]]]))
        p.syncAccessibility()
        let target = try XCTUnwrap(p.views[3])
        XCTAssertTrue(target.isFirstResponder, "the focusable takes its autofocus")
        XCTAssertFalse(fab.isFirstResponder)
    }

    /// A button focused any other way (`focus(id)`, Tab) keeps the focus
    /// against a later autofocus, as before, even after a touch focused it.
    func testAButtonFocusedWithoutATouchKeepsIt() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "button", ["id": "go"], handlers: ["press"])
                          + node(4, "button", handlers: ["press"], y: 300)
                          + [["op": "children", "id": 1, "ids": [2, 4]], ["op": "roots", "ids": [1]]])
        let button = try XCTUnwrap(p.views[2]), other = try XCTUnwrap(p.views[4])
        tap(button)
        XCTAssertTrue(button.focusedByTouch)
        p.focusElement(["go"])
        XCTAssertTrue(button.isFirstResponder)
        XCTAssertFalse(button.focusedByTouch, "focus(id) makes it the app's focus")
        p.apply(wireBatch(node(3, "textarea", ["autofocus": "true"], y: 100, w: 300, h: 120)
                          + [["op": "children", "id": 1, "ids": [2, 3, 4]]]))
        p.syncAccessibility()
        XCTAssertTrue(button.isFirstResponder, "the button keeps the focus focus(id) gave it")
        XCTAssertFalse(try XCTUnwrap(p.views[3]?.textArea).isFirstResponder)

        tap(other)
        XCTAssertTrue(other.focusedByTouch)
        button.focusedByTouch = true // Tab's destination, as if a touch had focused it once
        p.moveFocus(backward: false)
        let tabbed = try XCTUnwrap([button, other].first { $0.isFirstResponder })
        XCTAssertFalse(tabbed.focusedByTouch, "Tab makes it the keyboard's focus")
        p.apply(wireBatch(node(5, "view", ["tabIndex": "0", "autofocus": "true"], y: 250)
                          + [["op": "children", "id": 1, "ids": [2, 3, 4, 5]]]))
        p.syncAccessibility()
        XCTAssertTrue(tabbed.isFirstResponder, "the button keeps the focus Tab gave it")
    }

    /// UIKit hands the focus back to the button when its view moves (the
    /// viewport into a presented sheet): still the touch's focus.
    func testATouchFocusSurvivesUIKitHandingItBack() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "button", handlers: ["press"])
                          + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        let fab = try XCTUnwrap(p.views[2])
        tap(fab)
        XCTAssertTrue(fab.becomeFirstResponder(), "UIKit's own re-promotion calls this")
        XCTAssertTrue(fab.focusedByTouch)
    }

    /// The contrast, the existing rule: a field or a non-button focusable
    /// holding the focus keeps it; autofocus never steals it.
    func testAFieldOrAFocusableHoldingTheFocusKeepsIt() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "input", w: 300, h: 34) + node(4, "view", ["tabIndex": "0"], y: 300)
                          + [["op": "children", "id": 1, "ids": [2, 4]], ["op": "roots", "ids": [1]]])
        let field = try XCTUnwrap(p.views[2]?.field)
        XCTAssertTrue(field.becomeFirstResponder())
        p.apply(wireBatch(node(3, "textarea", ["autofocus": "true"], y: 100, w: 300, h: 120)
                          + [["op": "children", "id": 1, "ids": [2, 3, 4]]]))
        p.syncAccessibility()
        XCTAssertTrue(field.isFirstResponder, "the field being edited keeps the focus")
        XCTAssertFalse(try XCTUnwrap(p.views[3]?.textArea).isFirstResponder)

        let focusable = try XCTUnwrap(p.views[4])
        XCTAssertTrue(focusable.becomeFirstResponder())
        p.apply(wireBatch(node(5, "view", ["tabIndex": "0", "autofocus": "true"], y: 250)
                          + [["op": "children", "id": 1, "ids": [2, 3, 4, 5]]]))
        p.syncAccessibility()
        XCTAssertTrue(focusable.isFirstResponder, "a non-button focusable keeps the focus")
        XCTAssertFalse(try XCTUnwrap(p.views[5]).isFirstResponder)
    }
}
#endif

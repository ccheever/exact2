#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A press and the focus it takes on UIKit. A tap focuses only a node that
/// asked for focus (`focusesOnPress`), as UIKit's buttons take none from a
/// touch; a plain button's press ends the editing, so the field it ends
/// lowers the keyboard, and under `resizes-content` the button moves at
/// once, yet the press is the finger's where it went down and came up. A
/// button holding the focus a touch gave it (an opted-in FAB that opened a
/// sheet) is no focus a node took for the autofocus pass, which still gives
/// the sheet's field its autofocus, while a field, another focusable, or a
/// button focused any other way (Tab, `focus(id)`) keeps its own.
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
        XCTAssertFalse(button.isFirstResponder, "a plain button takes no focus from a touch")
        XCTAssertFalse(field.isFirstResponder)
    }

    func testAForwardedPressKeepsTheTouchedNodesFocusDecision() throws {
        for retained in [false, true] {
            let p = scrollingPress(), text = try XCTUnwrap(p.views[5]), button = try XCTUnwrap(p.views[2])
            p.apply(wireBatch(node(6, "input", w: 300, h: 34) + [["op": "children", "id": 1, "ids": [6, 2]],
                ["op": "props", "id": 5, "set": ["tabIndex": "0", "retainFocus": retained ? "true" : "false"]]]))
            text.handlers = ["focus"]
            let field = try XCTUnwrap(p.views[6]?.field)
            XCTAssertTrue(field.becomeFirstResponder())
            var events: [String] = []
            p.onFocus = { events.append("focus \($0)") }
            p.onPress = { events.append("press \($0)") }
            let touch = try textTouch(p)
            text.touchesBegan([touch], with: nil)
            text.touchesEnded([touch], with: nil)
            XCTAssertEqual(events, retained ? ["press 2"] : ["focus 5", "press 2"], "the touched node settles focus once before the ancestor's press")
            XCTAssertEqual(text.isFirstResponder, !retained)
            XCTAssertEqual(field.isFirstResponder, retained)
            XCTAssertFalse(button.isFirstResponder)
        }
    }

    func testAForwardedFocusCallbackCanRetireTheHitBeforeItsPress() throws {
        let p = scrollingPress(), text = try XCTUnwrap(p.views[5]), button = try XCTUnwrap(p.views[2])
        p.apply(wireBatch([["op": "props", "id": 5, "set": ["tabIndex": "0"]]]))
        text.handlers = ["focus"]
        var focused: [UInt32] = [], pressed: [UInt32] = []
        p.onFocus = { id in
            focused.append(id)
            p.apply(wireBatch([["op": "destroy", "id": 5]]))
        }
        p.onPress = { pressed.append($0) }
        let touch = try textTouch(p)
        text.touchesBegan([touch], with: nil)
        XCTAssertTrue(button.pressed)
        text.touchesEnded([touch], with: nil)
        XCTAssertEqual(focused, [5])
        XCTAssertNil(p.views[5])
        XCTAssertFalse(button.pressed, "retiring the hit cancels the captured ancestor before activation")
        XCTAssertEqual(pressed, [])
        text.touchesEnded([touch], with: nil)
        text.touchesCancelled([touch], with: nil)
        XCTAssertEqual(pressed, [])
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
        XCTAssertTrue(moved, "the field resigned as the button was pressed, moving the button")
        XCTAssertFalse(button.pressInside(touch), "the finger is outside the button where it now stands")
        XCTAssertFalse(button.isFirstResponder, "a button takes no focus from a touch, as UIKit's")
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

    // MARK: A tapped button does not stop a later autofocus

    /// A tap on the button, as a finger's: down and up at its centre.
    private func tap(_ button: NodeView) {
        let touch = PointTouch(button.convert(CGPoint(x: button.bounds.midX, y: button.bounds.midY), to: nil), on: button)
        button.touchesBegan([touch], with: nil)
        button.touchesEnded([touch], with: nil)
    }

    /// The FAB, tapped, opened a sheet whose textarea autofocuses: the
    /// textarea gets the focus.
    func testATappedButtonDoesNotStopALaterTextareasAutofocus() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "button", handlers: ["press"])
                          + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        let fab = try XCTUnwrap(p.views[2])
        tap(fab)
        XCTAssertFalse(fab.isFirstResponder, "a button takes no focus from a touch, as UIKit's")
        p.apply(wireBatch(node(3, "textarea", ["autofocus": "true"], y: 100, w: 300, h: 120)
                          + [["op": "children", "id": 1, "ids": [2, 3]]]))
        p.syncAccessibility()
        let area = try XCTUnwrap(p.views[3]?.textArea)
        XCTAssertTrue(area.isFirstResponder, "the sheet's textarea takes its autofocus")
        XCTAssertFalse(fab.isFirstResponder)
    }

    /// A FAB that asked for focus (a tabindex) holds the focus its tap gave
    /// it, and a later focusable's autofocus still takes it.
    func testAButtonHoldingItsTouchFocusDoesNotStopALaterFocusablesAutofocus() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "button", ["tabIndex": "0"], handlers: ["press"])
                          + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        let fab = try XCTUnwrap(p.views[2])
        tap(fab)
        XCTAssertTrue(fab.isFirstResponder && fab.focusedByTouch, "it asked for focus, and the tap gave it")
        p.apply(wireBatch(node(3, "view", ["tabIndex": "0", "autofocus": "true"], y: 100)
                          + [["op": "children", "id": 1, "ids": [2, 3]]]))
        p.syncAccessibility()
        let target = try XCTUnwrap(p.views[3])
        XCTAssertTrue(target.isFirstResponder, "the focusable takes its autofocus")
        XCTAssertFalse(fab.isFirstResponder)
    }

    /// A button focused another way (`focus(id)`, Tab) keeps the focus
    /// against a later autofocus, whatever a touch did before.
    func testAButtonFocusedWithoutATouchKeepsIt() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "button", ["id": "go"], handlers: ["press"])
                          + node(4, "button", handlers: ["press"], y: 300)
                          + [["op": "children", "id": 1, "ids": [2, 4]], ["op": "roots", "ids": [1]]])
        let button = try XCTUnwrap(p.views[2]), other = try XCTUnwrap(p.views[4])
        tap(button)
        XCTAssertFalse(button.isFirstResponder)
        p.focusElement(["go"])
        XCTAssertTrue(button.isFirstResponder)
        XCTAssertFalse(button.focusedByTouch, "focus(id) makes it the app's focus")
        p.apply(wireBatch(node(3, "textarea", ["autofocus": "true"], y: 100, w: 300, h: 120)
                          + [["op": "children", "id": 1, "ids": [2, 3, 4]]]))
        p.syncAccessibility()
        XCTAssertTrue(button.isFirstResponder, "the button keeps the focus focus(id) gave it")
        XCTAssertFalse(try XCTUnwrap(p.views[3]?.textArea).isFirstResponder)

        _ = button.resignFirstResponder() // a tap elsewhere would leave it focused
        button.focusedByTouch = true // Tab's destination, as if a touch had focused it once
        p.moveFocus(backward: false)
        let tabbed = try XCTUnwrap([button, other].first { $0.isFirstResponder })
        XCTAssertFalse(tabbed.focusedByTouch, "Tab makes it the keyboard's focus")
        p.apply(wireBatch(node(5, "view", ["tabIndex": "0", "autofocus": "true"], y: 250)
                          + [["op": "children", "id": 1, "ids": [2, 3, 4, 5]]]))
        p.syncAccessibility()
        XCTAssertTrue(tabbed.isFirstResponder, "the button keeps the focus Tab gave it")
    }

    /// UIKit hands the focus back to a button that asked for focus (a
    /// tabindex) when its view moves (the viewport into a presented sheet):
    /// still the touch's focus.
    func testATouchFocusSurvivesUIKitHandingItBack() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "button", ["tabIndex": "0"], handlers: ["press"])
                          + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        let fab = try XCTUnwrap(p.views[2])
        tap(fab)
        XCTAssertTrue(fab.becomeFirstResponder(), "UIKit's own re-promotion calls this")
        XCTAssertTrue(fab.focusedByTouch)
    }

    /// A button that asked for focus (it hears `focus`) takes it from a
    /// tap, before its press, the web's order; one that did not takes none.
    func testOnlyAButtonThatAskedForFocusTakesItFromATap() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "button", handlers: ["press", "focus"])
                          + node(3, "button", handlers: ["press"], y: 100)
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]])
        var log: [String] = []
        p.onPress = { id in log.append("press \(id) focused \(p.views[id]?.isFirstResponder == true)") }
        let asked = try XCTUnwrap(p.views[2]), plain = try XCTUnwrap(p.views[3])
        tap(plain)
        XCTAssertFalse(plain.isFirstResponder, "a button alone takes no focus")
        XCTAssertTrue(plain.canBecomeFirstResponder, "the keyboard can still focus it")
        tap(asked)
        XCTAssertTrue(asked.isFirstResponder)
        XCTAssertEqual(log, ["press 3 focused false", "press 2 focused true"])
    }

    /// A button that hears `keyup` asked for the focus its key releases
    /// need, as one hearing `key` did.
    func testAButtonHearingKeyUpTakesTheFocusFromATap() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "button", handlers: ["press", "keyup"])
                          + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        let button = try XCTUnwrap(p.views[2])
        tap(button)
        XCTAssertTrue(button.isFirstResponder)
    }

    /// A press ends only text editing, as a click blurs an input: any other
    /// focus stays where it is, as UIKit's buttons leave the first responder.
    /// A button focused by `focus(id)` keeps it through a finger's tap on it,
    /// and a box that asked for focus inside a pressed button keeps the focus
    /// its tap gave it as the press goes on to the button.
    func testAPressLeavesAFocusThatIsNotTextEditing() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "button", ["id": "go"], handlers: ["press"])
                          + node(3, "button", handlers: ["press"], y: 100, w: 200, h: 100) + node(4, "view", ["tabIndex": "0"], w: 60, h: 40)
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "children", "id": 3, "ids": [4]], ["op": "roots", "ids": [1]]])
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let go = try XCTUnwrap(p.views[2]), box = try XCTUnwrap(p.views[4])
        p.focusElement(["go"])
        tap(go)
        XCTAssertTrue(go.isFirstResponder, "focus(id)'s focus stays through a tap")
        tap(box)
        XCTAssertTrue(box.isFirstResponder, "the box keeps the focus it asked for")
        XCTAssertEqual(pressed, [2, 3], "and the press went on to the button")
    }

    /// A button's `focus` handler that mounts an autofocus textarea leaves
    /// it editing through the press: a press ends only the text editing under
    /// way as it began, never focus its own callbacks chose.
    func testAFocusHandlersAutofocusSurvivesThePress() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "button", handlers: ["press", "focus"])
                          + [["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]])
        p.onFocus = { _ in
            p.apply(wireBatch(self.node(3, "textarea", ["autofocus": "true"], y: 100, w: 300, h: 120)
                              + [["op": "children", "id": 1, "ids": [2, 3]]]))
            p.syncAccessibility()
        }
        tap(try XCTUnwrap(p.views[2]))
        XCTAssertTrue(try XCTUnwrap(p.views[3]?.textArea).isFirstResponder, "the textarea its focus handler mounted is editing")
    }

    /// Focus a press's callbacks choose stays: a box that asked for focus
    /// inside a pressed button, whose `focus` handler focuses an input, and a
    /// button that asked for focus whose handler focuses the input being
    /// edited again. The press settles its focus once, before its callbacks.
    func testFocusAPresssCallbacksChooseStays() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "input", ["id": "editor"], w: 300, h: 34)
                          + node(3, "button", handlers: ["press"], y: 100, w: 200, h: 100) + node(4, "view", ["tabIndex": "0"], handlers: ["focus"], w: 60, h: 40)
                          + node(5, "button", handlers: ["press", "focus"], y: 300)
                          + [["op": "children", "id": 1, "ids": [2, 3, 5]], ["op": "children", "id": 3, "ids": [4]], ["op": "roots", "ids": [1]]])
        p.onFocus = { id in if id == 4 || id == 5 { p.focusElement(["editor"]) } }
        let field = try XCTUnwrap(p.views[2]?.field)
        tap(try XCTUnwrap(p.views[4]))
        XCTAssertTrue(field.isFirstResponder, "the box's focus handler chose the input")
        tap(try XCTUnwrap(p.views[5]))
        XCTAssertTrue(field.isFirstResponder, "the button's focus handler chose the input being edited again")
    }

    /// A touch on a node inside the button, which bubbles to it, resolves the
    /// press where the finger went down and came up too, before the field
    /// resigns and the button moves.
    func testAPressThroughAChildSurvivesTheButtonMoving() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "input", w: 300, h: 34) + node(3, "button", handlers: ["press"], y: 200)
                          + node(4, "view", ["testId": "label"], w: 60, h: 30) + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "children", "id": 3, "ids": [4]], ["op": "roots", "ids": [1]]])
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let field = try XCTUnwrap(p.views[2]?.field), button = try XCTUnwrap(p.views[3]), child = try XCTUnwrap(p.views[4])
        XCTAssertTrue(field.becomeFirstResponder())
        observers.append(NotificationCenter.default.addObserver(forName: UITextField.textDidEndEditingNotification, object: field, queue: nil) { _ in
            button.frame.origin.y += 150
        })
        let touch = PointTouch(child.convert(CGPoint(x: child.bounds.midX, y: child.bounds.midY), to: nil), on: child)
        child.touchesBegan([touch], with: nil)
        XCTAssertTrue(button.pressed)
        child.touchesEnded([touch], with: nil)
        XCTAssertFalse(field.isFirstResponder)
        XCTAssertEqual(pressed, [3], "the press is resolved before the focus moved")
    }

    /// A press that changes nothing changes no focus: under a `retainFocus`
    /// node it lands in, though its target is an ancestor that asked for
    /// focus, and past a disabled button it lands inside.
    func testAPressThatChangesNothingKeepsTheEditing() throws {
        let p = presenter(node(1, "view", w: 400, h: 400) + node(2, "input", w: 300, h: 34)
                          + node(3, "view", ["tabIndex": "0"], handlers: ["press", "focus"], y: 100, w: 300, h: 200)
                          + node(4, "button", ["retainFocus": "true"], w: 80) + node(5, "button", ["disabled": "true"], handlers: ["press"], y: 100, w: 80)
                          + node(6, "view", ["testId": "label"], w: 40, h: 20)
                          + [["op": "children", "id": 1, "ids": [2, 3]], ["op": "children", "id": 3, "ids": [4, 5]], ["op": "children", "id": 5, "ids": [6]],
                             ["op": "roots", "ids": [1]]])
        let field = try XCTUnwrap(p.views[2]?.field), owner = try XCTUnwrap(p.views[3])
        XCTAssertTrue(field.becomeFirstResponder())
        tap(try XCTUnwrap(p.views[4]))
        XCTAssertTrue(field.isFirstResponder, "retainFocus where the press landed keeps the editing")
        XCTAssertFalse(owner.isFirstResponder)
        tap(try XCTUnwrap(p.views[6]))
        XCTAssertTrue(field.isFirstResponder, "a disabled button stops the press and its focus")
        XCTAssertFalse(owner.isFirstResponder)
    }

    /// The agent's tap goes as a finger's: a button that takes no focus
    /// passes it to the ancestor that asked for it (a `tabindex`) on the way
    /// to the press's target.
    func testAnAgentTapPassesTheFocusPastAButtonToTheAncestorThatAskedForIt() throws {
        let session = ExactApp.shared.makeSession(label: "tap-focus-walk")
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 400, height: 400)).error)
        let view = ExactView(session: session)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        view.frame = window.bounds
        window.addSubview(view)
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        session.apply(wireBatch([
            ["op": "create", "id": 9101, "kind": "view", "props": ["tabIndex": "0"], "handlers": ["press", "focus"], "style": [:]],
            ["op": "create", "id": 9102, "kind": "button", "props": [:], "handlers": [], "style": [:]],
            ["op": "children", "id": 9101, "ids": [9102]],
            ["op": "roots", "ids": [9101]],
            ["op": "frame", "id": 9101, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "frame", "id": 9102, "x": 20.0, "y": 20.0, "w": 120.0, "h": 44.0],
        ]))
        view.layoutIfNeeded()
        let reply = Agent(session: session).tap(["op": "tap", "id": 9102])
        XCTAssertFalse(try XCTUnwrap(session.presenter.views[9102]).isFirstResponder, "the button takes none")
        XCTAssertTrue(try XCTUnwrap(session.presenter.views[9101]).isFirstResponder, "the ancestor that asked does: \(reply) \(String(describing: FirstResponder.current))")
    }

    /// The agent's tap under a `retainFocus` button, whose press goes to an
    /// ancestor that asked for focus, keeps the editing, as a finger's does.
    func testAnAgentTapUnderRetainFocusKeepsTheEditing() throws {
        let session = ExactApp.shared.makeSession(label: "tap-focus-retain")
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 400, height: 400)).error)
        let view = ExactView(session: session)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        view.frame = window.bounds
        window.addSubview(view)
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        session.apply(wireBatch([
            ["op": "create", "id": 9101, "kind": "view", "props": ["tabIndex": "0"], "handlers": ["press", "focus"], "style": [:]],
            ["op": "create", "id": 9102, "kind": "button", "props": ["retainFocus": "true"], "handlers": [], "style": [:]],
            ["op": "create", "id": 9103, "kind": "input", "props": [:], "handlers": [], "style": [:]],
            ["op": "children", "id": 9101, "ids": [9102, 9103]],
            ["op": "roots", "ids": [9101]],
            ["op": "frame", "id": 9101, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "frame", "id": 9102, "x": 20.0, "y": 20.0, "w": 120.0, "h": 44.0],
            ["op": "frame", "id": 9103, "x": 20.0, "y": 100.0, "w": 200.0, "h": 34.0],
        ]))
        view.layoutIfNeeded()
        let field = try XCTUnwrap(session.presenter.views[9103]?.field)
        XCTAssertTrue(field.becomeFirstResponder())
        let reply = Agent(session: session).tap(["op": "tap", "id": 9102])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertTrue(field.isFirstResponder, "retainFocus where the tap landed keeps the editing")
        XCTAssertFalse(try XCTUnwrap(session.presenter.views[9101]).isFirstResponder)
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

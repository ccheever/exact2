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
    private var observers: [NSObjectProtocol] = []

    override func tearDown() {
        observers.forEach(NotificationCenter.default.removeObserver)
        observers = []
        window?.isHidden = true
        window = nil
        super.tearDown()
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
        let point: CGPoint, target: UIView
        init(_ point: CGPoint, on target: UIView) { self.point = point; self.target = target; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint { view.map { $0.convert(point, from: nil) } ?? point }
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
        let up = PointTouch(button.convert(CGPoint(x: button.bounds.midX, y: button.bounds.maxY + 60), to: nil), on: button)
        button.touchesEnded([up], with: nil)
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

#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// The transparent button over a popover's invoker (`InvokerButton`): what
/// keeps a touch inside the invoker, the press it dispatches, and its share
/// of the invoker's press look.
final class InvokerButtonIOSTests: XCTestCase {
    private final class Touch: UITouch {
        let target: UIView
        init(_ target: UIView) { self.target = target; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint {
            target.convert(CGPoint(x: target.bounds.midX, y: target.bounds.midY), to: view)
        }
    }
    private var window: UIWindow?

    override func tearDown() {
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    /// A page (1) holding an invoker (2) of a popover (6) with `role`, the invoker holding `children`.
    private func page(_ children: [[String: Any]], ids: [Int], rootHandlers: [String] = [], role: String = "menu") -> Presenter {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        self.window = window
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": rootHandlers],
            ["op": "create", "id": 2, "kind": "button", "props": ["popovertarget": "menu"]],
            ["op": "create", "id": 6, "kind": "view", "props": ["id": "menu", "popover": "auto", "accessibilityRole": role]],
            ["op": "create", "id": 7, "kind": "button", "props": ["text": "Row"], "handlers": ["press"]],
            ["op": "children", "id": 6, "ids": [7]],
        ] + children + [
            ["op": "children", "id": 2, "ids": ids],
            ["op": "children", "id": 1, "ids": [2, 6]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 400],
            ["op": "frame", "id": 2, "x": 20, "y": 40, "w": 200, "h": 44],
            ["op": "frame", "id": 6, "x": 20, "y": 120, "w": 200, "h": 100],
            ["op": "frame", "id": 7, "x": 0, "y": 0, "w": 200, "h": 44],
        ]))
        p.viewport.layoutIfNeeded()
        return p
    }
    private func overlay(_ p: Presenter) throws -> InvokerButton {
        try XCTUnwrap(p.views[2]?.subviews.compactMap { $0 as? InvokerButton }.first, "the invoker has its button")
    }
    /// What a finger at `x` points into the invoker lands on.
    private func hit(_ p: Presenter, x: CGFloat) throws -> UIView? {
        let invoker = try XCTUnwrap(p.views[2])
        return window?.hitTest(invoker.convert(CGPoint(x: x, y: 22), to: nil), with: nil)
    }

    func testAPanCancelsAConfirmationInvokerButKeepsPrimaryMenuTracking() throws {
        let scroll = ScrollView()
        let confirmationPage = page([], ids: [], role: "alertdialog")
        let confirmation = try overlay(confirmationPage)
        XCTAssertTrue(type(of: confirmation) == InvokerButton.self, "the initial confirmation uses the ordinary wrapper")
        XCTAssertFalse(confirmation.showsMenuAsPrimaryAction)
        XCTAssertTrue(scroll.touchesShouldCancel(in: confirmation), "a pan cancels a fresh confirmation button")

        let invoker = try XCTUnwrap(confirmationPage.views[2])
        confirmation.isHighlighted = true
        XCTAssertTrue(invoker.pressed)
        confirmation.touchesCancelled([], with: nil)
        XCTAssertFalse(invoker.pressed, "cancellation releases the invoker's press look")
        window?.isHidden = true

        let p = page([], ids: [])
        let menu = try overlay(p)
        XCTAssertTrue(menu.showsMenuAsPrimaryAction)
        XCTAssertFalse(scroll.touchesShouldCancel(in: menu), "a touch-down menu keeps UIKit's tracking")

        p.apply(wireBatch([["op": "props", "id": 6, "set": ["accessibilityRole": "alertdialog"]]]))
        let retargeted = try overlay(p)
        XCTAssertFalse(retargeted.showsMenuAsPrimaryAction)
        XCTAssertTrue(scroll.touchesShouldCancel(in: retargeted), "a reused menu wrapper follows its current primary action")
        XCTAssertFalse(scroll.touchesShouldCancel(in: UISlider()))
        XCTAssertFalse(scroll.touchesShouldCancel(in: UITextField()))
    }

    func testAFlatLeafOverAnInteractiveChildKeepsTheMenu() throws {
        let p = page([
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"]],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 200, "h": 44],
            ["op": "create", "id": 4, "kind": "view",
             "style": ["background_color": [0, 122, 255, 255], "written": ["background_color"]]],
            ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 100, "h": 44],
        ], ids: [3, 4])
        XCTAssertNil(p.views[4], "the decoration is a flat leaf")
        let button = try overlay(p)
        XCTAssertTrue(try hit(p, x: 50) === button, "the leaf paints over the child there: the menu's")
        XCTAssertTrue(try hit(p, x: 150) === p.views[3], "the child's own button keeps its touch")
    }

    func testAPlatformViewThatTakesTouchesKeepsThem() throws {
        let p = page([
            ["op": "create", "id": 3, "kind": "view", "props": ["testId": "host"]],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 200, "h": 44],
        ], ids: [3])
        let host = try XCTUnwrap(p.views[3]), button = try overlay(p)
        let platform = UIView(frame: host.bounds)
        host.addSubview(platform)
        XCTAssertTrue(try hit(p, x: 50) === button, "a view that only draws is the invoker's content")
        let tap = UITapGestureRecognizer()
        platform.addGestureRecognizer(tap)
        XCTAssertTrue(try hit(p, x: 50) === platform, "a view with its own recognizer keeps the touch")
        tap.isEnabled = false
        XCTAssertTrue(try hit(p, x: 50) === button)
    }

    func testTheInvokersOwnOverflowScrollerKeepsItsMenu() throws {
        let p = page([
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Open"]],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 200, "h": 100],
        ], ids: [3])
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["overflow_y": "auto"]]]))
        let invoker = try XCTUnwrap(p.views[2])
        XCTAssertNotNil(invoker.scroll)
        XCTAssertTrue(try hit(p, x: 50) === overlay(p), "its own pan does not take the menu's tap")
    }

    func testTheMenusPressIsTheOneATouchResolves() throws {
        let p = page([
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Open"]],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 200, "h": 44],
        ], ids: [3], rootHandlers: ["press"])
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        let invoker = try XCTUnwrap(p.views[2])
        p.menus.pressAsTouched(invoker)
        XCTAssertEqual(presses, [1], "an invoker with no handler presses its ancestor, as a touch on it would")
        invoker.handlers = ["press"]
        p.menus.pressAsTouched(invoker)
        XCTAssertEqual(presses, [1, 2])
    }

    func testThePressLookStaysWhileEitherHolderHoldsIt() throws {
        let p = page([], ids: [])
        let invoker = try XCTUnwrap(p.views[2]), button = try overlay(p)
        invoker.handlers = ["press"]
        button.isHighlighted = true
        XCTAssertTrue(invoker.pressed)
        let finger = Touch(invoker)
        invoker.touchesBegan([finger], with: nil)
        invoker.touchesCancelled([finger], with: nil)
        XCTAssertTrue(invoker.pressed, "a second finger leaving does not end the button's hold")
        button.isHighlighted = false
        XCTAssertFalse(invoker.pressed)
        let other = Touch(invoker)
        invoker.touchesBegan([other], with: nil)
        XCTAssertTrue(invoker.pressed)
        button.isHighlighted = true
        button.isHighlighted = false
        XCTAssertTrue(invoker.pressed, "the button letting go does not end a finger's press")
        invoker.touchesCancelled([other], with: nil)
        XCTAssertFalse(invoker.pressed)
        button.isHighlighted = true
        button.removeFromSuperview()
        XCTAssertFalse(invoker.pressed, "a button taken away lets go")
    }
}
#endif

#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A presented alert survives a rebuilt invoker; an anchored popover needs its source view.
final class InvokerLifetimeIOSTests: XCTestCase {
    private var window: UIWindow?
    private var presenter: Presenter?
    private var pressed: [UInt32] = []

    override func tearDown() {
        presenter?.menus.reset()
        if let controller = window?.rootViewController {
            waitFor("the owned presentation dismisses") { controller.presentedViewController == nil }
        }
        window?.isHidden = true
        window = nil
        presenter = nil
        super.tearDown()
    }

    func testAModalConfirmationOutlivesItsInvoker() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a presentation completes only in a scene's window") }
        let (p, controller) = try page()
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[40])), true, "its invoker opens it")
        waitForTitle("Lock your vehicle?", p, controller)
        let dialog = try XCTUnwrap(p.views[20])
        XCTAssertEqual(p.menus.observation()?["source"] as? Int, 40)
        try XCTUnwrap(p.views[40]).props["disabled"] = "true"
        p.menus.sync()
        XCTAssertTrue(p.menus.isOpen(dialog), "a disabled invoker leaves the question up")
        p.apply(wireBatch([["op": "roots", "ids": [20, 30, 41]], ["op": "destroy", "id": 40]]))
        p.menus.sync()
        XCTAssertTrue(p.menus.isOpen(dialog), "a removed invoker leaves the question up")
        XCTAssertEqual((controller.presentedViewController as? UIAlertController)?.title, "Lock your vehicle?")
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[22])), true, "its action can still be chosen")
        waitFor("the alert leaves and the action presses once") {
            controller.presentedViewController == nil && !p.menus.inTransition && self.pressed == [22]
        }
    }

    func testAPopoverLeavesWithTheInvokerItIsAnchoredAt() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a popover presents only in a scene's window") }
        let (p, controller) = try page()
        XCTAssertEqual(p.menus.activate(try XCTUnwrap(p.views[41])), true, "its invoker opens it")
        let chooser = try XCTUnwrap(p.views[30])
        waitFor("the chooser presents as a popover at its invoker") {
            (controller.presentedViewController as? UIAlertController)?.popoverPresentationController?.sourceView != nil && !p.menus.inTransition
        }
        p.apply(wireBatch([["op": "roots", "ids": [20, 30, 40]], ["op": "destroy", "id": 41]]))
        p.menus.sync()
        waitFor("the popover leaves with its anchor") { !p.menus.isOpen(chooser) && controller.presentedViewController == nil }
        XCTAssertEqual(pressed, [])
    }

    /// A modal alert `confirm-lock` (20) with its action (22) and cancel
    /// (23), opened by the `show-modal` button 40; a chooser popover
    /// `choose` (30) with one action (32), opened by the `popovertarget`
    /// button 41.
    private func page() throws -> (Presenter, UIViewController) {
        let scene = try XCTUnwrap(hostedWindowScene())
        let p = Presenter(), window = UIWindow(windowScene: scene), controller = UIViewController()
        window.rootViewController = controller
        window.makeKeyAndVisible()
        controller.view.addSubview(p.viewport)
        p.viewport.frame = controller.view.bounds
        p.buttonFace = { id in
            var face = ButtonFace()
            face.title = id == 22 ? "Lock" : id == 23 ? "Cancel" : "Pick"
            return face
        }
        p.onPress = { [weak self] in self?.pressed.append($0) }
        self.window = window
        presenter = p
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "view", "props": ["id": "confirm-lock", "semanticTag": "dialog", "accessibilityRole": "alertdialog", "accessibilityLabel": "Lock your vehicle?"]],
            ["op": "create", "id": 21, "kind": "text", "props": ["text": "The doors lock now."]],
            ["op": "create", "id": 22, "kind": "control", "handlers": ["press"], "props": ["type": "button", "commandfor": "confirm-lock", "command": "close"]],
            ["op": "create", "id": 23, "kind": "control", "handlers": [], "props": ["type": "button", "commandfor": "confirm-lock", "command": "close"]],
            ["op": "children", "id": 20, "ids": [21, 22, 23]],
            ["op": "create", "id": 30, "kind": "view", "props": ["id": "choose", "popover": "auto", "accessibilityRole": "alertdialog"], "handlers": [], "style": ["text_color": [0, 0, 0, 255]]],
            ["op": "frame", "id": 30, "x": 0.0, "y": 0.0, "w": 300.0, "h": 120.0],
            ["op": "create", "id": 31, "kind": "text", "props": ["text": "Which app?"]],
            ["op": "create", "id": 32, "kind": "control", "handlers": ["press"], "props": ["type": "button", "popovertarget": "choose", "popovertargetaction": "hide"]],
            ["op": "frame", "id": 32, "x": 0.0, "y": 40.0, "w": 80.0, "h": 40.0],
            ["op": "children", "id": 30, "ids": [31, 32]],
            ["op": "create", "id": 40, "kind": "button", "handlers": [], "props": ["commandfor": "confirm-lock", "command": "show-modal"], "style": ["text_color": [0, 0, 0, 255]]],
            ["op": "frame", "id": 40, "x": 0.0, "y": 200.0, "w": 100.0, "h": 40.0],
            ["op": "create", "id": 41, "kind": "button", "handlers": [], "props": ["popovertarget": "choose"], "style": ["text_color": [0, 0, 0, 255]]],
            ["op": "frame", "id": 41, "x": 0.0, "y": 260.0, "w": 100.0, "h": 40.0],
            ["op": "roots", "ids": [20, 30, 40, 41]],
        ]))
        return (p, controller)
    }

    private func waitForTitle(_ title: String, _ p: Presenter, _ controller: UIViewController) {
        waitFor("the alert titled \(title) presents") {
            (controller.presentedViewController as? UIAlertController)?.title == title && !p.menus.inTransition
        }
    }
}
#endif

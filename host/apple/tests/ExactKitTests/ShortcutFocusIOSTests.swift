#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class ShortcutFocusIOSTests: XCTestCase {
    private final class Host: UIViewController {
        var acceptsFocus = false
        override var canBecomeFirstResponder: Bool { acceptsFocus }
    }
    private var session: ExactSession?
    private var window: UIWindow?
    private var host: Host?
    override func tearDown() {
        session?.destroy(); session = nil
        window?.isHidden = true; window = nil; host = nil
        super.tearDown()
    }
    private func turn() {
        let done = expectation(description: "main queue turn")
        DispatchQueue.main.async { done.fulfill() }
        wait(for: [done], timeout: 5)
    }
    private func fixture() throws -> (ExactSession, ExactView) {
        let s = ExactApp.shared.makeSession(label: "shortcut-focus"), view = ExactView(session: s), host = Host()
        session = s; self.host = host
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let w = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        w.frame = w.screen.bounds; w.rootViewController = host; w.makeKeyAndVisible(); host.view.addSubview(view)
        window = w
        XCTAssertNil(s.boot(plan: try fixturePlan("FIELDLESS"), size: w.bounds.size).error)
        view.frame = host.view.bounds; view.layoutIfNeeded(); turn()
        s.presenter.apply(wireBatch([
            ["op": "create", "id": 70000, "kind": "view"],
            ["op": "create", "id": 70001, "kind": "button", "handlers": ["press"], "style": ["appearance": "none"]],
            ["op": "create", "id": 70002, "kind": "input", "props": ["id": "editor"]],
            ["op": "children", "id": 70000, "ids": [70001, 70002]], ["op": "roots", "ids": [70000]],
            ["op": "frame", "id": 70000, "x": 0, "y": 0, "w": 300, "h": 300],
            ["op": "frame", "id": 70001, "x": 20, "y": 20, "w": 80, "h": 44],
            ["op": "frame", "id": 70002, "x": 20, "y": 100, "w": 200, "h": 44],
        ]))
        return (s, view)
    }
    private func shortcut(_ s: ExactSession, _ on: Bool) {
        s.presenter.apply(wireBatch([["op": "props", "id": 70001,
                                      "set": on ? ["accessibilityKeyShortcuts": "Meta+k"] : [:],
                                      "clear": on ? [] : ["accessibilityKeyShortcuts"]]]))
    }
    private func clearResponder() { _ = FirstResponder.current?.resignFirstResponder(); turn() }

    func testNoKeyboardDeclarationsLeaveTheSessionViewUnfocused() throws {
        let (s, view) = try fixture()
        XCTAssertFalse(view.canBecomeFirstResponder)
        XCTAssertFalse(view.isFirstResponder)
        clearResponder()
        shortcut(s, true)
        XCTAssertTrue(view.canBecomeFirstResponder)
        XCTAssertTrue(view.isFirstResponder)
        shortcut(s, false)
        XCTAssertFalse(view.canBecomeFirstResponder)
        XCTAssertFalse(view.isFirstResponder)
    }

    func testAShortcutIntroducedDuringAMenuWithNoSavedResponderWaitsForItsEnd() throws {
        let (s, view) = try fixture()
        clearResponder()
        s.presenter.menus.focus.setAside()
        shortcut(s, true)
        XCTAssertFalse(view.isFirstResponder, "the menu has no saved responder but still suspends focus")
        s.presenter.menus.focus.restore()
        XCTAssertTrue(view.isFirstResponder)
        s.presenter.menus.focus.setAside()
        shortcut(s, false)
        s.presenter.menus.focus.restore()
        XCTAssertFalse(view.isFirstResponder, "the last shortcut went while the menu was open")
        s.presenter.menus.focus.setAside(untilTouch: true)
        shortcut(s, true)
        XCTAssertFalse(view.isFirstResponder)
        s.presenter.menus.focus.touched()
        XCTAssertTrue(view.isFirstResponder, "a bar menu with no saved responder ends at the next page touch")
    }

    func testAFieldAndAViewControllerKeepTheirFocusAndBlurCanReturnIt() throws {
        let (s, view) = try fixture()
        let field = try XCTUnwrap(s.presenter.views[70002]?.field)
        XCTAssertTrue(field.becomeFirstResponder())
        shortcut(s, true)
        XCTAssertTrue(field.isFirstResponder)
        XCTAssertFalse(view.isFirstResponder)
        XCTAssertTrue(field.resignFirstResponder())
        turn()
        XCTAssertTrue(view.isFirstResponder, "an authored blur gives idle shortcuts their transport")
        let host = try XCTUnwrap(host)
        host.acceptsFocus = true
        XCTAssertTrue(host.becomeFirstResponder())
        shortcut(s, false); shortcut(s, true)
        XCTAssertTrue(host.isFirstResponder, "a controller is a responder too")
        XCTAssertFalse(view.isFirstResponder)
    }

    func testRootKeyListenersStillHaveTheirKeyboardTransport() throws {
        let (s, view) = try fixture()
        clearResponder()
        s.presenter.keyRoot?.handlers = ["keycapture", "keyup", "copy"]
        s.presenter.apply(wireBatch([]))
        XCTAssertTrue(view.canBecomeFirstResponder)
        XCTAssertTrue(view.isFirstResponder)
    }
}
#endif

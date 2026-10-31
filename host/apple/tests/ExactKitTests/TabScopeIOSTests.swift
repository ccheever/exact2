#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// Tab on a hardware keyboard stays in a presented sheet, as UIKit's focus
/// system keeps it, and scrolls the control it reaches into view, as a
/// browser does.
final class TabScopeIOSTests: XCTestCase {
    private var window: UIWindow!

    override func tearDown() {
        window?.rootViewController?.dismiss(animated: false)
        window?.isHidden = true
        window = nil
    }

    private func button(_ id: UInt32, _ p: Presenter, frame: CGRect) -> NodeView {
        let node = NodeView(id: id, kind: "button", presenter: p)
        node.handlers = ["press"]
        node.frame = frame
        p.views[id] = node
        return node
    }

    func testTabStaysInAPresentedSheet() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "a presentation completes only in a scene's window")
        }
        let p = Presenter()
        defer { withExtendedLifetime(p) {} }
        window = UIWindow(windowScene: scene)
        window.frame = scene.coordinateSpace.bounds
        let root = UIViewController()
        window.rootViewController = root
        p.viewport.frame = window.bounds
        root.view.addSubview(p.viewport)
        let page = button(1, p, frame: CGRect(x: 0, y: 0, width: 200, height: 44))
        p.root.addSubview(page)
        window.makeKeyAndVisible()
        let sheet = UIViewController()
        let inSheet = button(2, p, frame: CGRect(x: 0, y: 0, width: 200, height: 44))
        sheet.view.addSubview(inSheet)
        let shown = expectation(description: "presented")
        root.present(sheet, animated: false) { shown.fulfill() }
        wait(for: [shown], timeout: 5)
        XCTAssertTrue(p.moveFocus(backward: false))
        XCTAssertTrue(inSheet.isFirstResponder, "Tab starts in the sheet")
        p.moveFocus(backward: false)
        XCTAssertTrue(inSheet.isFirstResponder, "and wraps there, never to the page under it")
        XCTAssertFalse(page.isFirstResponder)
    }

    func testTabScrollsTheControlItReachesIntoView() throws {
        let p = Presenter()
        defer { withExtendedLifetime(p) {} }
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "button"],
            ["op": "create", "id": 4, "kind": "button"],
            ["op": "children", "id": 2, "ids": [3, 4]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "content", "id": 1, "w": 400.0, "h": 2000.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 2000.0],
            ["op": "frame", "id": 3, "x": 16.0, "y": 10.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 4, "x": 16.0, "y": 1500.0, "w": 200.0, "h": 44.0],
        ]))
        window.makeKeyAndVisible()
        let first = try XCTUnwrap(p.views[3]), far = try XCTUnwrap(p.views[4])
        first.handlers = ["press"]; far.handlers = ["press"]
        p.moveFocus(backward: false)
        XCTAssertTrue(first.isFirstResponder)
        p.moveFocus(backward: false)
        XCTAssertTrue(far.isFirstResponder)
        let shown = far.convert(far.bounds, to: window)
        XCTAssertTrue(window.bounds.contains(shown), "the control Tab reached is in view: \(shown)")
        p.moveFocus(backward: false)
        XCTAssertTrue(first.isFirstResponder, "Tab wraps to the top")
        XCTAssertTrue(window.bounds.contains(first.convert(first.bounds, to: window)), "and scrolls back to it")
    }
}
#endif

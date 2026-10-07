#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A menu-shaped popover's UIMenu survives batches that do not change it:
/// a page that re-renders every second (a poll) must not reload a menu the
/// person has open. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class MenuStabilityIOSTests: XCTestCase {
    private var window: UIWindow!

    private func presenter() -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        // A screen (1): the invoker (2), its menu (3) with one row (4).
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "props": ["popovertarget": "more", "testId": "more-button"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "more", "popover": "auto", "role": "menu"]],
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"], "props": ["role": "menuitem", "popovertarget": "more", "popovertargetaction": "hide"]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 340.0, "y": 0.0, "w": 44.0, "h": 44.0],
            ["op": "frame", "id": 3, "x": 200.0, "y": 48.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
        ]))
        return p
    }

    private func menu(_ p: Presenter) throws -> UIMenu {
        let invoker = try XCTUnwrap(p.views[2])
        let button = try XCTUnwrap(invoker.subviews.compactMap { $0 as? UIButton }.first { $0.showsMenuAsPrimaryAction })
        return try XCTUnwrap(button.menu)
    }

    func testABatchThatChangesNothingKeepsTheMenu() throws {
        let p = presenter()
        let before = try menu(p)
        // The page re-renders: an unrelated node changes.
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 399.0]]))
        XCTAssertTrue(try menu(p) === before, "an open menu would reload")
    }
}
#endif

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
            ["op": "create", "id": 2, "kind": "button", "props": ["popovertarget": "more", "testId": "more-button"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "more", "popover": "auto", "role": "menu"]],
            ["op": "create", "id": 4, "kind": "button", "handlers": ["press"], "props": ["role": "menuitem", "popovertarget": "more", "popovertargetaction": "hide"]],
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

    func testCommandsCloseANativePullDownBeforeItsEndCallback() throws {
        let p = presenter()
        defer { p.menus.reset() }
        let pop = try XCTUnwrap(p.views[3])
        let button = try XCTUnwrap(p.views[2]?.subviews.compactMap { $0 as? MenuButton }.first)
        let interaction = try XCTUnwrap(button.contextMenuInteraction)
        let configuration = UIContextMenuConfiguration(identifier: nil, previewProvider: nil) { _ in button.menu }
        for command in ["hidePopover", "togglePopover"] {
            button.contextMenuInteraction(interaction, willDisplayMenuFor: configuration, animator: nil)
            XCTAssertTrue(p.menus.isOpen(pop))
            p.menus.action(command, ["more"])
            XCTAssertFalse(p.menus.isOpen(pop), command)
            XCTAssertTrue(pop.isHidden, "toggle must not open a painted duplicate")
            button.contextMenuInteraction(interaction, willEndFor: configuration, animator: nil)
        }
    }

    /// Only a button invokes a popover (HTML): a view naming one gets no menu.
    func testAViewThatNamesAPopoverIsNoInvoker() throws {
        let p = presenter()
        defer { p.menus.reset() }
        p.apply(wireBatch([["op": "create", "id": 5, "kind": "view", "props": ["popovertarget": "more"]],
                           ["op": "children", "id": 1, "ids": [2, 3, 5]],
                           ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 44.0, "h": 44.0]]))
        XCTAssertNotNil(p.views[2]?.subviews.compactMap { $0 as? MenuButton }.first, "the button has its menu")
        XCTAssertNil(p.views[5]?.subviews.compactMap { $0 as? UIButton }.first, "the view has none")
    }

    func testABatchThatChangesNothingKeepsTheMenu() throws {
        let p = presenter()
        let before = try menu(p)
        // The page re-renders: an unrelated node changes.
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 399.0]]))
        XCTAssertTrue(try menu(p) === before, "an open menu would reload")
    }

    /// A bar item's button is hidden in the page while the bar shows it, and
    /// that alone does not make it ineligible; hidden any other way, as by
    /// its author's `display: none`, it is.
    func testABarButtonHiddenByItsAuthorIsNotEligible() throws {
        let p = presenter()
        let invoker = try XCTUnwrap(p.views[2])
        XCTAssertTrue(p.menus.eligible(invoker, inBar: true))
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["display": "none"]]]))
        XCTAssertTrue(invoker.isHidden)
        XCTAssertFalse(p.menus.eligible(invoker, inBar: true))
    }

    func testARIAUnavailableMenuInvokerAndActionKeepInputEnabled() throws {
        let p = presenter()
        p.apply(wireBatch([
            ["op": "props", "id": 2, "set": ["accessibilityDisabled": "true"]],
            ["op": "props", "id": 4, "set": ["accessibilityDisabled": "true"]],
        ]))
        let invoker = try XCTUnwrap(p.views[2])
        let button = try XCTUnwrap(invoker.subviews.compactMap { $0 as? UIButton }.first { $0.showsMenuAsPrimaryAction })
        XCTAssertTrue(button.isEnabled)
        XCTAssertTrue(button.accessibilityTraits.contains(.notEnabled))
        let action = try XCTUnwrap(p.menus.items(of: try XCTUnwrap(p.views[3])).first as? UIAction)
        XCTAssertFalse(action.attributes.contains(.disabled))
    }
}
#endif

#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// exact2's menu rule on UIKit (LLP 1021; theme2 2026-10-21 19:47, 22:05):
/// a popover a row of a menu opens, no menu itself, is no submenu: the row is
/// an action that presses it and then opens the popover as if by the menu's
/// own invoker (the node the context menu opened on), so the menu closing
/// as its row is chosen doesn't take it along. A row opening a menu is still
/// its submenu. UIKit, so a simulator runs it.
final class MenuRowPopoverIOSTests: XCTestCase {
    private var window: UIWindow!

    /// A message (1) whose context menu is `message-menu` (2): React (3,
    /// opening `picker`, 5, a popover of an emoji button, 6) and More (4,
    /// opening `more-menu`, 7, a menu of one row, 8).
    private func presenter() -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 600))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        func button(_ id: Int, _ label: String, _ props: [String: String] = [:]) -> [[String: Any]] {
            [["op": "create", "id": 100 + id, "kind": "text", "props": ["text": label]],
             ["op": "frame", "id": 100 + id, "x": 0.0, "y": 0.0, "w": 180.0, "h": 20.0],
             ["op": "create", "id": id, "kind": "button", "handlers": ["press"], "props": props],
             ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0], ["op": "children", "id": id, "ids": [100 + id]]]
        }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "button", "handlers": ["press"], "props": ["contextPopover": "message-menu", "id": "message"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["popover": "auto", "id": "message-menu", "accessibilityRole": "menu"]],
            ["op": "create", "id": 5, "kind": "view", "props": ["popover": "auto", "id": "picker"]],
            ["op": "create", "id": 7, "kind": "view", "props": ["popover": "auto", "id": "more-menu", "accessibilityRole": "menu"]],
            ["op": "create", "id": 9, "kind": "view"],
        ]
        ops += button(3, "Add Reaction", ["popovertarget": "picker"]) + button(4, "More", ["popovertarget": "more-menu"])
        ops += button(6, "👍") + button(8, "Deep")
        ops += [["op": "children", "id": 2, "ids": [3, 4]], ["op": "children", "id": 5, "ids": [6]], ["op": "children", "id": 7, "ids": [8]],
                ["op": "children", "id": 9, "ids": [1, 2, 5, 7]], ["op": "roots", "ids": [9]],
                ["op": "frame", "id": 9, "x": 0.0, "y": 0.0, "w": 400.0, "h": 600.0],
                ["op": "frame", "id": 1, "x": 20.0, "y": 300.0, "w": 200.0, "h": 40.0],
                ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 220.0, "h": 120.0],
                ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 220.0, "h": 60.0],
                ["op": "frame", "id": 7, "x": 0.0, "y": 0.0, "w": 220.0, "h": 60.0]]
        p.apply(wireBatch(ops))
        return p
    }

    func testAMenuRowsPopoverOpensFromTheMenusInvokerAndAMenuIsStillASubmenu() throws {
        let p = presenter()
        let items = p.menus.items(of: try XCTUnwrap(p.views[2]), opener: p.views[1])
        let titles = items.flatMap { ($0 as? UIMenu)?.options.contains(.displayInline) == true ? ($0 as! UIMenu).children : [$0] }
        let react = try XCTUnwrap(titles.first { $0.title == "Add Reaction" } as? UIAction, "the picker is no submenu: an action")
        XCTAssertNotNil(titles.first { $0.title == "More" } as? UIMenu, "a menu is still its submenu")
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        UIButton().sendAction(react)
        let picker = try XCTUnwrap(p.views[5])
        let end = Date(timeIntervalSinceNow: 5) // the picker opens a turn after the press
        while !p.menus.isOpen(picker) && Date() < end { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02)) }
        XCTAssertEqual(pressed, [3], "the row's own press")
        XCTAssertTrue(p.menus.isOpen(picker), "then the picker opens")
        // From the message the menu opened on: beside it, not where the menu's row was.
        let at = picker.convert(picker.bounds, to: window), message = try XCTUnwrap(p.views[1]).convert(p.views[1]!.bounds, to: window)
        XCTAssertLessThan(min(abs(at.minY - message.maxY), abs(at.maxY - message.minY)), 40, "anchored to the message: \(at) by \(message)")
        // Chosen again, the row toggles it closed, as its invoker would.
        UIButton().sendAction(react)
        let closing = Date(timeIntervalSinceNow: 5)
        while p.menus.isOpen(picker) && Date() < closing { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02)) }
        XCTAssertFalse(p.menus.isOpen(picker), "toggled closed")
    }
}
#endif

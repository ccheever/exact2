#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1021 §5, "Submenus" (#141) on UIKit: a menu row whose
/// `popovertarget` names another menu is a nested `UIMenu`, its check marks
/// kept, and choosing a nested item presses its row. UIKit, so a simulator
/// runs it: bun host/apple/build.mjs --test --ios
final class SubmenuIOSTests: XCTestCase {
    private var window: UIWindow!

    /// A row (1) whose context menu is `row-menu` (2): Pinned (4, checked),
    /// Copy (7, opening `copy-menu`), an `hr` (6), Delete (5). `copy-menu`
    /// (8): Copy path (10), Copy link (11), an `hr` (12), Absolute paths
    /// (13, checked).
    private func presenter(copyProps: [String: String] = [:]) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 600))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        func row(_ id: Int, _ label: String, _ props: [String: String] = [:], press: Bool = true) -> [[String: Any]] {
            let hide = props["popovertarget"] == nil ? ["popovertarget": "row-menu", "popovertargetaction": "hide"] : [:]
            return [["op": "create", "id": 100 + id, "kind": "text", "props": ["text": label]],
                    ["op": "frame", "id": 100 + id, "x": 0.0, "y": 0.0, "w": 180.0, "h": 20.0],
                    ["op": "create", "id": id, "kind": "button", "handlers": press ? ["press"] : [], "props": hide.merging(props) { $1 }],
                    ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0], ["op": "children", "id": id, "ids": [100 + id]]]
        }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "button", "handlers": ["press"], "props": ["contextPopover": "row-menu"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["popover": "auto", "id": "row-menu", "accessibilityRole": "menu"]],
            ["op": "create", "id": 8, "kind": "view", "props": ["popover": "auto", "id": "copy-menu", "accessibilityRole": "menu"]],
            ["op": "create", "id": 6, "kind": "view", "props": ["semanticTag": "hr"]],
            ["op": "create", "id": 12, "kind": "view", "props": ["semanticTag": "hr"]],
            ["op": "create", "id": 9, "kind": "view"],
        ]
        ops += row(4, "Pinned", ["accessibilityChecked": "true"])
        ops += row(7, "Copy", ["popovertarget": "copy-menu"].merging(copyProps) { $1 }, press: false)
        ops += row(5, "Delete") + row(10, "Copy path") + row(11, "Copy link") + row(13, "Absolute paths", ["accessibilityChecked": "true"])
        ops += [["op": "children", "id": 2, "ids": [4, 7, 6, 5]], ["op": "children", "id": 8, "ids": [10, 11, 12, 13]],
                ["op": "children", "id": 9, "ids": [1, 2, 8]], ["op": "roots", "ids": [9]],
                ["op": "frame", "id": 9, "x": 0.0, "y": 0.0, "w": 400.0, "h": 600.0],
                ["op": "frame", "id": 1, "x": 20.0, "y": 20.0, "w": 200.0, "h": 40.0],
                ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 220.0, "h": 200.0],
                ["op": "frame", "id": 8, "x": 0.0, "y": 0.0, "w": 220.0, "h": 200.0]]
        p.apply(wireBatch(ops))
        return p
    }
    /// The menu as titles: `✓` for a check mark, `▸ [...]` for a submenu,
    /// `|` between inline sections.
    private func shape(_ elements: [UIMenuElement]) -> [String] {
        elements.map { element in
            if let action = element as? UIAction { return action.title + (action.state == .on ? " ✓" : "") }
            guard let menu = element as? UIMenu else { return "?" }
            if menu.options.contains(.displayInline) { return shape(menu.children).joined(separator: ", ") }
            return menu.title + " ▸ [" + shape(menu.children).joined(separator: " | ") + "]"
        }
    }

    func testARowThatOpensAMenuIsANestedUIMenu() throws {
        let p = presenter()
        let items = p.menus.items(of: try XCTUnwrap(p.views[2]))
        XCTAssertEqual(shape(items), ["Pinned ✓, Copy ▸ [Copy path, Copy link | Absolute paths ✓]", "Delete"])
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let copy = try XCTUnwrap((items.first as? UIMenu)?.children.compactMap { $0 as? UIMenu }.first)
        let path = try XCTUnwrap((copy.children.first as? UIMenu)?.children.first as? UIAction)
        XCTAssertEqual(path.title, "Copy path")
        UIButton().sendAction(path)
        XCTAssertEqual(pressed, [10], "a nested item presses its own row")
    }

    /// A row naming its own menu (a cycle) is no submenu; a disabled opener
    /// is a dimmed action, as UIMenu has no disabled state.
    func testACycleIsNoSubmenuAndADisabledOpenerIsDimmed() throws {
        let cycle = presenter(copyProps: ["popovertarget": "row-menu"])
        XCTAssertEqual(shape(cycle.menus.items(of: try XCTUnwrap(cycle.views[2]))), ["Pinned ✓", "Delete"],
                       "a row without press that opens no submenu separates sections, as before")
        let disabled = presenter(copyProps: ["disabled": "true"])
        let first = try XCTUnwrap(disabled.menus.items(of: try XCTUnwrap(disabled.views[2])).first as? UIMenu)
        let copy = try XCTUnwrap(first.children.last as? UIAction)
        XCTAssertEqual(copy.title, "Copy")
        XCTAssertTrue(copy.attributes.contains(.disabled))
    }
}
#endif

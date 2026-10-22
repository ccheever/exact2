#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// exact2's menu rule on the Mac (LLP 1021; theme2 2026-10-21 19:47, 22:05):
/// a popover a row of an open menu opens, no menu itself, opens as if by the
/// menu's own invoker, so it isn't nested in the menu and the menu closes
/// as an auto popover not its ancestor; only a menu is a row's submenu.
/// Painted, as the driver shows popovers, so no NSMenu tracks here.
final class MenuRowPopoverMacTests: XCTestCase {
    private var windows: [NSWindow] = []

    /// An invoker (2) of `menu` (3: React, 10, opening `picker`, 4, a
    /// popover of a heading, 7, and a button, 6; More, 11, opening `more`, 5,
    /// a menu of one row, 8), and Grid, 31, opening `grid`, 30, a menu's
    /// shape (one button row) with no role. The picker is no menu's shape, so
    /// it is painted, not the Mac's menu of its rows (D3).
    private func fixture() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.menus.painted = true
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        windows.append(window)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "props": ["popovertarget": "menu"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["popover": "auto", "id": "menu", "accessibilityRole": "menu"]],
            ["op": "create", "id": 10, "kind": "button", "props": ["popovertarget": "picker"]],
            ["op": "create", "id": 11, "kind": "button", "props": ["popovertarget": "more"]],
            ["op": "create", "id": 4, "kind": "view", "props": ["popover": "auto", "id": "picker"]],
            ["op": "create", "id": 5, "kind": "view", "props": ["popover": "auto", "id": "more", "accessibilityRole": "menu"]],
            ["op": "create", "id": 6, "kind": "button", "props": [String: String]()], ["op": "create", "id": 16, "kind": "text", "props": ["text": "Thumbs up"]],
            ["op": "create", "id": 8, "kind": "button", "props": ["popovertarget": "picker"]], ["op": "create", "id": 18, "kind": "text", "props": ["text": "Deep"]],
            ["op": "create", "id": 20, "kind": "text", "props": ["text": "Add Reaction"]], ["op": "create", "id": 21, "kind": "text", "props": ["text": "More"]],
            ["op": "children", "id": 10, "ids": [20]], ["op": "children", "id": 11, "ids": [21]],
            ["op": "children", "id": 6, "ids": [16]], ["op": "children", "id": 8, "ids": [18]],
            ["op": "create", "id": 7, "kind": "text", "props": ["text": "Pick a reaction"]],
            ["op": "children", "id": 4, "ids": [7, 6]], ["op": "children", "id": 5, "ids": [8]],
            ["op": "create", "id": 30, "kind": "view", "props": ["popover": "auto", "id": "grid"]],
            ["op": "create", "id": 31, "kind": "button", "props": ["popovertarget": "grid"]], ["op": "create", "id": 32, "kind": "button", "props": [String: String]()],
            ["op": "create", "id": 33, "kind": "text", "props": ["text": "Grid"]], ["op": "create", "id": 34, "kind": "text", "props": ["text": "Cell"]],
            ["op": "children", "id": 31, "ids": [33]], ["op": "children", "id": 32, "ids": [34]], ["op": "children", "id": 30, "ids": [32]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5, 30]], ["op": "children", "id": 3, "ids": [10, 11, 31]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 500, "h": 400],
            ["op": "frame", "id": 2, "x": 400, "y": 340, "w": 80, "h": 30],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 200, "h": 80],
            ["op": "frame", "id": 10, "x": 0, "y": 0, "w": 200, "h": 30],
            ["op": "frame", "id": 11, "x": 0, "y": 40, "w": 200, "h": 30],
            ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 200, "h": 60],
            ["op": "frame", "id": 5, "x": 0, "y": 0, "w": 200, "h": 60],
        ]))
        return p
    }

    func testAMenuRowsPopoverOpensFromTheMenusInvokerAndOnlyAMenuIsASubmenu() throws {
        let p = fixture()
        defer { p.menus.reset() }
        let invoker = try XCTUnwrap(p.views[2]), menu = try XCTUnwrap(p.views[3])
        let picker = try XCTUnwrap(p.views[4]), more = try XCTUnwrap(p.views[5])
        p.menus.show(menu, from: invoker)
        XCTAssertTrue(p.menus.isOpen(menu))
        p.menus.show(picker, from: p.views[10])
        XCTAssertTrue(p.menus.isOpen(picker), "the row's popover opens")
        XCTAssertFalse(p.menus.isOpen(menu), "and the menu, not its ancestor, closes")
        XCTAssertEqual(p.menus.observation?["source"] as? Int, 2, "from the menu's invoker")
        // In its menu's native presentation a row opening a menu is its
        // submenu; one opening any other popover, menu-shaped or not, is an
        // item, which opens it as above.
        XCTAssertTrue(p.menus.submenu(of: try XCTUnwrap(p.views[11]), path: [menu]) === more, "a menu: a submenu")
        XCTAssertNil(p.menus.submenu(of: try XCTUnwrap(p.views[31]), path: [menu]), "a menu's shape that is no menu: an item")
    }

    /// A row chosen in the native menu presses once the menu has closed:
    /// its popover still opens from the menu's invoker.
    func testARowChosenInTheNativeMenuOpensItsPopoverFromTheMenusInvoker() throws {
        let p = fixture()
        defer { p.menus.reset() }
        let picker = try XCTUnwrap(p.views[4])
        let menu = p.menus.menu(of: try XCTUnwrap(p.views[3]), from: p.views[2])
        let react = try XCTUnwrap(menu.items.first { $0.title == "Add Reaction" }, "\(menu.items.map(\.title))")
        XCTAssertNil(react.submenu, "a popover that is no menu: an item")
        NSApp.sendAction(try XCTUnwrap(react.action), to: react.target, from: react)
        let end = Date(timeIntervalSinceNow: 5)
        while !p.menus.isOpen(picker) && Date() < end { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05)) }
        XCTAssertTrue(p.menus.isOpen(picker), "the chosen row's popover opens")
        XCTAssertEqual(p.menus.observation?["source"] as? Int, 2, "from the menu's invoker")
    }

    /// A row of a submenu written beside its menu opens its popover from the
    /// menu's invoker too.
    func testARowOfASubmenuOpensItsPopoverFromTheMenusInvoker() throws {
        let p = fixture()
        defer { p.menus.reset() }
        let picker = try XCTUnwrap(p.views[4])
        let menu = p.menus.menu(of: try XCTUnwrap(p.views[3]), from: p.views[2])
        let more = try XCTUnwrap(menu.items.first { $0.title == "More" }?.submenu, "\(menu.items.map(\.title))")
        let deep = try XCTUnwrap(more.items.first { $0.title == "Deep" })
        NSApp.sendAction(try XCTUnwrap(deep.action), to: deep.target, from: deep)
        let end = Date(timeIntervalSinceNow: 5)
        while !p.menus.isOpen(picker) && Date() < end { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05)) }
        XCTAssertTrue(p.menus.isOpen(picker))
        XCTAssertEqual(p.menus.observation?["source"] as? Int, 2)
    }

    /// As the context menu of a node (2, right-clicked): a row of a submenu
    /// written beside the menu opens its popover from that node.
    func testASubmenusRowInAContextMenuOpensItsPopoverFromTheNode() throws {
        let p = fixture()
        defer { p.menus.reset() }
        let picker = try XCTUnwrap(p.views[4])
        let menu = p.menus.contextMenu(of: try XCTUnwrap(p.views[3]), on: try XCTUnwrap(p.views[2]))
        let more = try XCTUnwrap(menu.items.first { $0.title == "More" }?.submenu, "\(menu.items.map(\.title))")
        let deep = try XCTUnwrap(more.items.first { $0.title == "Deep" })
        NSApp.sendAction(try XCTUnwrap(deep.action), to: deep.target, from: deep)
        let end = Date(timeIntervalSinceNow: 5)
        while !p.menus.isOpen(picker) && Date() < end { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05)) }
        XCTAssertTrue(p.menus.isOpen(picker), "the submenu row's popover opens")
        XCTAssertEqual(p.menus.observation?["source"] as? Int, 2, "from the right-clicked node")
    }
}
#endif

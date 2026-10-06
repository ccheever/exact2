#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1021 §5.1 on AppKit: a context menu is its popover's menu rows as an
/// NSMenu (a Mac's has no preview, so the `contextPreview` row is no item),
/// and a picked item presses its row on the next turn, as a button menu's.
final class ContextMenuMacTests: XCTestCase {
    private var windows: [NSWindow] = []
    override func tearDown() { windows.forEach { $0.close() }; windows.removeAll() }

    /// A source (1) naming popover 2: a preview row (3) and two items (4, 5)
    /// with an `hr` (6) between them.
    private func presenter() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        windows.append(window)
        func text(_ id: Int, _ value: String) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "text", "props": ["text": value]], ["op": "frame", "id": id, "x": 0, "y": 0, "w": 180, "h": 20]]
        }
        func row(_ id: Int, _ label: Int, _ props: [String: String] = [:]) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "button", "handlers": ["press"], "props": ["popovertarget": "m", "popovertargetaction": "hide"].merging(props) { $1 }],
             ["op": "frame", "id": id, "x": 0, "y": 0, "w": 200, "h": 40], ["op": "children", "id": id, "ids": [label]]]
        }
        let ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "button", "handlers": ["press", "contextmenu"], "props": ["contextPopover": "m"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["popover": "auto", "id": "m", "accessibilityRole": "menu"]],
            ["op": "create", "id": 6, "kind": "view", "props": ["semanticTag": "hr"]],
            ["op": "create", "id": 9, "kind": "view"],
        ] + text(13, "Open the chat") + row(3, 13, ["contextPreview": "true"]) + text(14, "Pin") + row(4, 14)
            + text(15, "Delete") + row(5, 15, ["destructive": "true"])
            + [["op": "children", "id": 2, "ids": [3, 4, 6, 5]], ["op": "children", "id": 9, "ids": [1, 2]], ["op": "roots", "ids": [9]],
               ["op": "frame", "id": 9, "x": 0, "y": 0, "w": 500, "h": 400], ["op": "frame", "id": 1, "x": 20, "y": 20, "w": 120, "h": 30],
               ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 220, "h": 200]]
        p.apply(wireBatch(ops))
        return p
    }

    func testTheContextMenuIsItsPopoversItemsWithoutThePreview() throws {
        let p = presenter()
        let pop = try XCTUnwrap(p.views[2])
        XCTAssertTrue(p.menus.isMenuShaped(pop), "a preview row does not keep it from being a menu")
        let menu = p.menus.menu(of: pop)
        XCTAssertEqual(menu.items.map { $0.isSeparatorItem ? "—" : $0.title }, ["Pin", "—", "Delete"])
        var pressed: [UInt32] = []
        let picked = expectation(description: "the picked row is pressed")
        p.onPress = { pressed.append($0); picked.fulfill() }
        NSApp.sendAction(try XCTUnwrap(menu.items[2].action), to: menu.items[2].target, from: menu.items[2])
        XCTAssertEqual(pressed, [], "on the next turn")
        // That turn, however late a loaded machine runs it: a fixed 50 ms
        // spin of the run loop could return before the main queue's turn.
        wait(for: [picked], timeout: 10)
        XCTAssertEqual(pressed, [5])
    }
}
#endif

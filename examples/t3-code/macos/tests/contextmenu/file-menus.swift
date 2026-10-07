import AppKit
import XCTest

// context-menu-gaps: the three menus as context-menus.ts builds them (the items below are what
// its unit tests expect), laid out by T3ContextMenu as ElectronMenu.ts buildTemplate does.
final class FileMenuTests: XCTestCase {
    private func titles(_ menu: NSMenu) -> [String] { menu.items.map { $0.isSeparatorItem ? "—" : $0.title } }

    /// FileBrowserPanel: Open, the server-worded reveal, "Open with ▸" the detected editors,
    /// Copy mention, Add to chat; no separators.
    func testFilesTreeMenuHasOpenWithSubmenuAndPicksTheChildEditor() {
        _ = NSApplication.shared
        let owner = T3ContextMenu()
        let menu = owner.menu(for: [
            ["id": "open", "label": "Open"], ["id": "reveal-in-folder", "label": "Reveal in Finder"],
            ["id": "open-with", "label": "Open with", "children": [["id": "editor:cursor", "label": "Cursor"], ["id": "editor:vscode", "label": "VS Code"]]],
            ["id": "copy-mention", "label": "Copy mention"], ["id": "add-to-chat", "label": "Add to chat"],
        ])
        XCTAssertEqual(titles(menu), ["Open", "Reveal in Finder", "Open with", "Copy mention", "Add to chat"])
        XCTAssertTrue(menu.items.allSatisfy { $0.isEnabled && $0.image == nil })
        let openWith = menu.items[2]
        XCTAssertTrue(openWith.hasSubmenu)
        XCTAssertNil(openWith.representedObject) // the parent is never the pick
        XCTAssertEqual(openWith.submenu.map(titles), ["Cursor", "VS Code"])
        openWith.submenu?.performActionForItem(at: 1)
        XCTAssertEqual(owner.picked, "editor:vscode")
        menu.performActionForItem(at: 3)
        XCTAssertEqual(owner.picked, "copy-mention")
    }

    /// buildTemplate: an item whose children are empty is a plain item, not an empty submenu.
    func testEmptyChildrenAreAPlainItemAndNoSubmenu() {
        let menu = T3ContextMenu().menu(for: [["id": "open-with", "label": "Open with", "children": [[String: Any]]()],
                                               ["id": "copy-mention", "label": "Copy mention"]])
        XCTAssertEqual(titles(menu), ["Open with", "Copy mention"])
        XCTAssertFalse(menu.items[0].hasSubmenu)
    }

    /// pullRequestLinkContextMenu: Copy link first, then the host-named open.
    func testPullRequestLinkMenu() {
        let owner = T3ContextMenu()
        let menu = owner.menu(for: [["id": "copy-link", "label": "Copy link"], ["id": "open-external", "label": "Open on GitHub"]])
        XCTAssertEqual(titles(menu), ["Copy link", "Open on GitHub"])
        menu.performActionForItem(at: 1)
        XCTAssertEqual(owner.picked, "open-external")
    }

    /// ChatMarkdown's file-link menu for a media link: Preview media, the open and reveal items, the copies.
    func testChatMediaLinkMenu() {
        let menu = T3ContextMenu().menu(for: [["id": "preview-media", "label": "Preview media"], ["id": "open", "label": "Open in editor"],
            ["id": "reveal", "label": "Reveal in Finder"], ["id": "copy-relative", "label": "Copy relative path"], ["id": "copy-full", "label": "Copy full path"]])
        XCTAssertEqual(titles(menu), ["Preview media", "Open in editor", "Reveal in Finder", "Copy relative path", "Copy full path"])
    }
}

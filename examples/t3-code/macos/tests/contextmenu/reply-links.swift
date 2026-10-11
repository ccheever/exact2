import AppKit
import XCTest

// reply-links RL-1, RL-2: a reply's mailto, irc, xmpp or fragment link is a link node with its own `contextmenu` (the one
// FlowRuns gives every link), whose handler leaves the click to the shell for a link with no web host, as ChatMarkdown's
// anchor returns before `preventDefault` and Electron's `context-menu` fires (external-link-menu.ts → `shellMenu`). The
// reference's menu there (CDP, its main process's Menu.popup read back, 2026-10-11): Cut, Copy, Paste, Select All, with
// no Copy Link (parseSafeExternalUrl refuses the URL).

/// A link node that wrote its own `contextmenu` (NodeViewMac's `dispatchContextMenu` answers it), exposed as ExactKit
/// exposes a `link` node: an AXLink, here with the URL a host could give it (X78: today it gives none).
private final class LinkNode: NSView {
    private(set) var menus = 0
    var url = "mailto:team@example.test"
    override func rightMouseDown(with event: NSEvent) { menus += 1 }
    override func accessibilityRole() -> NSAccessibility.Role? { .link }
    override func accessibilityURL() -> URL? { URL(string: url) }
}

final class ShellMenuForLinkTests: ShellMenuCase {
    func testALinksHandlerLeavesItsClickToTheShellOnce() {
        let link = LinkNode(frame: NSRect(x: 20, y: 20, width: 80, height: 22))
        page.addSubview(link)
        rightClick(at: at(link, NSPoint(x: 10, y: 10)))
        XCTAssertEqual(link.menus, 1, "the node's own contextmenu runs first")
        XCTAssertTrue(popped.isEmpty, "and the shell waits for it")
        XCTAssertTrue(shell.shellMenuForLastClick())
        XCTAssertEqual(popped.map(\.titles), [["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"]],
                       "the reference's menu on a mailto link: no Copy Link for a URL parseSafeExternalUrl refuses")
        XCTAssertTrue(popped.first?.view === link)
        XCTAssertFalse(shell.shellMenuForLastClick(), "one menu per click")
        XCTAssertEqual(popped.count, 1)
    }

    func testPageTextSelectedEnablesCopyAsOnThePage() {
        let link = LinkNode(frame: NSRect(x: 20, y: 20, width: 80, height: 22))
        link.url = "#notes"
        page.addSubview(link)
        page.selected = true
        rightClick(at: at(link, NSPoint(x: 10, y: 10)))
        XCTAssertTrue(shell.shellMenuForLastClick())
        XCTAssertEqual(popped.map(\.titles), [["Cut (disabled)", "Copy", "Paste (disabled)", "Select All"]])
    }

    func testNoClickOrAStaleOneOpensNothing() {
        XCTAssertFalse(shell.shellMenuForLastClick(), "no page click yet")
        let link = LinkNode(frame: NSRect(x: 20, y: 20, width: 80, height: 22))
        page.addSubview(link)
        rightClick(at: at(link, NSPoint(x: 10, y: 10)))
        XCTAssertFalse(shell.shellMenuForLastClick(now: ProcessInfo.processInfo.systemUptime + 10), "a click from long ago")
        XCTAssertTrue(popped.isEmpty)
    }

    func testAWebLinkWouldStillGetCopyLinkFromTheTemplate() {
        // The handler asks for the shell only without a web host; the template itself still offers Copy Link for a safe URL.
        let link = LinkNode(frame: NSRect(x: 20, y: 20, width: 80, height: 22))
        link.url = "https://example.com/c"
        page.addSubview(link)
        rightClick(at: at(link, NSPoint(x: 10, y: 10)))
        XCTAssertTrue(shell.shellMenuForLastClick())
        XCTAssertEqual(popped.first?.titles.first, "Copy Link")
    }
}

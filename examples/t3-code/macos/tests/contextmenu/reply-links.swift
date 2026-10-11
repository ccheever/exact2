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

/// A link node that holds the focus, as ExactKit's focused `link` (a Tab stop) does.
private final class FocusedLinkNode: NSView {
    override var acceptsFirstResponder: Bool { true }
    override func accessibilityRole() -> NSAccessibility.Role? { .link }
}

final class ShellMenuForLinkTests: ShellMenuCase {
    // reply-links-reference-behavior RL-6: the ContextMenu key on a focused mailto, irc, xmpp or fragment link. ExactKit
    // raises the link node's `contextmenu` at the focus with no click (MouseEventsMac `contextMenuKey`); the reference
    // (CDP's ContextMenu key on the focused anchor, its main process read back, 2026-10-12) gets Chromium's keyboard
    // `context-menu` at the anchor's middle and pops the shell's Cut, Copy, Paste, Select All, no Copy Link.
    func testTheContextMenuKeyOnAFocusedLinkOpensTheShellMenuAtTheLink() {
        var points: [NSPoint] = []
        shell.present = { [unowned self] menu, event, view in popped.append((T3ShellMenu.describe(menu), view)); points.append(event.locationInWindow) }
        let link = FocusedLinkNode(frame: NSRect(x: 40, y: 30, width: 80, height: 22))
        page.addSubview(link)
        XCTAssertTrue(window.makeFirstResponder(link))
        XCTAssertTrue(shell.shellMenuForLastClick(windows: [window]), "no click raised it: the menu for the focused link")
        XCTAssertEqual(popped.map(\.titles), [["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"]])
        XCTAssertTrue(popped.first?.view === link)
        XCTAssertEqual(points.first, link.convert(NSPoint(x: 40, y: 11), to: nil), "at the link's middle")
        page.selected = true
        XCTAssertTrue(shell.shellMenuForLastClick(windows: [window]), "each key its own menu")
        XCTAssertEqual(popped.last?.titles, ["Cut (disabled)", "Copy", "Paste (disabled)", "Select All"], "Copy with page text selected")
    }

    func testAFreshClickStillWinsAndNoFocusedLinkOpensNothing() {
        let link = FocusedLinkNode(frame: NSRect(x: 40, y: 30, width: 80, height: 22))
        page.addSubview(link)
        XCTAssertFalse(shell.shellMenuForLastClick(windows: [window]), "no click and no focused page node")
        window.makeFirstResponder(page)
        XCTAssertFalse(shell.shellMenuForLastClick(windows: [window]), "the page itself is no link")
        let field = NSTextView(frame: NSRect(x: 200, y: 30, width: 120, height: 22))
        page.addSubview(field)
        window.makeFirstResponder(field)
        XCTAssertFalse(shell.shellMenuForLastClick(windows: [window]), "a text input's menu is its right-click's")
        XCTAssertTrue(popped.isEmpty)
        window.makeFirstResponder(link)
        var points: [NSPoint] = []
        shell.present = { [unowned self] menu, event, view in popped.append((T3ShellMenu.describe(menu), view)); points.append(event.locationInWindow) }
        let other = LinkNode(frame: NSRect(x: 300, y: 120, width: 80, height: 22))
        page.addSubview(other)
        rightClick(at: at(other, NSPoint(x: 10, y: 10)))
        XCTAssertTrue(shell.shellMenuForLastClick(windows: [window]))
        XCTAssertTrue(popped.first?.view === other, "a right-click's menu is the click's, wherever the focus is")
        XCTAssertEqual(points.first, other.convert(NSPoint(x: 10, y: 10), to: nil))
    }

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

    func testAMenuThatAnsweredItOrALaterKeyOrClickEndsTheClick() {
        // A `contextmenu` the last page click did not raise (the ContextMenu key on a focused mailto link, after a click the
        // tail or a node's own menu answered) gets no shell menu at that old click's place (reply-links review).
        rightClick(at: NSPoint(x: 400, y: 200))
        XCTAssertEqual(popped.count, 1, "the tail answered a click no node took")
        XCTAssertFalse(shell.shellMenuForLastClick(), "and that click is done")
        let link = LinkNode(frame: NSRect(x: 20, y: 20, width: 80, height: 22))
        page.addSubview(link)
        rightClick(at: at(link, NSPoint(x: 10, y: 10)))
        NSApp.sendEvent(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                         windowNumber: window.windowNumber, context: nil, characters: "a", charactersIgnoringModifiers: "a",
                                         isARepeat: false, keyCode: 0)!)
        XCTAssertFalse(shell.shellMenuForLastClick(), "a key came after the click")
        rightClick(at: at(link, NSPoint(x: 10, y: 10)))
        NSApp.sendEvent(NSEvent.mouseEvent(with: .leftMouseDown, location: NSPoint(x: 400, y: 200), modifierFlags: [],
                                           timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil,
                                           eventNumber: 0, clickCount: 1, pressure: 1)!)
        XCTAssertFalse(shell.shellMenuForLastClick(), "another click came after it")
        // A node's own app menu (T3Module's `contextMenu` op) took the click; a path with no event follows (the agent's
        // keys go to the window, not through NSApplication's monitor; VoiceOver's show menu sends none).
        rightClick(at: at(link, NSPoint(x: 10, y: 10)))
        shell.pageClickAnswered()
        XCTAssertFalse(shell.shellMenuForLastClick(), "the app's own menu answered it")
        XCTAssertEqual(link.menus, 3)
        XCTAssertEqual(popped.count, 1, "only the tail's menu")
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

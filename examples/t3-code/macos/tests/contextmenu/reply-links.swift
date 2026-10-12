import AppKit
import XCTest

// reply-links RL-1, RL-2: a reply's mailto, irc, xmpp or fragment link is a link node with its own `contextmenu` (the one
// FlowRuns gives every link), whose handler leaves the click to the shell for a link with no web host, as ChatMarkdown's
// anchor returns before `preventDefault` and Electron's `context-menu` fires (external-link-menu.ts → `shellMenu`). The
// reference's menu there (CDP, its main process's Menu.popup read back, 2026-10-11): Cut, Copy, Paste, Select All, with
// no Copy Link (parseSafeExternalUrl refuses the URL).

/// A link node that wrote its own `contextmenu` (NodeViewMac's `dispatchContextMenu` answers it), exposed as ExactKit
/// exposes a `link` node: an AXLink whose URL is its `href` (X78, fixed on main by #455).
private final class LinkNode: NSView {
    private(set) var menus = 0
    var url = "mailto:team@example.test"
    override func rightMouseDown(with event: NSEvent) { menus += 1 }
    override func accessibilityRole() -> NSAccessibility.Role? { .link }
    override func accessibilityURL() -> URL? { URL(string: url) }
}

/// A node that holds the focus, as ExactKit's focused `link` (a Tab stop) does; `url` is its `AXURL`, a `link` node's
/// `href` (X78, fixed on main by #455), or none (a fragment link's later-word box, a node with no `href`).
private final class FocusedLinkNode: NSView {
    var url: String?
    override var acceptsFirstResponder: Bool { true }
    override func accessibilityRole() -> NSAccessibility.Role? { .link }
    override func accessibilityURL() -> URL? { url.flatMap { URL(string: $0) } }
    override func keyDown(with event: NSEvent) {} // ExactKit raises the node's `contextmenu` here (MouseEventsMac `contextMenuKey`)
}

final class ShellMenuForLinkTests: ShellMenuCase {
    /// A key through NSApplication, as the window server delivers it: the ContextMenu key (kVK_ContextualMenu) by default.
    func press(_ keyCode: UInt16 = 0x6E, _ characters: String = "\u{F735}", modifiers: NSEvent.ModifierFlags = []) {
        NSApp.sendEvent(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers, timestamp: ProcessInfo.processInfo.systemUptime,
                                         windowNumber: window.windowNumber, context: nil, characters: characters, charactersIgnoringModifiers: characters,
                                         isARepeat: false, keyCode: keyCode)!)
    }

    /// A focused link node on the page.
    fileprivate func focusedLink(_ frame: NSRect = NSRect(x: 40, y: 30, width: 80, height: 22)) -> FocusedLinkNode {
        let link = FocusedLinkNode(frame: frame)
        page.addSubview(link)
        XCTAssertTrue(window.makeFirstResponder(link))
        return link
    }

    // reply-links-reference-behavior RL-6: the ContextMenu key on a focused mailto, irc, xmpp or fragment link. ExactKit
    // raises the link node's `contextmenu` at the focus with no click (MouseEventsMac `contextMenuKey`); the reference
    // (CDP's ContextMenu key on the focused anchor, its main process read back, 2026-10-12) gets Chromium's keyboard
    // `context-menu` at the anchor's middle and pops the shell's Cut, Copy, Paste, Select All, no Copy Link.
    func testTheContextMenuKeyOnAFocusedLinkOpensTheShellMenuAtTheLinkOnce() {
        var points: [NSPoint] = []
        shell.present = { [unowned self] menu, event, view in popped.append((T3ShellMenu.describe(menu), view)); points.append(event.locationInWindow) }
        let link = focusedLink()
        link.url = "mailto:team@example.test"
        press()
        XCTAssertTrue(shell.shellMenuForLastClick(href: "mailto:team@example.test"), "the ContextMenu key raised it: the menu for the focused link")
        XCTAssertEqual(popped.map(\.titles), [["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"]])
        XCTAssertTrue(popped.first?.view === link)
        XCTAssertEqual(points.first, link.convert(NSPoint(x: 40, y: 11), to: nil), "at the link's middle")
        XCTAssertFalse(shell.shellMenuForLastClick(href: "mailto:team@example.test"), "one menu per key")
        page.selected = true
        press()
        XCTAssertTrue(shell.shellMenuForLastClick(href: "mailto:team@example.test"), "each key its own menu")
        XCTAssertEqual(popped.last?.titles, ["Cut (disabled)", "Copy", "Paste (disabled)", "Select All"], "Copy with page text selected")
    }

    func testTheKeysMenuIsAtTheLinksClippedMiddle() {
        // ExactKit clips the focused node by the ancestors that clip (`overflow: hidden`, a clip view), not by every
        // ancestor's bounds (MouseEventsMac `contextMenuKey`), and so does `visibleRect` from macOS 14 (`clipsToBounds`): a
        // link half outside a box that does not clip has its whole middle; one in a clipping box, the shown part's.
        var points: [NSPoint] = []
        shell.present = { menu, event, _ in points.append(event.locationInWindow) }
        let open = NSView(frame: NSRect(x: 20, y: 20, width: 60, height: 40))
        page.addSubview(open)
        let link = FocusedLinkNode(frame: NSRect(x: 20, y: 10, width: 80, height: 20))
        open.addSubview(link)
        XCTAssertTrue(window.makeFirstResponder(link))
        press()
        XCTAssertTrue(shell.shellMenuForLastClick())
        XCTAssertEqual(points.last, link.convert(NSPoint(x: 40, y: 10), to: nil), "a box that does not clip: the link's own middle")
        open.clipsToBounds = true
        press()
        XCTAssertTrue(shell.shellMenuForLastClick())
        XCTAssertEqual(points.last, link.convert(NSPoint(x: 20, y: 10), to: nil), "overflow: hidden: the middle of the shown 40 points")
        link.frame.origin.x = 70
        press()
        XCTAssertFalse(shell.shellMenuForLastClick(), "nothing of it shows: no menu, as ExactKit raises none")
    }

    func testAContextmenuNoFreshClickOrContextMenuKeyRaisedOpensNothingAtTheFocus() {
        _ = focusedLink()
        XCTAssertFalse(shell.shellMenuForLastClick(), "no click and no key, with a link focused")
        press(0, "a")
        XCTAssertFalse(shell.shellMenuForLastClick(), "another key is no ContextMenu key")
        press(modifiers: .shift)
        XCTAssertFalse(shell.shellMenuForLastClick(), "⇧ContextMenu: ExactKit raises no contextmenu for it")
        press()
        press(0, "a")
        XCTAssertFalse(shell.shellMenuForLastClick(), "a key came after the ContextMenu key (a contextmenu still on its way)")
        press()
        NSApp.sendEvent(NSEvent.mouseEvent(with: .leftMouseDown, location: NSPoint(x: 400, y: 200), modifierFlags: [],
                                           timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil,
                                           eventNumber: 0, clickCount: 1, pressure: 1)!)
        XCTAssertFalse(shell.shellMenuForLastClick(), "a click came after it")
        _ = focusedLink()
        press()
        XCTAssertFalse(shell.shellMenuForLastClick(now: ProcessInfo.processInfo.systemUptime + 10), "a key from long ago")
        press()
        shell.pageClickAnswered()
        XCTAssertFalse(shell.shellMenuForLastClick(), "the app's own menu answered it")
        XCTAssertTrue(popped.isEmpty)
    }

    func testTheKeyOpensNothingWhereTheFocusMovedOrIsNoLinkOrNamesAnotherLink() {
        let link = focusedLink()
        press()
        let other = focusedLink(NSRect(x: 200, y: 120, width: 80, height: 22))
        XCTAssertFalse(shell.shellMenuForLastClick(), "the focus moved after the key")
        other.url = "mailto:team@example.test"
        press()
        XCTAssertFalse(shell.shellMenuForLastClick(href: "irc://irc.example.test/t3"), "the focused node names another link")
        press()
        XCTAssertTrue(shell.shellMenuForLastClick(href: "mailto:team@example.test"), "the link the request names")
        XCTAssertTrue(popped.last?.view === other)
        XCTAssertTrue(window.makeFirstResponder(link))
        press()
        XCTAssertTrue(shell.shellMenuForLastClick(href: "#notes"), "a node that exposes no URL (a fragment link's later-word box)")
        XCTAssertTrue(popped.last?.view === link)
        window.makeFirstResponder(nil)
        press()
        XCTAssertFalse(shell.shellMenuForLastClick(), "no focused node: the window has the focus")
        let field = NSTextView(frame: NSRect(x: 300, y: 30, width: 120, height: 22))
        page.addSubview(field)
        window.makeFirstResponder(field)
        press()
        XCTAssertFalse(shell.shellMenuForLastClick(), "a text input's menu is its right-click's")
        XCTAssertEqual(popped.count, 2)
    }

    func testAFreshClickStillWinsOverTheFocus() {
        _ = focusedLink()
        var points: [NSPoint] = []
        shell.present = { [unowned self] menu, event, view in popped.append((T3ShellMenu.describe(menu), view)); points.append(event.locationInWindow) }
        let other = LinkNode(frame: NSRect(x: 300, y: 120, width: 80, height: 22))
        page.addSubview(other)
        rightClick(at: at(other, NSPoint(x: 10, y: 10)))
        XCTAssertTrue(shell.shellMenuForLastClick(href: "mailto:team@example.test"))
        XCTAssertTrue(popped.first?.view === other, "a right-click's menu is the click's, wherever the focus is")
        XCTAssertEqual(points.first, other.convert(NSPoint(x: 10, y: 10), to: nil))
    }

    func testUnderTheAgentItsKeyLeavesNoRecordAndTheFocusOfItsWindowAnswers() {
        // The agent's keys go to its window past NSApplication (AgentMac `win.sendEvent`); its right-click goes through it.
        shell.destroy()
        shell = T3TextContextMenu(agent: true)
        shell.install()
        XCTAssertFalse(shell.shellMenuForLastClick(windows: [window]), "no focused page node")
        let link = focusedLink()
        window.sendEvent(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                          windowNumber: window.windowNumber, context: nil, characters: "\u{F735}", charactersIgnoringModifiers: "\u{F735}",
                                          isARepeat: false, keyCode: 0x6E)!)
        XCTAssertTrue(shell.shellMenuForLastClick(windows: [window]), "the agent's ContextMenu key")
        XCTAssertEqual(shell.lastShown, ["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"], "the log line's items (t3.textmenu:)")
        link.url = "#notes"
        XCTAssertFalse(shell.shellMenuForLastClick(href: "mailto:team@example.test", windows: [window]), "a focused node that names another link")
    }

    func testALinksHandlerLeavesItsClickToTheShellOnce() {
        _ = focusedLink(NSRect(x: 200, y: 120, width: 80, height: 22))
        let link = LinkNode(frame: NSRect(x: 20, y: 20, width: 80, height: 22))
        page.addSubview(link)
        rightClick(at: at(link, NSPoint(x: 10, y: 10)))
        XCTAssertEqual(link.menus, 1, "the node's own contextmenu runs first")
        XCTAssertTrue(popped.isEmpty, "and the shell waits for it")
        XCTAssertTrue(shell.shellMenuForLastClick())
        XCTAssertEqual(popped.map(\.titles), [["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"]],
                       "the reference's menu on a mailto link: no Copy Link for a URL parseSafeExternalUrl refuses")
        XCTAssertTrue(popped.first?.view === link)
        XCTAssertFalse(shell.shellMenuForLastClick(), "one menu per click, with another link focused")
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

    func testNoClickOrAStaleOneOpensNothingEvenWithALinkFocused() {
        _ = focusedLink(NSRect(x: 200, y: 120, width: 80, height: 22))
        XCTAssertFalse(shell.shellMenuForLastClick(), "no page click yet")
        let link = LinkNode(frame: NSRect(x: 20, y: 20, width: 80, height: 22))
        page.addSubview(link)
        rightClick(at: at(link, NSPoint(x: 10, y: 10)))
        XCTAssertFalse(shell.shellMenuForLastClick(now: ProcessInfo.processInfo.systemUptime + 10), "a click from long ago, and no ContextMenu key")
        XCTAssertTrue(popped.isEmpty)
    }

    func testAMenuThatAnsweredItOrALaterKeyOrClickEndsTheClick() {
        // A `contextmenu` the last page click did not raise gets no shell menu at that old click's place (reply-links
        // review), nor at the focused link unless a ContextMenu key raised it (reply-links-reference-behavior review).
        _ = focusedLink(NSRect(x: 200, y: 120, width: 80, height: 22))
        rightClick(at: NSPoint(x: 400, y: 200))
        XCTAssertEqual(popped.count, 1, "the tail answered a click no node took")
        XCTAssertFalse(shell.shellMenuForLastClick(), "and that click is done")
        let link = LinkNode(frame: NSRect(x: 20, y: 20, width: 80, height: 22))
        page.addSubview(link)
        rightClick(at: at(link, NSPoint(x: 10, y: 10)))
        NSApp.sendEvent(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                         windowNumber: window.windowNumber, context: nil, characters: "a", charactersIgnoringModifiers: "a",
                                         isARepeat: false, keyCode: 0)!)
        XCTAssertFalse(shell.shellMenuForLastClick(), "a key (no ContextMenu key) came after the click")
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

import AppKit
import XCTest

// realinput-1010d RD-4: a right-click on selected read-only text shows the desktop shell's menu (DesktopWindow.ts
// installContextMenu: Cut disabled, Copy, Paste disabled, Select All), not ExactKit's NSTextView menu (Look Up, Copy,
// Speech, Services). `HostParagraph` stands in for an ExactKit text node: its `menu(for:)` is the host's read-only text
// menu while it has a selection, its `copy:` and `selectAll:` are the node's, and its `rightMouseDown` counts where
// NodeViewMac's calls `super`, which pops the host's menu.

private final class HostParagraph: NSView {
    var selected = true
    private(set) var copies = 0, selections = 0, hostMenus = 0
    override func rightMouseDown(with event: NSEvent) { hostMenus += 1 }
    override func menu(for event: NSEvent) -> NSMenu? {
        guard selected else { return super.menu(for: event) }
        let menu = NSMenu()
        menu.addItem(withTitle: "Look Up “selected”", action: NSSelectorFromString("lookUpSelection:"), keyEquivalent: "").target = self
        menu.addItem(.separator())
        menu.addItem(withTitle: "Copy", action: #selector(copy(_:)), keyEquivalent: "").target = self
        return menu
    }
    @objc func lookUpSelection(_ sender: Any?) {}
    @objc func copy(_ sender: Any?) { copies += 1 }
    override func selectAll(_ sender: Any?) { selections += 1 }
}

/// A view with a menu of its own that is not the host's text menu (a field's, a web view's).
private final class OtherMenuView: NSView {
    private(set) var ownMenus = 0
    override func menu(for event: NSEvent) -> NSMenu? { let menu = NSMenu(); menu.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: ""); return menu }
    override func rightMouseDown(with event: NSEvent) { ownMenus += 1 }
}

final class TextContextMenuTests: XCTestCase {
    private var window: NSWindow!
    private var paragraph: HostParagraph!
    private var other: OtherMenuView!

    override func setUp() {
        _ = NSApplication.shared
        window = NSWindow(contentRect: NSRect(x: 220, y: 220, width: 400, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        paragraph = HostParagraph(frame: NSRect(x: 20, y: 20, width: 200, height: 40))
        other = OtherMenuView(frame: NSRect(x: 240, y: 20, width: 100, height: 40))
        window.contentView!.addSubview(paragraph)
        window.contentView!.addSubview(other)
        window.orderFrontRegardless()
    }
    override func tearDown() { window.orderOut(nil) }

    private func rightClick(at point: NSPoint) -> NSEvent {
        NSEvent.mouseEvent(with: .rightMouseDown, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                           windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
    }

    func testSelectedReadOnlyTextGetsTheShellsFourEditingRoles() throws {
        let (menu, target) = try XCTUnwrap(T3TextContextMenu.replacement(for: rightClick(at: NSPoint(x: 60, y: 40))))
        XCTAssertTrue(target === paragraph, "the menu acts on the text view the click is on")
        XCTAssertEqual(menu.items.map(\.title), ["Cut", "Copy", "Paste", "Select All"])
        XCTAssertEqual(menu.items.map(\.isEnabled), [false, true, false, true], "read-only text: canCut and canPaste are false")
        XCTAssertEqual(menu.items.map(\.keyEquivalent), ["x", "c", "v", "a"], "Electron shows each role's accelerator")
        XCTAssertTrue(menu.items.allSatisfy { $0.keyEquivalentModifierMask == .command })
        XCTAssertFalse(menu.items.contains { $0.title.hasPrefix("Look Up") || $0.title == "Speech" || $0.title == "Services" })
        XCTAssertEqual(T3TextContextMenu.describe(menu), ["Cut (disabled)", "Copy", "Paste (disabled)", "Select All"])
        menu.performActionForItem(at: 1)
        XCTAssertEqual(paragraph.copies, 1, "Copy is the text node's copy: (its `copy` event, then the selection)")
        menu.performActionForItem(at: 3)
        XCTAssertEqual(paragraph.selections, 1, "Select All is the node's selectAll:")
    }

    func testOtherClicksKeepTheirOwnMenus() {
        paragraph.selected = false
        XCTAssertNil(T3TextContextMenu.replacement(for: rightClick(at: NSPoint(x: 60, y: 40))), "no selection: the host shows no text menu, nor does the shell's take its place")
        paragraph.selected = true
        XCTAssertNil(T3TextContextMenu.replacement(for: rightClick(at: NSPoint(x: 280, y: 40))), "another view's own menu (a field, a page) stays")
        XCTAssertNil(T3TextContextMenu.replacement(for: rightClick(at: NSPoint(x: 380, y: 160))), "empty window: nothing")
    }

    func testTheMonitorTakesTheClickAndUnderTheAgentLogsTheMenuInsteadOfTracking() {
        let menu = T3TextContextMenu(agent: true)
        XCTAssertNil(menu.handle(rightClick(at: NSPoint(x: 60, y: 40))), "the click ends here: no host text menu follows")
        XCTAssertEqual(menu.lastShown, ["Cut (disabled)", "Copy", "Paste (disabled)", "Select All"])
        let elsewhere = rightClick(at: NSPoint(x: 280, y: 40))
        XCTAssertTrue(menu.handle(elsewhere) === elsewhere, "a click the host does not answer with its text menu goes on")
    }

    /// The click as the window server or the agent delivers it (AgentMouseMac.otherClick: `NSApp.sendEvent`), through
    /// the monitor `install()` registers: the shell's menu pops and the host's never follows.
    func testTheInstalledMonitorPopsTheShellsMenuAndTheHostsNeverFollows() {
        let menu = T3TextContextMenu(agent: false)
        var popped: [(titles: [String], view: NSView)] = []
        menu.present = { shown, _, view in popped.append((shown.items.map(\.title), view)) }
        menu.install()
        defer { menu.destroy() }
        NSApp.sendEvent(rightClick(at: NSPoint(x: 60, y: 40)))
        XCTAssertEqual(popped.map(\.titles), [["Cut", "Copy", "Paste", "Select All"]], "the shell's menu pops once")
        XCTAssertTrue(popped.first?.view === paragraph, "on the text view the click is on")
        XCTAssertEqual(paragraph.hostMenus, 0, "the click ends in the monitor: ExactKit's Look Up menu does not pop after it")
        NSApp.sendEvent(rightClick(at: NSPoint(x: 280, y: 40)))
        XCTAssertEqual(other.ownMenus, 1, "a view with its own menu still gets its click")
        XCTAssertEqual(popped.count, 1)
    }

    func testUnderTheAgentTheInstalledMonitorLogsTheMenuAndTheHostsNeverTracks() {
        let menu = T3TextContextMenu(agent: true)
        var popped = 0
        menu.present = { _, _, _ in popped += 1 }
        menu.install()
        defer { menu.destroy() }
        NSApp.sendEvent(rightClick(at: NSPoint(x: 60, y: 40)))
        XCTAssertEqual(menu.lastShown, ["Cut (disabled)", "Copy", "Paste (disabled)", "Select All"], "the items go to the log")
        XCTAssertEqual(popped, 0, "nothing tracks in the agent's never-key window")
        XCTAssertEqual(paragraph.hostMenus, 0, "nor does ExactKit's menu (the after drive's 120 s hang)")
        NSApp.sendEvent(rightClick(at: NSPoint(x: 280, y: 40)))
        XCTAssertEqual(other.ownMenus, 1)
    }
}

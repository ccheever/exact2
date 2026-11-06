import AppKit
import XCTest

// Since main #487 a Control-click is the secondary click ExactKit takes as a right-click
// where a node wrote `contextmenu` (`NodeView.contextClick`). Chromium sends a Control-click as `contextmenu` (no
// `click`), and where the page takes none Electron's `context-menu` pops the shell's menu (DesktopWindow.ts
// `installContextMenu`).

/// A link node that wrote its own `contextmenu`, as ExactKit runs it since #487: a right-click, or a Control-click that
/// `contextClick` sends to `rightMouseDown`; a plain click presses it.
private final class LinkNode: NSView {
    private(set) var menus = 0, presses = 0
    var url = "mailto:team@example.test"
    override func rightMouseDown(with event: NSEvent) { menus += 1 }
    override func mouseDown(with event: NSEvent) {
        if event.modifierFlags.contains(.control) { return rightMouseDown(with: event) }
        presses += 1
    }
    override func accessibilityRole() -> NSAccessibility.Role? { .link }
    override func accessibilityURL() -> URL? { URL(string: url) }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true } // the test's window need not be key
}

/// A node that is not pressable: ExactKit's `mouseDown` passes the click up the responder chain (`super.mouseDown`).
private final class PlainNode: NSView {
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
}

/// A text view that counts the menu AppKit would have shown for a Control-click (its own editing menu).
private final class ControlCountingTextView: NSTextView {
    private(set) var ownMenus = 0
    override func menu(for event: NSEvent) -> NSMenu? { ownMenus += 1; return super.menu(for: event) }
}

final class SecondaryClickTests: ShellMenuCase {
    func controlClick(at point: NSPoint) {
        NSApp.sendEvent(NSEvent.mouseEvent(with: .leftMouseDown, location: point, modifierFlags: [.control], timestamp: ProcessInfo.processInfo.systemUptime,
                                           windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!)
    }

    func testAPlainClickIsNoPageClick() {
        let link = LinkNode(frame: NSRect(x: 20, y: 20, width: 80, height: 22))
        page.addSubview(link)
        NSApp.sendEvent(NSEvent.mouseEvent(with: .leftMouseDown, location: at(link, NSPoint(x: 10, y: 10)), modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                           windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!)
        XCTAssertEqual([link.menus, link.presses], [0, 1])
        XCTAssertTrue(popped.isEmpty)
    }

    func testAControlClickNoNodeAnswersGetsTheShellsPageMenu() {
        let node = PlainNode(frame: NSRect(x: 20, y: 20, width: 200, height: 100))
        page.addSubview(node)
        controlClick(at: at(node, NSPoint(x: 50, y: 50)))
        XCTAssertEqual(popped.map(\.titles), [["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"]],
                       "the click went up the chain to the tail, as an unanswered right-click does")
    }

    func testAControlClickInATextInputGetsTheShellsTextMenu() {
        let area = ControlCountingTextView(frame: NSRect(x: 20, y: 120, width: 400, height: 60))
        area.font = .systemFont(ofSize: 14)
        area.string = "theme colors"
        page.addSubview(area)
        var actual = NSRange()
        let screen = area.firstRect(forCharacterRange: NSRange(location: 1, length: 1), actualRange: &actual)
        controlClick(at: window.convertPoint(fromScreen: NSPoint(x: screen.midX, y: screen.midY)))
        XCTAssertEqual(popped.map(\.titles), [["Cut", "Copy", "Paste (disabled)", "Select All"]], "the word is selected, as for a right-click")
        XCTAssertEqual((area.string as NSString).substring(with: area.selectedRange()), "theme")
        XCTAssertEqual(area.ownMenus, 0, "AppKit's own editing menu never shows")
    }
}

import AppKit
import XCTest

// Since main #1114 the ContextMenu key or VoiceOver's show menu that no node answers goes to the session's view as a
// secondary click's press (`MouseEventsMac.swift` `unansweredContextMenu`), which climbs the responder chain to the
// shell's tail: the reference's shell answers it as it does a right-click (DesktopWindow.ts `installContextMenu`). The
// press comes from no event the module's monitor sees, so the tail is in place before any right-click.

final class KeyboardRequestTests: ShellMenuCase {
    func press(on view: NSView, at point: NSPoint) {
        view.rightMouseDown(with: NSEvent.mouseEvent(with: .rightMouseDown, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                     windowNumber: view.window!.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!)
    }

    func testTheKeysPressReachesTheTailBeforeAnyRightClick() {
        press(on: page, at: NSPoint(x: 400, y: 200))
        XCTAssertEqual(popped.map(\.titles), [["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"]])
        XCTAssertTrue(popped.first?.view === page)
    }

    func testAWindowTheModuleSeesAfterInstallGetsItsTailThen() {
        let later = NSWindow(contentRect: NSRect(x: 260, y: 260, width: 400, height: 240), styleMask: [.titled], backing: .buffered, defer: false)
        later.isReleasedWhenClosed = false
        let laterPage = ExactView(frame: NSRect(x: 0, y: 0, width: 400, height: 240))
        later.contentView!.addSubview(laterPage)
        later.orderFrontRegardless()
        defer { later.orderOut(nil) }
        press(on: laterPage, at: NSPoint(x: 200, y: 120))
        XCTAssertTrue(popped.isEmpty, "a window the module has not seen keeps no tail")
        shell.attachTail(to: later) // T3Module.element: an element mounted in the window
        press(on: laterPage, at: NSPoint(x: 200, y: 120))
        XCTAssertEqual(popped.map(\.titles), [["Cut (disabled)", "Copy (disabled)", "Paste (disabled)", "Select All"]])
    }

    func testAWindowWithNoPageGetsNoTail() {
        let panel = NSWindow(contentRect: NSRect(x: 280, y: 280, width: 200, height: 120), styleMask: [.titled], backing: .buffered, defer: false)
        panel.isReleasedWhenClosed = false
        XCTAssertFalse(T3TextContextMenu.holdsPage(panel.contentView!))
        XCTAssertTrue(T3TextContextMenu.holdsPage(window.contentView!))
    }
}

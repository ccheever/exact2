// A press outside an open `popover="auto"` light-dismisses it at the
// release (HTML, "popover light dismiss"), also when the view pressed is
// one that tracks the press in its own loop and takes the release from the
// queue: NSTextView, a field's editor, NSSearchField.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// The test's window is never key; in a key window an NSTextView hears
/// every click, as `clicksArriveAsInTheKeyWindow` makes a node's view.
private final class KeyWindowTextView: NSTextView {
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
}

final class PopoverLightDismissMacTests: XCTestCase {
    private var window: NSWindow!
    private var monitor: Any?
    override func tearDown() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        window?.close(); window = nil
    }

    /// A page (1) with the popover's invoker (2), a search field (6) and a
    /// plain NSTextView, as an app module's; the popover (3) holds a field (4).
    private func fixture() throws -> (Presenter, NSTextView) {
        _ = NSApplication.shared
        let p = Presenter()
        p.menus.painted = true
        p.dialogs.painted = true
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        clicksArriveAsInTheKeyWindow()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "props": ["popovertarget": "menu"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["popover": "auto", "id": "menu"]],
            ["op": "create", "id": 4, "kind": "input", "props": ["value": "inside"]],
            ["op": "create", "id": 6, "kind": "input", "props": ["type": "search", "placeholder": "Search settings"]],
            ["op": "children", "id": 1, "ids": [2, 6, 3]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 500, "h": 400],
            ["op": "frame", "id": 2, "x": 10, "y": 10, "w": 80, "h": 30],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 240, "h": 60],
            ["op": "frame", "id": 4, "x": 10, "y": 10, "w": 200, "h": 30],
            ["op": "frame", "id": 6, "x": 260, "y": 300, "w": 200, "h": 30],
        ]))
        let text = KeyWindowTextView(frame: NSRect(x: 260, y: 10, width: 200, height: 60))
        text.string = "An app module's text view"
        try XCTUnwrap(p.views[1]).addSubview(text)
        window.orderFrontRegardless()
        // ExactView's monitor (ExactViewMac.swift): the application's local
        // monitors see what `NSApplication.sendEvent` dispatches, and none of
        // what a view's own tracking loop takes from the queue.
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseUp]) { [weak p] event in
            p?.menus.pointer(event); return event
        }
        return (p, text)
    }

    /// A click as a hand's comes, through the application: the up is in the
    /// queue first, for a view's tracking loop; one left there is sent next.
    /// Then the run loop turns, as the application's does between events.
    /// True when a tracking loop took the up.
    @discardableResult private func click(at point: NSPoint, releasedAt release: NSPoint? = nil) -> Bool {
        func event(_ type: NSEvent.EventType) -> NSEvent {
            NSEvent.mouseEvent(with: type, location: type == .leftMouseUp ? release ?? point : point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                               windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!
        }
        let down = event(.leftMouseDown)
        NSApp.postEvent(event(.leftMouseUp), atStart: true)
        NSApp.sendEvent(down)
        let left = NSApp.nextEvent(matching: [.leftMouseUp], until: nil, inMode: .default, dequeue: true)
        if let left { NSApp.sendEvent(left) }
        RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05))
        return left == nil
    }
    @discardableResult private func click(_ view: NSView) -> Bool { click(at: view.convert(NSPoint(x: view.bounds.midX, y: view.bounds.midY), to: nil)) }

    func testAPressInAnOutsideTextViewClosesThePopover() throws {
        let (p, text) = try fixture()
        let pop = try XCTUnwrap(p.views[3])
        p.press(2)
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertTrue(click(text), "the text view's loop took the release")
        XCTAssertTrue(window.firstResponder === text, "the text view took the press")
        XCTAssertFalse(p.menus.isOpen(pop), "light dismiss though the text view's loop took the release")
    }

    func testAPressInAnOutsideSearchFieldClosesThePopover() throws {
        let (p, _) = try fixture()
        let pop = try XCTUnwrap(p.views[3])
        let search = try XCTUnwrap(p.views[6]?.field as? NSSearchField)
        p.press(2)
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertTrue(click(search), "the field's loop took the release")
        XCTAssertNotNil(search.currentEditor(), "the search field is editing")
        XCTAssertFalse(p.menus.isOpen(pop), "light dismiss though the field editor's loop took the release")
        // Again, now that the field editor is the view pressed.
        p.press(2)
        XCTAssertTrue(p.menus.isOpen(pop))
        click(search)
        XCTAssertFalse(p.menus.isOpen(pop))
    }

    func testAPressDraggedFromAnOutsideTextViewOntoThePopoverKeepsItOpen() throws {
        let (p, text) = try fixture()
        let pop = try XCTUnwrap(p.views[3]), field = try XCTUnwrap(p.views[4]?.field)
        p.press(2)
        let from = text.convert(NSPoint(x: text.bounds.midX, y: text.bounds.midY), to: nil)
        let to = field.convert(NSPoint(x: field.bounds.midX, y: field.bounds.midY), to: nil)
        XCTAssertTrue(click(at: from, releasedAt: to))
        XCTAssertTrue(p.menus.isOpen(pop), "released over the popover: not the same topmost clicked popover")
    }

    func testAPressInAFieldInsideThePopoverKeepsItOpen() throws {
        let (p, _) = try fixture()
        let pop = try XCTUnwrap(p.views[3])
        p.press(2)
        let field = try XCTUnwrap(p.views[4]?.field)
        XCTAssertTrue(click(field))
        XCTAssertNotNil(field.currentEditor())
        XCTAssertTrue(p.menus.isOpen(pop), "a press that begins and ends in the popover is not outside it")
        click(field)
        XCTAssertTrue(p.menus.isOpen(pop))
    }
}
#endif

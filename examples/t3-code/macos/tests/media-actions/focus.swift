import AppKit
import XCTest

// T3MediaFocus (a press elsewhere takes the focus off a media node) and T3MenuDismissal
// (the secondary click that closed the media menu opens no new one), with events through NSApplication.sendEvent,
// the path a click from the window server takes (local monitors, then the window).
final class MediaFocusTests: XCTestCase {
    final class Focusable: NSView {
        override var acceptsFirstResponder: Bool { true }
        override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    }
    final class Plain: NSView {
        var downs = 0
        override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
        override func mouseDown(with event: NSEvent) { downs += 1 }
        override func rightMouseDown(with event: NSEvent) { downs += 1 }
    }
    /// A view that takes the focus on its own press, as a text field or a focusable node does.
    final class TakesFocus: NSView {
        override var acceptsFirstResponder: Bool { true }
        override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
        override func mouseDown(with event: NSEvent) { window?.makeFirstResponder(self) }
    }

    private func window() -> (NSWindow, Focusable, Plain, TakesFocus) {
        let window = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 600, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let media = Focusable(frame: NSRect(x: 20, y: 20, width: 100, height: 80))
        let other = Plain(frame: NSRect(x: 200, y: 20, width: 100, height: 80))
        let field = TakesFocus(frame: NSRect(x: 400, y: 20, width: 100, height: 80))
        for view in [media, other, field] { window.contentView!.addSubview(view) }
        window.orderFrontRegardless()
        return (window, media, other, field)
    }
    private func send(_ type: NSEvent.EventType, at point: NSPoint, in window: NSWindow, timestamp: TimeInterval = ProcessInfo.processInfo.systemUptime, flags: NSEvent.ModifierFlags = []) {
        NSApp.sendEvent(NSEvent.mouseEvent(with: type, location: point, modifierFlags: flags, timestamp: timestamp, windowNumber: window.windowNumber,
                                           context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!)
    }
    private func click(at point: NSPoint, in window: NSWindow) {
        send(.leftMouseDown, at: point, in: window)
        send(.leftMouseUp, at: point, in: window)
        RunLoop.main.run(until: Date().addingTimeInterval(0.1))
    }

    func testAPressOnSomethingThatTakesNoFocusTakesItOffTheMedia() {
        let (window, media, other, _) = window(), focus = T3MediaFocus()
        defer { focus.destroy(); window.close() }
        focus.add(media)
        XCTAssertTrue(window.makeFirstResponder(media))
        click(at: NSPoint(x: 250, y: 60), in: window)
        XCTAssertEqual(other.downs, 1, "the press still reaches what it landed on")
        XCTAssertFalse(window.firstResponder === media, "the media no longer holds the focus, so Shift+F10 or the ContextMenu key reaches it no more")
    }
    func testAPressOnTheMediaKeepsItsFocus() {
        let (window, media, _, _) = window(), focus = T3MediaFocus()
        defer { focus.destroy(); window.close() }
        focus.add(media)
        XCTAssertTrue(window.makeFirstResponder(media))
        click(at: NSPoint(x: 70, y: 60), in: window)
        XCTAssertTrue(window.firstResponder === media)
    }
    func testAPressThatTakesTheFocusKeepsWhatItTook() {
        let (window, media, _, field) = window(), focus = T3MediaFocus()
        defer { focus.destroy(); window.close() }
        focus.add(media)
        XCTAssertTrue(window.makeFirstResponder(media))
        click(at: NSPoint(x: 450, y: 60), in: window)
        XCTAssertTrue(window.firstResponder === field)
    }
    func testAnUnregisteredViewKeepsItsFocus() {
        let (window, media, _, _) = window(), focus = T3MediaFocus()
        defer { focus.destroy(); window.close() }
        focus.add(Focusable(frame: .zero))
        XCTAssertTrue(window.makeFirstResponder(media))
        click(at: NSPoint(x: 250, y: 60), in: window)
        XCTAssertTrue(window.firstResponder === media)
    }
    func testTheSecondaryClickThatClosedTheMenuOpensNoNewOne() {
        let (window, _, other, _) = window()
        defer { T3MenuDismissal.disarm(); window.close() }
        let closedAt = ProcessInfo.processInfo.systemUptime
        T3MenuDismissal.arm(closedAt: closedAt)
        send(.rightMouseDown, at: NSPoint(x: 250, y: 60), in: window, timestamp: closedAt - 0.05)
        XCTAssertEqual(other.downs, 0, "the right-click made while the menu was open goes no further")
        send(.rightMouseDown, at: NSPoint(x: 250, y: 60), in: window, timestamp: closedAt + 0.01)
        XCTAssertEqual(other.downs, 1, "a right-click after the menu closed is a new one")
        T3MenuDismissal.arm(closedAt: closedAt)
        send(.leftMouseDown, at: NSPoint(x: 250, y: 60), in: window, timestamp: closedAt - 0.05, flags: .control)
        XCTAssertEqual(other.downs, 1, "a Control-click made while the menu was open goes no further")
        send(.leftMouseDown, at: NSPoint(x: 250, y: 60), in: window, timestamp: closedAt - 0.05)
        XCTAssertEqual(other.downs, 2, "a plain click is not a secondary click")
    }
    func testTheDismissalWatchEnds() {
        let (window, _, other, _) = window()
        defer { T3MenuDismissal.disarm(); window.close() }
        let closedAt = ProcessInfo.processInfo.systemUptime
        T3MenuDismissal.arm(closedAt: closedAt, for: 0.1)
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        send(.rightMouseDown, at: NSPoint(x: 250, y: 60), in: window, timestamp: closedAt - 0.05)
        XCTAssertEqual(other.downs, 1)
    }
}

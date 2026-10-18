// A press outside an open `popover="auto"` light-dismisses it at the
// release (HTML, "popover light dismiss"), also when the view pressed is
// one that tracks the press in its own loop and takes the release from the
// queue: a field's editor, NSSearchField, an NSTextView whose subclass
// overrides `mouseDown(with:)`.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// The test's window is never key; in a key window an NSTextView hears
/// every click, as `clicksArriveAsInTheKeyWindow` makes a node's view.
private class KeyWindowTextView: NSTextView {
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
}
/// An NSTextView that tracks a press in its own loop. On macOS 27 a stock
/// NSTextView takes the press through the gesture recognizers it installs
/// and leaves the release in the queue for the application; a subclass
/// that overrides `mouseDown(with:)` gets no recognizers, and the loop.
private final class TrackingTextView: KeyWindowTextView {
    override func mouseDown(with event: NSEvent) { super.mouseDown(with: event) }
}

private final class TrackingFieldEditorOwner: NSObject, NSWindowDelegate {
    let editor = TrackingTextView(frame: .zero)
    override init() { super.init(); editor.isFieldEditor = true }
    func windowWillReturnFieldEditor(_ sender: NSWindow, to client: Any?) -> Any? { editor }
}

final class PopoverLightDismissMacTests: XCTestCase {
    private var window: NSWindow!
    private var monitor: Any?
    private var handRelease: ((Int, NSWindow) -> (time: TimeInterval, point: NSPoint))!
    /// The window server's last release, as `MenuHost.handRelease` reads
    /// it: none, until a click `byHand` makes one.
    private var hand: (time: TimeInterval, point: NSPoint) = (0, .zero)
    override func setUp() {
        handRelease = MenuHost.handRelease
        MenuHost.handRelease = { [unowned self] _, _ in hand }
    }
    override func tearDown() {
        MenuHost.handRelease = handRelease
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        window?.close(); window = nil
    }

    /// A page (1) with the popover's invoker (2), a search field (6), a
    /// field (7) and an NSTextView, as an app module's: one that tracks a
    /// press in its own loop, or a stock one when `tracking` is false. The
    /// popover (3) holds a field (4).
    private func fixture(tracking: Bool = true) throws -> (Presenter, NSTextView) {
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
            ["op": "create", "id": 7, "kind": "input", "props": ["placeholder": "Message"]],
            ["op": "children", "id": 1, "ids": [2, 6, 7, 3]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 500, "h": 400],
            ["op": "frame", "id": 2, "x": 10, "y": 10, "w": 80, "h": 30],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 240, "h": 60],
            ["op": "frame", "id": 4, "x": 10, "y": 10, "w": 200, "h": 30],
            ["op": "frame", "id": 6, "x": 260, "y": 300, "w": 200, "h": 30],
            ["op": "frame", "id": 7, "x": 260, "y": 250, "w": 200, "h": 30],
        ]))
        let frame = NSRect(x: 260, y: 10, width: 200, height: 60)
        let text = tracking ? TrackingTextView(frame: frame) : KeyWindowTextView(frame: frame)
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

    /// A click through the application: the up is in the queue first, for
    /// a view's tracking loop; one left there is sent next. Then the run
    /// loop turns, as the application's does between events. True when a
    /// tracking loop took the up.
    ///
    /// `byHand` is the order a hand's click comes in (measured with real
    /// input on macOS 27, 2026-10-16): the window server records the
    /// release, and once a field editor or a text view has taken the press
    /// AppKit queues the `mouseExited`/`mouseEntered` events of the tracking
    /// areas it moved, and sends them before the run loop turns. The
    /// application's current event is then one of those, no longer the
    /// release. A click made in a test leaves nothing after the release, so
    /// the release stayed the current event: the earlier tests passed while
    /// the app failed.
    @discardableResult private func click(at point: NSPoint, releasedAt release: NSPoint? = nil, byHand: Bool = false) -> Bool {
        func event(_ type: NSEvent.EventType) -> NSEvent {
            NSEvent.mouseEvent(with: type, location: type == .leftMouseUp ? release ?? point : point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                               windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!
        }
        let down = event(.leftMouseDown), up = event(.leftMouseUp)
        if byHand { hand = (up.timestamp, up.locationInWindow) }
        NSApp.postEvent(up, atStart: true)
        NSApp.sendEvent(down)
        if byHand {
            // Tracking has ended; append behind any release left for the application.
            for type in [NSEvent.EventType.mouseEntered, .mouseExited] {
                NSApp.postEvent(NSEvent.enterExitEvent(with: type, location: up.locationInWindow, modifierFlags: [], timestamp: up.timestamp,
                                                       windowNumber: window.windowNumber, context: nil, eventNumber: 0, trackingNumber: 0, userData: nil)!, atStart: false)
            }
        }
        var left = false
        // The events the hand's click left are sent before the run loop turns
        // in the default mode: taken here in a mode no check is scheduled in.
        while byHand, let next = NSApp.nextEvent(matching: [.leftMouseUp, .mouseEntered, .mouseExited], until: nil, inMode: .eventTracking, dequeue: true) {
            if next.type == .leftMouseUp { left = true }
            NSApp.sendEvent(next)
        }
        // As `NSApplication.run`: what is queued is sent, then the run loop turns.
        while let next = NSApp.nextEvent(matching: .any, until: Date(timeIntervalSinceNow: 0.05), inMode: .default, dequeue: true) {
            if next.type == .leftMouseUp { left = true }
            NSApp.sendEvent(next)
        }
        return !left
    }
    @discardableResult private func click(_ view: NSView, byHand: Bool = false) -> Bool {
        click(at: view.convert(NSPoint(x: view.bounds.midX, y: view.bounds.midY), to: nil), byHand: byHand)
    }

    func testAPressInAnOutsideTextViewClosesThePopover() throws {
        let (p, text) = try fixture()
        let pop = try XCTUnwrap(p.views[3])
        p.press(2)
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertTrue(click(text), "the text view's loop took the release")
        XCTAssertTrue(window.firstResponder === text, "the text view took the press")
        XCTAssertFalse(p.menus.isOpen(pop), "light dismiss though the text view's loop took the release")
    }

    func testAPressInAnOutsideStockTextViewClosesThePopover() throws {
        let (p, text) = try fixture(tracking: false)
        let pop = try XCTUnwrap(p.views[3])
        p.press(2)
        XCTAssertTrue(p.menus.isOpen(pop))
        click(text)
        XCTAssertTrue(window.firstResponder === text, "the text view took the press")
        XCTAssertFalse(p.menus.isOpen(pop), "light dismiss whichever way the text view took the press")
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

    /// The real-input session's failure (2026-10-16): a hand's click in
    /// the composer closed the menu once, then never again.
    func testAHandsPressesInOutsideTextViewsCloseThePopoverEachTime() throws {
        let (p, text) = try fixture()
        let pop = try XCTUnwrap(p.views[3])
        let search = try XCTUnwrap(p.views[6]?.field as? NSSearchField), message = try XCTUnwrap(p.views[7]?.field)
        for (round, view) in [text, text, search, search, message, message, text].enumerated() {
            p.press(2)
            XCTAssertTrue(p.menus.isOpen(pop), "round \(round)")
            // On macOS 27 the earlier default-mode turns install the fields'
            // gesture recognizers. Even a field's first press can then leave
            // the release to the application, through its normal editor.
            let tracked = click(view, byHand: true)
            if view === text { XCTAssertTrue(tracked, "round \(round): the text view's loop took the release") }
            if let field = view as? NSTextField { XCTAssertNotNil(field.currentEditor(), "round \(round): the field took the press") }
            XCTAssertNotEqual(NSApp.currentEvent?.type, .leftMouseUp, "round \(round): AppKit sent another event after the release")
            XCTAssertFalse(p.menus.isOpen(pop), "round \(round): light dismiss by a hand's press in \(type(of: view))")
        }
    }

    func testAHandsPressesInTrackingFieldEditorsCloseThePopoverEachTime() throws {
        let (p, _) = try fixture()
        let pop = try XCTUnwrap(p.views[3]), page = try XCTUnwrap(p.views[1])
        // Enter editing before the press so it reaches an editor that runs
        // mouseDown's tracking loop. The session's own fields above retain
        // their normal editor and also cover the press that begins editing.
        let owner = TrackingFieldEditorOwner()
        window.delegate = owner
        let search = NSSearchField(frame: NSRect(x: 260, y: 100, width: 200, height: 30))
        let field = NSTextField(frame: NSRect(x: 260, y: 180, width: 200, height: 30))
        page.addSubview(search); page.addSubview(field)
        withExtendedLifetime(owner) {
            for (round, control) in [search, search, field, field, search].enumerated() {
                p.press(2)
                XCTAssertTrue(p.menus.isOpen(pop), "round \(round)")
                XCTAssertTrue(window.makeFirstResponder(control))
                XCTAssertTrue(control.currentEditor() === owner.editor)
                XCTAssertTrue(click(owner.editor, byHand: true), "round \(round): the field editor's loop took the release")
                XCTAssertTrue(control.currentEditor() === owner.editor, "round \(round): the field took the press")
                XCTAssertNotEqual(NSApp.currentEvent?.type, .leftMouseUp, "round \(round): AppKit sent another event after the release")
                XCTAssertFalse(p.menus.isOpen(pop), "round \(round): light dismiss though the field editor's loop took the release")
            }
        }
    }

    func testAHandsPressDraggedFromAnOutsideTextViewOntoThePopoverKeepsItOpen() throws {
        let (p, text) = try fixture()
        let pop = try XCTUnwrap(p.views[3]), field = try XCTUnwrap(p.views[4]?.field)
        p.press(2)
        let from = text.convert(NSPoint(x: text.bounds.midX, y: text.bounds.midY), to: nil)
        let to = field.convert(NSPoint(x: field.bounds.midX, y: field.bounds.midY), to: nil)
        XCTAssertTrue(click(at: from, releasedAt: to, byHand: true))
        XCTAssertTrue(p.menus.isOpen(pop), "released over the popover: not the same topmost clicked popover")
    }

    /// A press no view tracks (the driver's, or a hand's still held) is
    /// settled by its own release, however often the run loop turns first.
    func testAPressStillHeldOutsideWaitsForItsRelease() throws {
        let (p, _) = try fixture()
        let pop = try XCTUnwrap(p.views[3]), page = try XCTUnwrap(p.views[1]), field = try XCTUnwrap(p.views[4]?.field)
        p.press(2)
        let at = page.convert(NSPoint(x: 100, y: 200), to: nil)
        let down = try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseDown, location: at, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                    windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1))
        NSApp.sendEvent(down)
        RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05))
        XCTAssertTrue(p.menus.isOpen(pop), "not released yet")
        let to = field.convert(NSPoint(x: field.bounds.midX, y: field.bounds.midY), to: nil)
        NSApp.sendEvent(try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseUp, location: to, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                         windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 0)))
        XCTAssertTrue(p.menus.isOpen(pop), "released over the popover")
    }
}
#endif

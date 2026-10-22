#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A click whose press mounts an `autofocus` text field begins editing it,
/// as AppKit's click-to-edit does: the field takes the focus from the field
/// that held it, and the caret goes under the click (FocusMac `pressMount`).
final class PressMountCaretMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    /// A composer (2) and a box (3) whose press mounts the editor.
    private func presenter() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["value": "draft"], "handlers": ["input"]],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["press"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 30.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 40.0, "w": 300.0, "h": 200.0],
        ]))
        return p
    }

    /// The editor (`id`), mounted over the box by one batch.
    private func mount(_ p: Presenter, _ id: Int, text: String = "line one\nline two\nline three") {
        p.apply(wireBatch([
            ["op": "create", "id": id, "kind": "textarea", "props": ["value": text, "autofocus": "true"], "handlers": ["input"]],
            ["op": "children", "id": 3, "ids": [id]],
            ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
        ]))
    }

    private func click(_ p: Presenter, at point: NSPoint) throws -> NSEvent {
        try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseUp, location: p.viewport.convert(point, to: nil), modifierFlags: [],
                                         timestamp: 0, windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 0))
    }

    func testAClickThatMountsAnAutofocusFieldPutsTheCaretUnderIt() throws {
        let p = presenter()
        let composer = try XCTUnwrap(p.views[2]?.textArea)
        XCTAssertTrue(window.makeFirstResponder(composer))
        // On the first line, a few characters in.
        p.pressClick = try click(p, at: NSPoint(x: 30, y: 48))
        mount(p, 4)
        p.pressClick = nil
        let editor = try XCTUnwrap(p.views[4]?.textArea)
        XCTAssertTrue(window.firstResponder === editor, "the field the press mounted takes the focus from the composer")
        let caret = editor.selectedRange()
        XCTAssertEqual(caret.length, 0)
        XCTAssertTrue((1..<9).contains(caret.location), "under the click on the first line, not at its start or end: \(caret.location)")
    }

    func testAClickBelowTheTextPutsTheCaretAtTheEnd() throws {
        let p = presenter()
        XCTAssertTrue(window.makeFirstResponder(try XCTUnwrap(p.views[2]?.textArea)))
        p.pressClick = try click(p, at: NSPoint(x: 30, y: 220))
        mount(p, 4)
        p.pressClick = nil
        let editor = try XCTUnwrap(p.views[4]?.textArea)
        XCTAssertTrue(window.firstResponder === editor)
        XCTAssertEqual(editor.selectedRange(), NSRange(location: ("line one\nline two\nline three" as NSString).length, length: 0))
    }

    /// A field mounted with no press under way (a timer, a data answer)
    /// leaves the focus where it is, as before.
    func testAnAutofocusMountedWithoutAPressKeepsTheHeldFocus() throws {
        let p = presenter()
        let composer = try XCTUnwrap(p.views[2]?.textArea)
        XCTAssertTrue(window.makeFirstResponder(composer))
        mount(p, 4)
        XCTAssertTrue(window.firstResponder === composer)
    }

    /// The press of a click on the box itself, through `mouseDown`/`mouseUp`
    /// and `press(id)`: its runner mounts the editor as the press is applied.
    func testAClickOnTheBoxThroughItsMousePathBeginsEditing() throws {
        let p = presenter()
        XCTAssertTrue(window.makeFirstResponder(try XCTUnwrap(p.views[2]?.textArea)))
        p.onPress = { [unowned self] id in if id == 3 { self.mount(p, 4) } }
        let box = try XCTUnwrap(p.views[3]), at = p.viewport.convert(NSPoint(x: 30, y: 48), to: nil)
        func event(_ type: NSEvent.EventType) throws -> NSEvent {
            try XCTUnwrap(NSEvent.mouseEvent(with: type, location: at, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber,
                                             context: nil, eventNumber: 7, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0))
        }
        box.mouseDown(with: try event(.leftMouseDown))
        box.mouseUp(with: try event(.leftMouseUp))
        let editor = try XCTUnwrap(p.views[4]?.textArea)
        XCTAssertTrue(window.firstResponder === editor, "the click's press mounted it and it took the focus")
        XCTAssertTrue((1..<9).contains(editor.selectedRange().location), "under the click: \(editor.selectedRange())")
        XCTAssertNil(p.pressClick, "the click is the press's alone")
    }

    /// VoiceOver's press, with an old click still the app's current event,
    /// carries no click: the held focus stays, as for any press without one.
    func testAnAssistivePressWithAnOldClickCurrentKeepsTheHeldFocus() throws {
        let p = presenter()
        let composer = try XCTUnwrap(p.views[2]?.textArea)
        XCTAssertTrue(window.makeFirstResponder(composer))
        p.onPress = { [unowned self] id in if id == 3 { self.mount(p, 4) } }
        // An earlier click's release, made the current event as `NSApp.nextEvent` makes it.
        NSApp.postEvent(try click(p, at: NSPoint(x: 30, y: 48)), atStart: true)
        _ = NSApp.nextEvent(matching: .leftMouseUp, until: .distantPast, inMode: .default, dequeue: true)
        XCTAssertEqual(NSApp.currentEvent?.type, .leftMouseUp)
        XCTAssertTrue(try XCTUnwrap(p.views[3]).accessibilityPerformPress())
        let pressed = expectation(description: "the assistive press ran")
        DispatchQueue.main.async { pressed.fulfill() }
        wait(for: [pressed], timeout: 10)
        XCTAssertNotNil(p.views[4], "the press mounted the editor")
        XCTAssertTrue(window.firstResponder === composer, "an old click is not this press's")
    }

    /// A press from the keyboard carries no click: no handoff.
    func testAKeyPressCarriesNoClick() throws {
        let p = presenter()
        let composer = try XCTUnwrap(p.views[2]?.textArea)
        XCTAssertTrue(window.makeFirstResponder(composer))
        p.pressClick = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber,
                                        context: nil, characters: " ", charactersIgnoringModifiers: " ", isARepeat: false, keyCode: 49)
        mount(p, 4)
        p.pressClick = nil
        XCTAssertTrue(window.firstResponder === composer)
    }
}
#endif

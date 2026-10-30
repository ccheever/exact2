#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A press in a text area or an input reaches `pointerdown` and `pointerup`
/// from its node up, as a web `<textarea>`'s does, around AppKit's own
/// tracking, which still places the caret. A press that moves the focus
/// into an input hears `focus` first here: NSWindow gives the hit view the
/// focus before its `mouseDown`, and a bare presenter has no session
/// window's monitor to send the down before it, as Chrome orders them
/// (EditorPointerOrderMacTests). An editor that does not track (disabled)
/// leaves the press to the node: its up is the release.
final class EditorPointerMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    private func presenter(_ log: @escaping (String) -> Void) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        clicksArriveAsInTheKeyWindow()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown", "pointerup"]],
            ["op": "create", "id": 2, "kind": "view", "handlers": ["pointerdown"]],
            ["op": "create", "id": 3, "kind": "textarea", "props": ["value": "hello world"], "handlers": ["focus"]],
            ["op": "create", "id": 4, "kind": "input", "props": ["value": "hello world"], "handlers": ["focus"]],
            ["op": "create", "id": 5, "kind": "input", "props": ["value": "hunter22", "type": "password"], "handlers": ["focus"]],
            ["op": "create", "id": 6, "kind": "textarea", "props": ["value": "off", "disabled": "true"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3, 4, 5, 6]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 280.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 100.0, "w": 300.0, "h": 24.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 140.0, "w": 300.0, "h": 24.0],
            ["op": "frame", "id": 6, "x": 0.0, "y": 180.0, "w": 300.0, "h": 60.0],
        ]))
        p.viewport.layoutSubtreeIfNeeded()
        window.orderFrontRegardless()
        p.onPointer = { id, kind, _ in log("\(kind == .down ? "down" : kind == .up ? "up" : "move") \(id)") }
        p.onFocus = { log("focus \($0)") }
        return p
    }

    /// A press at the start of node `id`'s text as a hand's comes, through the
    /// application and the window (ClickFocusMacTests): both queued, the down
    /// taken as the run loop takes it (the application's current event) and
    /// sent, the up left for a tracking loop and sent when no loop took it.
    /// `between` runs after the down, before the up is sent.
    private func press(_ p: Presenter, _ id: UInt32, between: () -> Void = {}) {
        let node = p.views[id]!
        // Inside an input's editor, past its inset; at a text area's first character.
        let at = node.convert(NSPoint(x: node.textArea == nil ? 4 : 1, y: 6), to: nil)
        func event(_ type: NSEvent.EventType) -> NSEvent {
            NSEvent.mouseEvent(with: type, location: at, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                               windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!
        }
        let up = event(.leftMouseUp)
        NSApp.postEvent(up, atStart: true)
        NSApp.postEvent(event(.leftMouseDown), atStart: true)
        if let down = NSApp.nextEvent(matching: [.leftMouseDown], until: nil, inMode: .default, dequeue: true) { NSApp.sendEvent(down) }
        between()
        if NSApp.nextEvent(matching: [.leftMouseUp], until: nil, inMode: .default, dequeue: true) != nil { NSApp.sendEvent(up) }
    }

    func testATextAreaPressReachesItsAncestorsAroundItsSelection() throws {
        var log: [String] = []
        let p = presenter { log.append($0) }
        let area = try XCTUnwrap(p.views[3]?.textArea)
        area.setSelectedRange(NSRange(location: 5, length: 0))
        press(p, 3)
        XCTAssertEqual(log.filter { !$0.hasPrefix("focus") }, ["down 2", "down 1", "up 1"], "down bubbles; up to the node that hears it")
        XCTAssertEqual(area.selectedRange(), NSRange(location: 0, length: 0), "AppKit's tracking placed the caret")
        XCTAssertTrue(p.focusByPointer, "a focus the press moves next shows no focus-visible")
        XCTAssertNil(p.pointerHeld)
        log = []
        // Already focused: the press of GAPS C1, a composer expanding on a
        // press in its own editor.
        press(p, 3)
        XCTAssertEqual(log, ["down 2", "down 1", "up 1"])
    }

    func testAnInputPressIsReportedOnceThroughItsFieldEditor() throws {
        var log: [String] = []
        let p = presenter { log.append($0) }
        let field = try XCTUnwrap(p.views[4]?.field)
        press(p, 4)
        XCTAssertEqual(log, ["focus 4", "down 2", "down 1", "up 1"], "the field hands its down to the field editor: one press")
        let editor = try XCTUnwrap(field.currentEditor() as? FieldEditor)
        editor.setSelectedRange(NSRange(location: 5, length: 0))
        log = []
        press(p, 4)
        XCTAssertEqual(log, ["down 2", "down 1", "up 1"])
        XCTAssertEqual(editor.selectedRange(), NSRange(location: 0, length: 0), "AppKit's tracking placed the caret")
        XCTAssertNil(p.pointerHeld)
    }

    /// A password field edits in AppKit's own secure editor: every press, the
    /// focused field's too, is reported.
    func testEveryPressInAPasswordFieldIsReported() throws {
        var log: [String] = []
        let p = presenter { log.append($0) }
        let field = try XCTUnwrap(p.views[5]?.field)
        press(p, 5)
        XCTAssertEqual(log, ["focus 5", "down 2", "down 1", "up 1"])
        let editor = try XCTUnwrap(field.currentEditor())
        XCTAssertFalse(editor is FieldEditor, "AppKit's secure editor")
        log = []
        press(p, 5)
        XCTAssertEqual(log, ["down 2", "down 1", "up 1"], "the focused field's press")
        XCTAssertEqual(editor.selectedRange.length, 0, "AppKit's tracking placed the caret; the field did not begin editing again")
        log = []
        press(p, 5)
        XCTAssertEqual(log, ["down 2", "down 1", "up 1"])
        XCTAssertNil(p.pointerHeld)
    }

    /// A disabled text area tracks nothing: its ancestors hear the down,
    /// and the up only once the button is released.
    func testADisabledTextAreasPressIsTheNodesUntilTheRelease() throws {
        var log: [String] = []
        let p = presenter { log.append($0) }
        press(p, 6) { XCTAssertEqual(log, ["down 2", "down 1"], "no up while the button is down") }
        XCTAssertEqual(log, ["down 2", "down 1", "up 1"])
        XCTAssertNil(p.pointerHeld)
    }
}
#endif

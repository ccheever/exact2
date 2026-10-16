#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1115.000 D7.9: what a window that is not key does with the pointer,
/// as an AppKit window does. The click that brings it forward presses a
/// control and nothing else (every node used to take it, a browser's rule),
/// and nothing in it is hovered: hover is tracked in the key window only (it
/// was tracked always).
extension XCTestCase {
    /// Clicks this test sends through the application reach the node under
    /// them, content included, as in the window a person is using. A test's
    /// process is never the active app, so AppKit treats each of its clicks
    /// as the one that would activate the window, which now presses only a
    /// control (LLP 1115.000 D7.9). Given back when the test ends.
    func clicksArriveAsInTheKeyWindow() {
        NodeView.everyClickCounts = true
        addTeardownBlock { NodeView.everyClickCounts = ExactEnv.agentMode }
    }
}

final class PointerMannersMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    private func fixture() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        func text(_ id: Int, _ value: String) -> [String: Any] { ["op": "create", "id": id, "kind": "text", "props": ["text": value]] }
        func frame(_ id: Int, _ y: Int, x: Int = 10) -> [String: Any] { ["op": "frame", "id": id, "x": x, "y": y, "w": 200, "h": 30] }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"]], text(3, "A button's label"),
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"]], text(5, "A row's words"),
            text(6, "A paragraph"),
            ["op": "create", "id": 7, "kind": "view", "props": ["accessibilityRole": "tab"], "handlers": ["press"]], text(8, "A tab"),
            ["op": "create", "id": 9, "kind": "view", "handlers": ["hover", "pointerdown", "pointerup"]],
            ["op": "create", "id": 10, "kind": "view", "handlers": ["hover"]],
            ["op": "children", "id": 1, "ids": [2, 4, 6, 7, 9, 10]],
            ["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 4, "ids": [5]], ["op": "children", "id": 7, "ids": [8]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            frame(2, 10), frame(3, 0, x: 0), frame(4, 50), frame(5, 0, x: 0), frame(6, 90), frame(7, 130), frame(8, 0, x: 0), frame(9, 170), frame(10, 210),
        ]))
        p.viewport.layoutSubtreeIfNeeded()
        return p
    }

    func testTheClickThatActivatesAWindowPressesAControlAndNothingElse() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent's window is never key: its every click counts")
        let p = fixture()
        func first(_ id: UInt32) throws -> Bool { try XCTUnwrap(p.views[id]).acceptsFirstMouse(for: nil) }
        XCTAssertTrue(try first(2), "a button clicks through, as an NSButton does")
        XCTAssertTrue(try first(3), "and so does its label, which is what the click lands on")
        XCTAssertTrue(try first(7), "a node with a control's role")
        XCTAssertTrue(try first(8))
        XCTAssertFalse(try first(4), "a row that is only pressable is content: the click brings the window forward")
        XCTAssertFalse(try first(5))
        XCTAssertFalse(try first(6), "a paragraph: no selection begins")
        XCTAssertFalse(try first(1), "the ground")
        XCTAssertFalse(DialogBackdrop(frame: .zero).acceptsFirstMouse(for: nil), "a dialog's backdrop is not dismissed by it")
        // A list's row is a table's: the click that brings the window forward presses it, its words included.
        let list = NodeView(id: 20, kind: "list", presenter: p), row = try XCTUnwrap(p.views[4]), words = try XCTUnwrap(p.views[5])
        p.root.addSubview(list); p.views[20] = list
        list.container.addSubview(row)
        XCTAssertTrue(row.isListRow)
        XCTAssertTrue(row.acceptsFirstMouse(for: nil), "a list's row, as an NSTableView's")
        XCTAssertTrue(words.acceptsFirstMouse(for: nil))
        // Text a drag selects begins no selection with it, in a list or out of one.
        let reading = try XCTUnwrap(p.views[6])
        list.container.addSubview(reading)
        XCTAssertTrue(reading.selectsText)
        XCTAssertFalse(reading.acceptsFirstMouse(for: nil), "as an NSTextView does not")
    }

    func testPositionedOverlayConsumesTheClickAboveAButton() throws {
        let p = fixture(), button = try XCTUnwrap(p.views[2]), overlay = try XCTUnwrap(p.views[9])
        overlay.frame = button.frame
        clicksArriveAsInTheKeyWindow()
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(button)
        let at = overlay.convert(NSPoint(x: 20, y: 15), to: nil)
        XCTAssertTrue(window.contentView?.hitTest(at) === overlay)
        var presses: [UInt32] = [], pointers: [UInt32] = []
        p.onPress = { presses.append($0) }
        p.onPointer = { id, _, _ in pointers.append(id) }
        func click() throws {
            for type: NSEvent.EventType in [.leftMouseDown, .leftMouseUp] {
                window.sendEvent(try XCTUnwrap(NSEvent.mouseEvent(with: type, location: at, modifierFlags: [],
                    timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                    context: nil, eventNumber: 1, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)))
            }
        }
        try click()
        XCTAssertEqual(pointers, [9, 9])
        XCTAssertEqual(presses, [], "the overlay has no press, but still covers the button")
        overlay.handlers = []
        try click()
        XCTAssertEqual(presses, [], "a handlerless overlay also consumes the click")
        p.views[1]?.handlers = ["press"]
        try click()
        XCTAssertEqual(presses, [1], "the overlay's ancestor still receives the click")
        overlay.applyStyle(["pointer_events": "none"])
        try click()
        XCTAssertEqual(presses, [1, 2], "pointer-events: none lets the button receive the click")
        overlay.applyStyle(["pointer_events": "auto"])
        overlay.isHidden = true
        try click()
        XCTAssertEqual(presses, [1, 2, 2], "the uncovered button still presses")
    }

    func testHoverIsTheKeyWindowsOnly() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent's hover is where its drive put it")
        let p = fixture()
        var hovered: [String] = []
        p.onHover = { id, over in hovered.append("\(id)\(over ? "+" : "-")") }
        let node = try XCTUnwrap(p.views[9])
        node.syncHoverTracking()
        p.viewport.updateTrackingAreas()
        for area in node.trackingAreas + p.viewport.trackingAreas.filter({ $0.owner === p.viewport }) {
            XCTAssertTrue(area.options.contains(.activeInKeyWindow), "tracked in the key window")
            XCTAssertFalse(area.options.contains(.activeAlways), "and not in one behind it")
        }
        XCTAssertFalse(node.trackingAreas.isEmpty)
        // The pointer over the node, as the tracking area reports it in a key window.
        let point = node.convert(NSPoint(x: 20, y: 15), to: nil)
        p.trackPointer(try XCTUnwrap(NSEvent.mouseEvent(with: .mouseMoved, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                        windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 0, pressure: 0)))
        XCTAssertEqual(hovered, ["9+"])
        // The window stops being key: nothing in it is hovered any more.
        NotificationCenter.default.post(name: NSWindow.didResignKeyNotification, object: window)
        XCTAssertEqual(hovered, ["9+", "9-"])
        XCTAssertTrue(p.hoveredNodes.isEmpty)
        XCTAssertNil(p.restingPointer(), "and a layout under the pointer does not hover it again")
    }

    /// X80, re-read under LLP 1115.000 D7. While a mouse button is held the
    /// hover stays where it was: AppKit's tracking areas report nothing
    /// during a drag, and a browser's following hover is not the Mac's rule.
    /// When the button comes up, what is under the pointer is hovered with
    /// no further move, as AppKit reports an enter "on mouseUp after a drag"
    /// (it stayed on the pressed node until the pointer next moved).
    func testHoverHoldsThroughADragAndFollowsTheRelease() throws {
        let p = fixture()
        var hovered: [String] = []
        p.onHover = { id, over in hovered.append("\(id)\(over ? "+" : "-")") }
        let pressed = try XCTUnwrap(p.views[9]), other = try XCTUnwrap(p.views[10])
        func mouse(_ type: NSEvent.EventType, over node: NodeView) throws -> NSEvent {
            try XCTUnwrap(NSEvent.mouseEvent(with: type, location: node.convert(NSPoint(x: 20, y: 15), to: nil), modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                             windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: type == .mouseMoved ? 0 : 1, pressure: type == .leftMouseUp || type == .mouseMoved ? 0 : 1))
        }
        p.trackPointer(try mouse(.mouseMoved, over: pressed))
        XCTAssertEqual(hovered, ["9+"])
        pressed.mouseDown(with: try mouse(.leftMouseDown, over: pressed))
        XCTAssertEqual(p.pointerHeld, 9)
        pressed.mouseDragged(with: try mouse(.leftMouseDragged, over: other))
        p.trackPointer(try mouse(.mouseMoved, over: other)) // a move that did reach the page while the button is held changes nothing
        XCTAssertEqual(hovered, ["9+"], "held, the hover stays")
        // The button comes up over the other node. In a test the agent's pointer stands for the cursor.
        p.agentPointer = other.convert(NSPoint(x: 20, y: 15), to: nil)
        pressed.mouseUp(with: try mouse(.leftMouseUp, over: other))
        XCTAssertNil(p.pointerHeld)
        XCTAssertTrue(p.followPending, "the release asks for the pointer to be hit-tested at the next frame")
        p.hoverUnderPointer()
        XCTAssertEqual(hovered, ["9+", "9-", "10+"], "what is under the released pointer is hovered, with no further move")
    }
}
#endif

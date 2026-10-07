#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A mouse drag on a `reorderFor` handle, through AppKit's own mouse methods,
/// makes the web host's sequence: recognize past the slop, catch at zero
/// travel, follow, drop once, keep the pin until the source finishes.
final class ReorderMacTests: XCTestCase {
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil }

    /// A virtualized List (1) whose mounted row wrapper (2) holds a grip (3)
    /// with a text child (4), in a window.
    private func fixture(_ calls: ReorderRecorder) -> (Presenter, NodeView) {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        p.reorderCalls = calls
        p.apply(wireBatch([
            ["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0, "count": 50,
                "totalExtent": 2000, "rows": [["view": 2, "root": 3, "epoch": 7]], "correction": NSNull()]]],
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view", "props": ["reorderFor": "items"]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "grip"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 4, "x": 10.0, "y": 10.0, "w": 100.0, "h": 20.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]
        ]))
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
        return (p, p.views[4]!)
    }
    private func event(_ type: NSEvent.EventType, _ view: NSView, down: CGFloat, right: CGFloat = 0) -> NSEvent {
        // Window coordinates grow upward: a downward drag lowers y.
        let at = view.convert(NSPoint(x: view.bounds.midX, y: view.bounds.midY), to: nil)
        return NSEvent.mouseEvent(with: type, location: NSPoint(x: at.x + right, y: at.y - down), modifierFlags: [],
            timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window!.windowNumber, context: nil,
            eventNumber: 1, clickCount: 1, pressure: 1)!
    }

    func testAVerticalDragCatchesFollowsAndDropsOnceThenFinishes() throws {
        let calls = ReorderRecorder()
        let (p, text) = fixture(calls)
        text.mouseDown(with: event(.leftMouseDown, text, down: 0))
        text.mouseDragged(with: event(.leftMouseDragged, text, down: 3))
        XCTAssertEqual(calls.log, [], "inside the slop nothing is recognized")
        text.mouseDragged(with: event(.leftMouseDragged, text, down: 10))
        text.mouseDragged(with: event(.leftMouseDragged, text, down: 70))
        XCTAssertEqual(p.views[2]?.layer?.zPosition, 0.001, "the lifted row paints above later rows")
        text.mouseUp(with: event(.leftMouseUp, text, down: 70))
        XCTAssertEqual(calls.log, ["begin 3 top=0.0", "move 7 dy=0.0", "move 7 dy=60.0", "drop 7 dy=60.0"])
        let hold = try XCTUnwrap(p.reorder)
        XCTAssertEqual(hold.phase, .settling, "the pin stays while the source returns")
        XCTAssertEqual(p.collections.interaction, 3)
        hold.observe(ReorderRecorder.finished())
        XCTAssertNil(p.reorder)
        XCTAssertEqual(p.views[2]?.layer?.zPosition, 0)
        let released = expectation(description: "the pin retires after the finish")
        DispatchQueue.main.async { released.fulfill() }
        wait(for: [released], timeout: 1)
        XCTAssertNil(p.collections.interaction)
    }

    func testAHorizontalStartOrARefusedCatchIsNoArrange() {
        let calls = ReorderRecorder()
        let (p, text) = fixture(calls)
        text.mouseDown(with: event(.leftMouseDown, text, down: 0))
        text.mouseDragged(with: event(.leftMouseDragged, text, down: 1, right: 20))
        text.mouseDragged(with: event(.leftMouseDragged, text, down: 60, right: 20))
        text.mouseUp(with: event(.leftMouseUp, text, down: 60, right: 20))
        XCTAssertEqual(calls.log, [])
        calls.admit = false
        text.mouseDown(with: event(.leftMouseDown, text, down: 0))
        text.mouseDragged(with: event(.leftMouseDragged, text, down: 10))
        text.mouseDragged(with: event(.leftMouseDragged, text, down: 30))
        text.mouseUp(with: event(.leftMouseUp, text, down: 30))
        XCTAssertEqual(calls.log, ["begin 3 top=0.0"], "a refused catch sends nothing more")
        XCTAssertNil(p.reorder)
    }

    func testCancelEndsWithoutADropAndAStaleSettleCannotEndASuccessor() throws {
        let calls = ReorderRecorder()
        let (p, text) = fixture(calls)
        text.mouseDown(with: event(.leftMouseDown, text, down: 0))
        text.mouseDragged(with: event(.leftMouseDragged, text, down: 10))
        p.mouseReorder.cancel()
        XCTAssertEqual(calls.log, ["begin 3 top=0.0", "move 7 dy=0.0", "cancel 7"])
        let hold = try XCTUnwrap(p.reorder)
        hold.observe(ReorderState(["token": "8", "list": 1, "wrapper": 2, "phase": "finished"]))
        XCTAssertTrue(p.reorder === hold, "another contact's finish is not this one's")
    }
}
#endif

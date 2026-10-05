#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// The grouped session's calls, recorded (LLP 1094 D5): the drop answers
/// `holding` until the test says otherwise.
final class ReorderGroupRecorder: ReorderGroupCalls {
    var log: [String] = []
    var hold = true
    func state(_ phase: String, target: Int = 1, row: Int = 2, ending: String? = nil, dispatched: Bool = false) -> Batch {
        var op: [String: Any] = ["op": "reorder", "group": true, "token": "7", "list": 1, "wrapper": 2,
            "phase": phase, "target": target, "row": row, "dispatched": dispatched]
        op["ending"] = ending ?? NSNull()
        return wireBatch([op])
    }
    func groupBegin(_ handle: UInt32, scrollTop: Double, ghost: Bool, now: Double) -> Batch {
        log.append("begin \(handle)\(ghost ? " ghost" : "")")
        return state("active")
    }
    func moveInto(_ token: UInt64, target: UInt32, contentY: Double, scrollTop: Double, inside: Bool, now: Double) -> Batch {
        log.append("into \(target)")
        return state("active", target: Int(target))
    }
    func groupStep(_ token: UInt64, step: ReorderGroupStep, now: Double) -> Batch {
        log.append("step \(step.rawValue)")
        return state("active", target: 5)
    }
    func groupEnd(_ token: UInt64, drop: Bool, now: Double) -> Batch {
        log.append(drop ? "drop" : "cancel")
        if !drop { return state("cancelling", row: 2) }
        return hold ? state("holding", target: 5, dispatched: true)
            : state("settling", target: 5, row: 6, ending: "landed", dispatched: true)
    }
    func groupFinish(_ token: UInt64, now: Double) -> Batch {
        log.append("finish")
        return state("finished", target: 0, row: 0)
    }
}

/// A grouped grip on a Mac (LLP 1094 stage 4): a ghost that follows a drag
/// in any direction into another grouped list, a drop that holds with the
/// ghost kept, the hold's end landing it, and the keys.
final class ReorderGroupMacTests: XCTestCase {
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil }

    /// Two grouped lists side by side: 1 (row wrapper 2, grip 3, text 4)
    /// and 5 (row wrapper 6, grip 7).
    private func fixture(_ calls: ReorderGroupRecorder) -> (Presenter, NodeView) {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        p.reorderGroupCalls = calls
        let entry = { (list: Int, wrapper: Int, root: Int) -> [String: Any] in
            ["view": list, "revision": 1, "scrollSequence": 0, "count": 1, "totalExtent": 40,
             "rows": [["view": wrapper, "root": root, "epoch": 7]], "correction": NSNull()]
        }
        p.apply(wireBatch([
            ["op": "collections", "items": [entry(1, 2, 3), entry(5, 6, 7)]],
            ["op": "create", "id": 9, "kind": "view"],
            ["op": "create", "id": 1, "kind": "list", "props": ["id": "col-a", "reorderGroup": "cards"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view", "props": ["reorderFor": "col-a"]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "grip"]],
            ["op": "create", "id": 5, "kind": "list", "props": ["id": "col-b", "reorderGroup": "cards"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 6, "kind": "view"],
            ["op": "create", "id": 7, "kind": "view", "props": ["reorderFor": "col-b"]],
            ["op": "children", "id": 9, "ids": [1, 5]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 5, "ids": [6]],
            ["op": "children", "id": 6, "ids": [7]],
            ["op": "roots", "ids": [9]],
            ["op": "frame", "id": 9, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 150.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 150.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 150.0, "h": 40.0],
            ["op": "frame", "id": 4, "x": 10.0, "y": 10.0, "w": 100.0, "h": 20.0],
            ["op": "frame", "id": 5, "x": 200.0, "y": 0.0, "w": 150.0, "h": 200.0],
            ["op": "frame", "id": 6, "x": 0.0, "y": 0.0, "w": 150.0, "h": 40.0],
            ["op": "frame", "id": 7, "x": 0.0, "y": 0.0, "w": 150.0, "h": 40.0],
            ["op": "content", "id": 1, "w": 150.0, "h": 200.0],
            ["op": "content", "id": 5, "w": 150.0, "h": 200.0]
        ]))
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
        return (p, p.views[4]!)
    }
    private func event(_ type: NSEvent.EventType, _ view: NSView, down: CGFloat, right: CGFloat = 0) -> NSEvent {
        let at = view.convert(NSPoint(x: view.bounds.midX, y: view.bounds.midY), to: nil)
        return NSEvent.mouseEvent(with: type, location: NSPoint(x: at.x + right, y: at.y - down), modifierFlags: [],
            timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window!.windowNumber, context: nil,
            eventNumber: 1, clickCount: 1, pressure: 1)!
    }

    func testStateReadsOnlyAGroupedOp() {
        let op: [String: Any] = ["op": "reorder", "group": true, "token": "18446744073709551615", "list": 1, "wrapper": 2,
            "phase": "settling", "ending": "timeout", "target": 5, "row": 6, "dispatched": true]
        let state = ReorderGroupState(op)
        XCTAssertEqual(state?.token, UInt64.max)
        XCTAssertEqual(state?.ending, "timeout")
        XCTAssertEqual(state?.row, 6)
        var plain = op; plain["group"] = nil
        XCTAssertNil(ReorderGroupState(plain), "Arrange's own op is not the group's")
        XCTAssertEqual(ReorderGroupStep(key: "ArrowRight"), .nextList)
        XCTAssertEqual(ReorderGroupStep.actions.map(\.0), ["Move earlier", "Move later", "Move to previous list", "Move to next list"])
    }

    func testADragInAnyDirectionLiftsAGhostThatHoldsUntilTheMoveShows() throws {
        let calls = ReorderGroupRecorder()
        let (p, text) = fixture(calls)
        text.mouseDown(with: event(.leftMouseDown, text, down: 0))
        text.mouseDragged(with: event(.leftMouseDragged, text, down: 0, right: 3))
        XCTAssertEqual(calls.log, [], "inside the slop nothing is recognized")
        text.mouseDragged(with: event(.leftMouseDragged, text, down: 0, right: 20))
        XCTAssertEqual(calls.log.first, "begin 3 ghost", "a sideways move lifts a grouped grip")
        let hold = try XCTUnwrap(p.reorderGroup)
        let ghost = try XCTUnwrap(hold.ghost)
        XCTAssertTrue(ghost.view.superview != nil, "the ghost is in the window's top layer")
        // Over the second list: its port takes the ghost's centre.
        text.mouseDragged(with: event(.leftMouseDragged, text, down: 10, right: 220))
        XCTAssertEqual(calls.log.last, "into 5")
        text.mouseUp(with: event(.leftMouseUp, text, down: 10, right: 220))
        XCTAssertEqual(calls.log.suffix(2), ["into 5", "drop"])
        XCTAssertEqual(hold.state.phase, "holding")
        XCTAssertTrue(p.reorderGroup === hold, "a hold keeps the ghost and the pin")
        XCTAssertNotNil(hold.ghost?.view.superview)
        // Escape does nothing while holding: its send is out.
        p.mouseReorder.cancel()
        XCTAssertFalse(calls.log.contains("cancel"))
        // The answer showed the move: the ghost lands on the row, then the session finishes.
        let landed = expectation(description: "the ghost lands, then the session finishes")
        hold.observe(ReorderGroupState(calls.state("settling", target: 5, row: 6, ending: "landed", dispatched: true).ops[0].payload))
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) { landed.fulfill() }
        wait(for: [landed], timeout: 2)
        XCTAssertEqual(calls.log.last, "finish")
        XCTAssertNil(p.reorderGroup)
        XCTAssertNil(ghost.view.superview, "the ghost is gone")
    }

    func testTheKeysLiftStepAndDropWithoutAGhost() throws {
        let calls = ReorderGroupRecorder()
        calls.hold = false
        let (p, _) = fixture(calls)
        let grip = try XCTUnwrap(p.views[3])
        XCTAssertTrue(grip.reorderKeys)
        XCTAssertTrue(grip.acceptsFirstResponder, "a grouped grip is in the key view loop")
        XCTAssertTrue(grip.reorderKey(" "))
        XCTAssertNil(p.reorderGroup?.ghost)
        XCTAssertTrue(grip.reorderKey("ArrowRight"))
        XCTAssertTrue(grip.reorderKey("Enter"))
        XCTAssertEqual(calls.log, ["begin 3", "step 4", "drop", "finish"], "no ghost: it finishes as the hold ends")
        XCTAssertNil(p.reorderGroup)
        // A custom action is the same, in one turn.
        calls.log = []
        let actions = try XCTUnwrap(grip.accessibilityCustomActions())
        XCTAssertEqual(actions.map(\.name), ["Move earlier", "Move later", "Move to previous list", "Move to next list"])
        XCTAssertTrue(grip.reorderAction(.later))
        XCTAssertEqual(calls.log, ["begin 3", "step 2", "drop", "finish"])
    }
}
#endif

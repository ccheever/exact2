#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class CollectionMacTests: XCTestCase {
    private func snapshot(revision: Int = 1, correction: [String: Any]? = nil) -> [String: Any] {
        ["view": 1, "revision": revision, "scrollSequence": 0, "count": 100,
         "totalExtent": 3000, "rows": [["view": 2, "root": 3, "epoch": 7]],
         "correction": correction as Any? ?? NSNull()]
    }
    private func batch(_ ops: [[String: Any]]) -> Batch {
        Batch(ops: ops, timers: false, motion: false, clock: nil, error: nil)
    }
    private func fixture() -> (Presenter, NodeView) {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 900, height: 700)
        p.apply(batch([
            ["op": "collections", "items": [snapshot()]],
            ["op": "create", "id": 1, "kind": "list"],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "style", "id": 1, "style": ["overflow_y": "scroll", "padding_left": 10.0, "padding_right": 10.0, "padding_top": 20.0]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 20.0, "w": 280.0, "h": 72.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 3020.0]
        ]))
        return (p, p.views[1]!)
    }
    func testMeasuresActualNestedClipViewWithoutAuthoredScrollHandler() throws {
        let (p, list) = fixture()
        defer { p.collections.reset() }
        XCTAssertTrue(list.handlers.isEmpty)
        let clip = try XCTUnwrap(list.scroll?.contentView)
        clip.scroll(to: NSPoint(x: 0, y: 120))
        let facts = try XCTUnwrap(p.collections.geometry(1))
        XCTAssertEqual(facts.top, 100)
        XCTAssertEqual(facts.portWidth, Double(clip.bounds.width))
        XCTAssertEqual(facts.portHeight, Double(clip.bounds.height))
        XCTAssertEqual(facts.rowWidth, 280)
        XCTAssertNotEqual(facts.portHeight, Double(p.viewportSize.height))
        XCTAssertEqual(p.collections.height(2), 72)
        var feedback: [Data] = []
        p.collections.onFeedback = { feedback.append($0) }
        p.collections.flush()
        XCTAssertEqual(feedback.count, 1)
        XCTAssertEqual(feedback[0].count, 88)
        p.collections.changed(1)
        p.collections.flush()
        XCTAssertEqual(feedback.count, 1, "identical layout must not reenter Rust")
    }
    func testCorrectionLandsBeforeDeferredAuthoredEvent() throws {
        let (p, list) = fixture()
        defer { p.collections.reset() }
        let clip = try XCTUnwrap(list.scroll?.contentView)
        var received = false
        p.onViewportFit = { p.key(1, "x") }
        p.onKey = { _, _ in
            received = true
            XCTAssertEqual(clip.bounds.minY, 220)
        }
        p.apply(batch([
            ["op": "collections", "items": [snapshot(revision: 2, correction: ["scrollSequence": 0, "scrollTop": 200])]],
            ["op": "props", "id": 1, "set": ["viewportFit": "cover"]]
        ]))
        XCTAssertTrue(received)
        XCTAssertEqual(clip.bounds.minY, 220)
        p.collections.userIntent(1)
        p.apply(batch([["op": "collections", "items": [snapshot(revision: 3,
            correction: ["scrollSequence": 0, "scrollTop": 900])]]]))
        XCTAssertEqual(clip.bounds.minY, 220, "stale correction must not replace newer user intent")
    }
    func testClearedSnapshotRetiresStateAndObserver() {
        let (p, _) = fixture()
        p.collections.pointer(3)
        p.apply(batch([["op": "collections", "items": []]]))
        XCTAssertTrue(p.collections.entries.isEmpty)
        XCTAssertNil(p.collections.interaction)
        XCTAssertNil(p.collections.stopTracking)
    }

    func testResponderMousePathPinsWithoutDependingOnApplicationEventMonitor() throws {
        let (p, list) = fixture()
        defer { p.collections.reset() }
        let row = try XCTUnwrap(p.views[3])
        row.handlers = ["press"]
        let event = try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseDown, location: .zero,
            modifierFlags: [], timestamp: 0, windowNumber: 0, context: nil,
            eventNumber: 1, clickCount: 1, pressure: 1))
        row.mouseDown(with: event)
        XCTAssertEqual(p.collections.interaction, 3)
        list.mouseDown(with: event)
        XCTAssertEqual(p.collections.interaction, 3, "bubbling must keep the original descendant")
    }

    func testNestedCollectionsOwnOnlyNearestPinsAndReleaseBeforeTransfer() throws {
        let (p, _) = fixture()
        defer { p.collections.reset() }
        let inner: [String: Any] = ["view": 4, "revision": 1, "scrollSequence": 0,
            "totalExtent": 1000, "rows": [["view": 5, "root": 6, "epoch": 8]]]
        p.apply(batch([
            ["op": "collections", "items": [snapshot(), inner]],
            ["op": "create", "id": 4, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 5, "kind": "view"],
            ["op": "create", "id": 6, "kind": "button", "handlers": ["press"]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 5, "ids": [6]],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 240.0, "h": 100.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 240.0, "h": 40.0],
            ["op": "frame", "id": 6, "x": 0.0, "y": 0.0, "w": 240.0, "h": 40.0],
            ["op": "content", "id": 4, "w": 240.0, "h": 1000.0]
        ]))
        XCTAssertEqual(p.collections.owningCollection(6), 4)
        XCTAssertEqual(p.collections.owningCollection(3), 1)
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        defer { window.makeFirstResponder(nil); window.close() }
        var accepted: [UInt32: (UInt32, UInt32)] = [:]
        func u32(_ bytes: Data, _ offset: Int) -> UInt32 {
            (0..<4).reduce(0) { $0 | UInt32(bytes[offset + $1]) << ($1 * 8) }
        }
        p.collections.onFeedback = { bytes in
            accepted[u32(bytes, 4)] = (u32(bytes, 56), u32(bytes, 60))
            XCTAssertLessThanOrEqual(accepted.values.filter { $0.0 != 0 }.count, 1)
            XCTAssertLessThanOrEqual(accepted.values.filter { $0.1 != 0 }.count, 1)
        }
        func drain() {
            let done = expectation(description: "pin feedback continuation")
            DispatchQueue.main.async { DispatchQueue.main.async { done.fulfill() } }
            wait(for: [done], timeout: 1)
        }
        XCTAssertTrue(window.makeFirstResponder(p.views[6]!))
        p.collections.pointer(6); drain()
        XCTAssertEqual(accepted[4]?.0, 6)
        XCTAssertEqual(accepted[4]?.1, 6)
        XCTAssertEqual(accepted[1]?.0, 0)
        XCTAssertEqual(accepted[1]?.1, 0)
        // Outer id sorts before inner id: clearing must override normal order.
        p.collections.pointer(3); drain()
        XCTAssertEqual(accepted[4]?.1, 0)
        XCTAssertEqual(accepted[1]?.1, 3)
        XCTAssertEqual(accepted[4]?.0, 6, "focus and interaction stay independent")
        p.collections.beginBatch(batch([]))
        p.views[3]!.handlers = ["press"]
        XCTAssertTrue(window.makeFirstResponder(p.views[3]!))
        p.collections.pointer(6)
        p.collections.endBatch(); p.collections.flush(); drain()
        XCTAssertEqual(accepted[1]?.0, 3)
        XCTAssertEqual(accepted[4]?.1, 6)
        p.collections.pointer(nil); window.makeFirstResponder(nil)
        p.collections.pinsChanged(); drain()
        XCTAssertTrue(accepted.values.allSatisfy { $0.0 == 0 && $0.1 == 0 })
    }

    func testOldReleaseDoesNotClearNewContactOnSameRow() {
        let (p, _) = fixture()
        defer { p.collections.reset() }
        p.collections.pointer(3)
        p.collections.releaseInteractionLater()
        p.collections.pointer(3)
        let done = expectation(description: "after old release callback")
        DispatchQueue.main.async {
            XCTAssertEqual(p.collections.interaction, 3)
            done.fulfill()
        }
        wait(for: [done], timeout: 1)
    }
}
#endif

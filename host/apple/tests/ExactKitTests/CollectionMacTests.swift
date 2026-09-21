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
    private func fixture(collection: Bool = true, estimatedItemHeight: String? = nil,
                         configure: (Presenter) -> Void = { _ in }) -> (Presenter, NodeView) {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 900, height: 700)
        configure(p)
        p.apply(batch([
            ["op": "collections", "items": collection ? [snapshot()] : []],
            ["op": "create", "id": 1, "kind": "list",
             "props": estimatedItemHeight.map { ["estimatedItemHeight": $0] } ?? [:]],
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
        var legacyReports: [UInt32] = []
        let (p, list) = fixture(estimatedItemHeight: "24") { p in
            p.onList = { id, _, _, _, _, _, _, _ in
                legacyReports.append(id)
                return false
            }
        }
        defer { p.collections.reset() }
        XCTAssertTrue(p.collections.owns(list.id))
        XCTAssertTrue(legacyReports.isEmpty, "collection ownership must precede initial legacy reporting")
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
        XCTAssertTrue(legacyReports.isEmpty, "collection geometry uses only common feedback")
    }
    func testOrdinaryListStillReportsLegacyGeometryOnInitialApply() {
        var legacyReports: [UInt32] = []
        let (p, list) = fixture(collection: false, estimatedItemHeight: "24") { p in
            p.onList = { id, _, _, _, _, _, _, _ in
                legacyReports.append(id)
                return false
            }
        }
        defer { p.collections.reset() }
        XCTAssertFalse(p.collections.owns(list.id))
        XCTAssertEqual(legacyReports, [list.id])
    }
    func testSharedCollectionResizeUsesOnlyRevisionedFeedback() {
        let (p, _) = fixture()
        defer { p.collections.reset() }
        var legacyReports = 0
        var collectionReports = 0
        p.onList = { _, _, _, _, _, _, _, _ in legacyReports += 1; return false }
        p.collections.onFeedback = { _ in collectionReports += 1 }
        p.apply(batch([
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 240.0]
        ]))
        p.collections.flush()
        XCTAssertEqual(legacyReports, 0, "the legacy window protocol rejects a shared collection tree")
        XCTAssertEqual(collectionReports, 1, "the resized collection still reports actual geometry")
    }

    func testFeedbackMembershipCommitsContinueOnLaterTurnsWithoutScroll() {
        let (p, _) = fixture()
        defer { p.collections.reset() }
        let finished = expectation(description: "new row epochs measured after two passes")
        var reports = 0
        p.collections.onFeedback = { _ in
            reports += 1
            if reports < 7 {
                var next = self.snapshot(revision: reports + 1)
                next["rows"] = [["view": 2, "root": 3, "epoch": reports + 7]]
                p.apply(self.batch([["op": "collections", "items": [next]]]))
            } else { finished.fulfill() }
        }
        p.collections.flush()
        XCTAssertEqual(reports, 2, "two reports per turn, never recursive")
        wait(for: [finished], timeout: 2)
        XCTAssertEqual(reports, 7)
    }
    func testActivationRetriesUnchangedFactsOnce() {
        let (p, _) = fixture()
        defer { p.collections.reset() }
        var reports = 0
        p.collections.onFeedback = { _ in reports += 1 }
        p.collections.flush()
        XCTAssertEqual(reports, 1)
        p.collections.dataReady()
        p.collections.flush()
        XCTAssertEqual(reports, 2)
        p.collections.changed(1)
        p.collections.flush()
        XCTAssertEqual(reports, 2)
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

    func testNativeUpCannotReleasePinBeforeDeferredGestureCompletion() {
        let (p, _) = fixture()
        defer { p.collections.reset() }
        let lease = p.collections.holdPointer(3)
        p.collections.releaseInteractionLater()
        let done = expectation(description: "gesture completion owns release")
        DispatchQueue.main.async {
            XCTAssertEqual(p.collections.interaction, 3)
            p.collections.releaseInteractionLater(ifCurrent: lease)
            DispatchQueue.main.async {
                XCTAssertNil(p.collections.interaction)
                done.fulfill()
            }
        }
        wait(for: [done], timeout: 1)
    }

    func testStaleGestureCompletionCannotClearAlreadyReplacedPin() {
        let (p, _) = fixture()
        defer { p.collections.reset() }
        let old = p.collections.holdPointer(3)
        let current = p.collections.holdPointer(2)
        p.collections.releaseInteractionLater(ifCurrent: old)
        let done = expectation(description: "old completion after pin transfer")
        DispatchQueue.main.async {
            XCTAssertEqual(p.collections.interaction, 2)
            p.collections.releaseInteractionLater(ifCurrent: current)
            DispatchQueue.main.async {
                XCTAssertNil(p.collections.interaction)
                done.fulfill()
            }
        }
        wait(for: [done], timeout: 1)
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
    func testBudgetPendingReportsSameGeometryWithoutRecursiveAdmission() {
        let (p, list) = fixture(collection: false)
        defer { p.reset() }
        list.props["estimatedItemHeight"] = "24"
        var limits: [UInt32] = []
        p.onList = { _, _, _, _, _, _, _, limit in
            limits.append(limit)
            // Actual delivery reenters apply; its finalization must not admit
            // another report and multiply this slice's budget.
            p.apply(self.batch([]))
            return true
        }
        p.syncLists(limit: 2)
        XCTAssertEqual(limits, [2])
        p.pump()
        XCTAssertTrue(limits.allSatisfy { $0 == 2 }, "every pump report admits only one overscan row")
        let afterList = limits.count
        p.requestTextPublication()
        p.pump()
        XCTAssertEqual(limits.count, afterList, "pending list work leaves a slice for text publication")
        p.pump()
        XCTAssertTrue(limits.dropFirst(afterList).allSatisfy { $0 == 2 })
        // A descheduled test can exhaust the entire deadline before admission.
        // Pending demand must survive that empty slice and settle synchronously.
        p.settlePump()
        XCTAssertGreaterThan(limits.count, afterList)
        XCTAssertEqual(limits.last, 0)
        let afterSecondList = limits.count
        p.reset()
        p.pump()
        XCTAssertEqual(limits.count, afterSecondList, "reset clears pending list identities and the pump flag")
    }

    func testNativeViewportCorrectionRetriesOnlyUncoveredPixelsWithoutOverscan() {
        for covered in [false, true] {
            let (p, list) = fixture(collection: false)
            defer { p.reset() }
            p.apply(batch([
                ["op": "frame", "id": 2, "x": 10.0, "y": 20.0, "w": 280.0, "h": 3000.0],
                ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 280.0, "h": covered ? 1000.0 : 300.0]
            ]))
            list.props["estimatedItemHeight"] = "24"
            var limits: [UInt32] = []
            var tops: [Double] = []
            p.onList = { _, top, _, _, _, _, _, limit in
                limits.append(limit); tops.append(top)
                if limits.count == 1 {
                    // A report can cause this through native anchoring/clamping
                    // while its nested viewport notification is suppressed.
                    list.scroll!.contentView.scroll(to: NSPoint(x: 0, y: 600))
                } else {
                    p.apply(self.batch([
                        ["op": "frame", "id": 3, "x": 0.0, "y": 580.0, "w": 280.0, "h": 300.0]
                    ]))
                }
                return false
            }
            p.syncLists(limit: 2)
            XCTAssertEqual(limits, covered ? [2] : [2, 1])
            if !covered { XCTAssertEqual(tops.last, 600) }
        }
    }

    func testScrollOnlyReportsVisibleRowsAndRefreshDeterminesSliceBudget() {
        XCTAssertEqual(Presenter.listSliceBudget(1.0 / 60), 0.004, accuracy: 0.000001)
        XCTAssertEqual(Presenter.listSliceBudget(1.0 / 120), 0.002, accuracy: 0.000001)
        let (p, list) = fixture(collection: false)
        defer { p.reset() }
        list.props["estimatedItemHeight"] = "24"
        var limits: [UInt32] = []
        p.onList = { _, _, _, _, _, _, _, limit in limits.append(limit); return true }
        p.scrolled()
        XCTAssertEqual(limits, [1], "uncovered user scroll never fills overscan synchronously")
    }

    func testSynchronousSettlementUsesUnlimitedReportsAndRetirementClearsPending() {
        let (p, list) = fixture(collection: false)
        defer { p.reset() }
        list.props["estimatedItemHeight"] = "24"
        var limits: [UInt32] = []
        p.onList = { _, _, _, _, _, _, _, limit in
            limits.append(limit)
            return limits.count < 3
        }
        p.syncLists(limit: 2)
        p.settlePump()
        XCTAssertEqual(limits, [2, 0, 0], "agent settlement drains pending reports without an overscan budget")
        p.pump()
        XCTAssertEqual(limits, [2, 0, 0])
        p.onList = { _, _, _, _, _, _, _, limit in limits.append(limit); return true }
        list.scroll!.contentView.scroll(to: NSPoint(x: 0, y: 200))
        p.syncLists(limit: 2)
        let beforeRetirement = limits.count
        p.apply(batch([["op": "destroy", "id": Int(list.id)]]))
        p.pump()
        XCTAssertEqual(limits.count, beforeRetirement, "destroyed lists cannot retain a pending report")
    }

    func testCommonOwnershipRetiresLegacyPendingWorkAndGeometry() {
        for duringReport in [false, true] {
            let (p, list) = fixture(collection: false)
            defer { p.reset() }
            list.props["estimatedItemHeight"] = "24"
            var reports = 0
            let takeover = batch([["op": "collections", "items": [snapshot()]]])
            p.onList = { _, _, _, _, _, _, _, _ in
                reports += 1
                if duringReport && reports == 1 { p.apply(takeover) }
                return true
            }
            p.syncLists(limit: 2)
            if !duringReport { p.apply(takeover) }
            XCTAssertTrue(p.collections.owns(list.id))
            p.pump(); p.pump()
            XCTAssertEqual(reports, 1, "common ownership must stop legacy continuations")
            // Remove common ownership without apply's automatic legacy sync,
            // so an obsolete pump continuation is observable independently.
            p.collections.beginBatch(batch([["op": "collections", "items": []]]))
            p.collections.endBatch()
            p.pump()
            XCTAssertEqual(reports, 1, "the previous owner's pending work was retired")
            p.syncLists(limit: 2)
            XCTAssertEqual(reports, 2, "returning legacy ownership must report even unchanged geometry")
        }
    }

    func testPrependingANewNativeChildKeepsRetainedSiblingsMounted() throws {
        for decorated in [false, true] {
            _ = NSApplication.shared
            let p = Presenter()
            defer { p.reset() }
            p.apply(batch([
                ["op": "create", "id": 1, "kind": "view", "style": ["overflow_y": "scroll"]],
                ["op": "create", "id": 2, "kind": "view"],
                ["op": "create", "id": 3, "kind": "view"],
                ["op": "create", "id": 4, "kind": "text", "props": ["text": "new row"]]
            ]))
            let parent = try XCTUnwrap(p.views[1]), first = try XCTUnwrap(p.views[2])
            let second = try XCTUnwrap(p.views[3]), added = try XCTUnwrap(p.views[4])
            let container = MountObservedView()
            try XCTUnwrap(parent.scroll).documentView = container
            p.apply(batch([["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]]]))
            let decoration = NSView()
            if decorated { container.addSubview(decoration, positioned: .below, relativeTo: second) }
            container.added.removeAll(); container.removed.removeAll()
            p.apply(batch([["op": "children", "id": 1, "ids": [4, 2, 3]]]))
            XCTAssertEqual(container.subviews.compactMap { ($0 as? NodeView)?.id }, [4, 2, 3])
            XCTAssertTrue(first.superview === container && second.superview === container)
            XCTAssertTrue(added.wantsLayer, "inserting at the front still prepares native text backing")
            if decorated {
                XCTAssertTrue(container.subviews.first === decoration, "preserve the mixed native-decoration path")
            } else {
                XCTAssertEqual(container.added, [4], "mount only the new child")
                XCTAssertEqual(container.removed, [], "retained rows must never detach during prepend")
                container.added.removeAll()
                p.apply(batch([["op": "children", "id": 1, "ids": [2, 3]]]))
                XCTAssertEqual(container.removed, [4], "removing the inserted row leaves surviving siblings mounted")
                XCTAssertEqual(container.added, [])
                XCTAssertEqual(container.subviews.compactMap { ($0 as? NodeView)?.id }, [2, 3])
            }
        }
    }

}

private final class MountObservedView: NSView {
    var added: [UInt32] = []
    var removed: [UInt32] = []
    override func didAddSubview(_ subview: NSView) {
        super.didAddSubview(subview)
        if let node = subview as? NodeView { added.append(node.id) }
    }
    override func willRemoveSubview(_ subview: NSView) {
        if let node = subview as? NodeView { removed.append(node.id) }
        super.willRemoveSubview(subview)
    }
}
#endif

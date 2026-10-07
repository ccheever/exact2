#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1070.000 §6.2 on AppKit, as on UIKit: a smooth correction is the
/// clip view's animated scroll; while it runs the list reports where it is
/// headed, a further one is held and taken when it lands, and an ordinary
/// correction stops it.
final class SmoothCollectionMacTests: XCTestCase {
    private var window: NSWindow?
    override func tearDown() { window?.orderOut(nil); window = nil; super.tearDown() }
    private func batch(_ ops: [[String: Any]]) -> Batch {
        batchFixture(ops: ops, timers: false, motion: false, clock: nil, error: nil)
    }
    private func collections(revision: Int, sequence: UInt64, correction: Any) -> [[String: Any]] {
        [["op": "collections", "items": [["view": 1, "revision": revision, "scrollSequence": 0, "count": 50, "totalExtent": 2000,
            "rows": [["view": 10, "root": 10, "epoch": 1]], "correction": correction]]]]
    }
    private func fixture() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = p.viewport
        self.window = window
        p.apply(batch(collections(revision: 1, sequence: 0, correction: NSNull()) + [
            ["op": "create", "id": 1, "kind": "list"], ["op": "style", "id": 1, "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 10, "kind": "view"],
            ["op": "children", "id": 1, "ids": [10]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]]))
        return p
    }
    private func sequence(_ p: Presenter) -> String { String(p.collections.entries[1]?.cursor.sequence ?? 0) }

    func testASmoothCorrectionAnimatesReportsItsDestinationAndHoldsTheNext() throws {
        let p = fixture()
        defer { p.collections.reset() }
        let clip = try XCTUnwrap(p.views[1]?.scroll?.contentView)
        p.apply(batch(collections(revision: 2, sequence: 0, correction: ["scrollSequence": sequence(p), "offset": 1200, "smooth": true])))
        XCTAssertTrue(p.collections.animating.contains(1), "AppKit animates to it")
        XCTAssertEqual(try XCTUnwrap(p.collections.geometry(1)).offset, 1200, accuracy: 0.5, "the runner plans from where it is headed")
        p.apply(batch(collections(revision: 3, sequence: 0, correction: ["scrollSequence": sequence(p), "offset": 1500, "smooth": true])))
        XCTAssertEqual(p.collections.owedTargets[1]?.y, 1500, "held, not restarted")
        // An ordinary correction stops it where it is told.
        p.apply(batch(collections(revision: 4, sequence: 0, correction: ["scrollSequence": sequence(p), "offset": 300])))
        XCTAssertFalse(p.collections.animating.contains(1))
        XCTAssertNil(p.collections.owedTargets[1])
        XCTAssertEqual(clip.bounds.minY, 300, accuracy: 0.5)
    }

    /// An ordinary correction to where the list already is still stops a
    /// running animation; a stale continuation for a stopped animation does
    /// not end the one begun after it.
    func testAStoppedAnimationsCallbacksDoNotEndTheNextOne() throws {
        let p = fixture()
        defer { p.collections.reset() }
        let clip = try XCTUnwrap(p.views[1]?.scroll?.contentView)
        let at = clip.bounds.minY
        p.apply(batch(collections(revision: 2, sequence: 0, correction: ["scrollSequence": sequence(p), "offset": 1200, "smooth": true])))
        p.apply(batch(collections(revision: 3, sequence: 0, correction: ["scrollSequence": sequence(p), "offset": 1500, "smooth": true])))
        // The first animation ends with a target owed: its continuation is
        // queued for the next turn.
        p.collections.animationEnded(1)
        p.apply(batch(collections(revision: 4, sequence: 0, correction: ["scrollSequence": sequence(p), "offset": Double(at)])))
        XCTAssertFalse(p.collections.animating.contains(1), "stopped, though already there")
        p.apply(batch(collections(revision: 5, sequence: 0, correction: ["scrollSequence": sequence(p), "offset": 900, "smooth": true])))
        XCTAssertTrue(p.collections.animating.contains(1))
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertTrue(p.collections.animating.contains(1) || clip.bounds.minY == 900, "the stale continuation left the new animation alone")
        XCTAssertNil(p.collections.owedTargets[1])
    }

    func testAnOrdinaryCorrectionIsSetAtOnce() throws {
        let p = fixture()
        defer { p.collections.reset() }
        let clip = try XCTUnwrap(p.views[1]?.scroll?.contentView)
        p.apply(batch(collections(revision: 2, sequence: 0, correction: ["scrollSequence": sequence(p), "offset": 900])))
        XCTAssertFalse(p.collections.animating.contains(1))
        XCTAssertEqual(clip.bounds.minY, 900, accuracy: 0.5)
    }
}
#endif

#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1050.000 stage 1 on AppKit: a collection's scroll callback builds only
/// what shows when a gap shows, defers the rest to the pump, and responsive
/// scrolling may carry only the rows it has built (D5).
final class CollectionFillMacTests: XCTestCase {
    private func batch(_ ops: [[String: Any]]) -> Batch {
        batchFixture(ops: ops, timers: false, motion: false, clock: nil, error: nil)
    }
    /// Rows of 72 points from y = 20, one per view id 2, 3, 4…, in a 200-point port.
    private func rows(_ count: Int, revision: Int) -> [[String: Any]] {
        var ops: [[String: Any]] = [["op": "collections", "items": [[
            "view": 1, "revision": revision, "scrollSequence": 0, "count": 100, "totalExtent": 7200, "pending": false,
            "rows": (0..<count).map { ["view": 2 + $0, "root": 2 + $0, "epoch": 1, "index": $0] },
            "correction": NSNull()]]]]
        for i in 0..<count {
            ops.append(["op": "create", "id": 2 + i, "kind": "view"])
            ops.append(["op": "frame", "id": 2 + i, "x": 10.0, "y": 20.0 + 72.0 * Double(i), "w": 280.0, "h": 72.0])
        }
        ops.append(["op": "children", "id": 1, "ids": (0..<count).map { 2 + $0 }])
        return ops
    }
    private func fixture(rows count: Int) -> (Presenter, NodeView, FlippedView) {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 900, height: 700)
        p.apply(batch([
            ["op": "create", "id": 1, "kind": "list"],
            ["op": "style", "id": 1, "style": ["overflow_y": "scroll", "overscroll_behavior_y": "contain", "padding_top": 20.0]]
        ] + rows(count, revision: 1) + [
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 7220.0]
        ]))
        let list = p.views[1]!
        return (p, list, list.scroll!.documentView as! FlippedView)
    }
    private func limit(_ wire: Data) -> UInt32 {
        wire.subdata(in: 72..<76).withUnsafeBytes { $0.loadUnaligned(as: UInt32.self) }
    }

    func testResponsiveScrollingIsPreparedOnlyAsFarAsTheBuiltRows() {
        let (p, _, document) = fixture(rows: 4)
        defer { p.collections.reset() }
        // Padding above the first item counts; the fourth row ends at 308.
        XCTAssertEqual(p.collections.preparedCover(1), NSRect(x: 0, y: 0, width: document.bounds.width, height: 308))
        document.prepareContent(in: NSRect(x: 0, y: 0, width: 300, height: 1000))
        XCTAssertEqual(document.preparedContentRect.maxY, 308, "AppKit pauses where the built rows end")
        // A batch that builds more widens what AppKit asked for, up to them.
        p.apply(batch(rows(6, revision: 2)))
        XCTAssertEqual(document.preparedContentRect.maxY, 452)
        // A plain scroll document keeps AppKit's own answer.
        let plain = FlippedView(frame: NSRect(x: 0, y: 0, width: 100, height: 1000))
        plain.prepareContent(in: NSRect(x: 0, y: 0, width: 100, height: 600))
        XCTAssertEqual(plain.preparedContentRect.maxY, 600)
    }

    func testCoveredScrollDefersToThePumpAndAGapIsRescuedWithSlack() throws {
        let (p, list, _) = fixture(rows: 4)
        defer { p.collections.reset() }
        var wires: [Data] = []
        p.collections.onFeedback = { wires.append($0) }
        let clip = try XCTUnwrap(list.scroll?.contentView)
        // 60…260 is inside 0…308: nothing is built in the callback.
        clip.scroll(to: NSPoint(x: 0, y: 60))
        XCTAssertTrue(wires.isEmpty)
        XCTAssertTrue(p.collections.covers(1))
        XCTAssertEqual(p.collections.fillPending, [1])
        // A slice builds its limit.
        p.collections.fillSlice(1, limit: 5)
        XCTAssertEqual(wires.count, 1)
        XCTAssertEqual(limit(wires[0]), 5)
        // 200…400 shows a spacer: the rows it shows now, and two more.
        clip.scroll(to: NSPoint(x: 0, y: 200))
        XCTAssertFalse(p.collections.covers(1))
        XCTAssertEqual(wires.count, 2)
        XCTAssertEqual(limit(wires[1]), 2)
    }

    func testTravelLeadsTheSliceByTheRowsItWillUncover() {
        let (p, _, _) = fixture(rows: 4)
        defer { p.collections.reset() }
        // At the 60-point offset the port reaches 260 of the cover's 308;
        // 200 points more travel needs ceil(152 / 72) = 3 rows.
        p.views[1]!.scroll!.contentView.scroll(to: NSPoint(x: 0, y: 60))
        XCTAssertEqual(p.collections.rowsToCover(1, ahead: 200), 3)
        XCTAssertEqual(p.collections.rowsToCover(1, ahead: -200), 0, "the first item is built")
        XCTAssertEqual(p.collections.rowsToCover(1, ahead: 0), 0)
    }
}
#endif

#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A plain AppKit scroller keeps the reader's content still when a box above
/// the port changes size or is inserted, as CSS scroll anchoring does in
/// Chrome (#138 X23d): a header folding 48 → 0 above row 20 left row 20 where
/// it was on the web and moved it up 48 points here.
final class ScrollAnchoringMacTests: XCTestCase {
    /// A 200-point scroller: a 48-point header (id 2), then twenty 30-point
    /// rows (ids 10…29), scrolled to `top`.
    private func fixture(top: CGFloat, followEnd: Bool = false) -> (Presenter, NSClipView) {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 400)
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "view", "props": followEnd ? ["scrollFollowEnd": "true"] : [:], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view", "style": ["height": 48.0]],
        ]
        for i in 0..<20 { ops.append(["op": "create", "id": 10 + i, "kind": "view", "style": ["height": 30.0]]) }
        ops += [
            ["op": "children", "id": 1, "ids": [2] + Array(10..<30)],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
        ]
        ops += rows(header: 48)
        p.apply(wireBatch(ops))
        let clip = p.views[1]!.scroll!.contentView
        clip.scroll(to: NSPoint(x: 0, y: top))
        return (p, clip)
    }
    private func rows(header: Double, inserted: Double = 0) -> [[String: Any]] {
        var ops: [[String: Any]] = [["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": header]]
        for i in 0..<20 { ops.append(["op": "frame", "id": 10 + i, "x": 0.0, "y": header + inserted + Double(30 * i), "w": 300.0, "h": 30.0]) }
        ops.append(["op": "content", "id": 1, "w": 300.0, "h": header + inserted + 600])
        return ops
    }
    /// Row `id`'s top in the port.
    private func y(_ p: Presenter, _ id: UInt32) -> CGFloat {
        let row = p.views[id]!, clip = p.views[1]!.scroll!.contentView
        return row.convert(row.bounds, to: clip).minY - clip.bounds.minY
    }

    func testAFoldAboveThePortLeavesTheContentStill() {
        let (p, clip) = fixture(top: 200)
        let before = y(p, 15)
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["height": 0.0]]] + rows(header: 0)))
        XCTAssertEqual(clip.bounds.minY, 152)
        XCTAssertEqual(y(p, 15), before)
    }

    func testRowsInsertedAboveThePortLeaveTheContentStill() {
        let (p, clip) = fixture(top: 200)
        let before = y(p, 15)
        var ops: [[String: Any]] = []
        for i in 0..<5 { ops.append(["op": "create", "id": 50 + i, "kind": "view", "style": ["height": 40.0]]) }
        ops.append(["op": "children", "id": 1, "ids": [2] + Array(50..<55) + Array(10..<30)])
        for i in 0..<5 { ops.append(["op": "frame", "id": 50 + i, "x": 0.0, "y": 48.0 + Double(40 * i), "w": 300.0, "h": 40.0]) }
        p.apply(wireBatch(ops + rows(header: 48, inserted: 200)))
        XCTAssertEqual(clip.bounds.minY, 400)
        XCTAssertEqual(y(p, 15), before)
    }

    /// A `scrollFollowEnd` scroller above its end is anchored as a plain one
    /// (the web leaves it to the browser's anchoring, LLP 1001); at its end it
    /// follows the end.
    func testAFollowerAboveItsEndIsAnchoredAndAtItsEndFollows() {
        let (p, clip) = fixture(top: 200, followEnd: true)
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["height": 0.0]]] + rows(header: 0)))
        XCTAssertEqual(clip.bounds.minY, 152)
        clip.scroll(to: NSPoint(x: 0, y: 400))
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["height": 100.0]]] + rows(header: 100)))
        XCTAssertEqual(clip.bounds.minY, 500, "the end, followed")
    }

    /// A view leaving with an exit animation is not the presenter's any more
    /// and is never the anchor.
    func testALeavingViewIsNoCandidate() {
        let (p, clip) = fixture(top: 200)
        // Row 15 (198…228), the first visible, is the presenter's no longer.
        let ghost = p.views.removeValue(forKey: 15)
        defer { p.views[15] = ghost }
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["height": 0.0]]] + rows(header: 0).filter { ($0["id"] as? Int) != 15 }))
        XCTAssertEqual(clip.bounds.minY, 152, "row 16 anchored instead")
    }

    func testAtTheStartNothingIsAnchored() {
        let (p, clip) = fixture(top: 0)
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["height": 100.0]]] + rows(header: 100)))
        XCTAssertEqual(clip.bounds.minY, 0, "content inserted on top shows, as in Chrome")
    }

    func testALayoutChangeOnTheAnchorSuppressesTheAdjustment() {
        let (p, clip) = fixture(top: 200)
        // Row 15 (198…228) is the anchor; its own height changes in the batch.
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["height": 0.0]], ["op": "style", "id": 15, "style": ["height": 31.0]]] + rows(header: 0)))
        XCTAssertEqual(clip.bounds.minY, 200)
    }

    func testAnExplicitScrollTopWins() {
        let (p, clip) = fixture(top: 200)
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["height": 0.0]], ["op": "props", "id": 1, "set": ["scrollTop": "100"]]] + rows(header: 0)))
        XCTAssertEqual(clip.bounds.minY, 100)
    }

    func testARemovedAnchorKeepsTheNumericOffset() {
        let (p, clip) = fixture(top: 200)
        p.apply(wireBatch([
            ["op": "style", "id": 2, "style": ["height": 0.0]],
            ["op": "children", "id": 1, "ids": [2] + Array(10..<15) + Array(16..<30)],
            ["op": "destroy", "id": 15],
        ] + rows(header: 0).filter { ($0["id"] as? Int) != 15 }))
        XCTAssertEqual(clip.bounds.minY, 200)
    }
}
#endif

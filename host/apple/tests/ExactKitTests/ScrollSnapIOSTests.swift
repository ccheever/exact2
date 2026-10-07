#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1008's horizontal snap: `scroll-snap-align: start` rests an area's
/// start at the viewport's, `end` its end, and an area wider than the
/// viewport is free while it covers it (CSS Scroll Snap §5.2.2).
final class ScrollSnapIOSTests: XCTestCase {
    private func strip(_ aligns: [(String, Double, Double)]) throws -> (NodeView, UIScrollView) {
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 300, height: 100)
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view", "style": ["overflow_x": "scroll", "scroll_snap_type": "x mandatory"]]]
        for (i, (align, x, w)) in aligns.enumerated() {
            ops.append(["op": "create", "id": 10 + i, "kind": "view", "style": ["scroll_snap_align": align]])
            ops.append(["op": "frame", "id": 10 + i, "x": x, "y": 0.0, "w": w, "h": 100.0])
        }
        ops += [["op": "children", "id": 1, "ids": aligns.indices.map { 10 + $0 }], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0],
                ["op": "content", "id": 1, "w": 900.0, "h": 100.0]]
        p.apply(wireBatch(ops))
        let node = try XCTUnwrap(p.views[1])
        return (node, try XCTUnwrap(node.scroll))
    }
    private func rest(_ node: NodeView, _ scroll: UIScrollView, from x: CGFloat) -> CGFloat {
        var target = CGPoint(x: x, y: 0)
        node.scrollViewWillEndDragging(scroll, withVelocity: .zero, targetContentOffset: &target)
        return target.x
    }

    func testEndRestsAnAreasEndAtTheViewportsEnd() throws {
        // A at 0 (start); B 300–450 (end: rests at 150); C 450–600 (start: 450).
        let (node, scroll) = try strip([("start", 0, 300), ("end", 300, 150), ("start", 450, 150)])
        XCTAssertEqual(rest(node, scroll, from: 60), 0)
        XCTAssertEqual(rest(node, scroll, from: 140), 150, "B's end at the viewport's end")
        XCTAssertEqual(rest(node, scroll, from: 400), 450)
    }

    func testAWideEndAreaIsFreeWhileItCoversTheViewport() throws {
        // 0–600, wider than the 300-point viewport: anywhere in 0…300.
        let (node, scroll) = try strip([("end", 0, 600), ("start", 600, 300)])
        XCTAssertEqual(rest(node, scroll, from: 120), 120)
        XCTAssertEqual(rest(node, scroll, from: 520), 600)
    }
}
#endif

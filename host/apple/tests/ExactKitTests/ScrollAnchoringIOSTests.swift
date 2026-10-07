#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A plain UIKit scroller (not `scrollFollowEnd`, not a list) keeps the
/// reader's content still when a box above the port changes size or is
/// inserted, as CSS scroll anchoring does in Chrome (#138 X23d).
final class ScrollAnchoringIOSTests: XCTestCase {
    private var window: UIWindow!

    /// A 200-point scroller: a 48-point header (id 2), then twenty 30-point
    /// rows (ids 10…29), scrolled to `top`.
    private func fixture(top: CGFloat) -> (Presenter, UIScrollView) {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "header"], "style": ["height": 48.0]],
        ]
        // A prop keeps each row a view, not a flat leaf (LLP 1068 §6.1).
        for i in 0..<20 { ops.append(["op": "create", "id": 10 + i, "kind": "view", "props": ["id": "r\(i)"], "style": ["height": 30.0]]) }
        ops += [
            ["op": "children", "id": 1, "ids": [2] + Array(10..<30)],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
        ]
        p.apply(wireBatch(ops + rows(header: 48)))
        let sv = p.views[1]!.scroll!
        sv.contentOffset = CGPoint(x: 0, y: top)
        return (p, sv)
    }
    private func rows(header: Double, inserted: Double = 0) -> [[String: Any]] {
        var ops: [[String: Any]] = [["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": header]]
        for i in 0..<20 { ops.append(["op": "frame", "id": 10 + i, "x": 0.0, "y": header + inserted + Double(30 * i), "w": 300.0, "h": 30.0]) }
        ops.append(["op": "content", "id": 1, "w": 300.0, "h": header + inserted + 600])
        return ops
    }

    func testAFoldAboveThePortLeavesTheContentStill() {
        let (p, sv) = fixture(top: 200)
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["height": 0.0]]] + rows(header: 0)))
        XCTAssertEqual(sv.contentOffset.y, 152)
    }

    func testRowsInsertedAboveThePortLeaveTheContentStill() {
        let (p, sv) = fixture(top: 200)
        var ops: [[String: Any]] = []
        for i in 0..<5 { ops.append(["op": "create", "id": 50 + i, "kind": "view", "props": ["id": "n\(i)"], "style": ["height": 40.0]]) }
        ops.append(["op": "children", "id": 1, "ids": [2] + Array(50..<55) + Array(10..<30)])
        for i in 0..<5 { ops.append(["op": "frame", "id": 50 + i, "x": 0.0, "y": 48.0 + Double(40 * i), "w": 300.0, "h": 40.0]) }
        p.apply(wireBatch(ops + rows(header: 48, inserted: 200)))
        XCTAssertEqual(sv.contentOffset.y, 400)
    }

    func testAtTheStartNothingIsAnchored() {
        let (p, sv) = fixture(top: 0)
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["height": 100.0]]] + rows(header: 100)))
        XCTAssertEqual(sv.contentOffset.y, 0)
    }

    func testALayoutChangeOnTheAnchorSuppressesTheAdjustment() {
        let (p, sv) = fixture(top: 200)
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["height": 0.0]], ["op": "style", "id": 15, "style": ["height": 31.0]]] + rows(header: 0)))
        XCTAssertEqual(sv.contentOffset.y, 200)
    }

    func testAnExplicitScrollTopWins() {
        let (p, sv) = fixture(top: 200)
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["height": 0.0]], ["op": "props", "id": 1, "set": ["scrollTop": "100"]]] + rows(header: 0)))
        XCTAssertEqual(sv.contentOffset.y, 100)
    }
}
#endif

#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class CollectionFillIOSTests: XCTestCase {
    func testCoveredScrollDefersItsReportAndAGapRescuesOnlyWhatShows() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 200))
        window.addSubview(p.viewport)
        p.viewport.frame = window.bounds
        window.makeKeyAndVisible()
        defer { p.reset(); window.isHidden = true }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll", "padding_top": 20]],
            ["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0,
                "count": 100, "totalExtent": 7200, "pending": false,
                "rows": (0..<4).map { ["view": 2 + $0, "root": 2 + $0, "epoch": 1, "index": $0] },
                "correction": NSNull()]]],
        ]
        for i in 0..<4 {
            ops.append(["op": "create", "id": 2 + i, "kind": "view"])
            ops.append(["op": "frame", "id": 2 + i, "x": 10, "y": 20 + 72 * i, "w": 280, "h": 72])
        }
        ops += [["op": "children", "id": 1, "ids": [2, 3, 4, 5]], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 200],
                ["op": "content", "id": 1, "w": 300, "h": 7220]]
        p.apply(wireBatch(ops))
        window.layoutIfNeeded()
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        var reports: [Data] = []
        p.collections.onFeedback = { reports.append($0) }
        let start = CACurrentMediaTime()
        for i in 1...1000 { scroll.contentOffset.y = CGFloat(1 + i % 90) }
        let milliseconds = (CACurrentMediaTime() - start) * 1000
        XCTAssertTrue(p.collections.covers(1))
        XCTAssertTrue(reports.isEmpty, "covered UIKit callbacks return before a row report")
        XCTAssertTrue(p.collections.fillPending.contains(1))
        p.collections.fillSlice(1, limit: 5)
        func limit(_ data: Data) -> UInt32 { data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 88, as: UInt32.self) } }
        XCTAssertEqual(reports.count, 1)
        XCTAssertEqual(limit(try XCTUnwrap(reports.last)), 5)
        scroll.contentOffset.y = 200
        XCTAssertFalse(p.collections.covers(1))
        XCTAssertEqual(reports.count, 2, "a visible gap is rescued in its UIKit callback")
        XCTAssertEqual(limit(try XCTUnwrap(reports.last)), 2, "visible rows plus two, rather than a whole window")
        print("D3 scroll callbacks=1000 covered_reports=0 ms=\(milliseconds) slice_limit=5 rescue_limit=2")
    }
}
#endif

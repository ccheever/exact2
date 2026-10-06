#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A batch that only builds, moves or drops a list's rows (a fling's fills,
/// many a second) skips the route projection; anything else runs it.
final class ListRowsIOSTests: XCTestCase {
    private func presenter() -> Presenter {
        let p = Presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 10, "kind": "view"], ["op": "create", "id": 11, "kind": "view"],
            ["op": "create", "id": 30, "kind": "view"],
            ["op": "children", "id": 10, "ids": [11]], ["op": "children", "id": 1, "ids": [10]],
            ["op": "children", "id": 30, "ids": [1]], ["op": "roots", "ids": [30]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
        ]))
        return p
    }

    func testRowsBuiltMovedAndDroppedInsideAListAreOnlyListRows() {
        let p = presenter()
        // A new row under the list, its own child, and the list's children.
        XCTAssertTrue(p.onlyListRows(wireBatch([
            ["op": "create", "id": 20, "kind": "view"], ["op": "create", "id": 21, "kind": "view"],
            ["op": "children", "id": 20, "ids": [21]], ["op": "children", "id": 1, "ids": [10, 20]],
            ["op": "frame", "id": 20, "x": 0.0, "y": 40.0, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 21, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 2000.0],
            ["op": "style", "id": 11, "style": ["opacity": 0.5]],
            ["op": "destroy", "id": 11],
        ])))
    }

    func testAnythingOutsideAListsRowsIsNot() throws {
        let p = presenter()
        // The list's own props, a node outside it, a node placed nowhere.
        for ops: [[String: Any]] in [
            [["op": "props", "id": 1, "set": ["testId": "t"]]],
            [["op": "style", "id": 30, "style": ["opacity": 0.5]]],
            [["op": "create", "id": 40, "kind": "view"]],
            [["op": "roots", "ids": [30]]],
            [],
        ] {
            XCTAssertFalse(p.onlyListRows(wireBatch(ops)), "\(ops)")
        }
    }
}
#endif

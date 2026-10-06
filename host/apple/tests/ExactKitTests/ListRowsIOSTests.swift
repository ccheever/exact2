#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A batch that only builds, moves or drops a virtualized list's rows (a
/// fling's fills, many a second) skips the route projection; anything the
/// projection reads, or anything outside such rows, runs it.
final class ListRowsIOSTests: XCTestCase {
    private func snapshot(_ view: Int, revision: Int) -> [String: Any] {
        ["view": view, "revision": revision, "scrollSequence": 0, "count": 50, "totalExtent": 2000,
         "rows": [["view": view * 10, "root": view * 10, "epoch": 1]]]
    }
    /// 30 holds a header 50, which holds a virtualized list 4 (row 40 > 42),
    /// a virtualized list 1 (row 10 > 11), a tab 70 whose face is 71, and a
    /// virtualized list 5 whose row 80 holds a tablist 81.
    private func presenter() -> Presenter {
        let p = Presenter()
        p.apply(wireBatch([
            ["op": "collections", "items": [snapshot(1, revision: 1), snapshot(4, revision: 1), snapshot(5, revision: 1)]],
            ["op": "create", "id": 30, "kind": "view"],
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 10, "kind": "view"], ["op": "create", "id": 11, "kind": "view"],
            ["op": "create", "id": 50, "kind": "view", "props": ["semanticTag": "header"]],
            ["op": "create", "id": 4, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 40, "kind": "view"], ["op": "create", "id": 42, "kind": "view"],
            ["op": "children", "id": 10, "ids": [11]], ["op": "children", "id": 1, "ids": [10]],
            ["op": "children", "id": 40, "ids": [42]], ["op": "children", "id": 4, "ids": [40]],
            ["op": "children", "id": 50, "ids": [4]],
            ["op": "create", "id": 70, "kind": "view", "props": ["accessibilityRole": "tab"]], ["op": "create", "id": 71, "kind": "view"],
            ["op": "children", "id": 70, "ids": [71]],
            ["op": "create", "id": 5, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 80, "kind": "view"], ["op": "create", "id": 81, "kind": "view", "props": ["accessibilityRole": "tablist"]],
            ["op": "children", "id": 80, "ids": [81]], ["op": "children", "id": 5, "ids": [80]],
            ["op": "children", "id": 30, "ids": [50, 1, 70, 5]], ["op": "roots", "ids": [30]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
        ]))
        return p
    }

    func testRowsBuiltMovedAndDroppedInAVirtualizedListAreOnlyListRows() {
        let p = presenter()
        XCTAssertTrue(p.collections.owns(1))
        XCTAssertTrue(p.onlyListRows(wireBatch([
            ["op": "create", "id": 20, "kind": "view"], ["op": "create", "id": 21, "kind": "view"],
            ["op": "children", "id": 20, "ids": [21]], ["op": "children", "id": 1, "ids": [10, 20]],
            ["op": "frame", "id": 20, "x": 0.0, "y": 40.0, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 21, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 2000.0],
            ["op": "style", "id": 11, "style": ["opacity": 0.5]],
            ["op": "present", "id": 11, "property": "opacity", "x": 0.5],
            ["op": "destroy", "id": 11],
        ])))
        // A fade outside any list that no tab, header or Back reads, and an
        // op on a node the presenter never made, change nothing it projects.
        XCTAssertTrue(p.onlyListRows(wireBatch([
            ["op": "present", "id": 30, "property": "opacity", "x": 0.0],
            ["op": "frame", "id": 999, "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0],
            ["op": "collections", "items": [snapshot(1, revision: 2)]],
        ])))
    }

    func testWhatTheProjectionReadsOrWhatIsOutsideTheRowsIsNot() {
        let p = presenter()
        for ops: [[String: Any]] in [
            [["op": "props", "id": 1, "set": ["testId": "t"]]],                    // the list's own props
            [["op": "style", "id": 30, "style": ["opacity": 0.5]]],                // outside any list
            [["op": "children", "id": 30, "ids": [1, 50]]],                        // reorders a header
            [["op": "frame", "id": 30, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0]],
            [["op": "present", "id": 71, "property": "opacity", "x": 0.0]],        // a tab's badge fading
            [["op": "style", "id": 42, "style": ["opacity": 0.5]]],                // a row under a header
            [["op": "children", "id": 4, "ids": [40]]],                             // a header's list
            [["op": "props", "id": 11, "set": ["accessibilityRole": "tab"]]],      // a row becoming a tab
            [["op": "style", "id": 80, "style": ["accent_color": [[0, 0, 255, 255]]]]], // a row holding a tablist
            [["op": "create", "id": 60, "kind": "view"]],                           // placed nowhere
            [["op": "roots", "ids": [30]]],
            [],
        ] {
            XCTAssertFalse(p.onlyListRows(wireBatch(ops)), "\(ops)")
        }
    }

    /// The gate itself: a rows batch asks no sync, anything else does.
    func testOnlyARowsBatchSkipsTheProjection() {
        let p = presenter()
        let before = p.navigation.syncCalls
        p.apply(wireBatch([["op": "style", "id": 11, "style": ["opacity": 0.5]]]))
        XCTAssertEqual(p.navigation.syncCalls, before, "rows only")
        p.apply(wireBatch([["op": "style", "id": 30, "style": ["opacity": 0.5]]]))
        XCTAssertEqual(p.navigation.syncCalls, before + 1, "outside the rows")
    }
}
#endif

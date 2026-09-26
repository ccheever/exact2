#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A virtualized list's retired row lends its views to the next row of its
/// shape (`NodePool`): the views stay in the list, parked, and come back
/// under the new ids with nothing of the old row's props, names, geometry or
/// presentation. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class NodePoolIOSTests: XCTestCase {
    private var window: UIWindow!

    private func collections(_ rows: [(view: Int, root: Int)]) -> [String: Any] {
        ["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0, "count": 50, "totalExtent": 2000,
            "rows": rows.map { ["view": $0.view, "root": $0.root, "epoch": 1] }, "correction": NSNull()]]]
    }
    /// A row (`base`) holding a symbol image (`base + 1`) and a button
    /// (`base + 2`), at `y` in the list.
    private func rowOps(_ base: Int, y: Double, label: String, symbol: String = "symbol:bookmark") -> [[String: Any]] {
        [
            ["op": "create", "id": base, "kind": "view", "props": ["testId": "row-\(base)", "id": "row-\(base)"],
             "style": ["background_color": [255, 255, 255, 255]]],
            ["op": "create", "id": base + 1, "kind": "image", "props": ["imageSource": symbol, "symbolName": "bookmark"],
             "style": ["font_size": 17.0]],
            ["op": "create", "id": base + 2, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": label]],
            ["op": "children", "id": base, "ids": [base + 1, base + 2]],
            ["op": "frame", "id": base, "x": 0.0, "y": y, "w": 300.0, "h": 44.0],
            ["op": "frame", "id": base + 1, "x": 8.0, "y": 10.0, "w": 20.0, "h": 24.0],
            ["op": "frame", "id": base + 2, "x": 200.0, "y": 0.0, "w": 44.0, "h": 44.0],
        ]
    }
    private func destroy(_ ids: [Int]) -> [[String: Any]] { ids.map { ["op": "destroy", "id": $0] } }
    private func fixture() -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([collections([(10, 10)]),
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]]]
            + rowOps(10, y: 0, label: "Save 10")
            + [["op": "children", "id": 1, "ids": [10]], ["op": "roots", "ids": [1]],
               ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
               ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]]))
        return p
    }

    func testARetiredRowsViewsComeBackUnderTheNextRowsIds() throws {
        let p = fixture()
        let row = try XCTUnwrap(p.views[10]), glyph = try XCTUnwrap(p.views[11]), button = try XCTUnwrap(p.views[12])
        let symbolView = try XCTUnwrap(glyph.symbolView)
        button.alpha = 0.5
        XCTAssertEqual(p.chrome.named["row-10"], [10])

        // The runner retires row 10 and builds row 20 of the same shape.
        p.apply(wireBatch([collections([(20, 20)])] + destroy([10, 11, 12]) + rowOps(20, y: 44, label: "Save 20")
            + [["op": "children", "id": 1, "ids": [20]]]))
        XCTAssertNil(p.views[10]); XCTAssertNil(p.views[11]); XCTAssertNil(p.views[12])
        XCTAssertTrue(p.views[20] === row && p.views[21] === glyph && p.views[22] === button, "the same views")
        XCTAssertEqual([row.id, glyph.id, button.id], [20, 21, 22])
        XCTAssertTrue(row.superview === p.views[1]?.scroll, "never left the list")
        XCTAssertFalse(row.isHidden)
        XCTAssertEqual(row.frame, CGRect(x: 0, y: 44, width: 300, height: 44))
        XCTAssertEqual(row.props["testId"], "row-20"); XCTAssertEqual(row.accessibilityIdentifier, "row-20")
        XCTAssertNil(p.chrome.named["row-10"]); XCTAssertEqual(p.chrome.named["row-20"], [20])
        XCTAssertEqual(button.accessibilityLabel, "Save 20")
        XCTAssertEqual(button.alpha, 1, "presentation is a fresh view's")
        XCTAssertTrue(glyph.symbolView === symbolView, "the same symbol keeps its glyph view")
        XCTAssertFalse(p.pool.isParked(row))
        XCTAssertEqual(p.pool.count, 0)
    }

    func testAPropTheNewRowLacksIsGoneAndAnotherSymbolIsSet() throws {
        let p = fixture()
        let glyph = try XCTUnwrap(p.views[11])
        var next = rowOps(20, y: 0, label: "Save 20", symbol: "symbol:bookmark-fill")
        next[0]["props"] = ["testId": "row-20"]
        p.apply(wireBatch([collections([(20, 20)])] + destroy([10, 11, 12]) + next
            + [["op": "children", "id": 1, "ids": [20]]]))
        let row = try XCTUnwrap(p.views[20])
        XCTAssertNil(row.props["id"], "no prop survives from the last row")
        XCTAssertTrue(p.views[21] === glyph)
        XCTAssertEqual(glyph.imageSource, "symbol:bookmark-fill")
    }

    func testARowOfAnotherShapeIsBuiltAndTheParkedOneWaits() throws {
        let p = fixture()
        let old = try XCTUnwrap(p.views[10])
        p.apply(wireBatch([collections([(30, 30)])] + destroy([10, 11, 12])
            + [["op": "create", "id": 30, "kind": "view"], ["op": "children", "id": 1, "ids": [30]],
               ["op": "frame", "id": 30, "x": 0.0, "y": 0.0, "w": 300.0, "h": 44.0]]))
        XCTAssertFalse(p.views[30] === old)
        XCTAssertTrue(p.pool.isParked(old)); XCTAssertTrue(old.isHidden)
        XCTAssertTrue(old.superview === p.views[1]?.scroll, "parked where it was")
        XCTAssertEqual(p.pool.count, 1)
        // The list's next children op leaves it parked.
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [30]]]))
        XCTAssertTrue(old.superview === p.views[1]?.scroll)
        p.reset()
        XCTAssertNil(old.superview, "a reset takes the parked views too")
        XCTAssertEqual(p.pool.count, 0)
    }

    func testARowWhoseNodeMovesAwayIsDestroyedAsBefore() throws {
        let p = fixture()
        let old = try XCTUnwrap(p.views[10]), button = try XCTUnwrap(p.views[12])
        // The button survives the batch (it moves to the list itself).
        p.apply(wireBatch([collections([(12, 12)])] + destroy([10, 11])
            + [["op": "children", "id": 1, "ids": [12]]]))
        XCTAssertNil(old.superview)
        XCTAssertFalse(p.pool.isParked(old))
        XCTAssertTrue(p.views[12] === button)
        XCTAssertTrue(button.superview === p.views[1]?.scroll)
    }
}
#endif

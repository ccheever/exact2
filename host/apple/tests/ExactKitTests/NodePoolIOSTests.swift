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

    /// A paragraph of inline runs parks with its row: the next row's
    /// `paragraph` op gives it only its own runs, and no bitmap is kept.
    func testAParagraphOfInlineRunsParksAndShowsOnlyTheNextRuns() throws {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds; window.addSubview(p.viewport); window.makeKeyAndVisible()
        func row(_ base: Int, _ words: [String]) -> [[String: Any]] {
            [["op": "create", "id": base, "kind": "view"],
             ["op": "create", "id": base + 1, "kind": "text", "style": ["font_size": 16.0]],
             ["op": "paragraph", "id": base + 1, "runs": words.enumerated().map { i, w in
                ["id": base + 2 + i, "parent": base + 1, "paint": true, "props": ["text": w], "style": ["font_weight": i == 0 ? 600.0 : 400.0]] }],
             ["op": "children", "id": base, "ids": [base + 1]],
             ["op": "frame", "id": base, "x": 0.0, "y": 0.0, "w": 300.0, "h": 44.0],
             ["op": "frame", "id": base + 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 20.0]]
        }
        p.apply(wireBatch([collections([(10, 10)]),
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]]]
            + row(10, ["Hello ", "world"])
            + [["op": "children", "id": 1, "ids": [10]], ["op": "roots", "ids": [1]],
               ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
               ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]]))
        let text = try XCTUnwrap(p.views[11])
        XCTAssertEqual(text.inlineText.count, 2)
        p.apply(wireBatch([collections([(20, 20)])] + destroy([10, 11]) + row(20, ["Bye"])
            + [["op": "children", "id": 1, "ids": [20]]]))
        XCTAssertTrue(p.views[21] === text, "the paragraph's view came back")
        XCTAssertEqual(text.inlineText.map(\.text), ["Bye"])
        XCTAssertEqual(text.paragraphText, "Bye")
        XCTAssertNil(p.inlineText(12), "the old runs are forgotten")
        XCTAssertNotNil(p.inlineText(22))
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

    // LLP 1068 stage 1: a row pools around its heavy leaves.

    /// A row (`base`) holding a symbol (`base + 1`), a heavy leaf of `kind`
    /// (`base + 2`) and a button (`base + 3`).
    private func heavyRowOps(_ base: Int, kind: String, y: Double, props: [String: String] = [:]) -> [[String: Any]] {
        [
            ["op": "create", "id": base, "kind": "view", "props": ["testId": "row-\(base)"].merging(props) { $1 }],
            ["op": "create", "id": base + 1, "kind": "image", "props": ["imageSource": "symbol:bookmark", "symbolName": "bookmark"],
             "style": ["font_size": 17.0]],
            ["op": "create", "id": base + 2, "kind": kind, "props": ["testId": "leaf-\(base)"]],
            ["op": "create", "id": base + 3, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Save \(base)"]],
            ["op": "children", "id": base, "ids": [base + 1, base + 2, base + 3]],
            ["op": "frame", "id": base, "x": 0.0, "y": y, "w": 300.0, "h": 120.0],
            ["op": "frame", "id": base + 1, "x": 8.0, "y": 10.0, "w": 20.0, "h": 24.0],
            ["op": "frame", "id": base + 2, "x": 40.0, "y": 0.0, "w": 150.0, "h": 100.0],
            ["op": "frame", "id": base + 3, "x": 200.0, "y": 0.0, "w": 44.0, "h": 44.0],
        ]
    }
    private func listFixture(_ rows: [[String: Any]], root: Int) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds; window.addSubview(p.viewport); window.makeKeyAndVisible()
        p.apply(wireBatch([collections([(root, root)]),
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]]]
            + rows
            + [["op": "children", "id": 1, "ids": [root]], ["op": "roots", "ids": [1]],
               ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
               ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]]))
        return p
    }

    func testARowPoolsAroundEachKindOfHeavyLeafWhichIsBuiltFresh() throws {
        for kind in ["video", "iframe", "native", "canvas", "canvas2d", "input", "textarea"] {
            let p = listFixture(heavyRowOps(10, kind: kind, y: 0), root: 10)
            let row = try XCTUnwrap(p.views[10]), glyph = try XCTUnwrap(p.views[11])
            let leaf = try XCTUnwrap(p.views[12]), button = try XCTUnwrap(p.views[13])
            let field = leaf.field, area = leaf.textArea, metal = leaf.metal
            let before = row.incarnation
            p.apply(wireBatch([collections([(20, 20)])] + destroy([10, 11, 12, 13]) + heavyRowOps(20, kind: kind, y: 0)
                + [["op": "children", "id": 1, "ids": [20]]]))
            XCTAssertTrue(p.views[20] === row && p.views[21] === glyph && p.views[23] === button, "\(kind): the plain views, by position")
            let fresh = try XCTUnwrap(p.views[22])
            XCTAssertFalse(fresh === leaf, "\(kind): the leaf is a new view")
            XCTAssertEqual(fresh.kind, kind)
            XCTAssertNil(leaf.superview, "\(kind): the old leaf left the row")
            XCTAssertNil(leaf.presenter, "\(kind): the old leaf was forgotten")
            XCTAssertTrue(fresh.superview === row.container, "\(kind): the new leaf is in the reused row")
            XCTAssertEqual(row.container.subviews.compactMap { $0 as? NodeView }.map(\.id), [21, 22, 23], "\(kind): in order")
            if kind == "input" { XCTAssertNotNil(fresh.field); XCTAssertFalse(fresh.field === field) }
            if kind == "textarea" { XCTAssertNotNil(fresh.textArea); XCTAssertFalse(fresh.textArea === area) }
            if kind == "canvas" { XCTAssertNotNil(fresh.metal); XCTAssertFalse(fresh.metal === metal) }
            XCTAssertNotEqual(row.incarnation, before, "\(kind): a new incarnation")
            XCTAssertEqual(fresh.props["testId"], "leaf-20")
            XCTAssertEqual(p.pool.count, 0)
            XCTAssertEqual(p.pool.leavesDropped[kind], 1); XCTAssertEqual(p.pool.leavesBuilt[kind], 1)
        }
    }

    /// A 2D canvas row (LLP 1056 D10, LLP 1068 §4.0): the canvas is a new
    /// view with no bitmap of the old row's, a late list for the old canvas
    /// lands nowhere, and the new canvas draws its own lifetime's lists.
    func testA2DCanvasRowPoolsAroundAFreshCanvasAndDropsStaleLists() throws {
        let p = listFixture(heavyRowOps(10, kind: "canvas2d", y: 0), root: 10)
        let draw = { (id: Int, lifetime: Int) -> [String: Any] in
            ["op": "canvas2d", "id": id, "lifetime": lifetime, "generation": 0, "seq": 0, "fresh": true,
             "w": 300, "h": 200, "scale": 2.0, "stretch": false, "box": [0.0, 0.0, 150.0, 100.0], "radii": [0.0, 0.0, 0.0, 0.0], "lists": [String]()]
        }
        p.apply(wireBatch([draw(12, 1)]))
        let old = try XCTUnwrap(p.views[12])
        XCTAssertEqual(old.layer.sublayers?.filter { $0.contents != nil }.count, 1, "the old canvas shows its bitmap")
        p.apply(wireBatch([collections([(20, 20)])] + destroy([10, 11, 12, 13]) + heavyRowOps(20, kind: "canvas2d", y: 0)
            + [["op": "children", "id": 1, "ids": [20]]]))
        let fresh = try XCTUnwrap(p.views[22])
        XCTAssertFalse(fresh === old)
        XCTAssertTrue(fresh.layer.sublayers?.allSatisfy { $0.contents == nil } ?? true, "no bitmap carried into the new canvas")
        p.apply(wireBatch([draw(12, 1)]))
        XCTAssertTrue(fresh.layer.sublayers?.allSatisfy { $0.contents == nil } ?? true, "a stale list lands nowhere")
        p.apply(wireBatch([draw(22, 2)]))
        XCTAssertEqual(fresh.layer.sublayers?.filter { $0.contents != nil }.count, 1, "the new canvas draws its own")
    }

    func testAParkedViewHasNoIncarnationUntilItIsTaken() throws {
        let p = listFixture(heavyRowOps(10, kind: "input", y: 0), root: 10)
        let row = try XCTUnwrap(p.views[10])
        let first = row.incarnation
        XCTAssertNotEqual(first, 0)
        p.apply(wireBatch([collections([(30, 30)])] + destroy([10, 11, 12, 13])
            + [["op": "create", "id": 30, "kind": "view"], ["op": "children", "id": 1, "ids": [30]]]))
        XCTAssertTrue(p.pool.isParked(row))
        XCTAssertEqual(row.incarnation, 0, "a callback issued for the old row never matches")
        p.apply(wireBatch([collections([(20, 20)])] + heavyRowOps(20, kind: "input", y: 0)
            + [["op": "children", "id": 1, "ids": [30, 20]]]))
        XCTAssertTrue(p.views[20] === row)
        XCTAssertNotEqual(row.incarnation, 0); XCTAssertNotEqual(row.incarnation, first)
    }

    func testARowWhoseLeafHasFocusIsDestroyedAsBefore() throws {
        let p = listFixture(heavyRowOps(10, kind: "input", y: 0), root: 10)
        let row = try XCTUnwrap(p.views[10]), leaf = try XCTUnwrap(p.views[12])
        p.editing = leaf
        p.apply(wireBatch([collections([(20, 20)])] + destroy([10, 11, 12, 13]) + heavyRowOps(20, kind: "input", y: 0)
            + [["op": "children", "id": 1, "ids": [20]]]))
        XCTAssertFalse(p.views[20] === row)
        XCTAssertNil(row.superview)
    }

    func testAMaterialRowComesBackWithANewEffectView() throws {
        for material in ["ultra-thin", "glass"] {
            let p = listFixture(rowOps(10, y: 0, label: "Save 10").enumerated().map { i, op in
                i == 0 ? op.merging(["props": ["testId": "row-10", "backgroundMaterial": material]]) { $1 } : op
            }, root: 10)
            let row = try XCTUnwrap(p.views[10]), glyph = try XCTUnwrap(p.views[11])
            let effect = try XCTUnwrap(row.materialView)
            XCTAssertTrue(p.materialNodes.contains(10))
            var next = rowOps(20, y: 0, label: "Save 20")
            next[0]["props"] = ["testId": "row-20", "backgroundMaterial": material]
            p.apply(wireBatch([collections([(20, 20)])] + destroy([10, 11, 12]) + next
                + [["op": "children", "id": 1, "ids": [20]]]))
            XCTAssertTrue(p.views[20] === row, "\(material): the row pools")
            XCTAssertTrue(p.views[21] === glyph)
            let fresh = try XCTUnwrap(row.materialView)
            XCTAssertFalse(fresh === effect, "\(material): a new effect view")
            XCTAssertNil(effect.superview)
            XCTAssertTrue(p.materialNodes.contains(20)); XCTAssertFalse(p.materialNodes.contains(10))
            XCTAssertTrue(glyph.superview === row.container, "\(material): the children are in the node's container")
        }
    }

    func testAMaterialRowWithoutTheMaterialComesBackWithout() throws {
        let p = listFixture(rowOps(10, y: 0, label: "Save 10").enumerated().map { i, op in
            i == 0 ? op.merging(["props": ["testId": "row-10", "backgroundMaterial": "glass"]]) { $1 } : op
        }, root: 10)
        let row = try XCTUnwrap(p.views[10])
        p.apply(wireBatch([collections([(20, 20)])] + destroy([10, 11, 12]) + rowOps(20, y: 0, label: "Save 20")
            + [["op": "children", "id": 1, "ids": [20]]]))
        XCTAssertTrue(p.views[20] === row)
        XCTAssertNil(row.materialView)
        XCTAssertFalse(p.materialNodes.contains(20))
        XCTAssertTrue(row.container === row)
        XCTAssertEqual(row.subviews.compactMap { $0 as? NodeView }.map(\.id), [21, 22])
    }

    func testAFullShapeEvictsItsOldestTreeAndItsRootLeavesTheList() throws {
        // Nine rows of one shape, retired together; a row of another shape is built.
        let bases = (1...9).map { $0 * 10 }
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds; window.addSubview(p.viewport); window.makeKeyAndVisible()
        p.apply(wireBatch([collections(bases.map { ($0, $0) }),
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]]]
            + bases.flatMap { rowOps($0, y: Double($0), label: "Save \($0)") }
            + [["op": "children", "id": 1, "ids": bases], ["op": "roots", "ids": [1]],
               ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
               ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]]))
        let roots = try bases.map { try XCTUnwrap(p.views[UInt32($0)]) }
        p.apply(wireBatch([collections([(500, 500)])] + destroy(bases.flatMap { [$0, $0 + 1, $0 + 2] })
            + [["op": "create", "id": 500, "kind": "view"], ["op": "children", "id": 1, "ids": [500]]]))
        XCTAssertEqual(p.pool.count, NodePool.perShape)
        XCTAssertEqual(p.pool.evictions, 1)
        XCTAssertNil(roots[0].superview, "the least recently parked tree left the list")
        XCTAssertFalse(p.pool.isParked(roots[0]))
        for root in roots.dropFirst() { XCTAssertTrue(p.pool.isParked(root)) }
    }

    func testHeavyLeafCostsLeaveOutEachKindsFirstCreation() {
        let kind = "test-kind-\(UUID().uuidString)"
        HeavyLeaves.record(kind, 0.5)
        XCTAssertNil(HeavyLeaves.cost(kind), "the first creation pays for loading")
        for s in [0.001, 0.02, 0.003, 0.004, 0.030, 0.002] { HeavyLeaves.record(kind, s) }
        XCTAssertEqual(HeavyLeaves.cost(kind), 0.004, "the median of the last five")
    }
}
#endif

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

    // LLP 1068 §4.2.1: a row that holds an inner virtualized list pools with
    // it, its scroll view reset and its cards reused.

    /// Outer list 1's collection and inner list `inner`'s, `cards` its rows.
    private func nested(_ row: Int, inner: Int, cards: [Int]) -> [String: Any] {
        ["op": "collections", "items": [
            ["view": 1, "revision": 1, "scrollSequence": 0, "count": 50, "totalExtent": 5000,
             "rows": [["view": row, "root": row, "epoch": 1]], "correction": NSNull()],
            ["view": inner, "axis": "x", "parent": 1, "revision": 1, "scrollSequence": 0, "count": 30, "totalExtent": 3600,
             "rows": cards.map { ["view": $0, "root": $0, "epoch": 1] }, "correction": NSNull()]]]
    }
    /// A carousel row: a title (`base + 1`) and an inner list (`base + 2`)
    /// of cards `base + 10 * k` (k = 1, 2), each a box holding an image.
    private func carouselOps(_ base: Int, y: Double) -> [[String: Any]] {
        let list = base + 2, cards = [base + 10, base + 20]
        var ops: [[String: Any]] = [
            ["op": "create", "id": base, "kind": "view", "props": ["testId": "row-\(base)"]],
            ["op": "create", "id": base + 1, "kind": "text", "props": ["text": "Row \(base)"], "style": ["font_size": 15.0]],
            ["op": "create", "id": list, "kind": "list", "style": ["overflow_x": "scroll", "overflow_y": "hidden"]],
        ]
        for (i, card) in cards.enumerated() {
            ops += [["op": "create", "id": card, "kind": "view", "props": ["testId": "card-\(card)"]],
                    ["op": "create", "id": card + 1, "kind": "image", "props": ["imageSource": "symbol:bookmark", "symbolName": "bookmark"],
                     "style": ["font_size": 17.0]],
                    ["op": "children", "id": card, "ids": [card + 1]],
                    ["op": "frame", "id": card, "x": Double(i) * 120, "y": 0.0, "w": 112.0, "h": 100.0],
                    ["op": "frame", "id": card + 1, "x": 0.0, "y": 0.0, "w": 112.0, "h": 100.0]]
        }
        ops += [["op": "children", "id": list, "ids": cards], ["op": "children", "id": base, "ids": [base + 1, list]],
                ["op": "frame", "id": base, "x": 0.0, "y": y, "w": 300.0, "h": 140.0],
                ["op": "frame", "id": base + 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 20.0],
                ["op": "frame", "id": list, "x": 0.0, "y": 30.0, "w": 300.0, "h": 100.0],
                ["op": "content", "id": list, "w": 3600.0, "h": 100.0]]
        return ops
    }
    private func carouselIDs(_ base: Int) -> [Int] { [base, base + 1, base + 2, base + 10, base + 11, base + 20, base + 21] }

    func testARowHoldingAnInnerListPoolsWithItsScrollResetAndItsCardsReused() throws {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds; window.addSubview(p.viewport); window.makeKeyAndVisible()
        p.apply(wireBatch([nested(100, inner: 102, cards: [110, 120]),
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]]]
            + carouselOps(100, y: 0)
            + [["op": "children", "id": 1, "ids": [100]], ["op": "roots", "ids": [1]],
               ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
               ["op": "content", "id": 1, "w": 300.0, "h": 5000.0]]))
        let row = try XCTUnwrap(p.views[100]), list = try XCTUnwrap(p.views[102])
        let scroll = try XCTUnwrap(list.scroll)
        let card = try XCTUnwrap(p.views[110]), other = try XCTUnwrap(p.views[120])
        scroll.contentOffset = CGPoint(x: 240, y: 0)
        let before = list.incarnation

        // The row retires and a row of the same shape is built.
        p.apply(wireBatch([nested(200, inner: 202, cards: [210, 220])] + destroy(carouselIDs(100)) + carouselOps(200, y: 140)
            + [["op": "children", "id": 1, "ids": [200]]]))
        XCTAssertTrue(p.views[200] === row, "the row's view came back")
        XCTAssertTrue(p.views[202] === list, "its inner list's view came back")
        XCTAssertTrue(list.scroll === scroll, "with its scroll view")
        XCTAssertEqual(scroll.contentOffset, .zero, "at a new list's offset")
        XCTAssertEqual(scroll.contentSize.width, 3600, "sized by the new content op")
        XCTAssertTrue(p.views[210] === card && p.views[220] === other, "the cards came back under the same list")
        XCTAssertTrue(card.superview === scroll)
        XCTAssertNotEqual(list.incarnation, before)
        XCTAssertFalse(row.isHidden || card.isHidden || other.isHidden)
        XCTAssertEqual(p.pool.count, 0); XCTAssertEqual(p.pool.innerCount, 0)
        XCTAssertTrue(p.scrollers.contains(202) && !p.scrollers.contains(102))
    }

    func testARowWhoseInnerListIsMovingIsDestroyedAsBefore() throws {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds; window.addSubview(p.viewport); window.makeKeyAndVisible()
        p.apply(wireBatch([nested(100, inner: 102, cards: [110, 120]),
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]]]
            + carouselOps(100, y: 0)
            + [["op": "children", "id": 1, "ids": [100]], ["op": "roots", "ids": [1]],
               ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
               ["op": "content", "id": 1, "w": 300.0, "h": 5000.0]]))
        let row = try XCTUnwrap(p.views[100]), list = try XCTUnwrap(p.views[102])
        // An authored scroll still pending on the inner list: not at rest.
        list.pendingScrollLeft = 480
        p.apply(wireBatch([nested(200, inner: 202, cards: [210, 220])] + destroy(carouselIDs(100)) + carouselOps(200, y: 140)
            + [["op": "children", "id": 1, "ids": [200]]]))
        XCTAssertFalse(p.views[200] === row)
        XCTAssertNil(row.superview, "destroyed")
        XCTAssertEqual(p.pool.count, 0)
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

    /// LLP 1069.011.000 D8: a row holding controls — a native button and a
    /// switch — pools around them; each is a new node with a new `UIControl`
    /// under the next row's id, pressing that id.
    func testARowPoolsAroundItsControlsWhichAreBuiltFresh() throws {
        func row(_ base: Int, y: Double) -> [[String: Any]] {
            [
                ["op": "create", "id": base, "kind": "view", "props": ["testId": "row-\(base)"]],
                ["op": "create", "id": base + 1, "kind": "text", "props": ["text": "Row \(base)"], "style": ["font_size": 17.0]],
                ["op": "create", "id": base + 2, "kind": "control", "handlers": ["press"],
                 "props": ["type": "button", "accessibilityRole": "button"], "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
                ["op": "create", "id": base + 3, "kind": "control", "handlers": ["change"],
                 "props": ["type": "checkbox", "accessibilityRole": "switch", "checked": "true"], "style": ["text_color": [0, 0, 0, 255]]],
                ["op": "children", "id": base, "ids": [base + 1, base + 2, base + 3]],
                ["op": "frame", "id": base, "x": 0.0, "y": y, "w": 300.0, "h": 60.0],
                ["op": "frame", "id": base + 1, "x": 8.0, "y": 10.0, "w": 120.0, "h": 24.0],
                ["op": "frame", "id": base + 2, "x": 140.0, "y": 10.0, "w": 80.0, "h": 34.0],
                ["op": "frame", "id": base + 3, "x": 230.0, "y": 10.0, "w": 51.0, "h": 31.0],
            ]
        }
        let p = listFixture(row(10, y: 0), root: 10)
        p.buttonFace = { _ in var f = ButtonFace(); f.title = "Go"; return f }
        p.apply(wireBatch([]))
        let rowView = try XCTUnwrap(p.views[10]), label = try XCTUnwrap(p.views[11])
        let button = try XCTUnwrap(p.controls.controls[12] as? NativeButtonIOS)
        let toggle = try XCTUnwrap(p.controls.controls[13])
        p.apply(wireBatch([collections([(20, 20)])] + destroy([10, 11, 12, 13]) + row(20, y: 0)
            + [["op": "children", "id": 1, "ids": [20]]]))
        XCTAssertTrue(p.views[20] === rowView && p.views[21] === label, "the plain views, by position")
        let freshButton = try XCTUnwrap(p.controls.controls[22] as? NativeButtonIOS)
        let freshToggle = try XCTUnwrap(p.controls.controls[23])
        XCTAssertFalse(freshButton === button, "a new UIButton")
        XCTAssertFalse(freshToggle === toggle, "a new switch")
        XCTAssertNil(p.controls.controls[12]); XCTAssertNil(p.controls.controls[13])
        XCTAssertNil(button.superview, "the old control left with its node")
        XCTAssertEqual(p.pool.leavesDropped["control"], 2); XCTAssertEqual(p.pool.leavesBuilt["control"], 2)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        freshButton.sendActions(for: .primaryActionTriggered)
        XCTAssertEqual(pressed, [22], "the new row's id")
        button.sendActions(for: .primaryActionTriggered)
        XCTAssertEqual(pressed, [22], "the old control reaches nothing")
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
        p.canvas2d.waitForReplays()  // replay runs off the main thread (LLP 1056 §8.3)
        let old = try XCTUnwrap(p.views[12])
        XCTAssertEqual(old.layer.sublayers?.filter { $0.contents != nil }.count, 1, "the old canvas shows its bitmap")
        p.apply(wireBatch([collections([(20, 20)])] + destroy([10, 11, 12, 13]) + heavyRowOps(20, kind: "canvas2d", y: 0)
            + [["op": "children", "id": 1, "ids": [20]]]))
        let fresh = try XCTUnwrap(p.views[22])
        XCTAssertFalse(fresh === old)
        XCTAssertTrue(fresh.layer.sublayers?.allSatisfy { $0.contents == nil } ?? true, "no bitmap carried into the new canvas")
        p.apply(wireBatch([draw(12, 1)]))
        p.canvas2d.waitForReplays()
        XCTAssertTrue(fresh.layer.sublayers?.allSatisfy { $0.contents == nil } ?? true, "a stale list lands nowhere")
        p.apply(wireBatch([draw(22, 2)]))
        p.canvas2d.waitForReplays()
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

    /// LLP 1053.000.000 D2: a glass group is the row's own view; parked, it
    /// goes, and the next row's props make a new one.
    func testAGroupRowPoolsWithANewGroupView() throws {
        guard #available(iOS 26.0, *) else { throw XCTSkip("Liquid Glass is iOS 26") }
        let p = listFixture(rowOps(10, y: 0, label: "Save 10").enumerated().map { i, op in
            i == 0 ? op.merging(["props": ["testId": "row-10", "glassGroup": "8"]]) { $1 } : op
        }, root: 10)
        let row = try XCTUnwrap(p.views[10]), glyph = try XCTUnwrap(p.views[11])
        let view = try XCTUnwrap(row.glassGroupView)
        var next = rowOps(20, y: 0, label: "Save 20")
        next[0]["props"] = ["testId": "row-20", "glassGroup": "8"]
        p.apply(wireBatch([collections([(20, 20)])] + destroy([10, 11, 12]) + next
            + [["op": "children", "id": 1, "ids": [20]]]))
        XCTAssertTrue(p.views[20] === row, "the row pools")
        let fresh = try XCTUnwrap(row.glassGroupView)
        XCTAssertFalse(fresh === view)
        XCTAssertNil(view.superview)
        XCTAssertTrue(glyph.superview === fresh.contentView)
    }

    /// LLP 1053.000.000.000 (grok's code review): a row recycled from an
    /// auto group onto a numeric one does not report the old `auto`.
    func testAnAutoGroupRowRecycledOntoANumericGroupIsNotAuto() throws {
        guard #available(iOS 26.0, *) else { throw XCTSkip("Liquid Glass is iOS 26") }
        let p = listFixture(rowOps(10, y: 0, label: "Save 10").enumerated().map { i, op in
            i == 0 ? op.merging(["props": ["testId": "row-10", "glassGroup": "8", "glassGroupAuto": "true"]]) { $1 } : op
        }, root: 10)
        var next = rowOps(20, y: 0, label: "Save 20")
        next[0]["props"] = ["testId": "row-20", "glassGroup": "12"]
        p.apply(wireBatch([collections([(20, 20)])] + destroy([10, 11, 12]) + next
            + [["op": "children", "id": 1, "ids": [20]]]))
        let row = try XCTUnwrap(p.views[20])
        XCTAssertNil(row.props["glassGroupAuto"])
        var native: [String: Any] = [:]
        row.glassAgentFields(&native)
        XCTAssertEqual((native["glassGroup"] as? [String: Any])?["spacing"] as? Double, 12)
        XCTAssertNil((native["glassGroup"] as? [String: Any])?["auto"])
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

    /// A native-module tag that opts into reuse (LLP 1068 §4.8, §4.9): a
    /// retired node's instance is reset and parked out of the window, the
    /// next node of the tag takes it with its props as a first mount, the
    /// view transparent until `load`; a callback the instance issues while
    /// parked never reaches the next node, one issued after the take does;
    /// an instance serves a bounded number of rows; a tag without the opt-in
    /// is destroyed as before.
    func testANativeTagThatOptsInIsResetAndLentToTheNextNode() throws {
        FakeModule.reset()
        FakeModule.table.withUnsafeBytes { NativeViews.install(table: $0.baseAddress!) }
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds; window.addSubview(p.viewport); window.makeKeyAndVisible()
        let natives = NativeViews()
        natives.install(module: UnsafeMutableRawPointer(bitPattern: 1)!)
        var sizes: [(UInt32, CGSize?)] = []
        p.onIntrinsic = { sizes.append(contentsOf: $0) }
        var loads: [UInt32] = []
        p.onLoad = { loads.append($0) }
        func node(_ id: UInt32, _ tag: String, _ props: String) -> NodeView {
            let v = NodeView(id: id, kind: "native", presenter: p)
            v.frame = CGRect(x: 0, y: 0, width: 200, height: 100)
            p.viewport.addSubview(v); p.views[id] = v
            v.handlers = ["load"]
            natives.create(owner: v)
            v.props = ["nativeViewModuleName": tag, "nativeViewProps": props]
            natives.update(v)
            return v
        }
        func retire(_ v: NodeView) { natives.destroy(id: v.id); p.views[v.id] = nil; v.removeFromSuperview() }
        let settle = {
            let delivered = self.expectation(description: "callbacks delivered")
            DispatchQueue.main.async { delivered.fulfill() }
            self.wait(for: [delivered], timeout: 2)
        }

        let a = node(10, "fake-map", #"{"place":"a"}"#)
        XCTAssertEqual(FakeModule.made.count, 1)
        let first = FakeModule.made[0]
        XCTAssertTrue(first.view.superview === a)
        a.style = ["padding_left": 4, "padding_right": 4, "padding_top": 4, "padding_bottom": 4, "border_width": 2]
        natives.laidOut(a)
        XCTAssertEqual(first.view.frame, CGRect(x: 6, y: 6, width: 188, height: 88), "module content excludes padding and border")
        a.frame.size = CGSize(width: 160, height: 80)
        natives.laidOut(a)
        XCTAssertEqual(first.view.frame, CGRect(x: 6, y: 6, width: 148, height: 68), "the content frame follows CSS resizing")
        first.send(9, "120,40"); first.send(9, "120,80")
        settle(); settle()
        XCTAssertEqual(sizes.count, 1, "one turn coalesces to the latest preferred size")
        XCTAssertEqual(sizes.last?.0, 10)
        XCTAssertEqual(sizes.last?.1, CGSize(width: 120, height: 80))
        first.send(9, "120,80"); first.send(9, "nan,20"); first.send(9, "0,20")
        settle(); settle()
        XCTAssertEqual(sizes.count, 1, "unchanged and invalid sizes do not relayout")
        first.send(9)
        settle(); settle()
        XCTAssertEqual(sizes.count, 2)
        XCTAssertNil(sizes.last!.1, "empty clears the preference")
        sizes.removeAll()
        // Let the callback hop accept the size, then park before its flush.
        first.send(9, "120,90")
        DispatchQueue.main.async { retire(a) }
        settle(); settle()
        XCTAssertTrue(sizes.isEmpty, "a queued report cannot survive retirement")
        XCTAssertEqual(first.resets, 1, "reset at the park")
        XCTAssertNil(first.view.superview, "parked out of the window")
        XCTAssertFalse(first.destroyed)
        first.send(9, "120,100") // a size issued while parked
        first.send(7) // a load issued while parked
        let b = node(20, "fake-map", #"{"place":"b"}"#)
        XCTAssertEqual(FakeModule.made.count, 1, "no new instance: the parked one is taken")
        XCTAssertTrue(first.view.superview === b)
        XCTAssertEqual(first.props.last, #"{"place":"b"}"#, "the new node's props, whole")
        XCTAssertEqual(first.view.alpha, 0, "transparent until the instance says nothing of the last row shows")
        settle()
        XCTAssertEqual(loads, [], "the parked instance's load reaches no node")
        XCTAssertTrue(sizes.isEmpty, "a parked size cannot reach a new incarnation")
        first.send(9, "120,60")
        settle(); settle()
        XCTAssertEqual(sizes.last?.0, 20)
        XCTAssertEqual(sizes.last?.1, CGSize(width: 120, height: 60))
        XCTAssertEqual(first.view.alpha, 0)
        first.send(7)
        settle()
        XCTAssertEqual(loads, [20], "a load after the take reaches the new node")
        XCTAssertEqual(first.view.alpha, 1)

        // Bounded: made once and reused up to the limit, then destroyed.
        var last = b
        for i in 1..<NativeViews.reuseLimit {
            retire(last)
            last = node(UInt32(30 + i), "fake-map", "{}")
        }
        XCTAssertEqual(FakeModule.made.count, 1)
        XCTAssertEqual(first.resets, NativeViews.reuseLimit)
        retire(last)
        XCTAssertTrue(first.destroyed, "past its limit the instance goes")
        _ = node(90, "fake-map", "{}")
        XCTAssertEqual(FakeModule.made.count, 2)

        // A tag without the opt-in is destroyed at its node's destroy.
        let plain = node(100, "fake-plain", "{}")
        let made = try XCTUnwrap(FakeModule.made.last)
        retire(plain)
        XCTAssertTrue(made.destroyed); XCTAssertEqual(made.resets, 0)
        // Recycling destroys an instance but leaves its kernel node and size.
        // A new incarnation's first nil must clear that retained preference.
        let far = node(110, "fake-plain", "{}")
        let old = try XCTUnwrap(FakeModule.made.last)
        old.send(9, "120,40")
        settle(); settle()
        sizes.removeAll()
        XCTAssertEqual(natives.recycleFar(hide: 1, release: 2) { $0 === far ? 3 : nil }.count, 1)
        natives.release(far)
        let fresh = try XCTUnwrap(FakeModule.made.last)
        XCTAssertFalse(old === fresh)
        fresh.send(9)
        settle(); settle()
        XCTAssertEqual(sizes.count, 1, "the first clear of a fresh instance must reach the retained node")
        XCTAssertEqual(sizes.last?.0, 110)
        XCTAssertNil(sizes.last!.1)
        retire(far)
        XCTAssertEqual(natives.observation["reused"] as? Int, NativeViews.reuseLimit)
        natives.drainParked()
        NativeViews.uninstallTable()
        window.isHidden = true
    }

    /// Activation hands default views to the turn after its transaction
    /// commits: nothing is made before the run loop gets there, and nothing
    /// when the session is no longer live by then.
    func testActivationMakesDefaultViewsAfterTheCommit() throws {
        FakeModule.reset()
        FakeModule.table.withUnsafeBytes { NativeViews.install(table: $0.baseAddress!) }
        defer { NativeViews.uninstallTable() }
        let p = Presenter()
        func mount(_ natives: NativeViews, _ id: UInt32) {
            natives.install(module: UnsafeMutableRawPointer(bitPattern: 1)!, gateOpen: false)
            let v = NodeView(id: id, kind: "native", presenter: p)
            v.frame = CGRect(x: 0, y: 0, width: 200, height: 100)
            p.viewport.addSubview(v); p.views[id] = v
            natives.create(owner: v)
            v.props = ["nativeViewModuleName": "fake-plain", "nativeViewProps": "{}"]
            natives.update(v)
        }
        let live = NativeViews(), stale = NativeViews()
        mount(live, 1); mount(stale, 2)
        live.activateAfterCommit { true }
        stale.activateAfterCommit { false }
        XCTAssertTrue(live.activationQueued)
        XCTAssertEqual(FakeModule.made.count, 0, "nothing is made inside activation's turn")
        let deadline = Date(timeIntervalSinceNow: 2)
        while live.activationQueued || stale.activationQueued, Date() < deadline { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01)) }
        XCTAssertFalse(live.activationQueued)
        XCTAssertFalse(stale.activationQueued)
        XCTAssertEqual(FakeModule.made.count, 1, "the live session's view is made; the stale one's is not")
    }

    func testHeavyLeafCostsLeaveOutEachKindsFirstCreation() {
        let kind = "test-kind-\(UUID().uuidString)"
        HeavyLeaves.record(kind, 0.5)
        XCTAssertNil(HeavyLeaves.cost(kind), "the first creation pays for loading")
        for s in [0.001, 0.02, 0.003, 0.004, 0.030, 0.002] { HeavyLeaves.record(kind, s) }
        XCTAssertEqual(HeavyLeaves.cost(kind), 0.004, "the median of the last five")
    }
}

/// A module table in memory for the reuse and activation tests: two tags,
/// `fake-map` (reuse) and `fake-plain`, each instance a plain view that
/// records what the host asked of it.
private enum FakeModule {
    final class Instance {
        let view = UIView()
        let nonce: UInt32
        let event: @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
        var props: [String] = []
        var resets = 0
        var destroyed = false
        init(nonce: UInt32, event: @escaping @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void) {
            self.nonce = nonce; self.event = event
        }
        func send(_ kind: UInt32, _ text: String = "") {
            let bytes = Array(text.utf8)
            bytes.withUnsafeBufferPointer { event(nil, nonce, kind, $0.baseAddress, UInt32($0.count)) }
        }
    }
    nonisolated(unsafe) static var made: [Instance] = []
    static func reset() { made = [] }
    static func instance(_ raw: UnsafeMutableRawPointer?) -> Instance { Unmanaged<Instance>.fromOpaque(raw!).takeUnretainedValue() }
    static let roster = strdup(#"{"fake-map":{"snapshot":false,"reuse":true},"fake-plain":{"snapshot":false}}"#)!
    static let table: [UInt8] = {
        var t = [UInt8](repeating: 0, count: 112)
        func put<T>(_ value: T, _ offset: Int) { withUnsafeBytes(of: value) { for (i, b) in $0.enumerated() { t[offset + i] = b } } }
        let create: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafePointer<UInt8>?, UInt32,
                                    (@convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void)?,
                                    UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> UnsafeMutableRawPointer? = {
            _, _, _, json, length, event, _, _, nonce, _, _ in
            let made = Instance(nonce: nonce, event: event!)
            made.props.append(String(decoding: UnsafeBufferPointer(start: json, count: Int(length)), as: UTF8.self))
            FakeModule.made.append(made)
            return Unmanaged.passRetained(made).toOpaque()
        }
        let view: @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? = { Unmanaged.passUnretained(FakeModule.instance($0).view).toOpaque() }
        let set: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UnsafeMutablePointer<UInt8>?, UInt32) -> Int32 = { raw, json, length, _, _ in
            FakeModule.instance(raw).props.append(String(decoding: UnsafeBufferPointer(start: json, count: Int(length)), as: UTF8.self)); return 0
        }
        let destroy: @convention(c) (UnsafeMutableRawPointer?) -> Void = { raw in
            FakeModule.instance(raw).destroyed = true; Unmanaged<Instance>.fromOpaque(raw!).release()
        }
        let none: @convention(c) (UnsafeMutableRawPointer?) -> Void = { _ in }
        let reuse: @convention(c) (UnsafeMutableRawPointer?) -> Int32 = { FakeModule.instance($0).resets += 1; return 0 }
        put(UInt32(3), 0); put(UInt32(112), 4); put(UnsafeRawPointer(roster), 8)
        put(unsafeBitCast(create, to: UnsafeRawPointer.self), 16); put(unsafeBitCast(view, to: UnsafeRawPointer.self), 24)
        put(unsafeBitCast(set, to: UnsafeRawPointer.self), 32); put(unsafeBitCast(destroy, to: UnsafeRawPointer.self), 48)
        // The module entries are never called here (the test installs an instance).
        for offset in [72, 80, 88, 96] { put(unsafeBitCast(none, to: UnsafeRawPointer.self), offset) }
        put(unsafeBitCast(reuse, to: UnsafeRawPointer.self), 104)
        return t
    }()
}
#endif

#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1115.000 D16 on iOS: rows of a list Contract marked (`listRows`) are
/// drawn as a plain list's cells, to what a `UICollectionView` plain list
/// measured on iOS 27 (`RowCellsIOS.swift` has the numbers): UIKit's
/// separator under each row from the row's text and above the first, the
/// cell's ground while a row is pressed in place of the dim, and the ground
/// staying while a screen the row pushed is up. What the author writes on a
/// row wins. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class RowCellsIOSTests: XCTestCase {
    private var window: UIWindow!
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }

    /// A page 1 holding a list 2 of four rows 44 points tall, each with a
    /// text 16 points in: 10 a row of text, 11 a row that presses, 12 a
    /// swipe container naming its content 32 (which presses), 13 the last.
    /// `list` and `rows` add to the list's and each row's create op.
    private func fixture(list: [String: Any] = ["props": ["listRows": "true"]], rows: [Int: [String: Any]] = [:]) throws -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 600))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        func create(_ id: Int, _ kind: String, _ more: [String: Any] = [:]) -> [String: Any] {
            ["op": "create", "id": id, "kind": kind].merging(more) { _, new in new }.merging(rows[id] ?? [:]) { _, new in new }
        }
        func frame(_ id: Int, _ x: Double, _ y: Double, _ w: Double, _ h: Double) -> [String: Any] { ["op": "frame", "id": id, "x": x, "y": y, "w": w, "h": h] }
        var ops: [[String: Any]] = [
            create(1, "view"),
            create(2, "list", ["style": ["overflow_y": "scroll"]].merging(list) { _, new in new }),
            create(10, "view"), create(11, "view", ["handlers": ["press"]]),
            create(12, "scroll", ["props": ["swipeContent": "content"], "style": ["overflow_x": "scroll"]]), create(13, "view"),
            create(32, "view", ["handlers": ["press"], "props": ["id": "content"]]),
        ]
        for (text, parent) in [(20, 10), (21, 11), (22, 32), (23, 13)] {
            ops.append(create(text, "text", ["props": ["text": "Row \(parent)"], "style": ["font_size": 17]]))
            ops.append(["op": "children", "id": parent, "ids": [text]])
        }
        ops += [
            ["op": "children", "id": 12, "ids": [32]], ["op": "children", "id": 2, "ids": [10, 11, 12, 13]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            frame(1, 0, 0, 400, 600), frame(2, 0, 100, 400, 400),
            frame(10, 0, 0, 400, 44), frame(11, 0, 44, 400, 44), frame(12, 0, 88, 400, 44), frame(13, 0, 132, 400, 44),
            frame(32, 0, 0, 400, 44),
            frame(20, 16, 12, 200, 20), frame(21, 16, 12, 200, 20), frame(22, 16, 12, 200, 20), frame(23, 16, 12, 200, 20),
            ["op": "content", "id": 2, "w": 400.0, "h": 176.0],
        ]
        p.apply(wireBatch(ops))
        return p
    }
    private func lines(_ p: Presenter) -> RowLinesView? {
        p.views[2]?.container.subviews.compactMap { $0 as? RowLinesView }.first
    }
    private func ground(_ p: Presenter) -> RowGroundView? {
        p.views[2]?.container.subviews.compactMap { $0 as? RowGroundView }.first
    }
    /// The separators a list draws, top to bottom.
    private func separators(_ p: Presenter) -> [CGRect] {
        var rects: [CGRect] = []
        lines(p)?.shape.path?.applyWithBlock { element in
            if element.pointee.type == .moveToPoint { rects.append(CGRect(origin: element.pointee.points[0], size: .zero)) }
            if element.pointee.type == .addLineToPoint, let last = rects.popLast() { rects.append(last.union(CGRect(origin: element.pointee.points[0], size: .zero))) }
        }
        return rects.sorted { $0.minY < $1.minY }
    }

    func testEveryRowHasUIKitsSeparatorFromItsTextAndTheFirstOneAboveIt() throws {
        let p = try fixture()
        let drawn = separators(p)
        XCTAssertEqual(drawn.map(\.maxY), [1, 44, 88, 132, 176], "above the first row, and under every row, the last included")
        for rect in drawn {
            XCTAssertEqual(rect.minX, 16, accuracy: 0.01, "from the row's text")
            XCTAssertEqual(rect.maxX, 400 - RowCells.margin, accuracy: 0.01, "to a margin short of the trailing edge")
            XCTAssertEqual(rect.height, RowCells.thickness, accuracy: 0.001, "a point thick")
        }
        let view = try XCTUnwrap(lines(p))
        XCTAssertEqual(view.shape.fillColor, RowCells.separator.resolvedColor(with: view.traitCollection).cgColor, "in UIKit's separator colour")
        XCTAssertNil(view.hitTest(CGPoint(x: 100, y: 44), with: nil), "a touch on a separator is the row's")
        // A row that goes: the lines follow the rows.
        p.apply(wireBatch([["op": "children", "id": 2, "ids": [10, 11, 12]]]))
        XCTAssertEqual(separators(p).map(\.maxY), [1, 44, 88, 132])
        XCTAssertEqual(p.rowCells.observation(try XCTUnwrap(p.views[11]))?["separator"] as? Bool, true)
        XCTAssertEqual(p.rowCells.observation(try XCTUnwrap(p.views[2]))?["rows"] as? String, "cells")
        // Not under a leading image: a row whose text starts past one has its separator from the text.
        p.apply(wireBatch([["op": "frame", "id": 20, "x": 56.0, "y": 12.0, "w": 200.0, "h": 20.0]]))
        XCTAssertEqual(separators(p).filter { $0.maxY == 44 || $0.maxY == 1 }.map(\.minX), [56, 56])
        // And from where the text's glyphs start: inside a padding of the text's own.
        p.apply(wireBatch([["op": "style", "id": 20, "style": ["font_size": 17, "padding_left": 4]]]))
        XCTAssertEqual(separators(p).filter { $0.maxY == 44 || $0.maxY == 1 }.map(\.minX), [60, 60])
    }

    func testWhatTheAuthorWritesOnARowWins() throws {
        // Row 11 wrote its bottom edge and row 13 its top edge (`none` is
        // written too): no separator of UIKit's at either.
        let p = try fixture(rows: [11: ["style": ["border_width_bottom": 2, "border_style_bottom": "solid", "written": ["border_width_bottom", "border_style_bottom"]]],
                                   13: ["style": ["border_style_top": "none", "written": ["border_style_top"]]]])
        XCTAssertEqual(separators(p).map(\.maxY), [1, 44, 176], "none under the row that wrote its bottom edge, nor above the one that wrote its top")
        // A list whose author opted out, and one Contract did not mark, draw none.
        for list: [String: Any] in [["style": ["overflow_y": "scroll", "appearance": "none"]], [:]] {
            let q = try fixture(list: list)
            XCTAssertNil(lines(q))
            XCTAssertTrue(separators(q).isEmpty)
        }
    }

    func testARowsPressIsItsCellsGroundAndNotTheDim() throws {
        let p = try fixture()
        let row = try XCTUnwrap(p.views[11])
        row.pressed = true
        XCTAssertNil(row.layer.animation(forKey: "pressDim"), "a row is not dimmed")
        let shown = try XCTUnwrap(ground(p), "the cell's ground shows")
        XCTAssertEqual(shown.frame, row.frame, "the whole row, edge to edge")
        XCTAssertEqual(shown.backgroundColor, RowCells.highlight)
        XCTAssertEqual(shown.alpha, 1, "at once")
        XCTAssertLessThan(shown.layer.zPosition, row.layer.zPosition, "under the row's own views")
        XCTAssertEqual(p.rowCells.observation(row)?["ground"] as? String, "highlighted")
        XCTAssertEqual(separators(p).map(\.maxY), [1, 132, 176], "the separators above and below a highlighted row are hidden")
        // The finger leaves and comes back, then lets go outside: the ground follows, and fades.
        row.pressFollows(inside: false)
        row.pressFollows(inside: true)
        XCTAssertNotNil(ground(p))
        row.pressed = false
        XCTAssertEqual(separators(p).count, 5, "and are back at once when it is let go")
        RunLoop.main.run(until: Date(timeIntervalSinceNow: RowCells.hold + RowCells.fade + 0.5))
        XCTAssertNil(ground(p), "faded and gone")
        // A swipe container's row is pressed through the content it names: the ground is the row's.
        let content = try XCTUnwrap(p.views[32]), swipe = try XCTUnwrap(p.views[12])
        content.pressed = true
        XCTAssertNil(content.layer.animation(forKey: "pressDim"))
        XCTAssertEqual(ground(p)?.frame, swipe.frame)
        content.touchesCancelled([], with: nil)
        XCTAssertNil(p.rowCells.observation(swipe)?["ground"], "a cancelled touch lets the cell go")
    }

    func testTheAuthorsOwnPressAndGroundAndAListThatIsNoCellListKeepTheirOwn() throws {
        // The author's press: a written scale, or pointer handlers of its own.
        let scaled = try fixture(rows: [11: ["style": ["press_scale": 0.96]]])
        try XCTUnwrap(scaled.views[11]).pressed = true
        XCTAssertNil(ground(scaled))
        let drawn = try fixture(rows: [11: ["handlers": ["press", "pointerdown", "pointerup"]]])
        try XCTUnwrap(drawn.views[11]).pressed = true
        XCTAssertNil(ground(drawn))
        // A row with a ground of its own is its author's card: it dims, as a pressable that is no row does.
        let card = try fixture(rows: [11: ["style": ["background_color": [240, 240, 240, 255], "written": ["background_color"]]]])
        let row = try XCTUnwrap(card.views[11])
        row.pressed = true
        XCTAssertNil(ground(card))
        XCTAssertNotNil(row.layer.animation(forKey: "pressDim"))
        // `appearance="none"`, and a list Contract did not mark (a carousel): ordinary pressables, which dim.
        for list: [String: Any] in [["style": ["overflow_y": "scroll", "appearance": "none"]], [:]] {
            let q = try fixture(list: list)
            let row = try XCTUnwrap(q.views[11])
            row.pressed = true
            XCTAssertNil(ground(q))
            XCTAssertNotNil(row.layer.animation(forKey: "pressDim"))
        }
    }

    func testATappedRowStaysSelectedUnderTheScreenItPushedAndLetsGoOnTheWayBack() throws {
        let p = try fixture()
        let row = try XCTUnwrap(p.views[11]), route = try XCTUnwrap(p.views[1])
        // The list's screen in a stack, as a route's is.
        let screen = UIViewController()
        p.viewport.removeFromSuperview()
        screen.view.addSubview(p.viewport)
        let stack = UINavigationController(rootViewController: screen)
        window.rootViewController = stack
        window.layoutIfNeeded()
        // A tap that pushes nothing: selected while its press runs, then let go.
        XCTAssertTrue(p.rowCells.select(row) === row)
        row.pressed = false
        XCTAssertEqual(p.rowCells.observation(row)?["ground"] as? String, "selected", "the release does not take the ground")
        p.rowCells.ran(row, on: p.rowCells.screen(of: row))
        XCTAssertNil(p.rowCells.observation(row)?["ground"], "nothing was pushed: the cell lets go")
        RunLoop.main.run(until: Date(timeIntervalSinceNow: RowCells.hold + RowCells.fade + 0.5))
        XCTAssertNil(ground(p), "and its ground has faded and gone")
        // A tap whose press pushes a screen over the list's: the cell stays selected under it.
        XCTAssertNotNil(p.rowCells.select(row))
        let before = p.rowCells.screen(of: row)
        XCTAssertTrue(before === screen)
        stack.pushViewController(UIViewController(), animated: false)
        p.rowCells.ran(row, on: before)
        XCTAssertEqual(p.rowCells.observation(row)?["ground"] as? String, "selected")
        XCTAssertNotNil(ground(p))
        // The screen comes back with no transition to ride: at once.
        stack.popViewController(animated: false)
        p.rowCells.appearing(route, alongside: nil)
        XCTAssertNil(ground(p))
        XCTAssertNil(p.rowCells.observation(row)?["ground"])
    }
}
#endif

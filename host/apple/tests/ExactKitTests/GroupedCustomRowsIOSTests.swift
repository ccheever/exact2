#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// A custom grouped row, its views carried into a list cell, drawn as a
/// standard cell beside it is (Harmony, 2026-10-21: a row with a badge
/// before its chevron was custom, its symbol a fifth smaller than the
/// standard row's and its line running from the card's edge). UIKit, so a
/// simulator runs it.
final class GroupedCustomRowsIOSTests: GroupedListFixture {
    /// A custom row first, a standard row of the same symbol after it.
    func customFirst() -> GroupedListModel {
        GroupedListModel(style: "inset-grouped", sections: [
            .init(view: 2, header: nil, footer: nil, rows: [
                Row(view: 21, custom: true, pressable: true),
                Row(view: 10, symbol: "person.circle", title: "Profile", accessory: "disclosure", pressable: true),
                Row(view: 11, title: "Notifications", secondary: "On", accessory: "disclosure", pressable: true),
            ]),
        ])
    }

    func testACustomRowsSymbolIsAListCellsImageAndItsLineStartsUnderItsContent() throws {
        let p = presenter { self.customFirst() }
        // The kernel's boxes for a row of a leading symbol (the sheet's 24 by
        // 20, 16 in), its content inset 56 past it, spanning its card.
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "image", "props": ["imageSource": "symbol:person", "symbolName": "person.circle"], "style": ["font_size": 17.0]],
            ["op": "create", "id": 31, "kind": "image", "props": ["imageSource": "symbol:forward-chevron", "symbolName": "chevron.forward"], "style": ["font_size": 17.0]],
            ["op": "children", "id": 21, "ids": [30, 31]],
            ["op": "frame", "id": 21, "x": 0.0, "y": 0.0, "w": 370.0, "h": 61.0],
            ["op": "frame", "id": 30, "x": 16.0, "y": 20.5, "w": 24.0, "h": 20.0],
            ["op": "frame", "id": 31, "x": 344.0, "y": 23.0, "w": 10.0, "h": 14.0],
        ]))
        let row = try XCTUnwrap(p.views[21]), leaf = try XCTUnwrap(p.views[30])
        row.kernelContentBox = CGRect(x: 56, y: 0, width: 298, height: 61)
        let l = try list(p)
        l.collection.collectionViewLayout.invalidateLayout()
        l.collection.layoutIfNeeded()
        let custom = try cell(p, 21), standard = try cell(p, 10)
        XCTAssertTrue(row.superview === custom.contentView)
        XCTAssertTrue(leaf.listSymbol, "the first child, a symbol")
        XCTAssertFalse(try XCTUnwrap(p.views[31]).listSymbol, "an accessory's chevron is no leading symbol")
        // UIKit draws a cell's image at the large scale of the body's size
        // (its `preferredSymbolConfiguration`), at its own size, centred.
        func images(_ v: UIView) -> [UIImageView] { (v as? UIImageView).map { [$0] } ?? v.subviews.flatMap(images) }
        let theirs = try XCTUnwrap(images(standard.contentView).first { $0.frame.width > 20 })
        let ours = try XCTUnwrap(leaf.symbolView)
        let drawn = try XCTUnwrap(ours.image).size
        XCTAssertEqual(drawn.width, theirs.bounds.width, accuracy: 0.5, "the symbol as wide as the standard row's")
        XCTAssertEqual(drawn.height, theirs.bounds.height, accuracy: 0.5, "and as tall")
        XCTAssertEqual(ours.contentMode, .center)
        XCTAssertFalse(ours.clipsToBounds, "its own size, past the sheet's box")
        XCTAssertEqual(ours.convert(ours.bounds, to: custom).midX, theirs.convert(theirs.bounds, to: standard).midX, accuracy: 0.5,
                       "centred where the standard row's is")
        // The line under the custom row starts under its content, where the
        // standard row's starts under its text.
        func lines(_ v: UIView) -> [UIView] { (String(describing: type(of: v)).contains("Separator") ? [v] : []) + v.subviews.flatMap(lines) }
        let found = lines(l.collection).map { $0.convert($0.bounds, to: l.collection) }
        let under = { (cell: UICollectionViewCell) -> CGRect? in
            let frame = cell.convert(cell.bounds, to: l.collection)
            return found.first { abs($0.maxY - frame.maxY) < 1.5 }.map { $0.offsetBy(dx: -frame.minX, dy: 0) }
        }
        let mine = try XCTUnwrap(under(custom), "a line under the custom row")
        let theirLine = try XCTUnwrap(under(standard), "a line under the standard row")
        XCTAssertEqual(mine.minX, 56, accuracy: 0.5, "under its content")
        XCTAssertEqual(mine.minX, theirLine.minX, accuracy: 0.5, "as the standard row's")
    }
}
#endif

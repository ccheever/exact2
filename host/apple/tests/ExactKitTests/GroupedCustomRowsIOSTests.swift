#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// A custom grouped row, its views carried into a list cell, drawn as a
/// standard cell beside it is (Harmony, 2026-10-21). UIKit, so a simulator
/// runs it.
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
            ["op": "create", "id": 30, "kind": "image", "props": ["imageSource": "symbol:person", "symbolName": "person.circle"], "style": ["font_size": 17.0, "object_fit": "contain"]],
            ["op": "create", "id": 31, "kind": "image", "props": ["imageSource": "symbol:forward-chevron", "symbolName": "chevron.forward"], "style": ["font_size": 17.0]],
            ["op": "children", "id": 21, "ids": [30, 31]],
            ["op": "frame", "id": 21, "x": 0.0, "y": 0.0, "w": 370.0, "h": 61.0],
            ["op": "frame", "id": 30, "x": 16.0, "y": 20.5, "w": 24.0, "h": 20.0],
            ["op": "frame", "id": 31, "x": 344.0, "y": 23.0, "w": 10.0, "h": 14.0],
        ]))
        let row = try XCTUnwrap(p.views[21]), leaf = try XCTUnwrap(p.views[30])
        row.kernelContentBox = CGRect(x: 56, y: 0, width: 298, height: 61)
        // The next batch carries the row again, which notes its line and lays
        // the list out again for it.
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "custom"]]]))
        let l = try list(p)
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
        // Its content moves in a batch that keeps its height: the line follows.
        row.kernelContentBox = CGRect(x: 16, y: 0, width: 338, height: 61)
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "moved"]]]))
        l.collection.layoutIfNeeded()
        let moved = lines(l.collection).map { $0.convert($0.bounds, to: l.collection) }
        let frame = custom.convert(custom.bounds, to: l.collection)
        let line = try XCTUnwrap(moved.first { abs($0.maxY - frame.maxY) < 1.5 })
        XCTAssertEqual(line.minX - frame.minX, 16, accuracy: 0.5, "the line where the content now starts")
    }

    /// Right to left the line is UIKit's own (the row's views are not
    /// mirrored), and a row the model drops takes its mark with it.
    func testRightToLeftTheLineIsTheKernelsAndADroppedRowTakesItsMark() throws {
        var dropped = false
        let p = presenter { dropped ? GroupedListModel(style: "inset-grouped", sections: [.init(view: 2, header: nil, footer: nil, rows: [
            Row(view: 10, symbol: "person.circle", title: "Profile", accessory: "disclosure", pressable: true),
        ])]) : self.customFirst() }
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "image", "props": ["imageSource": "symbol:person", "symbolName": "person.circle"], "style": ["font_size": 17.0, "object_fit": "contain"]],
            ["op": "children", "id": 21, "ids": [30]],
            ["op": "frame", "id": 21, "x": 0.0, "y": 0.0, "w": 370.0, "h": 61.0],
            ["op": "frame", "id": 30, "x": 16.0, "y": 20.5, "w": 24.0, "h": 20.0],
        ]))
        let row = try XCTUnwrap(p.views[21]), image = try XCTUnwrap(p.views[30])
        row.kernelContentBox = CGRect(x: 56, y: 0, width: 298, height: 61)
        let l = try list(p)
        l.collection.semanticContentAttribute = .forceRightToLeft
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "custom"]]]))
        l.collection.collectionViewLayout.invalidateLayout()
        l.collection.layoutIfNeeded()
        let custom = try cell(p, 21)
        func lines(_ v: UIView) -> [UIView] { (String(describing: type(of: v)).contains("Separator") ? [v] : []) + v.subviews.flatMap(lines) }
        let frame = custom.convert(custom.bounds, to: l.collection)
        let line = try XCTUnwrap(lines(l.collection).map { $0.convert($0.bounds, to: l.collection) }.first { abs($0.maxY - frame.maxY) < 1.5 })
        XCTAssertNil(try list(p).lineStart(21), "no line of ours right to left")
        XCTAssertEqual(frame.maxX - line.maxX, 16, accuracy: 0.5, "UIKit's own: its margin")
        XCTAssertTrue(image.listSymbol)
        dropped = true
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "dropped"]]]))
        XCTAssertFalse(image.listSymbol, "the row the model dropped takes its mark")
    }

    /// An image its author sized or fitted is left as laid out, and a mark
    /// goes with the list.
    func testAnAuthoredImageIsLeftAsLaidOutAndAMarkGoesWithTheList() throws {
        let p = presenter { self.customFirst() }
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "image", "props": ["imageSource": "symbol:person", "symbolName": "person.circle"], "style": ["font_size": 17.0, "object_fit": "cover"]],
            ["op": "children", "id": 21, "ids": [30]],
            ["op": "frame", "id": 30, "x": 16.0, "y": 20.5, "w": 24.0, "h": 20.0],
        ]))
        let image = try XCTUnwrap(p.views[30])
        XCTAssertFalse(image.listSymbol, "its author's fit")
        p.apply(wireBatch([["op": "style", "id": 30, "style": ["font_size": 17.0, "object_fit": "contain", "written": ["width", "height"]]],
                           ["op": "frame", "id": 30, "x": 16.0, "y": 14.0, "w": 32.0, "h": 32.0]]))
        XCTAssertFalse(image.listSymbol, "its author's size")
        // The sheet's slot, at whatever size its ramps give it.
        p.apply(wireBatch([["op": "style", "id": 30, "style": ["font_size": 17.0, "object_fit": "contain"]],
                           ["op": "frame", "id": 30, "x": 16.0, "y": 16.5, "w": 32.11, "h": 26.76]]))
        XCTAssertTrue(image.listSymbol, "the sheet's slot")
        // A fit its author wrote, even the sheet's own value, is theirs.
        p.apply(wireBatch([["op": "style", "id": 30, "style": ["font_size": 17.0, "object_fit": "contain", "written": ["object_fit"]]]]))
        XCTAssertFalse(image.listSymbol, "its author's contain")
        p.apply(wireBatch([["op": "style", "id": 30, "style": ["font_size": 17.0, "object_fit": "contain"]]]))
        XCTAssertTrue(image.listSymbol)
        // At an accessibility size the slot keeps the default's drawing (its
        // medium scale, in its box) until UIKit's rule there is measured.
        window.traitOverrides.preferredContentSizeCategory = .accessibilityLarge
        window.layoutIfNeeded() // traits reach the views at layout
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "large"]]]))
        XCTAssertFalse(image.listSymbol, "an accessibility size")
        window.traitOverrides.preferredContentSizeCategory = .extraExtraExtraLarge
        window.layoutIfNeeded()
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "xxxl"]]]))
        XCTAssertTrue(image.listSymbol, "the largest size short of them")
        host(p).lists[1]?.remove()
        XCTAssertFalse(image.listSymbol, "gone with the list")
    }
}
#endif

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

    /// Right to left the kernel mirrors the row's insets, and the line starts
    /// past its symbol from the right; a row the model drops takes its mark
    /// with it.
    func testRightToLeftTheLineIsTheKernelsAndADroppedRowTakesItsMark() throws {
        var dropped = false
        let p = presenter(style: ["direction": "rtl"]) { dropped ? GroupedListModel(style: "inset-grouped", sections: [.init(view: 2, header: nil, footer: nil, rows: [
            Row(view: 10, symbol: "person.circle", title: "Profile", accessory: "disclosure", pressable: true),
        ])]) : self.customFirst() }
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "image", "props": ["imageSource": "symbol:person", "symbolName": "person.circle"], "style": ["font_size": 17.0, "object_fit": "contain"]],
            ["op": "children", "id": 21, "ids": [30]],
            ["op": "frame", "id": 21, "x": 0.0, "y": 0.0, "w": 370.0, "h": 61.0],
            ["op": "frame", "id": 30, "x": 330.0, "y": 20.5, "w": 24.0, "h": 20.0],
        ]))
        let row = try XCTUnwrap(p.views[21]), image = try XCTUnwrap(p.views[30])
        row.kernelContentBox = CGRect(x: 16, y: 0, width: 298, height: 61)
        let l = try list(p)
        XCTAssertEqual(l.collection.effectiveUserInterfaceLayoutDirection, .rightToLeft, "the list's direction")
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "custom"]]]))
        l.collection.collectionViewLayout.invalidateLayout()
        l.collection.layoutIfNeeded()
        // Where UIKit draws it is not read here: this fixture's window has no
        // scene, and the list layout's environment stays left to right in it.
        XCTAssertEqual(try XCTUnwrap(try list(p).lineStart(21)), 56, accuracy: 0.5, "from the right, past the symbol")
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
        // At an accessibility size too, drawn at UIKit's smaller size there.
        window.traitOverrides.preferredContentSizeCategory = .accessibilityLarge
        window.layoutIfNeeded() // traits reach the views at layout
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "large"]]]))
        XCTAssertTrue(image.listSymbol, "an accessibility size")
        let base = UIImage.SymbolConfiguration(pointSize: 33)
        XCTAssertEqual(image.symbolShape(base), base.applying(UIImage.SymbolConfiguration(pointSize: 25.14)).applying(UIImage.SymbolConfiguration(scale: .large)))
        window.traitOverrides.preferredContentSizeCategory = .extraExtraExtraLarge
        window.layoutIfNeeded()
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "xxxl"]]]))
        XCTAssertTrue(image.listSymbol, "the largest size short of them")
        host(p).lists[1]?.remove()
        XCTAssertFalse(image.listSymbol, "gone with the list")
    }

    /// A cell UIKit gives to another custom row takes that row's height,
    /// which is no resize: the layout already knows each row's height. When
    /// it was one, two cells handed back and forth between rows of two
    /// heights at a long list's bottom edge laid the list out again on every
    /// turn, and Hackers' feed spun a core at rest (LLP 1141.001 D13).
    func testACellTakingAnotherRowsHeightLaysNothingOutAgain() throws {
        let p = presenter {
            var m = self.model()
            m.sections[0].header = nil
            m.sections[0].footer = nil
            m.sections[0].rows[0] = Row(view: 10, custom: true, pressable: true)
            m.sections[0].rows[1] = Row(view: 11, custom: true, pressable: true)
            return m
        }
        p.apply(wireBatch([["op": "frame", "id": 10, "x": 16.0, "y": 0.0, "w": 354.0, "h": 85.0],
                           ["op": "frame", "id": 11, "x": 16.0, "y": 85.0, "w": 354.0, "h": 79.0]]))
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        let l = try list(p)
        let a = try XCTUnwrap(try cell(p, 10) as? GroupedCell), b = try XCTUnwrap(try cell(p, 11) as? GroupedCell)
        let pathA = try XCTUnwrap(l.collection.indexPath(for: a)), pathB = try XCTUnwrap(l.collection.indexPath(for: b))
        XCTAssertEqual([a.height, b.height], [85, 79])
        XCTAssertFalse(l.resized, "laid out")
        // UIKit shows each cell for the other's row, then gives them back.
        l.collectionView(l.collection, willDisplay: a, forItemAt: pathB)
        l.collectionView(l.collection, willDisplay: b, forItemAt: pathA)
        XCTAssertTrue(try XCTUnwrap(p.views[11]).superview === a.contentView)
        XCTAssertEqual(a.height, 79, "the cell takes its new row's height")
        XCTAssertFalse(l.resized, "which the layout already has")
        l.collectionView(l.collection, willDisplay: a, forItemAt: pathA)
        l.collectionView(l.collection, willDisplay: b, forItemAt: pathB)
        XCTAssertFalse(l.resized)
        // A row whose own height changes still lays the list out again.
        p.apply(wireBatch([["op": "frame", "id": 11, "x": 16.0, "y": 85.0, "w": 354.0, "h": 120.0]]))
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertEqual(try cell(p, 11).bounds.height, 120, accuracy: 0.5, "its row grew")
    }

    /// A row inserted at the top of a list of custom rows: the shown rows
    /// stay in their cells through the batch, as a collection view's cells
    /// do under an insert, and go home in the order the batch gave them.
    func testARowInsertedAboveCustomRowsLeavesTheShownOnesInTheirCells() throws {
        var inserted = false
        let p = presenter {
            var m = self.model()
            m.sections[0].rows[0] = Row(view: 10, custom: true, pressable: true)
            m.sections[0].rows[1] = Row(view: 11, custom: true, pressable: true)
            if inserted { m.sections[0].rows.insert(Row(view: 14, custom: true, pressable: true), at: 0) }
            return m
        }
        p.apply(wireBatch([["op": "frame", "id": 10, "x": 16.0, "y": 0.0, "w": 354.0, "h": 85.0],
                           ["op": "frame", "id": 11, "x": 16.0, "y": 85.0, "w": 354.0, "h": 79.0]]))
        let l = try list(p)
        let a = try cell(p, 10), row = try XCTUnwrap(p.views[10])
        XCTAssertTrue(row.superview === a.contentView)
        inserted = true
        let insert = wireBatch([
            ["op": "create", "id": 14, "kind": "button", "props": ["testId": "row14"], "handlers": ["press"], "style": [:]],
            ["op": "children", "id": 4, "ids": [14, 10, 11, 12]],
            ["op": "frame", "id": 14, "x": 16.0, "y": 0.0, "w": 354.0, "h": 70.0],
            ["op": "frame", "id": 10, "x": 16.0, "y": 70.0, "w": 354.0, "h": 85.0],
            ["op": "frame", "id": 11, "x": 16.0, "y": 155.0, "w": 354.0, "h": 79.0],
        ])
        host(p).prepare(insert.ops)
        XCTAssertTrue(row.superview === a.contentView, "a frame and its parent's children leave it in its cell")
        p.apply(insert)
        XCTAssertTrue(row.superview === a.contentView, "nor did the batch move it")
        XCTAssertEqual(row.frame.minY, 0, "at its cell's top")
        XCTAssertTrue(try XCTUnwrap(p.views[14]).superview === (try cell(p, 14)).contentView, "the new row in its own cell")
        // Home, each is where the batch put it, in its order.
        l.restore()
        let group = try XCTUnwrap(p.views[4]).container
        XCTAssertEqual(group.subviews.compactMap { ($0 as? NodeView)?.id }, [14, 10, 11, 12])
        XCTAssertEqual(row.frame.minY, 70, "and its box")
        // A prop on the row itself takes it home alone for the batch.
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0]]))
        let b = try cell(p, 11)
        host(p).prepare(wireBatch([["op": "props", "id": 10, "set": ["testId": "first"]]]).ops)
        XCTAssertTrue(row.superview === group, "the row read at home")
        XCTAssertTrue(try XCTUnwrap(p.views[11]).superview === b.contentView, "its neighbour still in its cell")
    }

    /// A swipe row inserted while a sheet is up (r25's contacts: the
    /// contact its form saves) offers its actions once the sheet closes.
    /// The sheet made it inert as it was mounted, and the batches after
    /// change nothing in it, so its row keeps what it read then: only the
    /// row's own `inert` may take its actions away.
    func testASwipeRowInsertedUnderASheetSwipesOnceItCloses() throws {
        var rows = [Row(view: 30, title: "Plain")]
        ExactGroupedLists.install()
        let p = Presenter()
        host(p).model = { $0 == 1 ? GroupedListModel(sections: [.init(view: 2, header: nil, footer: nil, rows: rows)]) : nil }
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let screen = scene.map { UIWindow(windowScene: $0) } ?? UIWindow()
        screen.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        let owner = UIViewController(), background = UIViewController()
        screen.rootViewController = owner
        screen.makeKeyAndVisible()
        window = screen
        addTeardownBlock { owner.presentedViewController?.dismiss(animated: false); screen.isHidden = true }
        p.viewport.frame = owner.view.bounds
        owner.view.addSubview(p.viewport)
        owner.addChild(background)
        background.view.frame = owner.view.bounds
        owner.view.addSubview(background.view)
        background.didMove(toParent: owner)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "list", "props": ["listStyle": "inset-grouped"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["semanticTag": "section"], "style": [:]],
            ["op": "create", "id": 4, "kind": "view", "props": [:], "style": [:]],
            ["op": "create", "id": 30, "kind": "view", "props": [:], "style": [:]],
            ["op": "create", "id": 90, "kind": "view", "props": ["navigationPresentation": "modal"], "style": [:]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [4]], ["op": "children", "id": 4, "ids": [30]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0],
            ["op": "frame", "id": 30, "x": 16.0, "y": 0.0, "w": 354.0, "h": 52.0],
        ]))
        background.view.addSubview(try XCTUnwrap(p.views[1]))
        p.modals.present(try XCTUnwrap(p.views[90]), navigation: UIViewController(), from: background, preceding: nil, owner: owner)
        XCTAssertTrue(try XCTUnwrap(p.views[1]).inert, "the sheet makes the list inert")

        rows.insert(Row(view: 21, custom: true), at: 0)
        p.apply(wireBatch([
            ["op": "create", "id": 21, "kind": "view", "props": ["swipeContent": "body", "swipeTrailing": "delete"],
             "style": ["overflow_x": "scroll", "overflow_y": "hidden"]],
            ["op": "create", "id": 22, "kind": "view", "props": ["id": "body"], "style": [:]],
            ["op": "create", "id": 23, "kind": "button", "props": ["id": "delete", "accessibilityLabel": "Delete", "destructive": "true"],
             "handlers": ["press"], "style": [:]],
            ["op": "children", "id": 4, "ids": [21, 30]], ["op": "children", "id": 21, "ids": [22, 23]],
            ["op": "frame", "id": 21, "x": 16.0, "y": 0.0, "w": 386.0, "h": 52.0],
            ["op": "frame", "id": 22, "x": 0.0, "y": 0.0, "w": 370.0, "h": 52.0],
            ["op": "frame", "id": 23, "x": 370.0, "y": 0.0, "w": 60.0, "h": 52.0],
            ["op": "frame", "id": 30, "x": 16.0, "y": 52.0, "w": 354.0, "h": 52.0],
        ]))
        p.modals.closeTop(animated: false)
        XCTAssertFalse(try XCTUnwrap(p.views[21]).inert, "the sheet closed")
        // A batch that changes nothing in the row keeps it as it is.
        p.apply(wireBatch([["op": "props", "id": 4, "set": ["testId": "rows"], "clear": [String]()]]))
        let trailing = try list(p).swipeActions(at: IndexPath(item: 0, section: 0), leading: false)
        XCTAssertEqual(trailing?.actions.count, 1, "its Delete, as the rows that were there before the sheet")
        // Its cell, configured under the sheet, takes a tap and is an element
        // (r28's expenses: a saved expense neither opened nor was read until relaunch).
        try list(p).collection.layoutIfNeeded()
        let cell = try XCTUnwrap(list(p).cell(21))
        XCTAssertTrue(cell.isUserInteractionEnabled, "a tap reaches it")
        XCTAssertFalse(cell.accessibilityElementsHidden, "VoiceOver reads it")
    }
}
#endif

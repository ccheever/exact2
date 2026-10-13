// @ref LLP 1115.002 D5, LLP 1115.000 D16 — which lists are tables on the
// Mac, and a list the runner does not window as one: its children are the
// table's rows. The runner's windowed list is `TableFeedMacTests`'; the keys
// and the clicks, `TableFeedKeyMacTests`'.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class TableListMacTests: XCTestCase {
    private var fixture: TableFixture?
    override func tearDown() { fixture?.close(); fixture = nil; super.tearDown() }

    /// A page of lists, one of each kind the rule tells apart.
    private static let kinds = """
    component Lists
      state seed = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
      derive rows = concat(seed, seed)
      state opened = 0 - 1
      action open(r: number)
        opened = r
      view
        column width="100%" height="100%" gap=4
          text `opened ${opened}` testId="status"
          list flex=1 min-height=0 width="100%" testId="rows"
            each r in seed key=`p${r}`
              row press=open(r) padding="6px 16px" testId=`plain-${r}`
                text `Plain ${r}`
          list virtualized=true estimated-item-height=28 flex=1 min-height=0 width="100%" testId="windowed"
            each r in rows key=`w${r}`
              row press=open(r) padding="6px 16px"
                text `Windowed ${r}`
          list appearance="none" flex=1 min-height=0 width="100%" testId="opted-out"
            each r in seed key=`n${r}`
              row press=open(r) padding="6px 16px"
                text `Authored ${r}`
          list appearance="auto" flex=1 min-height=0 width="100%" testId="grouped"
            section
              each r in seed key=`g${r}`
                row press=open(r)
                  text `Grouped ${r}`
          list virtualized=true display="flex" width="100%" height=60 overflow-x="scroll" overflow-y="hidden" estimated-item-width=80 testId="strip"
            each r in rows key=`s${r}`
              row press=open(r) width=80 height=44
                text `S ${r}`
          list display="flex" width="100%" height=60 overflow-x="scroll" overflow-y="hidden" testId="eager-strip"
            each r in seed key=`e${r}`
              row press=open(r) width=80 height=44 flex-shrink=0
                text `E ${r}`
          list display="flex" flex-wrap="wrap" width="100%" height=60 testId="wrapped"
            each r in seed key=`f${r}`
              row press=open(r) width=80 height=44
                text `F ${r}`
          list virtualized=true estimated-item-height=120 flex=1 min-height=0 width="100%" testId="outer"
            each r in seed key=`o${r}`
              column height=120 width="100%"
                text `Outer ${r}`
                list flex=1 min-height=0 width="100%" testId=`inner-${r}`
                  each q in seed key=`i${q}`
                    text `Inner ${q}`

    """

    func testAVerticalListIsATableAndTheOthersKeepTheirPaintedRows() throws {
        let f = try TableFixture(count: 0, size: CGSize(width: 480, height: 900), source: Self.kinds)
        fixture = f
        let tables = f.presenter.tables
        func list(_ id: String) throws -> NodeView { try f.node(id) }
        XCTAssertNotNil(tables.feeds[try list("rows").id], "a list is a table")
        XCTAssertNotNil(tables.feeds[try list("windowed").id], "and so is one the runner windows")
        XCTAssertNotNil(tables.feeds[try list("outer").id])
        XCTAssertNil(tables.feeds[try list("opted-out").id], "appearance=\"none\" keeps the author's rows")
        XCTAssertNil(tables.feeds[try list("grouped").id], "appearance=\"auto\" is the grouped form, painted")
        XCTAssertNil(tables.feeds[try list("strip").id], "a list that scrolls sideways is a carousel")
        XCTAssertNil(tables.feeds[try list("eager-strip").id], "windowed or not")
        XCTAssertNil(tables.feeds[try list("wrapped").id], "and a flex row that wraps is the author's arrangement, not rows")
        XCTAssertNil(tables.feeds[try list("inner-0").id], "a list in another list's row keeps its painted rows")
        // The driver's own chrome paints every list's rows, and gives them back.
        tables.painted = true
        XCTAssertTrue(tables.feeds.isEmpty)
        XCTAssertTrue(try f.node("plain-3").superview === list("rows").scroll?.documentView, "a painted list's rows are in its document")
        tables.painted = false
        f.settle()
        XCTAssertNotNil(tables.feeds[try list("rows").id])
        XCTAssertNotNil(tables.feeds[try list("windowed").id])
    }

    /// A list the runner does not window: a heading that presses nothing,
    /// then sixty rows that open, each with room under it (a margin), and a
    /// child that takes no room in the flow.
    private static let plain = """
    component Plain
      state seed = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
      derive d1 = concat(seed, seed)
      derive d2 = concat(d1, d1)
      derive d3 = concat(d2, d2)
      state from = 0
      derive rows = slice(map(d3, (x, i) => i), from, 60)
      state opened = 0 - 1
      state presses = 0
      action open(r: number)
        opened = r
        presses = presses + 1
      action drop
        from = from + 1
      view
        column width="100%" height="100%"
          row gap=8 padding=4
            text `opened ${opened} presses ${presses}` testId="status"
            button press=drop testId="drop"
              text "Drop"
          list flex=1 min-height=0 width="100%" padding="10px 16px" box-sizing="border-box" testId="rows"
            text "Sixty rows" testId="heading" padding="6px 0"
            box position="absolute" top=0 right=0 width=10 height=10 testId="badge"
            each r in rows key=`k${r}`
              row press=open(r) testId=`row-${r}` padding="6px 0" margin-bottom=8 width="100%"
                text `Row ${r}` testId=`title-${r}`

    """

    /// Every row the port shows is in its cell, where layout put it, and
    /// the table's rows tile the list: a row's cell is layout's box and
    /// half the room layout left on each side of it.
    private func check(_ f: TableFixture, _ note: String, rows: Int, file: StaticString = #filePath, line: UInt = #line) throws {
        XCTAssertEqual(f.table.numberOfRows, rows, note, file: file, line: line)
        let port = f.clip.bounds
        let shown = f.table.rows(in: f.table.convert(port, from: f.document))
        XCTAssertGreaterThan(shown.length, 3, note, file: file, line: line)
        var end: CGFloat?
        for index in shown.location..<shown.location + shown.length {
            let cell = try XCTUnwrap(f.table.view(atColumn: 0, row: index, makeIfNecessary: false) as? TableRowCell, "\(note): row \(index) shows and has a cell", file: file, line: line)
            let node = try XCTUnwrap(cell.row, "\(note): row \(index)'s cell holds a row", file: file, line: line)
            XCTAssertTrue(node.superview === cell, note, file: file, line: line)
            XCTAssertEqual(f.inDocument(node), cell.kernel, "\(note): row \(index) is where layout put it", file: file, line: line)
            let rect = f.table.convert(f.table.rect(ofRow: index), to: f.document)
            XCTAssertTrue(rect.insetBy(dx: 0, dy: -0.01).contains(NSRect(x: rect.minX, y: cell.kernel.minY, width: 1, height: cell.kernel.height)), "\(note): row \(index) is inside its table row (\(cell.kernel) in \(rect))", file: file, line: line)
            if let end { XCTAssertEqual(rect.minY, end, accuracy: 0.01, "\(note): row \(index) starts where the one before ends", file: file, line: line) }
            end = rect.maxY
        }
    }

    func testAListTheRunnerDoesNotWindowHasItsChildrenAsTheTablesRows() throws {
        let f = try TableFixture(count: 0, source: Self.plain)
        fixture = f
        XCTAssertFalse(f.presenter.collections.owns(f.list.id), "the runner does not window this list")
        XCTAssertTrue(f.table.superview === f.document, "the table is in the list's own scroller")
        // The heading and the sixty rows; the box out of the flow is no row, and stays in the document.
        try check(f, "at the top", rows: 61)
        XCTAssertTrue(try f.node("badge").superview === f.document)
        XCTAssertTrue(try f.node("heading").superview is TableRowCell)
        // Room under a row is shared with the row after it: half to each, so the row sits 4 points inside its cell.
        let third = try f.node("row-2"), cell = try XCTUnwrap(third.superview as? TableRowCell)
        XCTAssertEqual(f.table.rect(ofRow: 3).height, third.frame.height + 8, "a row's cell is its box and half the margin on each side")
        XCTAssertEqual(third.frame.minY, 4)
        XCTAssertEqual(cell.kernel.height, third.frame.height)
        // A row that presses is selected and pressed; the heading, which presses nothing, is neither.
        XCTAssertFalse(f.feed.tableView(f.table, shouldSelectRow: 0), "a row with no press is a row of text")
        XCTAssertFalse(f.feed.pressRow(0))
        XCTAssertTrue(f.feed.tableView(f.table, shouldSelectRow: 6))
        f.table.selectRowIndexes([6], byExtendingSelection: false)
        XCTAssertTrue(f.feed.pressRow(6))
        f.settle()
        XCTAssertEqual(f.status, "opened 5 presses 1")
        // Down the list: every row is mounted (the runner windows nothing), and the table takes each into a cell as it shows.
        XCTAssertEqual(f.mountedRows, 60)
        for y in stride(from: CGFloat(0), through: 1_800, by: 257) {
            f.scroll(to: y)
            try check(f, "at \(y)", rows: 61)
        }
        XCTAssertLessThan(f.feed.rowViews, 50, "the table keeps views for what shows, not for sixty rows")
        f.scroll(to: 0)
        // An item goes: the table has a row fewer, and the selection is still its item's, a row up.
        XCTAssertEqual(f.table.selectedRow, 6)
        f.presenter.press(try f.node("drop").id)
        f.settle()
        try check(f, "a row dropped", rows: 60)
        XCTAssertEqual(f.table.selectedRow, 5, "the selection moved with its item")
        XCTAssertTrue((f.table.view(atColumn: 0, row: 5, makeIfNecessary: false) as? TableRowCell)?.row === (try f.node("row-5")))
        XCTAssertFalse(f.presenter.views.values.contains { $0.props["testId"] == "row-0" }, "the dropped row is gone")
    }

    /// Rows as the list-rows recipe writes them, and Shelf's library: each a
    /// snap scroller that names its content, a link to the row's route, and
    /// its swipe action, a button.
    private static let swiped = """
    routes nav
      items "/"
        item "/item/:id"

    component Swiped
      state seed = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
      state removed = 0 - 1
      action remove(r: number)
        removed = r
      action back
        nav = back(nav)
      action follow(url: string)
        nav = go(nav, url)
      view
        main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow width="100%" height="100%"
          each e in stack(nav) key=e.id
            when e.name == "items"
              column navigationKey=`${e.id}` position="absolute" inset=0
                text `removed ${removed}` testId="status"
                list flex=1 min-height=0 width="100%" testId="rows"
                  each r in seed key=`k${r}`
                    scroll swipeContent=`row-${r}` swipeTrailing=`delete-${r}` testId=`swipe-${r}` width="100%" overflow-x="scroll" overflow-y="hidden" scrollbar-width="none" scroll-snap-type="x mandatory"
                      row width="100%"
                        link id=`row-${r}` href=`/item/${r}` testId=`row-${r}` width="100%" flex-shrink=0 scroll-snap-align="start" display="flex" padding="10px 16px"
                          text `Row ${r}` testId=`title-${r}`
                        button id=`delete-${r}` destructive=true press=remove(r) testId=`delete-${r}` flex-shrink=0 scroll-snap-align="start"
                          text "Delete"
            when e.name == "item"
              column navigationKey=`${e.id}` position="absolute" inset=0 padding=16 gap=8
                text `Item ${e.params.id}` testId="detail"
                button id="back" press=back testId="back"
                  text "Back"

    """

    func testARowThatIsASwipeContainerIsPressedThroughTheContentItNames() throws {
        let f = try TableFixture(count: 0, source: Self.swiped)
        fixture = f
        XCTAssertEqual(f.table.numberOfRows, 10)
        let swipe = try f.node("swipe-3"), link = try f.node("row-3")
        let cell = try XCTUnwrap(swipe.superview as? TableRowCell, "the row is its swipe container, in a cell")
        XCTAssertFalse(swipe.pressable, "the container itself presses nothing")
        XCTAssertTrue(f.feed.row(at: 3).flatMap(f.feed.pressing) === link, "the row's press is the content the container names")
        XCTAssertTrue(f.feed.tableView(f.table, shouldSelectRow: 3))
        // A click on the row's text is the table's, which selects the row and follows the link.
        let title = try f.node("title-3")
        let onTitle = cell.convert(NSPoint(x: title.bounds.midX, y: title.bounds.midY), from: title)
        XCTAssertTrue(cell.hitTest(cell.superview!.convert(onTitle, from: cell)) === f.table, "the table takes a click on the row")
        XCTAssertTrue(TableRowCell.whatIsThere { cell.hitTest(cell.superview!.convert(onTitle, from: cell)) } === title, "and the hover path sees the row's own text there")
        // The driver's `layout` names the row from its container and from its link.
        XCTAssertEqual(f.presenter.tables.observation(link)?["row"] as? Int, 3)
        XCTAssertEqual(f.presenter.tables.observation(swipe)?["row"] as? Int, 3)
        XCTAssertEqual(f.presenter.tables.observation(link)?["selectable"] as? Bool, true)
        f.table.selectRowIndexes([3], byExtendingSelection: false)
        XCTAssertTrue(f.feed.pressRow(3))
        f.settle()
        XCTAssertEqual(try f.node("detail").accessibleText, "Item 3", "pressing the row followed its link")
        XCTAssertTrue(f.feed.keyTarget === link, "and the keys of the selected row are its link's")
    }
}

private extension TableFixture {
    /// The rows of a list the runner does not window, wherever each is mounted.
    var mountedRows: Int { presenter.views.values.filter { ($0.props["testId"] ?? "").hasPrefix("row-") && $0.window != nil }.count }
}
#endif

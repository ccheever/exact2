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
      derive rows = seed.concat(seed)
      state opened = 0 - 1
      state display = "block"
      state direction = "column"
      action open(r: number)
        opened = r
      view
        column width="100%" height="100%" gap=4
          text `opened ${opened}` testId="status"
          list flex=1 min-height=0 width="100%" testId="rows"
            for r in seed key=`p${r}`
              row press=open(r) padding="6px 16px" testId=`plain-${r}`
                text `Plain ${r}`
          list virtualized=true estimated-item-height=28 flex=1 min-height=0 width="100%" testId="windowed"
            for r in rows key=`w${r}`
              row press=open(r) padding="6px 16px"
                text `Windowed ${r}`
          list appearance="none" flex=1 min-height=0 width="100%" testId="opted-out"
            for r in seed key=`n${r}`
              row press=open(r) padding="6px 16px"
                text `Authored ${r}`
          list appearance="auto" flex=1 min-height=0 width="100%" testId="grouped"
            section
              for r in seed key=`g${r}`
                row press=open(r)
                  text `Grouped ${r}`
          list virtualized=true display="flex" width="100%" height=60 overflow-x="scroll" overflow-y="hidden" estimated-item-width=80 testId="strip"
            for r in rows key=`s${r}`
              row press=open(r) width=80 height=44
                text `S ${r}`
          list display="flex" width="100%" height=60 overflow-x="scroll" overflow-y="hidden" testId="eager-strip"
            for r in seed key=`e${r}`
              row press=open(r) width=80 height=44 flex-shrink=0
                text `E ${r}`
          list display="flex" flex-wrap="wrap" width="100%" height=60 testId="wrapped"
            for r in seed key=`f${r}`
              row press=open(r) width=80 height=44
                text `F ${r}`
          list display=display width="100%" height=60 testId="bound-display"
            text "A bound display keeps authored rows"
          list display="flex" flex-direction=direction width="100%" height=60 testId="bound-direction"
            text "A bound direction keeps authored rows"
          list virtualized=true estimated-item-height=120 flex=1 min-height=0 width="100%" testId="outer"
            for r in seed key=`o${r}`
              column height=120 width="100%"
                text `Outer ${r}`
                list flex=1 min-height=0 width="100%" testId=`inner-${r}`
                  for q in seed key=`i${q}`
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
        for name in ["bound-display", "bound-direction"] {
            let node = try list(name)
            XCTAssertNotEqual(node.props["listRows"], "true", "Contract does not mark a list whose layout is bound")
            XCTAssertNil(tables.feeds[node.id], "\(name): AppKit follows Contract's mark even when the bound value currently lays rows out vertically")
        }
        XCTAssertNil(tables.feeds[try list("inner-0").id], "a list in another list's row keeps its painted rows")
        // The driver's own chrome paints every list's rows, and gives them back.
        tables.painted = true
        XCTAssertTrue(tables.feeds.isEmpty)
        XCTAssertTrue(try f.node("plain-3").superview === list("rows").scroll?.documentView, "a painted list's rows are in its document")
        tables.painted = false
        f.settle()
        XCTAssertNotNil(tables.feeds[try list("rows").id])
        XCTAssertNotNil(tables.feeds[try list("windowed").id])
        for name in ["bound-display", "bound-direction"] {
            XCTAssertNil(tables.feeds[try list(name).id], "\(name): changing chrome keeps the same admission rule")
        }
    }

    /// A list the runner does not window: a heading that presses nothing,
    /// then sixty rows that open, each with room under it (a margin), and a
    /// child that takes no room in the flow.
    private static let plain = """
    component Plain
      state seed = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
      derive d1 = seed.concat(seed)
      derive d2 = d1.concat(d1)
      derive d3 = d2.concat(d2)
      state from = 0
      derive rows = d3.map((x, i) => i).slice(from, 60)
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
            for r in rows key=`k${r}`
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
          for e in stack(nav) key=e.id
            if e.name == "items"
              column navigationKey=`${e.id}` position="absolute" inset=0
                text `removed ${removed}` testId="status"
                list flex=1 min-height=0 width="100%" testId="rows"
                  for r in seed key=`k${r}`
                    scroll swipeContent=`row-${r}` swipeTrailing=`delete-${r}` testId=`swipe-${r}` width="100%" overflow-x="scroll" overflow-y="hidden" scrollbar-width="none" scroll-snap-type="x mandatory"
                      row width="100%"
                        link id=`row-${r}` href=`/item/${r}` testId=`row-${r}` width="100%" flex-shrink=0 scroll-snap-align="start" display="flex" align-items="center" padding="10px 16px"
                          text `Row ${r}` testId=`title-${r}` flex=1
                          image "symbol:forward-chevron" aria-hidden=true testId=`chevron-${r}`
                        button id=`delete-${r}` destructive=true press=remove(r) testId=`delete-${r}` flex-shrink=0 scroll-snap-align="start"
                          text "Delete"
            if e.name == "item"
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
        // No disclosure chevron on a table's row: its glyph is not drawn in the cell, and is in the painted list.
        f.presenter.press(try f.node("back").id)
        f.settle()
        let chevron = try f.node("chevron-3")
        XCTAssertNotNil(chevron.symbolView?.image, "the row holds a chevron")
        XCTAssertEqual(chevron.symbolView?.alphaValue, 0, "which a table's row does not draw")
        XCTAssertGreaterThan(chevron.frame.width, 0, "its box is layout's still")
        f.presenter.tables.painted = true
        XCTAssertEqual(try f.node("chevron-3").symbolView?.alphaValue, 1, "painted rows draw theirs")
    }
}

extension TableListMacTests {
    /// A fix to stage 3b, found on Shelf's library as a `list` (its test "a
    /// swipe deletes" under the platform's chrome): a row is as wide as its
    /// list and an inset table's cell is narrower by the table's margin at
    /// each side, so what a row shows beside its cell was drawn and could
    /// not be clicked. Shelf's swipe action, a symbol 15 points wide at the
    /// row's trailing end, was all of it there: the click went to the table.
    func testWhatARowShowsBesideItsInsetCellIsStillTheRowsToClick() throws {
        let f = try TableFixture(count: 0, source: Self.swiped)
        fixture = f
        let swipe = try f.node("swipe-3"), delete = try f.node("delete-3")
        let cell = try XCTUnwrap(swipe.superview as? TableRowCell)
        let inWindow = { (view: NSView) in view.convert(view.bounds, to: nil) }
        XCTAssertGreaterThan(inWindow(swipe).maxX, inWindow(cell).maxX, "the row stands out of its inset cell at the trailing side")
        // The swipe, as two fingers on the row or the driver's `reveal` make it: the action comes into the row's port.
        let clip = try XCTUnwrap(swipe.scroll?.contentView)
        let shown = delete.convert(delete.bounds, to: clip)
        clip.scroll(to: clip.constrainBoundsRect(NSRect(x: shown.maxX - clip.bounds.width, y: 0, width: clip.bounds.width, height: clip.bounds.height)).origin)
        swipe.scroll?.reflectScrolledClipView(clip)
        f.settle()
        let button = inWindow(delete)
        XCTAssertEqual(button.maxX, inWindow(swipe).maxX, accuracy: 0.5, "the action is at the row's trailing end")
        // A point of the action beside the cell, and one inside it: a click at either is the button's own.
        for (x, where_) in [(button.maxX - 4, "beside the cell"), (button.minX + 4, "in the cell")] {
            let point = NSPoint(x: x, y: button.midY)
            XCTAssertEqual(inWindow(cell).contains(point), where_ == "in the cell", where_)
            let hit = try XCTUnwrap(f.window.contentView?.hitTest(point), "a click \(where_) lands on something")
            XCTAssertFalse(hit === f.table, "a click on the action \(where_) is not the table's")
            XCTAssertTrue(hit === delete || hit.isDescendant(of: delete), "it is the action's: \(hit)")
            XCTAssertTrue(TableRowCell.whatIsThere { f.window.contentView?.hitTest(point) }.map { $0 === delete || $0.isDescendant(of: delete) } ?? false, "and the hover path and the driver's aim see it \(where_)")
        }
        // The click itself, at the point beside the cell: the action runs, and the row is neither selected nor followed.
        let point = NSPoint(x: button.maxX - 4, y: button.midY)
        let event = { (type: NSEvent.EventType) in
            NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: f.window.windowNumber,
                               context: nil, eventNumber: 0, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!
        }
        let hit = try XCTUnwrap(f.window.contentView?.hitTest(point))
        hit.mouseDown(with: event(.leftMouseDown))
        hit.mouseUp(with: event(.leftMouseUp))
        f.settle()
        XCTAssertEqual(try f.node("status").accessibleText, "removed 3", "the swipe's action was pressed")
        XCTAssertEqual(f.table.selectedRow, -1, "and its row was not selected")
        // The leading margin holds nothing of this row's: a click there is the table's, as before.
        let lead = NSPoint(x: inWindow(swipe).minX + 4, y: button.midY)
        XCTAssertFalse(inWindow(cell).contains(lead))
        XCTAssertTrue(f.window.contentView?.hitTest(lead) === f.table, "a click on the row beside its cell, where nothing keeps it, is the table's")
    }
}

extension TableListMacTests {
    /// An interior authored line has room after it, so a second native line
    /// cannot disappear beneath the author's red pixels. The other owned
    /// boundaries say `none`, including the top of the following row.
    private static func separatorSource(windowed: Bool = false) -> String {
        """
        component Separators
          state seed = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
          derive d1 = seed.concat(seed)
          derive d2 = d1.concat(d1)
          derive d3 = d2.concat(d2)
          derive d4 = d3.concat(d3)
          derive rows = d4.map((n, i) => i).slice(0, \(windowed ? 160 : 10))
          state edge = 0
          state bumps = 0
          action change(n: number)
            edge = n
          action bump
            bumps = bumps + 1
          view
            column width="100%" height="100%"
              row gap=8
                button press=change(1) testId="own"
                  text "Own bottom"
                button press=change(2) testId="next"
                  text "Own next top"
                button press=change(0) testId="clear"
                  text "Clear"
                button press=change(3) testId="hide-next"
                  text "Hide next"
                text `Bumps ${bumps}` testId="status"
              list \(windowed ? "virtualized=true estimated-item-height=48" : "") flex=1 min-height=0 width="100%" padding="10px 16px" box-sizing="border-box" testId="rows"
                for r in rows key=`k${r}`
                  row testId=`edge-${r}` height=40 width="100%" box-sizing="border-box" align-items="center"
                    border-bottom=(r % 10 == 1 ? "3px solid #ff0000" : (r % 10 == 2 or (r == 5 and edge == 1) ? "none" : none))
                    border-top=(r % 10 == 4 or (r == 6 and edge == 2) ? "none" : none)
                    border-top-width=(r == 6 and edge == 3 ? 0 : none)
                    \(windowed ? "" : "display=(r == 6 and edge == 3 ? \"none\" : \"flex\")")
                    margin-bottom=\(windowed ? "8" : "(r % 10 == 0 or r % 10 == 1 ? 8 : 0)")
                    if r % 10 != 8
                      text `Row ${r}` flex=1
                    if r % 10 == 7
                      button press=bump testId=`control-${r}`
                        text "Bump"

        """
    }

    /// A 1x capture of the list, shared by all boundary checks in a state.
    private func separatorPicture(_ f: TableFixture, name: String, list: NodeView? = nil) throws -> NSBitmapImageRep {
        let view = try XCTUnwrap((list ?? f.list).scroll)
        f.window.layoutIfNeeded()
        f.window.displayIfNeeded()
        let size = view.bounds.size
        let rep = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(size.width), pixelsHigh: Int(size.height), bitsPerSample: 8,
                                                 samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
        rep.size = size
        view.cacheDisplay(in: view.bounds, to: rep)
        if let directory = ProcessInfo.processInfo.environment["EXACT_PROOF_SHOTS"] {
            let url = URL(fileURLWithPath: directory).appendingPathComponent("separator-\(name).png")
            XCTAssertFalse(FileManager.default.fileExists(atPath: url.path), "proof captures must not overwrite an earlier run")
            if !FileManager.default.fileExists(atPath: url.path) {
                try XCTUnwrap(rep.representation(using: .png, properties: [:])).write(to: url)
            }
        }
        return rep
    }

    private func separatorBoundary(_ f: TableFixture, row: NodeView, list: NodeView? = nil) throws -> CGFloat {
        let list = list ?? f.list, view = try XCTUnwrap(list.scroll)
        if let feed = f.presenter.tables.feeds[list.id], let item = feed.rows.first(where: { $0.node === row || $0.root == row.id }) {
            return feed.table.convert(feed.table.rect(ofRow: item.index), to: view).maxY
        }
        if let entry = f.presenter.collections.entries[list.id] {
            let wrapper = try XCTUnwrap(entry.snapshot.rows.compactMap { f.presenter.views[$0.view] }
                .first { row === $0 || row.isDescendant(of: $0) }, "the authored row is in a current mounted wrapper")
            XCTAssertNotNil(wrapper.props["listItemKey"])
            return wrapper.convert(wrapper.bounds, to: view).maxY
        }
        return row.convert(row.bounds, to: view).maxY
    }

    /// Nothing in this central crop is text or a control. In these clear
    /// rows its alpha is the separator's, not a background or selection's.
    private func boundaryInk(_ rep: NSBitmapImageRep, y: CGFloat) throws -> Int {
        let center = Int(y.rounded()), x = rep.pixelsWide / 2
        XCTAssertGreaterThanOrEqual(center - 2, 0)
        XCTAssertLessThan(center + 1, rep.pixelsHigh)
        guard center >= 2, center + 1 < rep.pixelsHigh else { return -1 }
        var count = 0
        for yy in center - 2...center + 1 {
            for xx in x - 1...x + 1 {
                if try XCTUnwrap(rep.colorAt(x: xx, y: yy)).alphaComponent > 0.02 { count += 1 }
            }
        }
        return count
    }

    private func separatorColor(_ appearance: NSAppearance) throws -> NSColor {
        var resolved: NSColor?
        appearance.performAsCurrentDrawingAppearance { resolved = NSColor.separatorColor.usingColorSpace(.deviceRGB) }
        return try XCTUnwrap(resolved)
    }

    private func lineColor(_ rep: NSBitmapImageRep, y: CGFloat) throws -> NSColor {
        return try XCTUnwrap(rep.colorAt(x: rep.pixelsWide / 2, y: Int(y.rounded()) - 1)?.usingColorSpace(.deviceRGB))
    }

    private func lineSpan(_ rep: NSBitmapImageRep, y: CGFloat, near expected: ClosedRange<Int>) throws -> ClosedRange<Int> {
        let y = Int(y.rounded()) - 1
        let from = max(0, expected.lowerBound - 2), through = min(rep.pixelsWide - 1, expected.upperBound + 2)
        let ink = try (from...through).filter { try XCTUnwrap(rep.colorAt(x: $0, y: y)).alphaComponent > 0.02 }
        let first = try XCTUnwrap(ink.first), last = try XCTUnwrap(ink.last)
        return first...last
    }

    private func assertSameColor(_ actual: NSColor, _ expected: NSColor, _ note: String,
                                 file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertEqual(actual.redComponent, expected.redComponent, accuracy: 1.0 / 255.0, note, file: file, line: line)
        XCTAssertEqual(actual.greenComponent, expected.greenComponent, accuracy: 1.0 / 255.0, note, file: file, line: line)
        XCTAssertEqual(actual.blueComponent, expected.blueComponent, accuracy: 1.0 / 255.0, note, file: file, line: line)
        XCTAssertEqual(actual.alphaComponent, expected.alphaComponent, accuracy: 1.0 / 255.0, note, file: file, line: line)
    }

    private func assertSeparator(_ f: TableFixture, _ rep: NSBitmapImageRep, row: String, present: Bool, note: String, list: NodeView? = nil,
                                 file: StaticString = #filePath, line: UInt = #line) throws {
        let y = try separatorBoundary(f, row: f.node(row), list: list)
        let ink = try boundaryInk(rep, y: y)
        if present {
            XCTAssertGreaterThan(ink, 0, "\(note), \(row), y=\(y): the unowned boundary has a separator", file: file, line: line)
            // The same-OS reference is alpha25/255. Two default lines painted
            // over each other are about49/255, including after a chrome change.
            let center = Int(y.rounded()), x = rep.pixelsWide / 2
            if center >= 2, center + 1 < rep.pixelsHigh {
                for yy in center - 2...center + 1 {
                    XCTAssertLessThan(try XCTUnwrap(rep.colorAt(x: x, y: yy)).alphaComponent, 0.15,
                                      "\(note): one default separator, not two composited lines", file: file, line: line)
                }
            }
        }
        else { XCTAssertEqual(ink, 0, "\(note), \(row), y=\(y): no platform separator at an owned boundary", file: file, line: line) }
    }

    private func checkInteriorSeparators(_ f: TableFixture, name: String, last: Bool) throws {
        let picture = try separatorPicture(f, name: name)
        for (row, present) in [(0, true), (2, false), (3, false), (4, true), (5, true), (6, true), (8, true)] {
            try assertSeparator(f, picture, row: "edge-\(row)", present: present, note: name)
        }
        if last { try assertSeparator(f, picture, row: "edge-9", present: false, note: name) }
        let own = try f.node("edge-1"), view = try XCTUnwrap(f.list.scroll)
        let end = own.convert(own.bounds, to: view).maxY
        // The author's three red pixels remain; the gap below them stays clear.
        let x = picture.pixelsWide / 2
        for y in Int(end.rounded()) - 3..<Int(end.rounded()) {
            let color = try XCTUnwrap(picture.colorAt(x: x, y: y)?.usingColorSpace(.sRGB))
            XCTAssertGreaterThan(color.alphaComponent, 0.95, name)
            XCTAssertGreaterThan(color.redComponent, 0.95, name)
            XCTAssertLessThan(color.greenComponent, 0.05, name)
            XCTAssertLessThan(color.blueComponent, 0.05, name)
        }
        let boundary = try separatorBoundary(f, row: own)
        // In the painted list the platform would draw at the authored edge;
        // in the native table it draws half way through the eight-point gap.
        if f.presenter.tables.feeds[f.list.id] != nil {
            XCTAssertGreaterThan(boundary - end, 2)
            XCTAssertEqual(try boundaryInk(picture, y: boundary), 0, "\(name): no second line below the authored red edge")
        }
        XCTAssertEqual(try boundaryInk(picture, y: end + 4), 0, "\(name): the gap holds no second line")
        let control = try f.node("control-7"), point = control.convert(NSPoint(x: control.bounds.midX, y: control.bounds.midY), to: nil)
        let hit = try XCTUnwrap(f.window.contentView?.hitTest(point))
        XCTAssertTrue(hit === control || hit.isDescendant(of: control), "a row's own button keeps its hit in \(name)")
    }

    func testInteriorAuthoredEdgesWinAndPaintedRowsKeepUnownedSeparators() throws {
        let f = try TableFixture(count: 0, source: Self.separatorSource())
        fixture = f
        let retained = try (0..<10).map { try f.node("edge-\($0)") }
        for (name, painted) in [("native", false), ("painted", true), ("native-again", false)] {
            f.presenter.tables.painted = painted
            f.settle()
            XCTAssertEqual(f.presenter.tables.feeds[f.list.id] == nil, painted)
            for (i, row) in retained.enumerated() { XCTAssertTrue(try f.node("edge-\(i)") === row, "changing chrome retains the authored rows") }
            try checkInteriorSeparators(f, name: name, last: true)
            let gapped = try f.node("edge-0"), view = try XCTUnwrap(f.list.scroll)
            let authoredEnd = gapped.convert(gapped.bounds, to: view).maxY
            let boundary = try separatorBoundary(f, row: gapped)
            XCTAssertEqual(boundary - authoredEnd, painted ? 0 : 4, accuracy: 0.01,
                           "\(name): native owns the middle of positive room; painted owns the authored edge")
            let textless = try f.node("edge-8")
            let textlessBoundary = try separatorBoundary(f, row: textless)
            let picture = try separatorPicture(f, name: "\(name)-textless-inset")
            XCTAssertGreaterThan(try boundaryInk(picture, y: boundary), 0, "\(name): the gapped row has its separator at the claimed boundary")
            if painted {
                XCTAssertEqual(try boundaryInk(picture, y: authoredEnd + 4), 0, "\(name): painted chrome does not move the line into CSS room")
            } else {
                XCTAssertEqual(try boundaryInk(picture, y: authoredEnd), 0, "\(name): native chrome leaves the authored edge clear")
            }
            let authored = textless.convert(textless.bounds, to: view)
            let expected = Int(authored.minX.rounded())...Int(authored.maxX.rounded()) - 1
            let span = try lineSpan(picture, y: textlessBoundary, near: expected)
            XCTAssertEqual(span.lowerBound, Int(authored.minX.rounded()), "\(name): a textless separator starts at the row")
            XCTAssertEqual(span.upperBound + 1, Int(authored.maxX.rounded()), "\(name): a textless separator ends at the row")
            if let cell = textless.superview as? TableRowCell {
                let start = cell.convert(NSPoint(x: try XCTUnwrap(cell.textField).frame.minX + 2, y: 0), to: textless).x
                XCTAssertEqual(start, textless.bounds.minX, accuracy: 0.01, "\(name): a textless native row begins its separator at the authored row edge")
            } else {
                let line = try XCTUnwrap(textless.layer?.sublayers?.first {
                    $0.backgroundColor != nil && abs($0.frame.height - 1) < 0.01 && abs($0.frame.maxY - textless.bounds.maxY) < 0.01
                })
                XCTAssertEqual(line.frame.minX, textless.bounds.minX, accuracy: 0.01, "\(name): a textless painted row begins its separator at the authored row edge")
            }
        }
    }

    func testSeparatorOwnershipChangesAndClearsOnTheSameRows() throws {
        let f = try TableFixture(count: 0, source: Self.separatorSource())
        fixture = f
        let own = try f.node("edge-5"), next = try f.node("edge-6"), bits = try f.node("edge-8")
        for painted in [false, true] {
            let mode = painted ? "painted" : "native"
            f.presenter.tables.painted = painted
            f.settle()
            let before = own.convert(own.bounds, to: f.document)
            for (step, item) in [("clear", true, false, false), ("own", false, true, false), ("clear", true, false, false),
                                  ("next", false, false, true), ("clear", true, false, false)].enumerated() {
                let (button, separated, bottom, top) = item
                f.presenter.press(try f.node(button).id)
                f.settle()
                XCTAssertTrue(try f.node("edge-5") === own)
                XCTAssertTrue(try f.node("edge-6") === next)
                XCTAssertEqual(own.convert(own.bounds, to: f.document), before, "ownership changes no geometry")
                XCTAssertEqual(own.wrote("border_style_bottom"), bottom, "a none binding clears ownership")
                XCTAssertEqual(next.wrote("border_style_top"), top, "the following row owns the preceding boundary")
                let picture = try separatorPicture(f, name: "\(mode)-binding-\(step)")
                try assertSeparator(f, picture, row: "edge-5", present: separated, note: "\(mode) \(button)")
                try assertSeparator(f, picture, row: "edge-6", present: true, note: "\(mode) \(button): the following row's bottom is unowned")
            }
            // A host batch may change provenance alone. Keep every effective
            // value, the same NodeView and the same frame for every state.
            let original = bits.style
            var plain = original
            plain["border_style_bottom"] = .string("none")
            plain["border_width_bottom"] = .number(0)
            let written = (plain["written"]?.array ?? []).filter { !["border_style_bottom", "border_width_bottom", "box_shadow"].contains($0.string ?? "") }
            plain["written"] = .array(written)
            let frame = bits.convert(bits.bounds, to: f.document)
            let owners: [String?] = [nil, "border_style_bottom", nil, "border_width_bottom", nil, "box_shadow", nil]
            for (step, owner) in owners.enumerated() {
                var style = plain
                if let owner { style["written"] = .array(written + [.string(owner)]) }
                f.presenter.apply(wireBatch([["op": "style", "id": bits.id, "style": style.mapValues(\.any)]]))
                f.settle()
                XCTAssertTrue(try f.node("edge-8") === bits)
                XCTAssertEqual(bits.convert(bits.bounds, to: f.document), frame)
                XCTAssertEqual(bits.style.filter { $0.key != "written" }, plain.filter { $0.key != "written" })
                for name in ["border_style_bottom", "border_width_bottom", "box_shadow"] { XCTAssertEqual(bits.wrote(name), owner == name) }
                let picture = try separatorPicture(f, name: "\(mode)-bits-\(step)")
                try assertSeparator(f, picture, row: "edge-8", present: owner == nil, note: "\(mode): only ownership changed, step \(step)")
            }
            f.presenter.apply(wireBatch([["op": "style", "id": bits.id, "style": original.mapValues(\.any)]]))
            let hidden = bits.props["accessibilityElementsHidden"]
            for (step, hidden) in [true, false].enumerated() {
                f.presenter.apply(wireBatch([["op": "props", "id": bits.id, "set": ["accessibilityElementsHidden": hidden ? "true" : "false"]]]))
                f.settle()
                XCTAssertTrue(try f.node("edge-8") === bits)
                XCTAssertFalse(bits.isHidden, "decorative row intent does not hide its authored pixels")
                let picture = try separatorPicture(f, name: "\(mode)-decorative-\(step)")
                try assertSeparator(f, picture, row: "edge-8", present: !hidden, note: "\(mode): decorative rows have no line")
            }
            f.presenter.apply(wireBatch([["op": "props", "id": bits.id, "set": hidden.map { ["accessibilityElementsHidden": $0] } ?? [:],
                                          "clear": hidden == nil ? ["accessibilityElementsHidden"] : []]]))
            // Drive display through Contract so the runner publishes the
            // hidden and restored frames as well as the authored top edge.
            let nextBefore = next.convert(next.bounds, to: f.document)
            let observedLock = NSLock()
            var observed: [(Data, Batch)] = []
            f.session.runtime.observeBatch = { data, batch in
                observedLock.lock(); defer { observedLock.unlock() }
                observed.append((data, batch))
            }
            defer { f.session.runtime.observeBatch = nil }
            for (step, hide) in [true, false].enumerated() {
                observedLock.lock(); observed.removeAll(); observedLock.unlock()
                f.presenter.press(try f.node(hide ? "hide-next" : "clear").id)
                f.settle()
                observedLock.lock(); let captured = observed; observedLock.unlock()
                let frames = captured.flatMap { $0.1.ops.filter { $0.op == .frame && $0.id == next.id } }
                if let directory = ProcessInfo.processInfo.environment["EXACT_PROOF_SHOTS"] {
                    func rect(_ r: NSRect) -> [Double] { [r.minX, r.minY, r.width, r.height].map(Double.init) }
                    let receipt: [String: Any] = ["hidden": hide, "row": own.id, "next": next.id,
                        "before": rect(nextBefore), "actualViewFrame": rect(next.frame),
                        "actualDocumentFrame": next.superview == nil ? NSNull() : rect(next.convert(next.bounds, to: f.document)),
                        "boundary": try separatorBoundary(f, row: own),
                        "batches": try captured.map { try JSONSerialization.jsonObject(with: $0.0) }]
                    let url = URL(fileURLWithPath: directory).appendingPathComponent("separator-\(mode)-hidden-next-\(step).json")
                    XCTAssertFalse(FileManager.default.fileExists(atPath: url.path), "proof receipts must not overwrite an earlier run")
                    if !FileManager.default.fileExists(atPath: url.path) {
                        try JSONSerialization.data(withJSONObject: receipt, options: [.prettyPrinted, .sortedKeys]).write(to: url)
                    }
                }
                XCTAssertTrue(try f.node("edge-6") === next)
                XCTAssertEqual(next.isHidden, hide)
                XCTAssertEqual(next.wrote("border_width_top"), hide)
                XCTAssertTrue(captured.allSatisfy { $0.1.error == nil })
                let published = try XCTUnwrap(frames.last, "the actual runner publishes the changed hidden/restored frame")
                XCTAssertEqual(published.w, hide ? 0 : Double(nextBefore.width))
                XCTAssertEqual(published.h, hide ? 0 : Double(nextBefore.height))
                if !hide { XCTAssertEqual(next.convert(next.bounds, to: f.document), nextBefore, "restoration returns to the runner's original frame") }
                XCTAssertEqual(own.convert(own.bounds, to: f.document), before, "hiding the next row leaves the predecessor's box")
                if !painted { XCTAssertEqual(f.presenter.tables.feeds[f.list.id]?.rows.contains { $0.node === next }, !hide) }
                let picture = try separatorPicture(f, name: "\(mode)-hidden-next-\(step)")
                try assertSeparator(f, picture, row: "edge-5", present: !hide, note: "\(mode): logical next child owns the boundary")
            }
        }
    }

    func testSeparatorColorFollowsTheWindowAndAnAuthoredLocalScheme() throws {
        let source = """
        component SeparatorSchemes
          view
            column width="100%" height="100%" gap=12 padding=12 box-sizing="border-box"
              list height=100 width="100%" testId="rows"
                text "Inherited first" height=40 padding="8px 16px" box-sizing="border-box" testId="inherited-0"
                text "Inherited last" height=40 padding="8px 16px" box-sizing="border-box" testId="inherited-1"
              list height=100 width="100%" color-scheme="light" testId="fixed-list"
                text "Fixed first" height=40 padding="8px 16px" box-sizing="border-box" testId="fixed-0"
                text "Fixed last" height=40 padding="8px 16px" box-sizing="border-box" testId="fixed-1"

        """
        let f = try TableFixture(count: 0, source: source)
        fixture = f
        let inherited = try f.node("inherited-0"), fixed = try f.node("fixed-0"), fixedList = try f.node("fixed-list")
        for (mode, painted) in [("native", false), ("painted", true)] {
            f.presenter.tables.painted = painted
            f.window.appearance = NSAppearance(named: .aqua)
            f.settle()
            let inheritedLightPicture = try separatorPicture(f, name: "\(mode)-scheme-light")
            let fixedLightPicture = try separatorPicture(f, name: "\(mode)-fixed-light", list: fixedList)
            let light = try lineColor(inheritedLightPicture, y: separatorBoundary(f, row: inherited))
            let fixedBefore = try lineColor(fixedLightPicture, y: separatorBoundary(f, row: fixed, list: fixedList))
            try assertSameColor(light, separatorColor(inherited.effectiveAppearance), "\(mode): inherited light separator")
            try assertSameColor(fixedBefore, separatorColor(fixed.effectiveAppearance), "\(mode): fixed light separator")

            f.window.appearance = NSAppearance(named: .darkAqua)
            f.settle()
            let inheritedDarkPicture = try separatorPicture(f, name: "\(mode)-scheme-dark")
            let fixedDarkPicture = try separatorPicture(f, name: "\(mode)-fixed-dark", list: fixedList)
            let dark = try lineColor(inheritedDarkPicture, y: separatorBoundary(f, row: inherited))
            let fixedAfter = try lineColor(fixedDarkPicture, y: separatorBoundary(f, row: fixed, list: fixedList))
            try assertSameColor(dark, separatorColor(inherited.effectiveAppearance), "\(mode): inherited dark separator")
            try assertSameColor(fixedAfter, separatorColor(fixed.effectiveAppearance), "\(mode): fixed light separator under a dark window")
            let changed = abs(light.redComponent - dark.redComponent) + abs(light.greenComponent - dark.greenComponent)
                + abs(light.blueComponent - dark.blueComponent) + abs(light.alphaComponent - dark.alphaComponent)
            XCTAssertGreaterThan(changed, 1.0 / 255.0, "\(mode): the inherited platform color changes with the window")
            try assertSameColor(fixedAfter, fixedBefore, "\(mode): authored color-scheme keeps its platform color")
        }
    }

    func testNativeSeparatorUsesItsDirectRowsLocalAppearance() throws {
        let source = """
        component RowScheme
          state scheme = 0
          action setScheme(next: number)
            scheme = next
          view
            column width="100%" height="100%" gap=12 padding=12 box-sizing="border-box"
              row gap=8
                button press=setScheme(1) testId="row-light"
                  text "Row light"
                button press=setScheme(2) testId="row-dark"
                  text "Row dark"
                button press=setScheme(0) testId="row-clear"
                  text "Clear row scheme"
              list height=100 width="100%" testId="rows"
                text "First" height=40 padding="8px 16px" box-sizing="border-box" color-scheme=(scheme == 1 ? "light" : (scheme == 2 ? "dark" : none)) testId="local-0"
                text "Last" height=40 padding="8px 16px" box-sizing="border-box" testId="local-1"

        """
        let f = try TableFixture(count: 0, source: source)
        fixture = f
        let row = try f.node("local-0")
        func press(_ id: String) throws {
            f.presenter.press(try f.node(id).id)
            f.settle()
        }
        func distance(_ a: NSColor, _ b: NSColor) -> CGFloat {
            abs(a.redComponent - b.redComponent) + abs(a.greenComponent - b.greenComponent)
                + abs(a.blueComponent - b.blueComponent) + abs(a.alphaComponent - b.alphaComponent)
        }
        var native: [NSColor]?
        for (mode, painted) in [("native", false), ("painted", true)] {
            f.presenter.tables.painted = painted
            f.window.appearance = NSAppearance(named: .darkAqua)
            try press("row-clear")
            var picture = try separatorPicture(f, name: "\(mode)-row-inherited-dark")
            let inheritedDark = try lineColor(picture, y: separatorBoundary(f, row: row))
            try assertSameColor(inheritedDark, separatorColor(row.effectiveAppearance), "\(mode): a clear row inherits the dark window")

            try press("row-light")
            picture = try separatorPicture(f, name: "\(mode)-row-light-in-dark")
            let lightInDark = try lineColor(picture, y: separatorBoundary(f, row: row))
            try assertSameColor(lightInDark, separatorColor(row.effectiveAppearance), "\(mode): the direct row's light scheme owns its separator")
            XCTAssertGreaterThan(distance(inheritedDark, lightInDark), 1.0 / 255.0, "\(mode): a local light row differs from its dark window")

            f.window.appearance = NSAppearance(named: .aqua)
            try press("row-dark")
            picture = try separatorPicture(f, name: "\(mode)-row-dark-in-light")
            let darkInLight = try lineColor(picture, y: separatorBoundary(f, row: row))
            try assertSameColor(darkInLight, separatorColor(row.effectiveAppearance), "\(mode): the direct row's dark scheme owns its separator")

            try press("row-clear")
            picture = try separatorPicture(f, name: "\(mode)-row-cleared-light")
            let restoredLight = try lineColor(picture, y: separatorBoundary(f, row: row))
            try assertSameColor(restoredLight, separatorColor(row.effectiveAppearance), "\(mode): clearing the row restores the light window")
            XCTAssertGreaterThan(distance(darkInLight, restoredLight), 1.0 / 255.0, "\(mode): clearing the local dark scheme restores inherited light")

            let colors = [inheritedDark, lightInDark, darkInLight, restoredLight]
            if let native {
                for (index, pair) in zip(native, colors).enumerated() {
                    try assertSameColor(pair.1, pair.0, "painted and native use the same row-local appearance at step \(index)")
                }
            } else {
                native = colors
            }
        }
    }

    func testWindowedSeparatorUsesItsAuthoredRowsLocalAppearanceAcrossRemounting() throws {
        let source = """
        component WindowedRowScheme
          state seed = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
          derive d1 = seed.concat(seed)
          derive d2 = d1.concat(d1)
          derive d3 = d2.concat(d2)
          derive d4 = d3.concat(d3)
          derive rows = d4.map((n, i) => i)
          state scheme = 0
          action setScheme(next: number)
            scheme = next
          view
            column width="100%" height="100%" gap=12 padding=12 box-sizing="border-box"
              row gap=8
                button press=setScheme(1) testId="windowed-light"
                  text "Row light"
                button press=setScheme(2) testId="windowed-dark"
                  text "Row dark"
                button press=setScheme(0) testId="windowed-clear"
                  text "Clear row scheme"
              list virtualized=true estimated-item-height=40 flex=1 min-height=0 width="100%" testId="rows"
                for r in rows key=`k${r}`
                  row height=40 width="100%" padding="8px 16px" box-sizing="border-box"
                    color-scheme=(scheme == 1 ? "light" : (scheme == 2 ? "dark" : none)) testId=`windowed-${r}`
                    text `Row ${r}`

        """
        let f = try TableFixture(count: 0, source: source)
        fixture = f
        let first = try f.node("windowed-0")
        let firstRow = try XCTUnwrap(f.feed.rows.first { $0.root == first.id })
        XCTAssertFalse(firstRow.node === first, "the table mounts the runner's wrapper while appearance belongs to its authored root")
        XCTAssertNotNil(firstRow.node.props["listItemKey"])

        f.scroll(to: 4_000)
        XCTAssertNil(f.presenter.views[first.id], "the old authored row left the mounted window")
        XCTAssertNil(f.presenter.views[firstRow.node.id], "the old wrapper left the mounted window")
        let portRows = f.table.rows(in: f.table.convert(f.clip.bounds, from: f.document))
        let mountedRow = try XCTUnwrap(f.feed.rows.first { NSLocationInRange($0.index, portRows) }, "a visible row is mounted after scrolling")
        let row = try XCTUnwrap(f.presenter.views[mountedRow.root])
        XCTAssertTrue(row.isDescendant(of: mountedRow.node))
        XCTAssertFalse(mountedRow.node === row, "the new wrapper and its authored appearance source remain distinct")

        func press(_ id: String) throws {
            f.presenter.press(try f.node(id).id)
            f.settle()
        }
        func distance(_ a: NSColor, _ b: NSColor) -> CGFloat {
            abs(a.redComponent - b.redComponent) + abs(a.greenComponent - b.greenComponent)
                + abs(a.blueComponent - b.blueComponent) + abs(a.alphaComponent - b.alphaComponent)
        }
        func colors(_ mode: String) throws -> [NSColor] {
            f.window.appearance = NSAppearance(named: .darkAqua)
            try press("windowed-clear")
            var picture = try separatorPicture(f, name: "\(mode)-windowed-inherited-dark")
            let inheritedDark = try lineColor(picture, y: separatorBoundary(f, row: row))
            try assertSameColor(inheritedDark, separatorColor(row.effectiveAppearance), "\(mode): the wrapper inherits its authored row's dark appearance")

            try press("windowed-light")
            picture = try separatorPicture(f, name: "\(mode)-windowed-light-in-dark")
            let lightInDark = try lineColor(picture, y: separatorBoundary(f, row: row))
            try assertSameColor(lightInDark, separatorColor(row.effectiveAppearance), "\(mode): the authored row's light scheme owns its wrapper's separator")
            XCTAssertGreaterThan(distance(inheritedDark, lightInDark), 1.0 / 255.0)

            f.window.appearance = NSAppearance(named: .aqua)
            try press("windowed-dark")
            picture = try separatorPicture(f, name: "\(mode)-windowed-dark-in-light")
            let darkInLight = try lineColor(picture, y: separatorBoundary(f, row: row))
            try assertSameColor(darkInLight, separatorColor(row.effectiveAppearance), "\(mode): the authored row's dark scheme owns its wrapper's separator")

            try press("windowed-clear")
            picture = try separatorPicture(f, name: "\(mode)-windowed-cleared-light")
            let restoredLight = try lineColor(picture, y: separatorBoundary(f, row: row))
            try assertSameColor(restoredLight, separatorColor(row.effectiveAppearance), "\(mode): clearing the authored row restores the light window")
            XCTAssertGreaterThan(distance(darkInLight, restoredLight), 1.0 / 255.0)
            return [inheritedDark, lightInDark, darkInLight, restoredLight]
        }

        let native = try colors("native")
        f.presenter.tables.painted = true
        f.settle()
        let painted = try colors("painted")
        for (index, pair) in zip(native, painted).enumerated() {
            try assertSameColor(pair.1, pair.0, "painted and native windowed rows agree at step \(index)")
        }

        f.scroll(to: 0)
        let returned = try f.node("windowed-0")
        XCTAssertFalse(returned === first, "returning creates the current authored row again")
        f.window.appearance = NSAppearance(named: .darkAqua)
        try press("windowed-light")
        var picture = try separatorPicture(f, name: "painted-windowed-returned-light")
        let paintedReturn = try lineColor(picture, y: separatorBoundary(f, row: returned))
        try assertSameColor(paintedReturn, separatorColor(returned.effectiveAppearance), "painted: the current authored source owns the returned row's separator")
        f.presenter.tables.painted = false
        f.settle()
        picture = try separatorPicture(f, name: "native-windowed-returned-light")
        let nativeReturn = try lineColor(picture, y: separatorBoundary(f, row: returned))
        try assertSameColor(nativeReturn, separatorColor(returned.effectiveAppearance), "native: the current authored source owns the returned row's separator")
        try assertSameColor(nativeReturn, paintedReturn, "native and painted returned-row associations agree")
    }

    func testWindowedShadowIntentBelongsToTheWrapperAndPaintedLinesTearDown() throws {
        let f = try TableFixture(count: 0, source: Self.separatorSource(windowed: true))
        fixture = f
        let child = try f.node("edge-0"), wrapper = try XCTUnwrap(f.feed.rows.first { $0.root == child.id }?.node)
        let childStyle = child.style, wrapperStyle = wrapper.style
        func shadow(_ node: NodeView, _ original: NodeStyle, owned: Bool) {
            var style = original
            let written = (original["written"]?.array ?? []).filter { $0.string != "box_shadow" }
            style["written"] = .array(written + (owned ? [.string("box_shadow")] : []))
            f.presenter.apply(wireBatch([["op": "style", "id": node.id, "style": style.mapValues(\.any)]]))
        }
        for painted in [false, true] {
            let mode = painted ? "painted" : "native"
            f.presenter.tables.painted = painted
            for (step, own) in [(false, false), (true, false), (true, true), (false, false)].enumerated() {
                shadow(child, childStyle, owned: own.0)
                shadow(wrapper, wrapperStyle, owned: own.1)
                f.settle()
                XCTAssertTrue(try f.node("edge-0") === child)
                XCTAssertTrue(f.presenter.views[wrapper.id] === wrapper)
                let picture = try separatorPicture(f, name: "\(mode)-wrapper-shadow-\(step)")
                try assertSeparator(f, picture, row: "edge-0", present: !own.1, note: "\(mode): the wrapper owns its shadow")
            }
        }
        f.presenter.tables.reset()
        let picture = try separatorPicture(f, name: "painted-after-reset")
        try assertSeparator(f, picture, row: "edge-0", present: false, note: "reset removes the painted separator")
    }

    func testWindowedSeparatorOwnershipSurvivesRemountingAndChromeChanges() throws {
        let f = try TableFixture(count: 0, source: Self.separatorSource(windowed: true))
        fixture = f
        let first = try f.node("edge-0"), own = try f.node("edge-1")
        let wrapped = try XCTUnwrap(f.feed.rows.first { $0.root == own.id })
        XCTAssertFalse(wrapped.node === own, "the runner's wrapper and authored row are different nodes")
        XCTAssertNotNil(wrapped.node.props["listItemKey"])
        try checkInteriorSeparators(f, name: "windowed-start", last: false)
        f.scroll(to: 4_000)
        XCTAssertNil(f.presenter.views[first.id], "the old first row left the mounted window")
        f.scroll(to: 0)
        XCTAssertFalse(try f.node("edge-0") === first)
        XCTAssertTrue(try f.node("edge-1").wrote("border_style_bottom"))
        try checkInteriorSeparators(f, name: "windowed-return", last: false)
        let retained = try f.node("edge-1")
        f.presenter.tables.painted = true
        f.settle()
        XCTAssertTrue(try f.node("edge-1") === retained)
        XCTAssertNil(f.presenter.tables.feeds[f.list.id])
        try checkInteriorSeparators(f, name: "windowed-painted", last: false)
        f.presenter.tables.painted = false
        f.settle()
        XCTAssertTrue(try f.node("edge-1") === retained)
        try checkInteriorSeparators(f, name: "windowed-native-again", last: false)
    }

    func testInteriorOrderRemovalAndListRemovalRefreshAndTearDownSeparators() throws {
        let source = """
        component SeparatorChanges
          state rows = [0, 1, 2, 3]
          state shown = true
          action reorder
            rows = [0, 2, 1, 3]
          action remove
            rows = rows.filter(r => r != 1)
          action hide
            shown = false
          view
            column width="100%" height="100%"
              row gap=8
                button press=reorder testId="reorder"
                  text "Reorder"
                button press=remove testId="remove-row"
                  text "Remove"
                button press=hide testId="remove-list"
                  text "Remove list"
              if shown
                list flex=1 min-height=0 width="100%" padding="10px 16px" box-sizing="border-box" testId="rows"
                  for r in rows key=`k${r}`
                    row height=40 width="100%" border-top=(r == 1 ? "none" : none) testId=`changing-${r}`
                      text `Row ${r}`

        """
        for (mode, painted) in [("native", false), ("painted", true)] {
            let f = try TableFixture(count: 0, source: source)
            fixture = f
            f.presenter.tables.painted = painted
            f.settle()
            let listID = f.list.id
            let original = try (0..<4).map { try f.node("changing-\($0)") }
            let oldTable = f.feed.table
            var picture = try separatorPicture(f, name: "\(mode)-changes-initial")
            try assertSeparator(f, picture, row: "changing-0", present: false, note: "\(mode): row 1 initially owns row 0's boundary")

            f.presenter.press(try f.node("reorder").id)
            f.settle()
            for (index, row) in original.enumerated() { XCTAssertTrue(try f.node("changing-\(index)") === row) }
            let positions = try [0, 2, 1, 3].map { index in
                let row = try f.node("changing-\(index)")
                return row.convert(row.bounds, to: f.document).minY
            }
            XCTAssertEqual(positions, positions.sorted(), "\(mode): the retained rows follow the authored reorder")
            picture = try separatorPicture(f, name: "\(mode)-changes-reordered")
            try assertSeparator(f, picture, row: "changing-0", present: true, note: "\(mode): row 0's new neighbour leaves its boundary unowned")
            try assertSeparator(f, picture, row: "changing-2", present: false, note: "\(mode): moved row 1 owns row 2's boundary")

            f.presenter.press(try f.node("remove-row").id)
            f.settle()
            XCTAssertFalse(f.presenter.views.values.contains { $0.props["testId"] == "changing-1" })
            if painted {
                XCTAssertNil(f.presenter.tables.feeds[listID])
            } else {
                XCTAssertEqual(f.presenter.tables.feeds[listID]?.rows.map { $0.node.props["testId"] ?? "" },
                               ["changing-0", "changing-2", "changing-3"])
            }
            picture = try separatorPicture(f, name: "\(mode)-changes-removed")
            try assertSeparator(f, picture, row: "changing-2", present: true, note: "\(mode): removing the owning neighbour restores the separator")
            let remaining = try f.node("changing-2")
            let paintedLine = painted ? try XCTUnwrap(remaining.layer?.sublayers?.first {
                $0.backgroundColor != nil && abs($0.frame.height - 1) < 0.01 && abs($0.frame.maxY - remaining.bounds.maxY) < 0.01
            }) : nil
            if painted {
                XCTAssertNil(oldTable.superview, "painted chrome already returned the list from its native table")
                XCTAssertNotNil(paintedLine?.superlayer, "the retained row owns its painted separator before removal")
            } else {
                XCTAssertNotNil(oldTable.superview, "native chrome still owns the table before list removal")
            }

            f.presenter.press(try f.node("remove-list").id)
            f.settle()
            XCTAssertNil(f.presenter.tables.feeds[listID], "\(mode): a removed ordinary list has no feed")
            XCTAssertNil(oldTable.superview, "\(mode): its native table is detached")
            XCTAssertNil(paintedLine?.superlayer, "\(mode): its painted separator layer is detached")
            XCTAssertFalse(f.presenter.views.values.contains { $0.id == listID }, "\(mode): the authored list is gone")
            f.close()
            fixture = nil
        }
    }

    func testPaintedSeparatorsRequireTheContractRowMark() throws {
        let source = """
        component Guards
          state display = "block"
          view
            column width="100%" height="100%"
              list height=100 width="100%" testId="rows"
                text "Marked first" height=40 padding="8px 16px" box-sizing="border-box" testId="marked-first"
                text "Marked last" height=40
              list appearance="none" height=100 width="100%" testId="opted"
                text "Authored first" height=40 padding="8px 16px" box-sizing="border-box" testId="opted-first"
                text "Authored last" height=40
              list display=display height=100 width="100%" testId="bound"
                text "Bound first" height=40 padding="8px 16px" box-sizing="border-box" testId="bound-first"
                text "Bound last" height=40
              list height=100 width="100%" testId="outer"
                column height=100
                  list height=100 width="100%" testId="inner"
                    text "Nested first" height=40 padding="8px 16px" box-sizing="border-box" testId="inner-first"
                    text "Nested last" height=40

        """
        let f = try TableFixture(count: 0, table: false, source: source)
        fixture = f
        for painted in [true, false] {
            f.presenter.tables.painted = painted
            f.settle()
            let mode = painted ? "agent" : "platform"
            for (id, row, marked) in [("rows", "marked-first", true), ("opted", "opted-first", false), ("bound", "bound-first", false), ("inner", "inner-first", true)] {
                let list = try f.node(id)
                XCTAssertEqual(list.props["listRows"] == "true", marked)
                XCTAssertEqual(f.presenter.tables.feeds[list.id] != nil, !painted && id == "rows", "nested rows remain painted under platform chrome")
                let picture = try separatorPicture(f, name: "guard-\(mode)-\(id)", list: list)
                try assertSeparator(f, picture, row: row, present: marked, note: "\(mode) \(id)", list: list)
            }
        }
    }
}

private extension TableFixture {
    /// The rows of a list the runner does not window, wherever each is mounted.
    var mountedRows: Int { presenter.views.values.filter { ($0.props["testId"] ?? "").hasPrefix("row-") && $0.window != nil }.count }
}
#endif

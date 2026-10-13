// @ref LLP 1115.002 D5 — stage 3a, the gate for a `list` as a table: one
// session's virtualized list in a view-based `NSTableView` inside the real
// host, the runner still making and removing the rows. Each test is one of
// the gate's questions (rows arriving and leaving, heights from layout,
// selection across a row's reuse, a control's own clicks). A fixture, in no
// app: `TableHost.show` is called from here alone.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// One session whose page is a list of `count` rows, in a window, with the
/// list's rows in a table.
final class TableFixture {
    let session: ExactSession
    let window: NSWindow
    let view: ExactView
    let list: NodeView
    /// Nil when the list is left as the presenter lays it out today.
    let feed: TableFeed!
    var presenter: Presenter { session.presenter }
    var table: NSTableView { feed.table }
    var clip: NSClipView { list.scroll!.contentView }
    var document: NSView { list.scroll!.documentView! }

    /// Rows of mixed heights: every fifth has a second paragraph that wraps,
    /// `grown` is 60 points taller, and every tenth from 3 holds a button, a
    /// checkbox and a field. `press` on a row opens it. Ten rows doubled
    /// until there are `count`.
    static func source(count: Int) -> String {
        var doublings = 0
        while 10 << doublings < count { doublings += 1 }
        let derived = (0..<doublings).map { "  derive d\($0 + 1) = concat(\($0 == 0 ? "seed" : "d\($0)"), \($0 == 0 ? "seed" : "d\($0)"))" }.joined(separator: "\n")
        return """
        component Rows
          state seed = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
          state extra = [0 - 1]
          state grown = 0 - 1
          state opened = 0 - 1
          state presses = 0
          state bumps = 0
          state on = false
          state draft = ""
        \(derived)
          derive base = (\(doublings == 0 ? "seed" : "d\(doublings)")).map((x, i) => i).slice(0, \(count))
          derive rows = extra.slice(1).concat(base)
          action open(r: number)
            opened = r
            presses = presses + 1
          action grow(r: number)
            grown = r
          action bump
            bumps = bumps + 1
          action check(v: bool)
            on = v
          action typed(v: string)
            draft = v
          action prepend
            extra = extra.concat([0 - extra.length - 1])
          view
            column width="100%" height="100%"
              row testId="bar" gap=8 padding=4
                text `opened ${opened} presses ${presses} bumps ${bumps} on ${on} draft ${draft}` testId="status"
                button press=grow(7) testId="grow"
                  text "Grow"
                button press=grow(0 - 1) testId="shrink"
                  text "Shrink"
                button press=prepend testId="prepend"
                  text "Prepend"
              list virtualized=true estimated-item-height=32 flex=1 min-height=0 width="100%" padding="10px 16px" box-sizing="border-box" overflow-x="hidden" testId="rows"
                for r in rows key=`k${r}`
                  row press=open(r) testId=`row-${r}` align-items="center" gap=8 padding="6px 0" width="100%"
                    column flex=1 min-width=0
                      text `Row ${r}` testId=`title-${r}`
                      if r % 5 == 0
                        text `A second paragraph for row ${r}, long enough that it wraps when the list is narrow and makes this row taller than its neighbours` font-size=11
                      if r == grown
                        box height=60 testId="extra"
                    if r % 10 == 3
                      button press=bump testId=`bump-${r}`
                        text "Bump"
                      input type="checkbox" checked=on change=check aria-label="On" testId=`check-${r}`
                      input value=draft input=typed aria-label="Draft" testId=`field-${r}` width=100

        """
    }

    static func plan(_ source: String) throws -> Data {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"], "build.mjs --test builds the Contract compiler and names it"))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        let errors = Pipe()
        compiler.standardError = errors
        try compiler.run()
        let said = String(decoding: errors.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
        compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0, said)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    init(count: Int, size: CGSize = CGSize(width: 480, height: 600), table: Bool = true, source: String? = nil) throws {
        _ = NSApplication.shared
        session = ExactApp.shared.makeSession(label: "table-feed")
        // A vertical list is a table unless its rows are to be painted, as the driver's own chrome paints them.
        session.presenter.tables.painted = !table
        view = ExactView(session: session)
        window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = view
        view.layoutSubtreeIfNeeded()
        XCTAssertNil(session.boot(plan: try Self.plan(source ?? Self.source(count: count)), size: size).error)
        let presenter = session.presenter
        list = try XCTUnwrap(presenter.views.values.first { $0.props["testId"] == "rows" })
        presenter.settlePump()
        feed = table ? try XCTUnwrap(presenter.tables.feeds[list.id], "a vertical list is a table") : nil
        settle()
    }
    func close() { session.destroy(); window.close() }

    /// Everything the list owes is built and measured, as before a frame.
    func settle() {
        for _ in 0..<4 {
            presenter.settlePump()
            session.drainFill()
            RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.002))
        }
        window.layoutIfNeeded()
    }
    func node(_ id: String) throws -> NodeView { try XCTUnwrap(presenter.views.values.first { $0.props["testId"] == id }, id) }
    func scroll(to y: CGFloat) {
        clip.scroll(to: NSPoint(x: 0, y: y))
        list.scroll!.reflectScrolledClipView(clip)
        settle()
    }
    var status: String { (try? node("status").accessibleText) ?? "" }
    /// The mounted rows by item index, as the runner's last snapshot has them.
    var mounted: [Int: NodeView] {
        var out: [Int: NodeView] = [:]
        for row in presenter.collections.entries[list.id]?.snapshot.rows ?? [] { out[row.index] = presenter.views[row.view] }
        return out
    }
    /// The cells the table has, by row.
    var cells: [Int: TableRowCell] {
        var out: [Int: TableRowCell] = [:]
        table.enumerateAvailableRowViews { view, row in out[row] = view.view(atColumn: 0) as? TableRowCell }
        return out
    }
    /// The item a mounted row shows: the number in its key.
    func item(_ row: NodeView) -> Int { Int((row.props["listItemKey"] ?? "").drop { !$0.isNumber && $0 != "-" }) ?? .min }
    /// A view's frame in the list's document.
    func inDocument(_ view: NSView) -> NSRect { view.convert(view.bounds, to: document) }
}

final class TableFeedMacTests: XCTestCase {
    private var fixture: TableFixture?
    override func tearDown() { fixture?.close(); fixture = nil; super.tearDown() }

    /// What the gate holds at every offset: the table has a row for every
    /// item and is as long as the list; every row the port shows has a cell
    /// holding the runner's own row for that item, where layout put it; and
    /// a mounted row the table has no cell for is in the document.
    private func check(_ f: TableFixture, _ note: String, file: StaticString = #filePath, line: UInt = #line) throws {
        let snapshot = try XCTUnwrap(f.presenter.collections.entries[f.list.id]?.snapshot, file: file, line: line)
        XCTAssertEqual(f.table.numberOfRows, snapshot.count, note, file: file, line: line)
        // The scroller's document is the runner's extent, with the list's padding: nothing of it is the table's guess.
        let content = f.list.contentBox(), port = f.clip.bounds
        XCTAssertEqual(f.document.frame.height, max(f.list.bounds.height, snapshot.extent + f.list.bounds.height - content.height), accuracy: 0.01, "\(note): the document is as long as the runner says the list is", file: file, line: line)
        XCTAssertEqual(f.table.frame.width, port.width, note, file: file, line: line)
        let mounted = f.mounted, cells = f.cells
        let shown = f.table.rows(in: f.table.convert(port, from: f.document))
        XCTAssertGreaterThan(shown.length, 0, note, file: file, line: line)
        for row in shown.location..<shown.location + shown.length {
            let cell = try XCTUnwrap(cells[row], "\(note): row \(row) shows and has a cell", file: file, line: line)
            let node = try XCTUnwrap(mounted[row], "\(note): row \(row) shows and is mounted", file: file, line: line)
            XCTAssertTrue(cell.row === node && node.superview === cell, "\(note): row \(row)'s cell holds the runner's row", file: file, line: line)
            XCTAssertEqual(f.inDocument(node), cell.kernel, "\(note): row \(row) is where layout put it", file: file, line: line)
            XCTAssertTrue(cell.bounds.insetBy(dx: -0.01, dy: -0.01).contains(node.frame), "\(note): row \(row) is inside its cell (\(node.frame) in \(cell.bounds)), where a click reaches it", file: file, line: line)
            let rect = f.table.convert(f.table.rect(ofRow: row), to: f.document)
            XCTAssertEqual(rect.minY, cell.kernel.minY, accuracy: 0.01, "\(note): the table's row \(row) starts where layout's does", file: file, line: line)
            XCTAssertEqual(rect.height, cell.kernel.height, accuracy: 0.01, "\(note): the table's row \(row) is as tall as layout made it", file: file, line: line)
            XCTAssertTrue(try f.node("title-\(f.item(node))").isDescendant(of: cell), "\(note): row \(row) is its item", file: file, line: line)
        }
        // Those rows cover the port: nothing of it shows a row that is not there.
        let top = f.table.convert(f.table.rect(ofRow: shown.location), to: f.document).minY
        let bottom = f.table.convert(f.table.rect(ofRow: shown.location + shown.length - 1), to: f.document).maxY
        XCTAssertLessThanOrEqual(top, max(port.minY, content.minY) + 0.01, "\(note): the rows that show reach the port's top", file: file, line: line)
        XCTAssertGreaterThanOrEqual(bottom, min(port.maxY, content.minY + snapshot.extent) - 0.01, "\(note): the rows that show reach the port's bottom", file: file, line: line)
        for (index, node) in mounted where cells[index] == nil {
            XCTAssertTrue(node.superview === f.document, "\(note): a mounted row with no cell (\(index)) is in the document", file: file, line: line)
        }
    }

    func testTheRunnersRowsAreTheTablesRowsAsTheListScrolls() throws {
        let f = try TableFixture(count: 10_000)
        fixture = f
        XCTAssertTrue(f.table.superview === f.document, "the table is in the list's own scroller")
        try check(f, "at the top")
        let before = f.mounted
        XCTAssertLessThan(before.count, 120, "the runner mounts a window, not the list")
        // Down a port at a time, then a row at a time: rows arrive in cells
        // and the ones that left are gone from the session.
        for y in stride(from: CGFloat(0), through: 6_000, by: 517) {
            f.scroll(to: y)
            try check(f, "at \(y)")
        }
        XCTAssertLessThan(f.mounted.count, 120)
        XCTAssertNil(before[0].flatMap { f.presenter.views[$0.id] }, "the first row was destroyed when it left the window")
        XCTAssertLessThan(f.presenter.views.count, 1_500, "views are the window's, not ten thousand rows'")
        for y in stride(from: CGFloat(6_000), through: 6_400, by: 7) {
            f.scroll(to: y)
            try check(f, "at \(y)")
        }
        // The end, in one move (twice where measuring the last rows made the
        // list longer than its estimate), and back to the start.
        for _ in 0..<4 where f.clip.bounds.maxY < f.document.frame.height - 0.5 {
            f.scroll(to: f.document.frame.height - f.clip.bounds.height)
        }
        try check(f, "at the end")
        let last = try XCTUnwrap(f.cells[9_999]?.row, "the last item's row is in the last cell")
        XCTAssertEqual(f.item(last), 9_999)
        XCTAssertEqual(f.clip.bounds.maxY, f.document.frame.height, accuracy: 0.01, "the port is at the document's end")
        XCTAssertEqual(f.inDocument(last).maxY, f.document.frame.height - 10, accuracy: 0.01, "and the last row ends at the list's padding")
        f.scroll(to: 0)
        try check(f, "back at the top")
        XCTAssertNotNil(f.cells[0]?.row)
    }

    /// The first row the port shows whole or in part, its item, and where
    /// its top is from the port's top: what a person's eye is on.
    private func top(_ f: TableFixture) throws -> (row: Int, item: Int, y: CGFloat) {
        let port = f.clip.bounds
        let row = f.table.rows(in: f.table.convert(port, from: f.document)).location
        let node = try XCTUnwrap(f.cells[row]?.row, "the port's first row is mounted")
        return (row, f.item(node), f.inDocument(node).minY - port.minY)
    }

    func testARowIsAsTallAsLayoutMakesItAndStaysSoWhenItChanges() throws {
        let f = try TableFixture(count: 10_000)
        fixture = f
        try check(f, "at the top")
        // Mixed heights, from layout: a row of one line, a row whose second
        // paragraph wraps, a row with controls.
        func height(_ row: Int) -> CGFloat { f.table.rect(ofRow: row).height }
        let plain = height(1), wrapped = height(5), controls = height(3)
        XCTAssertEqual(plain, try f.node("row-1").frame.height)
        XCTAssertGreaterThan(wrapped, plain, "a row with a second paragraph is taller")
        XCTAssertGreaterThan(controls, plain, "and so is a row holding a field")
        XCTAssertFalse(f.table.usesAutomaticRowHeights, "AppKit measures nothing")

        // A row's content changes: it grows by the box it gained, the rows
        // after it move down, the list is that much longer.
        let extent = f.document.frame.height, below = f.table.rect(ofRow: 8).minY
        f.presenter.press(try f.node("grow").id)
        f.settle()
        XCTAssertEqual(height(7), plain + 60)
        XCTAssertEqual(f.table.rect(ofRow: 8).minY, below + 60)
        XCTAssertEqual(f.document.frame.height, extent + 60)
        try check(f, "row 7 grown")
        f.presenter.press(try f.node("shrink").id)
        f.settle()
        XCTAssertEqual(height(7), plain)
        XCTAssertEqual(f.document.frame.height, extent)

        // A row above the port changes: what shows stays where it was on
        // the screen, and the offset moved with the rows (the runner's
        // anchor, as without a table).
        f.scroll(to: 400)
        XCTAssertNotNil(f.mounted[7], "row 7 is above the port and still mounted")
        XCTAssertLessThan(try f.inDocument(f.node("row-7")).maxY, f.clip.bounds.minY)
        let before = try top(f), length = f.document.frame.height
        f.presenter.press(try f.node("grow").id)
        f.settle()
        let after = try top(f)
        XCTAssertEqual(after.item, before.item, "the same row is at the port's top")
        XCTAssertEqual(after.y, before.y, accuracy: 0.01, "at the same place")
        XCTAssertEqual(f.clip.bounds.minY, 460, accuracy: 0.01, "the offset moved by what the row above grew")
        XCTAssertEqual(f.document.frame.height, length + 60)
        try check(f, "a row above grown")
        f.presenter.press(try f.node("shrink").id)
        f.settle()
        XCTAssertEqual(f.clip.bounds.minY, 400, accuracy: 0.01)
        XCTAssertEqual(try top(f).y, before.y, accuracy: 0.01)

        // The window narrows: the paragraphs wrap again, every such row is
        // taller, in the table as in layout, deep in the list too.
        f.scroll(to: 3_000)
        try check(f, "at 3000")
        let held = try top(f)
        let wide = f.table.rect(ofRow: held.row + 5 - held.item % 5).height
        f.window.setContentSize(NSSize(width: 300, height: 600))
        f.settle()
        try check(f, "narrow")
        let narrow = try top(f)
        XCTAssertEqual(narrow.item, held.item, "the row a person was reading is still at the port's top")
        XCTAssertEqual(narrow.y, held.y, accuracy: 0.51)
        XCTAssertGreaterThan(f.table.rect(ofRow: narrow.row + 5 - narrow.item % 5).height, wide, "a wrapped paragraph's row grew with its lines")
        XCTAssertEqual(f.table.frame.width, f.clip.bounds.width)
        f.window.setContentSize(NSSize(width: 480, height: 600))
        f.settle()
        try check(f, "wide again")
        XCTAssertEqual(f.table.rect(ofRow: narrow.row + 5 - narrow.item % 5).height, wide)
    }

    /// The list's scroller drawn into a bitmap of `scale` pixels a point,
    /// and whether anything was drawn at a pixel of it (x from the left, y
    /// from the top). This Mac's display is 1x: the 2x picture is AppKit's
    /// drawing for a context scaled by two, not a window on a 2x display.
    private func picture(_ f: TableFixture, scale: CGFloat) throws -> (rep: NSBitmapImageRep, inked: (CGFloat, CGFloat) -> Bool) {
        let view = try XCTUnwrap(f.list.scroll), size = view.bounds.size
        let rep = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(size.width * scale), pixelsHigh: Int(size.height * scale), bitsPerSample: 8,
                                                 samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
        rep.size = size
        view.cacheDisplay(in: view.bounds, to: rep)
        return (rep, { x, y in (rep.colorAt(x: Int(x), y: Int(y))?.alphaComponent ?? 0) > 0.02 })
    }

    func testTheSeparatorsAndTheSelectionAreTheTablesOwnInPixels() throws {
        let f = try TableFixture(count: 10_000)
        fixture = f
        f.table.selectRowIndexes([1], byExtendingSelection: false)
        f.table.enumerateAvailableRowViews { view, _ in view.isEmphasized = true }
        f.window.displayIfNeeded()
        let width = f.clip.bounds.width
        for scale in [CGFloat(1), 2] {
            let (rep, inked) = try picture(f, scale: scale)
            if let dir = ProcessInfo.processInfo.environment["EXACT_PROOF_SHOTS"] {
                try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: dir).appendingPathComponent("table-gate-\(Int(scale))x.png"))
            }
            // The separator under row 3 (no row beside it is selected): its
            // pixel rows down the middle, and how far along it runs.
            let boundary = f.table.convert(f.table.rect(ofRow: 3), to: f.list.scroll).maxY * scale
            let middle = width / 2 * scale
            let rows = (Int(boundary) - Int(4 * scale)..<Int(boundary) + Int(4 * scale)).filter { inked(middle, CGFloat($0)) }
            XCTAssertEqual(rows.count, Int(scale), "the table's separator is a point thick: \(Int(scale)) px at \(Int(scale))x (pixel rows \(rows))")
            XCTAssertEqual(rows.last.map { CGFloat($0 + 1) }, boundary, "and it is the last of its row's pixels")
            let y = CGFloat(try XCTUnwrap(rows.first))
            let across = (0..<Int(width * scale)).filter { inked(CGFloat($0), y) }
            XCTAssertEqual(across.first.map { CGFloat($0) / scale }, 16, "it starts where the row's text does")
            XCTAssertEqual(across.last.map { CGFloat($0 + 1) / scale }, width - 16, "and ends with the cell")
            // A row with a field at its end has the same one: the separator is the row's text's, not that field's.
            let under = f.table.convert(f.table.rect(ofRow: 13), to: f.list.scroll).maxY * scale - 1
            let controls = (0..<Int(width * scale)).filter { inked(CGFloat($0), under) }
            XCTAssertEqual(controls.first.map { CGFloat($0) / scale }, 16)
            XCTAssertEqual(controls.last.map { CGFloat($0 + 1) / scale }, width - 16)
            // The selection: the whole height of its row, 10 points in from
            // each side, rounded, with no separator above it.
            let selected = f.table.convert(f.table.rect(ofRow: 1), to: f.list.scroll)
            let line = (selected.midY * scale).rounded(.down)
            let fill = (0..<Int(width * scale)).filter { inked(CGFloat($0), line) }
            XCTAssertEqual(fill.first.map { CGFloat($0) / scale }, 10)
            XCTAssertEqual(fill.last.map { CGFloat($0 + 1) / scale }, width - 10)
            let column = (Int(selected.minY * scale) - Int(3 * scale)..<Int(selected.maxY * scale) + Int(3 * scale)).filter { inked(middle, CGFloat($0)) }
            XCTAssertEqual(column.first.map { CGFloat($0) / scale }, selected.minY, "the fill starts with its row, and nothing is drawn above it")
            XCTAssertEqual(column.last.map { CGFloat($0 + 1) / scale }, selected.maxY, "and ends with it")
            XCTAssertFalse(inked((width - 10) * scale - 1, selected.minY * scale), "its corners are rounded")
        }
    }
}
#endif

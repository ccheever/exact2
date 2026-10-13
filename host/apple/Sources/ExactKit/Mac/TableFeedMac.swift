// @ref LLP 1115.002 D5 — one list's rows in a real, view-based
// `NSTableView`, fed by the runner (`TableHost` says which lists; this is
// one list's feed). The runner still owns which rows are mounted and layout
// still owns every size; the table owns its cells, its selection and its
// keys. The table has one row for every item, mounted or not, and is a view
// in the list's document, never the document itself: AppKit asks a delegate
// for the heights of a few hundred rows around what shows and estimates the
// rest (measured on macOS 27.0: 301 of 10,000), so a table's own length and
// its far rows' places are AppKit's guesses. The list's extent and offset
// stay the runner's, and the table is put where its rows that show are
// where layout has them.
//
// What the feed costs a frame is what stage 3b cut (measured, D5):
// - A mounted row of a list the runner windows goes straight into its cell
//   and stays there until the runner destroys it. The table keeps a row
//   view for every row of the mounted run the port is in, not only for the
//   rows the port shows (`RunnerTableView.visibleRect`), so a row built
//   ahead of the port is mounted once, in the slice that built it, and
//   nothing moves when it scrolls into sight. A mounted row the table has
//   no cell for all the same (one pinned far from the port) is in the
//   list's document, where layout has it; correctness never rests on the
//   table having made a view.
// - AppKit is told a row's height when layout's differs from what the table
//   has for that row, and at no other time. A row that is not mounted is the
//   estimate tall, one height for the whole list, so a row that comes or
//   goes changes no other row's height, and a list of equal rows tells
//   AppKit nothing as it scrolls. No height is kept here for a row that is
//   not mounted (the height index is the runner's).
// - A row's place in its cell is arithmetic on frames, and where its text
//   begins is read when the row or something in it moved.
#if os(macOS)
import AppKit

/// The table's own view for one row: around the runner's row while that row
/// is mounted, empty otherwise.
final class TableRowCell: NSTableCellView {
    private(set) weak var feed: TableFeed?
    private(set) weak var row: NodeView?
    /// Where layout has the row, in the list's document.
    private(set) var kernel = NSRect.zero
    /// The row, or something in it, moved since its text's start was read.
    var textMoved = true
    /// Where the row's text begins, said as a table's cell says it: AppKit
    /// starts a row's separator at the text of its cell's text field, and
    /// failing one at whatever text field it finds in the cell (measured on
    /// macOS 27.0: an author's field at the row's end took the separator
    /// with it). So the separator is inset to the row's text, by AppKit.
    private let lead = NSTextField(labelWithString: "")
    init(feed: TableFeed) {
        self.feed = feed
        super.init(frame: .zero)
        lead.isHidden = true
        addSubview(lead)
        textField = lead
    }
    required init?(coder: NSCoder) { nil }
    override var isFlipped: Bool { true }
    /// AppKit says how the row is drawn: emphasized while it is selected,
    /// its window key and the table first responder. What Exact paints in
    /// the row takes the selected item's ink then (TableInkMac.swift).
    override var backgroundStyle: NSView.BackgroundStyle {
        didSet { if backgroundStyle != oldValue { row?.rowEmphasisChanged(backgroundStyle == .emphasized) } }
    }

    /// Take `node` in. It is placed when the cell is in its row's view
    /// (`place`): a cell the table is still making has no place yet.
    func adopt(_ node: NodeView) {
        if row !== node { sendHome(); textMoved = true }
        row = node
        if node.superview !== self {
            // Where the presenter has it: layout's frame, in the document.
            kernel = (node.superview as? TableRowCell)?.kernel ?? node.frame
            node.prepareToMount()
            addSubview(node)
            feed?.moved()
            textMoved = true
            if backgroundStyle == .emphasized { node.rowEmphasisChanged(true) }
        }
        place()
    }
    /// Layout moved or resized the row (`TableHost.frame`). Its size is
    /// layout's at once, for whatever reads its bounds in the same batch;
    /// its place in the cell waits for the feed, which knows where the
    /// cell will be.
    func place(_ node: NodeView, kernel frame: NSRect) {
        guard node === row else { return }
        kernel = frame
        textMoved = true
        if node.frame.size != frame.size { node.setFrameSize(frame.size) }
    }
    /// The row where layout has it, in this cell's space.
    func place() {
        guard let row, let feed, row.superview === self, let origin = feed.documentOrigin(of: self) else { return }
        let wanted = kernel.offsetBy(dx: -origin.x, dy: -origin.y)
        if row.frame != wanted { row.frame = wanted; textMoved = true }
        guard textMoved else { return }
        textMoved = false
        // A label's text starts 2 points inside its frame, and the separator starts at the text.
        let text = feed.textStart(of: row).map { convert(NSPoint(x: $0.bounds.minX, y: 0), from: $0).x } ?? wanted.minX
        let said = NSRect(x: text - 2, y: 0, width: 1, height: 1)
        if lead.frame != said { lead.frame = said; superview?.needsLayout = true }
    }
    /// Give the row back to the list's document, where the presenter has
    /// its siblings; a row that was destroyed in the cell simply goes.
    func sendHome() {
        defer { row = nil }
        guard let row, row.superview === self else { return }
        guard let feed, let home = feed.home, feed.holds(row) else { row.removeFromSuperview(); return }
        home.addSubview(row)
        row.frame = kernel
        feed.moved()
        if backgroundStyle == .emphasized { row.rowEmphasisChanged(false) }
    }

    /// Hit tests that ask what is under a point, not who takes a click
    /// there (the hover path, the pressed row's pin, the driver's aim), see
    /// the row's own views.
    nonisolated(unsafe) private static var asking = 0
    static func whatIsThere<T>(_ ask: () -> T) -> T {
        asking += 1
        defer { asking -= 1 }
        return ask()
    }
    /// A control in the row keeps its clicks, and so does everything in a
    /// row that has no press (its text is selected as in any paragraph).
    /// The rest of a row that presses is the table's, as a stock table's row
    /// is where no cell's control is: the table takes the keyboard as the
    /// mouse goes down, its click selects, and the feed presses the row's root.
    override func hitTest(_ point: NSPoint) -> NSView? {
        guard let hit = super.hitTest(point) else { return nil }
        if Self.asking > 0 { return hit }
        return feed?.keepsClick(hit, in: self, at: point) == true ? hit : feed?.table ?? self
    }
}

/// The table: AppKit's. It overrides no mouse method: a table class that
/// does is tracked in AppKit's older loop, which selects as the mouse goes
/// down, where a stock table and SwiftUI's list select as it comes up.
final class RunnerTableView: NSTableView {
    fileprivate(set) weak var feed: TableFeed?
    /// What the table keeps row views for: what shows, and with it the run
    /// of mounted rows the port is in (`TableFeed.cover`). AppKit reads this
    /// to decide which rows have views (measured on macOS 27.0, and in
    /// `TableFeedPerfMacTests`); where it does not, a mounted row with no
    /// view is in the list's document, as before.
    override var visibleRect: NSRect {
        let seen = super.visibleRect
        return feed?.cover(seen) ?? seen
    }
    /// The table moved its rows (new rows near what shows, a new guess for
    /// the rows that do not): it goes back where layout has them.
    override func layout() {
        super.layout()
        feed?.tableLaidOut()
    }
    /// A row the keyboard reached is shown in the port, which is the
    /// list's: the table's own idea of what shows is wider (`visibleRect`).
    override func scrollRowToVisible(_ row: Int) {
        if let feed { feed.show(row: row) } else { super.scrollRowToVisible(row) }
    }
    /// A secondary click (the right button, or Control and the left) on a
    /// row is heard by what is under it, as in a list of painted rows:
    /// AppKit asks the table for a menu, and the nearest node in the row
    /// with a `contextmenu` or a `contextPopover` answers. The empty menu
    /// says the click was taken: nothing of the table's own follows, and
    /// nothing above the table hears it again. With none in the row the
    /// table has no menu, and AppKit sends the click on up to the list.
    override func menu(for event: NSEvent) -> NSMenu? {
        feed?.contextClick(event) == true ? NSMenu() : super.menu(for: event)
    }
    /// Home and End are the list's: the table's own ends are estimates.
    override func scrollToBeginningOfDocument(_ sender: Any?) { feed?.scroll(toEnd: false) }
    override func scrollToEndOfDocument(_ sender: Any?) { feed?.scroll(toEnd: true) }
}

/// One list's rows in a table.
final class TableFeed: NSObject, NSTableViewDataSource, NSTableViewDelegate {
    /// A mounted row: its item's index, the runner's view for it and that
    /// view's authored root, where layout has it in the document, and the
    /// table's row for it there (`top`, `height`: layout's box, with half
    /// of any room layout left between it and a mounted neighbour).
    struct Row {
        let index: Int
        let node: NodeView
        let root: UInt32
        var kernel = NSRect.zero
        var top: CGFloat = 0, height: CGFloat = 0
    }

    let table = RunnerTableView()
    let id: UInt32
    private weak var presenter: Presenter?
    private weak var list: NodeView?
    /// The mounted rows, by index.
    private(set) var rows: [Row] = []
    private var count = 0
    /// The children of a list the runner does not window, as its last
    /// `children` op named them: each with a box in the flow is a row, and
    /// the feed mounts them all (`TableHost.owns`).
    var children: [NodeView]? { didSet { dirty = true } }
    /// How tall the table has a row that is not mounted: the commonest
    /// height among the mounted rows when the table was last loaded, or the
    /// runner's own estimate (its extent shared among its items) while none
    /// is mounted. It is read again when most rows that come are another
    /// height and most mounted rows agree on one (`agreed`, `differed`): a
    /// first layout before the text was measured, a list whose rows change.
    private(set) var estimate: CGFloat = 1
    private var agreed = 0, differed = 0
    /// AppKit refuses a row with no height; a row layout gave none takes this.
    static let least: CGFloat = 1
    /// For each mounted row's view, the index and height the table was last
    /// seen to have for it: a row is looked at again only when either moved.
    private var told: [UInt32: (index: Int, height: CGFloat)] = [:]
    private var held = Set<UInt32>()
    /// The run of mounted rows the port is in, from its first row's top to
    /// its last row's end in the document: the table keeps a view for each.
    private var covered: ClosedRange<CGFloat>?
    /// What the last sync read: nothing is done again for a batch that
    /// changed none of it.
    private var stamp: [UInt64] = []
    var dirty = true
    /// The selected row's item while its row is mounted: its key in a list
    /// the runner windows, its view in any other. The selection follows the
    /// item when rows before it come and go.
    private(set) var selectedKey: String?
    private var selectedNode: UInt32?
    /// A row selected before the runner had made it (a key that jumps to
    /// the list's end): AppKit showed where it guessed the row is, and the
    /// feed shows it once, when layout has it.
    private var reveal: Int?
    private var syncing = false, aligning = false
    /// Rows the runner made that the feed took to mount, rows moved into a
    /// cell or the document, and the table's rows told of a new height,
    /// since the feed began (the tests, the measures).
    var arrivals = 0
    private(set) var moves = 0, noted = 0
    private static let column = NSUserInterfaceItemIdentifier("exact.row")

    init(_ list: NodeView, presenter: Presenter, style: NSTableView.Style) {
        self.id = list.id
        self.list = list
        self.presenter = presenter
        super.init()
        let column = NSTableColumn(identifier: Self.column)
        column.resizingMask = .autoresizingMask
        table.addTableColumn(column)
        table.headerView = nil
        table.style = style
        table.backgroundColor = .clear
        // The table's own separators: a point thick, inset as the style's
        // cells are, none above the selected row (what SwiftUI's list shows).
        table.gridStyleMask = .solidHorizontalGridLineMask
        table.intercellSpacing = .zero
        table.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
        table.allowsMultipleSelection = false
        table.allowsEmptySelection = true
        table.selectionHighlightStyle = .regular
        table.usesAutomaticRowHeights = false
        table.dataSource = self
        table.delegate = self
        table.target = self
        table.action = #selector(clicked)
        table.doubleAction = #selector(doubleClicked)
        table.feed = self
        // A list the runner does not window: its children are in its document, in order.
        if presenter.collections.entries[id] == nil {
            children = list.container.subviews.compactMap { view in (view as? NodeView).flatMap { presenter.views[$0.id] === $0 ? $0 : nil } }
        }
        sync()
    }

    /// Where the presenter keeps the list's rows: its document, or the
    /// list itself when it is no scroller of its own.
    var home: NSView? { list.map { $0.scroll?.documentView ?? $0.container } }
    /// What the list shows of that: its port.
    private var port: NSRect { list?.scroll?.contentView.bounds ?? home?.bounds ?? .zero }
    func holds(_ node: NodeView) -> Bool { presenter?.views[node.id] === node && held.contains(node.id) }
    fileprivate func moved() { moves += 1 }
    /// The table's row views now (the tests, the measures).
    var rowViews: Int {
        var n = 0
        table.enumerateAvailableRowViews { _, _ in n += 1 }
        return n
    }

    /// A cell's origin in the document. A row's view is the table's own
    /// subview and the table the document's, none of them scaled or
    /// scrolled, so it is their origins added; nil for a cell in no row yet.
    fileprivate func documentOrigin(of cell: TableRowCell) -> NSPoint? {
        guard let rowView = cell.superview, let home else { return nil }
        guard rowView.superview === table, table.superview === home else { return home.convert(cell.frame.origin, from: rowView) }
        let at = table.frame.origin, row = rowView.frame.origin, own = cell.frame.origin
        return NSPoint(x: at.x + row.x + own.x, y: at.y + row.y + own.y)
    }

    /// The row's first paragraph, in the order it is written.
    fileprivate func textStart(of node: NodeView) -> NodeView? {
        if node.kind == "text" { return node }
        for case let child as NodeView in node.container.subviews where !child.isHidden {
            if let found = textStart(of: child) { return found }
        }
        return nil
    }

    /// Whether a click that hit `hit` in a row's cell is that view's own:
    /// anything in a row that has no press; in one that has, an AppKit
    /// control or a text view, a node that presses, scrolls, edits or draws
    /// for itself, or a link in a paragraph, anywhere between the hit and
    /// the row's root. The root's own press is the row's, and so the table's.
    fileprivate func keepsClick(_ hit: NSView, in cell: TableRowCell, at point: NSPoint) -> Bool {
        guard let row = cell.row, let mounted = self.row(of: row) else { return false }
        guard selectable(mounted) else { return true }
        var view: NSView? = hit
        while let current = view, current !== cell {
            if let node = current as? NodeView {
                if node.isParagraph {
                    let local = node.convert(point, from: cell.superview)
                    if node.inlineActivationTarget(at: local) != nil || node.inlineLink(at: local) != nil { return true }
                }
                if node === row || node.id == mounted.root { return false }
                if node.pressable || node.isButton || node.isSurfaceControl || node.scroll != nil || node.field != nil || node.textArea != nil
                    || ["control", "input", "textarea", "canvas", "native", "video", "iframe"].contains(node.kind)
                    || !node.handlers.isDisjoint(with: ["pointerdown", "pointerup", "pointermove", "contextmenu", "dblclick"]) { return true }
            } else if current is NSControl || current is NSText || current is NSScrollView { return true }
            view = current.superview
        }
        return false
    }

    // MARK: The runner's rows

    /// The mounted row for an item, if it has one.
    func row(at index: Int) -> Row? {
        var low = 0, high = rows.count - 1
        while low <= high {
            let middle = (low + high) / 2
            if rows[middle].index == index { return rows[middle] }
            if rows[middle].index < index { low = middle + 1 } else { high = middle - 1 }
        }
        return nil
    }
    func row(of node: NodeView) -> Row? { rows.first { $0.node === node } }
    /// The mounted row a view is in, itself or anything inside it.
    func row(holding view: NSView) -> Row? {
        var at: NSView? = view
        while let current = at, !(current is NSTableView) {
            if let cell = current as? TableRowCell { return cell.row.flatMap(row(of:)) }
            at = current.superview
        }
        return nil
    }
    /// Whether `node`, a child the list's `children` op names, is the
    /// feed's to mount: one of the rows the runner windows, as it has them
    /// in the batch being applied; any child of a list it does not window.
    func expects(_ node: NodeView) -> Bool {
        guard let entry = presenter?.collections.entries[id] else { return children != nil }
        return entry.snapshot.rows.contains { $0.view == node.id }
    }
    private func kernel(_ node: NodeView) -> NSRect { (node.superview as? TableRowCell)?.kernel ?? node.frame }

    /// The items and the mounted rows as the runner has them (a list it
    /// windows), or as the list's children are (any other: every child with
    /// a box in the flow is a row, and all of them are mounted).
    private func read(_ presenter: Presenter) -> (count: Int, rows: [Row], stamp: [UInt64]) {
        var next: [Row] = []
        if let snapshot = presenter.collections.entries[id]?.snapshot {
            // A table made before the runner said it windows the list.
            if children != nil { children = nil }
            next.reserveCapacity(snapshot.rows.count)
            var ordered = true
            for row in snapshot.rows where row.index >= 0 && row.index < snapshot.count {
                guard let node = presenter.views[row.view] else { continue }
                if let last = next.last, last.index >= row.index { ordered = false }
                next.append(Row(index: row.index, node: node, root: row.root, kernel: kernel(node)))
            }
            if !ordered { next.sort { $0.index < $1.index } }
            return (snapshot.count, next, [snapshot.revision, UInt64(snapshot.count), UInt64(snapshot.rows.count), snapshot.extent.bitPattern])
        }
        for node in children ?? [] where presenter.views[node.id] === node && TableHost.isRow(node) {
            next.append(Row(index: next.count, node: node, root: node.id, kernel: kernel(node)))
        }
        return (next.count, next, [UInt64(next.count)])
    }

    /// After a batch's ops and before the list's offset is corrected: the
    /// table's row count and heights from layout, the table where layout
    /// has the rows, and each mounted row in the cell the table has for it.
    func sync() {
        guard let presenter, let list, presenter.views[id] === list, home != nil else { return }
        let pixel = 1 / (table.window?.backingScaleFactor ?? 1)
        let port = self.port
        // A batch that changed no row, no frame of one and nothing of the port's width leaves the table as it is.
        var (items, next, now) = read(presenter)
        now += [Double(port.width).bitPattern, Double(pixel).bitPattern]
        guard dirty || now != stamp || table.superview !== home else { return }
        stamp = now
        dirty = false

        // The table's row for each: layout's box, and half the room layout
        // left between two mounted neighbours (a gap, a margin) in whole
        // pixels, so a list with room between its rows still tiles.
        for i in next.indices {
            var top = next[i].kernel.minY, end = next[i].kernel.maxY
            if i > 0, next[i - 1].index == next[i].index - 1 { top = next[i - 1].top + next[i - 1].height }
            if i + 1 < next.count, next[i + 1].index == next[i].index + 1 {
                let room = next[i + 1].kernel.minY - end
                end += room > 0 ? (room / 2 / pixel).rounded(.down) * pixel : room
            }
            next[i].top = top
            next[i].height = max(Self.least, end - top)
        }
        rows = next
        held = Set(next.map(\.node.id))

        /// The commonest height among the mounted rows, and how many have it.
        func common() -> (height: CGFloat, rows: Int)? {
            var seen: [CGFloat: Int] = [:]
            for row in next { seen[row.height, default: 0] += 1 }
            return seen.max { $0.value < $1.value || ($0.value == $1.value && $0.key > $1.key) }.map { ($0.key, $0.value) }
        }
        var recount = items != count
        if recount {
            // The items changed: every row is asked again.
            count = items
            let extent = CGFloat(presenter.collections.entries[id]?.snapshot.extent ?? 0)
            estimate = common()?.height ?? max(Self.least, (extent / CGFloat(max(1, items)) / pixel).rounded() * pixel)
        } else if differed >= 16, differed > agreed {
            // Most rows that came were not the estimate. Where most mounted
            // rows share another height, that is the estimate from here, and
            // every row is asked again; where they share none, it stays.
            if let now = common(), now.height != estimate, 2 * now.rows > next.count { estimate = now.height; recount = true }
            (agreed, differed) = (0, 0)
        }
        if recount { told.removeAll(keepingCapacity: true); (agreed, differed) = (0, 0) }
        // A row selected before it was made is its item's from now on.
        if selectedKey == nil, selectedNode == nil, let row = row(at: table.selectedRow) { remember(row) }
        // The selected item, wherever its row is now.
        let selected = selectedKey.flatMap { key in next.first { $0.node.props["listItemKey"] == key }?.index }
            ?? selectedNode.flatMap { node in next.first { $0.node.id == node }?.index }

        syncing = true
        defer { syncing = false }
        if recount {
            still { table.reloadData() }
            noted += count
        }
        // The rows whose height the table does not have as layout does.
        var changed = IndexSet(), seenNow: [UInt32: (index: Int, height: CGFloat)] = [:]
        seenNow.reserveCapacity(next.count)
        for row in next {
            if let was = told[row.node.id], was.index == row.index, was.height == row.height { seenNow[row.node.id] = was; continue }
            if table.rect(ofRow: row.index).height != row.height { changed.insert(row.index) }
            if row.height == estimate { agreed += 1 } else { differed += 1 }
            seenNow[row.node.id] = (row.index, row.height)
        }
        told = seenNow
        if !changed.isEmpty {
            still { table.noteHeightOfRows(withIndexesChanged: changed) }
            noted += changed.count
        }
        if let selected, table.selectedRow != selected { table.selectRowIndexes([selected], byExtendingSelection: false) }
        // The run of mounted rows the port is in: the table keeps a view
        // for each of them. A list the runner does not window has every
        // row mounted, so the table keeps what AppKit keeps: what shows.
        let anchor = anchor(port)
        var cover: ClosedRange<CGFloat>?
        if let anchor, children == nil {
            var first = anchor, last = anchor
            while first > 0, next[first - 1].index == next[first].index - 1 { first -= 1 }
            while last + 1 < next.count, next[last + 1].index == next[last].index + 1 { last += 1 }
            cover = next[first].top...max(next[first].top, next[last].top + next[last].height)
        }
        if cover != covered { covered = cover; table.needsLayout = true }
        align(port, anchor)
        table.layoutSubtreeIfNeeded()
        // AppKit's layout may have guessed the far rows again and moved the near ones with them.
        align(port, anchor)
        fill()
        // After the batch, never inside it: moving the port tells the runner.
        if let row = reveal, self.row(at: row) != nil {
            reveal = nil
            DispatchQueue.main.async { [weak self] in
                guard let self, self.table.selectedRow == row, self.row(at: row) != nil else { return }
                self.show(row: row)
            }
        }
    }

    /// AppKit animates a table's rows to a new height or count inside
    /// whatever animation is running; the feed's changes are layout's, at once.
    private func still(_ body: () -> Void) {
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0
            context.allowsImplicitAnimation = false
            body()
        }
    }

    /// The first mounted row that shows, else any: float error in a long
    /// list's offsets then lands in rows nobody sees.
    private func anchor(_ port: NSRect) -> Int? {
        guard !rows.isEmpty else { return nil }
        var low = 0, high = rows.count - 1
        // Rows are in index order, and a vertical list's rows go down it.
        while low < high {
            let middle = (low + high) / 2
            if rows[middle].top + rows[middle].height > port.minY { high = middle } else { low = middle + 1 }
        }
        return rows[low].top + rows[low].height > port.minY ? low : 0
    }

    /// The table where its rows are where layout has them: a mounted row's
    /// place in the table, under the table's own insets, is layout's.
    private func align(_ port: NSRect, _ anchor: Int?) {
        guard let list, let home, !aligning else { return }
        aligning = true
        defer { aligning = false }
        if table.superview !== home { home.addSubview(table, positioned: .below, relativeTo: nil) }
        // As wide as the port: a row that overflows widens the document, not the table.
        if table.frame.width != port.width {
            table.setFrameSize(NSSize(width: port.width, height: table.frame.height))
            // The one column fills it: a row is hit only inside its cell.
            table.sizeLastColumnToFit()
        }
        let y: CGFloat
        // Its view's place where it has one: AppKit rounds that to a pixel.
        if let anchor, anchor < rows.count {
            let row = rows[anchor]
            y = row.top - (table.rowView(atRow: row.index, makeIfNecessary: false)?.frame.minY ?? table.rect(ofRow: row.index).minY)
        } else { y = list.contentBox().minY - (table.numberOfRows > 0 ? table.rect(ofRow: 0).minY : 0) }
        if table.frame.origin != NSPoint(x: 0, y: y) { table.setFrameOrigin(NSPoint(x: 0, y: y)) }
    }
    fileprivate func tableLaidOut() {
        guard !syncing, !aligning else { return }
        let port = self.port
        align(port, anchor(port))
        table.enumerateAvailableRowViews { view, _ in (view.view(atColumn: 0) as? TableRowCell)?.place() }
    }
    /// What the table is to keep row views for, given what shows of it:
    /// that, and the mounted run the port is in when it reaches what shows.
    fileprivate func cover(_ seen: NSRect) -> NSRect {
        guard let covered, !seen.isEmpty, table.superview != nil else { return seen }
        let run = NSRect(x: seen.minX, y: covered.lowerBound - table.frame.minY, width: seen.width, height: covered.upperBound - covered.lowerBound)
        return run.intersects(seen) ? run.union(seen) : seen
    }
    /// The list's start or its end, as the runner has them.
    fileprivate func scroll(toEnd: Bool) {
        guard let scroll = list?.scroll, let home else { table.scrollToVisible(toEnd ? NSRect(x: 0, y: table.bounds.maxY - 1, width: 1, height: 1) : .zero); return }
        let clip = scroll.contentView
        clip.scroll(to: NSPoint(x: clip.bounds.minX, y: toEnd ? max(0, home.frame.height - clip.bounds.height) : 0))
        scroll.reflectScrolledClipView(clip)
    }
    /// The port moved the least that shows `row` whole; the list's first
    /// row with the padding before it, its last with the padding after.
    func show(row: Int) {
        guard row >= 0, row < count else { return }
        guard let scroll = list?.scroll, let home else { table.scrollToVisible(table.rect(ofRow: row)); return }
        let clip = scroll.contentView, port = clip.bounds
        let span = span(ofRow: row)
        let end = max(0, home.frame.height - port.height)
        var y = port.minY
        if row == 0 { y = 0 } else if row == count - 1 { y = end }
        else if span.lowerBound < port.minY { y = span.lowerBound } else if span.upperBound > port.maxY { y = span.upperBound - port.height }
        y = max(0, min(end, y))
        guard y != port.minY else { return }
        clip.scroll(to: NSPoint(x: port.minX, y: y))
        scroll.reflectScrolledClipView(clip)
    }
    /// Where a row is down the document: layout's place for a mounted row.
    /// For any other, its share of the room the runner's extent leaves
    /// around the mounted rows, which is where the list's own scroller has
    /// it; the table's place for such a row is AppKit's guess from the
    /// estimate, and the table is no longer than its rows' estimates.
    private func span(ofRow row: Int) -> ClosedRange<CGFloat> {
        if let mounted = self.row(at: row) { return mounted.top...(mounted.top + mounted.height) }
        let content = list?.contentBox().minY ?? 0
        let extent = content + CGFloat(presenter?.collections.entries[id]?.snapshot.extent ?? 0)
        var before: Row?, after: Row?
        for mounted in rows { if mounted.index < row { before = mounted } else { after = mounted; break } }
        let from = before.map { $0.top + $0.height } ?? content, first = (before?.index ?? -1) + 1
        let to = after?.top ?? extent, last = after?.index ?? count
        let each = max(0, to - from) / CGFloat(max(1, last - first))
        let top = from + CGFloat(row - first) * each
        return top...(top + each)
    }

    /// Each cell the table has holds the row mounted for its item, or
    /// nothing; a mounted row the table has no cell for is in the document,
    /// and so is a child that is no row (`children`).
    private func fill() {
        table.enumerateAvailableRowViews { view, index in
            guard let cell = view.view(atColumn: 0) as? TableRowCell else { return }
            if let row = self.row(at: index) { cell.adopt(row.node) } else { cell.sendHome() }
        }
        guard let home else { return }
        var kept = false
        func keep(_ node: NodeView) {
            guard !(node.superview is TableRowCell), node.superview !== home else { return }
            node.prepareToMount()
            home.addSubview(node)
            moves += 1
            kept = true
        }
        let all = children?.filter { presenter?.views[$0.id] === $0 } ?? rows.map(\.node)
        all.forEach(keep)
        // In the document they are in their order, as the presenter keeps its own.
        if kept { presenter?.collections.orderChildren(all, in: home) }
    }

    func numberOfRows(in tableView: NSTableView) -> Int { count }
    func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat { self.row(at: row)?.height ?? estimate }
    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let cell = tableView.makeView(withIdentifier: Self.column, owner: nil) as? TableRowCell ?? TableRowCell(feed: self)
        cell.identifier = Self.column
        if let mounted = self.row(at: row) { cell.adopt(mounted.node) } else { cell.sendHome() }
        return cell
    }
    func tableView(_ tableView: NSTableView, didAdd rowView: NSTableRowView, forRow row: Int) {
        (rowView.view(atColumn: 0) as? TableRowCell)?.place()
    }
    /// The table let a row's view go: the runner's row, if it is still
    /// mounted, stays in the document.
    func tableView(_ tableView: NSTableView, didRemove rowView: NSTableRowView, forRow row: Int) {
        (rowView.view(atColumn: 0) as? TableRowCell)?.sendHome()
    }

    // MARK: Selection and the row's press

    /// A row is selected, by a click or a key, when it presses: its
    /// authored root has a press and is not disabled. Any other row is a
    /// row of text, as in SwiftUI's list with no selection. A row that is
    /// not mounted may be reached by a key (the list's last row): what it
    /// is shows when the runner has made it.
    private func selectable(_ row: Row) -> Bool {
        guard let root = presenter?.views[row.root] else { return false }
        return root.pressable && !root.disabled && !root.inert
    }
    func tableView(_ tableView: NSTableView, shouldSelectRow row: Int) -> Bool { self.row(at: row).map(selectable) ?? true }
    /// A mounted row's text, for the table's type-select. A row that is not
    /// mounted has none here: the runner has its item, not the host.
    func tableView(_ tableView: NSTableView, typeSelectStringFor tableColumn: NSTableColumn?, row: Int) -> String? {
        self.row(at: row).map(\.node.accessibleName).flatMap { $0.isEmpty ? nil : $0 }
    }
    private func remember(_ row: Row?) {
        selectedKey = row?.node.props["listItemKey"]
        selectedNode = selectedKey == nil ? row?.node.id : nil
    }
    /// Whether a row's press is known to show another screen of the app:
    /// its root is a link to one of the app's routes. A row whose `press`
    /// is an action is not known to (the action may push, or delete): the
    /// compiler knows which writes the router and no host is told yet.
    private func navigates(_ row: Row) -> Bool {
        guard let root = presenter?.views[row.root], let url = root.defaultLink else { return false }
        return url.hasPrefix("/") && !url.hasPrefix("//")
    }
    func tableViewSelectionDidChange(_ notification: Notification) {
        guard !syncing else { return }
        let row = table.selectedRow, mounted = self.row(at: row)
        remember(mounted)
        reveal = row >= 0 && mounted == nil ? row : nil
        // The keyboard moved the selection to a navigation row: it is
        // pressed, as SwiftUI's list of navigation links pushes the row an
        // arrow key reaches (measured, macOS 27.0). A stock table's action
        // is the mouse's alone, and so is every other row's press here.
        if let mounted, NSApp.currentEvent?.type == .keyDown, navigates(mounted) { pressRow(row) }
    }
    /// A click on a row, when the mouse comes up in it: the table's action.
    /// In a hand-built table on macOS 27.0, key window, nothing a delegate
    /// or a target hears happens while the button is down. A click that
    /// was a context click (Control held, and a node in the row answered)
    /// presses nothing.
    @objc private func clicked() {
        if let taken = contextEvent, NSApp.currentEvent?.eventNumber == taken { return }
        contextEvent = nil
        pressRow(table.clickedRow)
    }
    /// The second click of two on a row whose root hears `dblclick`. A
    /// table sends its action for the first click and this for the second
    /// (measured, `TableFeedKeyMacTests`), so the row was pressed once: a
    /// row that pushes is not pushed twice, where the DOM sends two
    /// `click`s before its `dblclick`.
    @objc private func doubleClicked() {
        guard let row = row(at: table.clickedRow), selectable(row), let root = presenter?.views[row.root], root.handlers.contains("dblclick") else { return }
        presenter?.dblclick(root.id)
    }
    /// The mouse event a node in a row answered as a context click.
    private var contextEvent: Int?
    /// A secondary click at the event's point: the nearest node from what
    /// is under it up to its row's root that answers a context request
    /// hears it. False when none does.
    fileprivate func contextClick(_ event: NSEvent) -> Bool {
        guard let presenter, let content = table.window?.contentView else { return false }
        let point = event.locationInWindow
        var view = TableRowCell.whatIsThere { content.hitTest(content.superview?.convert(point, from: nil) ?? point) }
        while let current = view, !(current is TableRowCell), !(current is NSTableView) {
            if let node = current as? NodeView, presenter.views[node.id] === node, node.answersContextMenu, !node.disabled, !node.inert {
                contextEvent = event.eventNumber
                return presenter.contextMenu(node, at: point)
            }
            view = current.superview
        }
        return false
    }
    /// Who hears a key while the table has the keyboard: the selected
    /// row's authored root and everything above it, else the list. A
    /// handler that prevents a key's default keeps it from the table
    /// (`Presenter.routeKey`). It is the focus the runner is told of too
    /// (`CollectionHost.focusedView`), and the runner keeps a row that has
    /// the focus: while the table has the keyboard its selected row stays
    /// mounted however far the list scrolls, so the selection is its
    /// item's and not an index's.
    var keyTarget: NodeView? {
        guard let presenter else { return nil }
        return row(at: table.selectedRow).flatMap { presenter.views[$0.root] } ?? list.flatMap { presenter.views[$0.id] === $0 ? $0 : nil }
    }
    /// `state.tables` and `layout <list>` (the agent).
    var summary: [String: Any] {
        ["view": "NSTableView", "style": "inset", "list": Int(id), "testId": list?.props["testId"] ?? NSNull(), "rows": count, "mounted": rows.count, "rowViews": rowViews,
         "windowed": children == nil, "selected": table.selectedRow >= 0 ? table.selectedRow : NSNull(), "selectedKey": selectedKey ?? NSNull(),
         "firstResponder": table.window?.firstResponder === table, "estimate": Double(estimate)]
    }
    /// The press of the row's authored root, as a click on it was.
    @discardableResult
    func pressRow(_ row: Int) -> Bool {
        guard let mounted = self.row(at: row), selectable(mounted) else { return false }
        presenter?.press(mounted.root)
        return true
    }

    /// The table leaves the list: every row back in the document.
    func remove() {
        table.enumerateAvailableRowViews { view, _ in (view.view(atColumn: 0) as? TableRowCell)?.sendHome() }
        if let home { for row in rows where row.node.superview == nil && holds(row.node) { home.addSubview(row.node) } }
        table.dataSource = nil
        table.delegate = nil
        table.removeFromSuperview()
    }
}
#endif

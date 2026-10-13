// @ref LLP 1115.002 D5 — a vertical list's rows in a real, view-based
// `NSTableView`, fed by the runner. The runner still owns which rows are
// mounted and layout still owns every size; the table owns its cells, its
// selection and its keys. The table has one row for every item, mounted or
// not, and is a view in the list's document, never the document itself:
// AppKit asks a delegate for the heights of a few hundred rows around what
// shows and estimates the rest (measured on macOS 27.0: 301 of 10,000), so
// a table's own length and its far rows' places are AppKit's guesses. The
// list's extent and offset stay the runner's, and the table is put where
// its rows that show are where layout has them.
//
// What the feed costs a frame is what stage 3b cut (measured, D5):
// - A mounted row goes straight into its cell and stays there until the
//   runner destroys it. The table keeps a row view for every row of the
//   mounted run the port is in, not only for the rows the port shows
//   (`RunnerTableView.visibleRect`), so a row built ahead of the port is
//   mounted once, in the slice that built it, and nothing moves when it
//   scrolls into sight. A mounted row the table has no cell for all the same
//   (one pinned far from the port) is in the list's document, where layout
//   has it; correctness never rests on the table having made a view.
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
    private weak var feed: TableFeed?
    private(set) weak var row: NodeView?
    /// Where layout has the row, in the list's document.
    private(set) var kernel = NSRect.zero
    /// The row, or something in it, moved since its text's start was read.
    fileprivate var textMoved = true
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
    }

    /// A control in the row keeps its clicks. The rest of the row is the
    /// table's, as a stock table's row is where no cell's control is: the
    /// table takes the keyboard as the mouse goes down, its click selects,
    /// and the feed presses the row's root.
    override func hitTest(_ point: NSPoint) -> NSView? {
        guard let hit = super.hitTest(point) else { return nil }
        return feed?.keepsClick(hit, in: self) == true ? hit : feed?.table ?? self
    }
}

/// The table: AppKit's. It overrides no mouse method: a table class that
/// does is tracked in AppKit's older loop, which selects as the mouse goes
/// down, where a stock table and SwiftUI's list select as it comes up.
final class RunnerTableView: NSTableView {
    fileprivate weak var feed: TableFeed?
    /// What the table keeps row views for: what shows, and with it the run
    /// of mounted rows the port is in (`TableFeed.cover`). AppKit reads this
    /// to decide which rows have views (measured on macOS 27.0, and in
    /// `TableFeedMacTests`); where it does not, a mounted row with no view
    /// is in the list's document, as before.
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
    /// Home and End are the list's: the table's own ends are estimates.
    override func scrollToBeginningOfDocument(_ sender: Any?) { feed?.scroll(toEnd: false) }
    override func scrollToEndOfDocument(_ sender: Any?) { feed?.scroll(toEnd: true) }
}

/// One list's rows in a table.
final class TableFeed: NSObject, NSTableViewDataSource, NSTableViewDelegate {
    /// What presses a row's authored root. A click does, when the mouse
    /// comes up in the row: the table's action. In a hand-built table on
    /// macOS 27.0, key window, nothing a delegate or a target hears happens
    /// while the button is down. `selection` presses too when the keyboard
    /// moves the selection, as SwiftUI's list of navigation links pushes.
    enum Press { case action, selection }

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
    /// How tall the table has a row that is not mounted: the commonest
    /// height among the rows mounted when the table was last loaded.
    private(set) var estimate: CGFloat = 1
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
    fileprivate var dirty = true
    /// The selected item's key while its row is mounted: the selection
    /// follows the item when rows before it come and go.
    private(set) var selectedKey: String?
    /// A row selected before the runner had made it (a key that jumps to
    /// the list's end): AppKit showed where it guessed the row is, and the
    /// feed shows it once, when layout has it.
    private var reveal: Int?
    private var syncing = false, aligning = false
    var press = Press.action
    /// Rows moved into a cell or out of one, and the table's rows told of a
    /// new height, since the feed began (the tests, the measures).
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
        table.feed = self
        sync()
    }

    /// The list's document: where the presenter keeps the rows.
    var home: NSView? { list?.scroll?.documentView }
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

    /// Whether a click that hit `hit` in a row's cell is that view's own: an
    /// AppKit control or a text view, a node that presses, scrolls, edits
    /// or draws for itself, anywhere between the hit and the row's root. The
    /// root's own press is the row's, and so the table's.
    fileprivate func keepsClick(_ hit: NSView, in cell: TableRowCell) -> Bool {
        guard let row = cell.row else { return false }
        let root = self.row(of: row)?.root
        var view: NSView? = hit
        while let current = view, current !== cell {
            if let node = current as? NodeView {
                if node === row || node.id == root { return false }
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
    private func row(of node: NodeView) -> Row? { rows.first { $0.node === node } }
    /// Whether `node` is one of this list's rows as the runner has them in
    /// the batch being applied: the feed mounts it, not the presenter.
    fileprivate func expects(_ node: NodeView) -> Bool {
        presenter?.collections.entries[id]?.snapshot.rows.contains { $0.view == node.id } == true
    }
    private func kernel(_ node: NodeView) -> NSRect { (node.superview as? TableRowCell)?.kernel ?? node.frame }

    /// After a batch's ops and before the list's offset is corrected: the
    /// table's row count and heights from layout, the table where layout
    /// has the rows, and each mounted row in the cell the table has for it.
    func sync() {
        guard let presenter, let list, presenter.views[id] === list, let entry = presenter.collections.entries[id] else { return }
        let snapshot = entry.snapshot
        let pixel = 1 / (table.window?.backingScaleFactor ?? 1)
        let port = list.scroll?.contentView.bounds ?? .zero
        // A batch that changed no row, no frame of one and nothing of the port's width leaves the table as it is.
        let now = [snapshot.revision, UInt64(snapshot.count), UInt64(snapshot.rows.count), snapshot.extent.bitPattern, Double(port.width).bitPattern, Double(pixel).bitPattern]
        guard dirty || now != stamp || table.superview !== home else { return }
        stamp = now
        dirty = false

        var next: [Row] = []
        next.reserveCapacity(snapshot.rows.count)
        var ordered = true
        for row in snapshot.rows where row.index >= 0 && row.index < snapshot.count {
            guard let node = presenter.views[row.view] else { continue }
            if let last = next.last, last.index >= row.index { ordered = false }
            next.append(Row(index: row.index, node: node, root: row.root, kernel: kernel(node)))
        }
        if !ordered { next.sort { $0.index < $1.index } }
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

        let recount = snapshot.count != count
        if recount {
            // The items changed: every row is asked again, and the estimate is the commonest height now.
            count = snapshot.count
            var seen: [CGFloat: Int] = [:]
            for row in next { seen[row.height, default: 0] += 1 }
            estimate = seen.max { $0.value < $1.value || ($0.value == $1.value && $0.key > $1.key) }?.key
                ?? max(Self.least, (CGFloat(snapshot.extent) / CGFloat(max(1, snapshot.count)) / pixel).rounded() * pixel)
            told.removeAll(keepingCapacity: true)
        }
        // A row selected before it was made is its item's from now on.
        if selectedKey == nil, let row = row(at: table.selectedRow) { selectedKey = row.node.props["listItemKey"] }
        // The selected item, wherever its row is now.
        let selected = selectedKey.flatMap { key in next.first { $0.node.props["listItemKey"] == key }?.index }

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
            seenNow[row.node.id] = (row.index, row.height)
        }
        told = seenNow
        if !changed.isEmpty {
            still { table.noteHeightOfRows(withIndexesChanged: changed) }
            noted += changed.count
        }
        if let selected, table.selectedRow != selected { table.selectRowIndexes([selected], byExtendingSelection: false) }
        // The run of mounted rows the port is in: the table keeps a view for each of them.
        let anchor = anchor(port)
        var cover: ClosedRange<CGFloat>?
        if let anchor {
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
        let port = list?.scroll?.contentView.bounds ?? .zero
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
        guard let scroll = list?.scroll, let home else { return }
        let clip = scroll.contentView
        clip.scroll(to: NSPoint(x: clip.bounds.minX, y: toEnd ? max(0, home.frame.height - clip.bounds.height) : 0))
        scroll.reflectScrolledClipView(clip)
    }
    /// The port moved the least that shows `row` whole; the list's first
    /// row with the padding before it, its last with the padding after.
    fileprivate func show(row: Int) {
        guard row >= 0, row < count, let scroll = list?.scroll, let home else { return }
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
    /// nothing; a mounted row the table has no cell for is in the document.
    private func fill() {
        table.enumerateAvailableRowViews { view, index in
            guard let cell = view.view(atColumn: 0) as? TableRowCell else { return }
            if let row = self.row(at: index) { cell.adopt(row.node) } else { cell.sendHome() }
        }
        guard let home else { return }
        for row in rows where !(row.node.superview is TableRowCell) && row.node.superview !== home {
            row.node.prepareToMount()
            home.addSubview(row.node)
            moves += 1
        }
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

    /// A mounted row's text, for the table's type-select. A row that is not
    /// mounted has none here: the runner has its item, not the host.
    func tableView(_ tableView: NSTableView, typeSelectStringFor tableColumn: NSTableColumn?, row: Int) -> String? {
        self.row(at: row).map(\.node.accessibleName).flatMap { $0.isEmpty ? nil : $0 }
    }
    func tableViewSelectionDidChange(_ notification: Notification) {
        guard !syncing else { return }
        let row = table.selectedRow, mounted = self.row(at: row)
        selectedKey = mounted?.node.props["listItemKey"]
        reveal = row >= 0 && mounted == nil ? row : nil
        if press == .selection, NSApp.currentEvent?.type == .keyDown { pressRow(row) }
    }
    @objc private func clicked() { pressRow(table.clickedRow) }
    /// The press of the row's authored root, as a click on it was.
    func pressRow(_ row: Int) {
        guard let root = self.row(at: row)?.root, let target = presenter?.views[root], target.pressable, !target.disabled, !target.inert else { return }
        presenter?.press(root)
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

/// The lists shown as tables.
final class TableHost {
    private weak var presenter: Presenter?
    private(set) var feeds: [UInt32: TableFeed] = [:]
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Show `list`'s rows in a table. The list is a virtualized one the
    /// runner windows (`CollectionHost.owns`).
    @discardableResult
    func show(_ list: NodeView, style: NSTableView.Style = .inset) -> TableFeed? {
        guard let presenter, presenter.views[list.id] === list, presenter.collections.owns(list.id), list.scroll != nil else { return nil }
        if let feed = feeds[list.id] { return feed }
        let feed = TableFeed(list, presenter: presenter, style: style)
        feeds[list.id] = feed
        return feed
    }
    /// Give the list back its rows as the presenter lays them out.
    func release(_ list: NodeView) { feeds.removeValue(forKey: list.id)?.remove() }

    /// Whether the feed mounts `node`, a child the list's `children` op
    /// names: a row already in a table's cell stays there, and a row the
    /// runner has just made, in no view yet, goes straight into the cell the
    /// table has for it when the batch's ops are done (`sync`).
    func owns(_ node: NodeView, in list: UInt32) -> Bool {
        if node.superview is TableRowCell { return true }
        guard node.superview == nil, let feed = feeds[list], feed.expects(node) else { return false }
        feed.dirty = true
        return true
    }
    /// A row in a cell takes layout's frame in the cell's space. Any other
    /// frame while a table shows may be a row's the presenter has, or of
    /// something in a row (where its text begins may have moved).
    func frame(_ node: NodeView, _ frame: NSRect) -> Bool {
        guard !feeds.isEmpty else { return false }
        if let cell = node.superview as? TableRowCell {
            cell.place(node, kernel: frame)
            for feed in feeds.values { feed.dirty = true }
            return true
        }
        var above = node.superview, hops = 0
        while let view = above, hops < 24 {
            if let cell = view as? TableRowCell { cell.textMoved = true; break }
            if view is NSTableView { break }
            above = view.superview
            hops += 1
        }
        for feed in feeds.values { feed.dirty = true }
        return false
    }
    /// After a batch's ops (`Presenter.apply`).
    func sync() {
        guard !feeds.isEmpty, let presenter else { return }
        for (id, feed) in feeds {
            if presenter.views[id] == nil || !presenter.collections.owns(id) { feeds[id] = nil; feed.remove() } else { feed.sync() }
        }
    }
    func reset() {
        feeds.values.forEach { $0.remove() }
        feeds.removeAll()
    }
}
#endif

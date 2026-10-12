// @ref LLP 1115.002 D5 — stage 3a, the gate: a vertical list's rows in a
// real, view-based `NSTableView`, fed by the runner. The runner still owns
// which rows are mounted and layout still owns every size; the table owns
// its cells, its selection and its keys. A cell holds the runner's own row
// while that row is mounted and the table has a cell for it, and is empty
// otherwise; a mounted row the table has no cell for stays where the
// presenter put it, in the list's document under the table, at the same
// place on the page. The table has one row for every item, mounted or not:
// a mounted row is as tall as layout made it, and the rows between two
// mounted ones share the room layout left for them. The table is a view
// in the list's document, never the document itself: AppKit asks a delegate
// for the heights of a few hundred rows around what shows and estimates the
// rest (measured on macOS 27.0: 301 of 10,000), so a table's own length and
// its far rows' places are AppKit's guesses. The list's extent and offset
// stay the runner's, and the table is put where its rows that show are
// where layout has them.
// Nothing in an app uses this yet: `TableFeedMacTests` drives it, and stage
// 3b (a `list` as a table) is built on what it holds.
#if os(macOS)
import AppKit

/// Each item's row height in a table. Proportional to the mounted rows:
/// no height is kept for a row that is not mounted.
struct TableRowHeights: Equatable {
    /// `count` rows from `first`, each `height` tall.
    struct Run: Equatable { var first: Int, count: Int; var height: CGFloat }
    private(set) var runs: [Run] = []
    private(set) var count = 0
    /// AppKit refuses a row with no height; a row layout gave none takes this.
    static let least: CGFloat = 1

    init() {}
    /// `mounted`: each mounted row's index, its start from the first item's
    /// start and its size, as layout has them. `extent`: every item's,
    /// mounted or estimated. A run of rows not mounted whose share moved by
    /// less than half a point in all keeps `previous`'s heights: float noise
    /// in a long list's offsets is not news to the table.
    init(count: Int, extent: CGFloat, mounted: [(index: Int, start: CGFloat, size: CGFloat)], previous: TableRowHeights? = nil) {
        self.count = max(0, count)
        var next = 0, at: CGFloat = 0
        for row in mounted.sorted(by: { $0.index < $1.index }) where row.index >= next && row.index < self.count {
            share(first: next, count: row.index - next, room: row.start - at, previous)
            runs.append(Run(first: row.index, count: 1, height: max(Self.least, row.size)))
            next = row.index + 1
            at = row.start + row.size
        }
        share(first: next, count: self.count - next, room: extent - at, previous)
    }
    private mutating func share(first: Int, count: Int, room: CGFloat, _ previous: TableRowHeights?) {
        guard count > 0 else { return }
        var height = max(Self.least, room / CGFloat(count))
        if let previous, previous.count == self.count, let run = previous.run(first), run.first + run.count >= first + count,
           abs(run.height - height) * CGFloat(count) < 0.5 { height = run.height }
        runs.append(Run(first: first, count: count, height: height))
    }
    private func run(_ row: Int) -> Run? {
        guard !runs.isEmpty, row >= 0, row < count else { return nil }
        var low = 0, high = runs.count - 1
        while low < high {
            let middle = (low + high + 1) / 2
            if runs[middle].first <= row { low = middle } else { high = middle - 1 }
        }
        return runs[low]
    }
    func height(_ row: Int) -> CGFloat { run(row)?.height ?? Self.least }
    /// The rows whose height is not what `old` has for them.
    func changed(from old: TableRowHeights) -> IndexSet {
        guard old.count == count else { return IndexSet(integersIn: 0..<count) }
        var out = IndexSet(), i = 0, j = 0, row = 0
        while row < count, i < runs.count, j < old.runs.count {
            let a = runs[i], b = old.runs[j]
            let end = min(a.first + a.count, b.first + b.count)
            if a.height != b.height { out.insert(integersIn: row..<end) }
            row = end
            if a.first + a.count == end { i += 1 }
            if b.first + b.count == end { j += 1 }
        }
        return out
    }
}

/// The table's own view for one row: around the runner's row while that row
/// is mounted, empty otherwise.
final class TableRowCell: NSView {
    private weak var feed: TableFeed?
    private(set) weak var row: NodeView?
    /// Where layout has the row, in the list's document.
    private(set) var kernel = NSRect.zero
    init(feed: TableFeed) { self.feed = feed; super.init(frame: .zero) }
    required init?(coder: NSCoder) { nil }
    override var isFlipped: Bool { true }

    /// Take `node` in, at the place layout gave it.
    func adopt(_ node: NodeView, cell frame: NSRect) {
        if row !== node { sendHome() }
        row = node
        if node.superview !== self {
            // Where the presenter has it: layout's frame, in the document.
            kernel = (node.superview as? TableRowCell)?.kernel ?? node.frame
            node.prepareToMount()
            addSubview(node)
            feed?.moved(node)
        }
        place(cell: frame)
    }
    /// Layout moved or resized the row (`TableHost.frame`).
    func place(_ node: NodeView, kernel frame: NSRect) {
        guard node === row else { return }
        kernel = frame
        place(cell: nil)
    }
    /// The row where layout has it, in this cell's space. `cell`: the cell's
    /// frame in the table, when it is not yet this view's.
    func place(cell frame: NSRect?) {
        guard let row, let feed, row.superview === self else { return }
        let origin = feed.documentOrigin(ofCell: frame ?? self.frame, in: superview)
        let wanted = kernel.offsetBy(dx: -origin.x, dy: -origin.y)
        if row.frame != wanted { row.frame = wanted }
    }
    /// Give the row back to the list's document, where the presenter has
    /// its siblings; a row that was destroyed in the cell simply goes.
    func sendHome() {
        defer { row = nil }
        guard let row, row.superview === self else { return }
        guard let feed, let home = feed.home, feed.holds(row) else { row.removeFromSuperview(); return }
        home.addSubview(row)
        row.frame = kernel
        feed.moved(row)
    }

    /// A control in the row keeps its clicks. The rest of the row is the
    /// table's: its click selects, and the feed presses the row's root.
    override func hitTest(_ point: NSPoint) -> NSView? {
        guard let hit = super.hitTest(point) else { return nil }
        return feed?.keepsClick(hit, in: self) == true ? hit : self
    }
}

/// The table: AppKit's, with what a table's own first-responder rule would
/// take from a control in a row given back to the control.
final class RunnerTableView: NSTableView {
    fileprivate weak var feed: TableFeed?
    /// A table lets a text field in a row take the keyboard only on a
    /// second click (its cells are labels that may be renamed). A field an
    /// author put in a row is a field: it takes its own click.
    override func validateProposedFirstResponder(_ responder: NSResponder, for event: NSEvent?) -> Bool { true }
    /// The table moved its rows (new rows near what shows, a new guess for
    /// the rows that do not): it goes back where layout has them.
    override func layout() {
        super.layout()
        feed?.tableLaidOut()
    }
    /// Home and End are the list's: the table's own ends are estimates.
    override func scrollToBeginningOfDocument(_ sender: Any?) { feed?.scroll(toEnd: false) }
    override func scrollToEndOfDocument(_ sender: Any?) { feed?.scroll(toEnd: true) }
}

/// One list's rows in a table.
final class TableFeed: NSObject, NSTableViewDataSource, NSTableViewDelegate {
    /// When a click on a row presses its root. A hand-built table selects on
    /// the mouse going down and sends its action when it comes up.
    enum Press { case selection, action, never }

    let table = RunnerTableView()
    let id: UInt32
    private weak var presenter: Presenter?
    private weak var list: NodeView?
    private(set) var heights = TableRowHeights()
    /// The mounted rows by item index, and each one's authored root.
    private var mounted: [Int: NodeView] = [:]
    private var roots: [UInt32: UInt32] = [:]
    private var held = Set<UInt32>()
    /// The selected item's key while its row is mounted: the selection
    /// follows the item when rows before it come and go.
    private(set) var selectedKey: String?
    private var syncing = false, aligning = false
    var press = Press.selection
    /// Rows moved between the document and a cell, and the table's rows
    /// told of a new height, since the feed began (the tests, the measures).
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
    fileprivate func moved(_ node: NodeView) { moves += 1 }

    /// A cell's origin in the document, from its frame in `parent` (its row
    /// view; the table itself for a cell not yet in one).
    fileprivate func documentOrigin(ofCell frame: NSRect, in parent: NSView?) -> NSPoint {
        guard let home else { return frame.origin }
        return home.convert(frame.origin, from: parent ?? table)
    }

    /// Whether a click that hit `hit` in a row's cell is that view's own: an
    /// AppKit control or a text view, a node that presses, scrolls, edits
    /// or draws for itself, anywhere between the hit and the row's root. The
    /// root's own press is the row's, and so the table's.
    fileprivate func keepsClick(_ hit: NSView, in cell: TableRowCell) -> Bool {
        guard let row = cell.row else { return false }
        let root = roots[row.id]
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

    /// After a batch's ops and before the list's offset is corrected: the
    /// table's row count and heights from layout, the table where layout
    /// has the rows, and each mounted row in the cell the table has for it.
    func sync() {
        guard let presenter, let list, presenter.views[id] === list, let entry = presenter.collections.entries[id] else { return }
        let snapshot = entry.snapshot, content = list.contentBox()
        var rows: [(index: Int, start: CGFloat, size: CGFloat)] = []
        var mounted: [Int: NodeView] = [:], roots: [UInt32: UInt32] = [:]
        for row in snapshot.rows where row.index >= 0 {
            guard let node = presenter.views[row.view] else { continue }
            let frame = (node.superview as? TableRowCell)?.kernel ?? node.frame
            rows.append((row.index, frame.minY - content.minY, frame.height))
            mounted[row.index] = node
            roots[row.view] = row.root
        }
        let next = TableRowHeights(count: snapshot.count, extent: CGFloat(snapshot.extent), mounted: rows, previous: heights)
        let recount = next.count != heights.count
        let changed = recount ? IndexSet() : next.changed(from: heights)
        // The selected item, wherever its row is now.
        let selected = selectedKey.flatMap { key in mounted.first { $0.value.props["listItemKey"] == key }?.key }
        heights = next
        self.mounted = mounted
        self.roots = roots
        held = Set(mounted.values.map(\.id))
        syncing = true
        defer { syncing = false }
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0
            context.allowsImplicitAnimation = false
            if recount {
                table.reloadData()
                noted += next.count
            } else if !changed.isEmpty {
                table.noteHeightOfRows(withIndexesChanged: changed)
                noted += changed.count
            }
            if let selected, table.selectedRow != selected { table.selectRowIndexes([selected], byExtendingSelection: false) }
            align()
            table.layoutSubtreeIfNeeded()
            fill()
        }
    }

    /// The table where its rows are where layout has them: a mounted row's
    /// place in the table, under the table's own insets, is layout's.
    private func align() {
        guard let list, let home, !aligning else { return }
        aligning = true
        defer { aligning = false }
        if table.superview !== home { home.addSubview(table, positioned: .below, relativeTo: nil) }
        // As wide as the port: a row that overflows widens the document, not the table.
        let port = list.scroll?.contentView.bounds ?? .zero
        if table.frame.width != port.width { table.setFrameSize(NSSize(width: port.width, height: table.frame.height)) }
        // The first mounted row that shows, else any: float error in a long
        // list's offsets then lands in rows nobody sees.
        let shown = mounted.filter { kernel($0.value).maxY > port.minY }.min { $0.key < $1.key } ?? mounted.min { $0.key < $1.key }
        let y: CGFloat
        if let shown { y = kernel(shown.value).minY - table.rect(ofRow: shown.key).minY }
        else { y = list.contentBox().minY - (table.numberOfRows > 0 ? table.rect(ofRow: 0).minY : 0) }
        if table.frame.origin != NSPoint(x: 0, y: y) { table.setFrameOrigin(NSPoint(x: 0, y: y)) }
    }
    private func kernel(_ node: NodeView) -> NSRect { (node.superview as? TableRowCell)?.kernel ?? node.frame }
    fileprivate func tableLaidOut() {
        guard !syncing, !aligning else { return }
        align()
        table.enumerateAvailableRowViews { view, _ in (view.view(atColumn: 0) as? TableRowCell)?.place(cell: nil) }
    }
    /// The list's start or its end, as the runner has them.
    fileprivate func scroll(toEnd: Bool) {
        guard let scroll = list?.scroll, let home else { return }
        let clip = scroll.contentView
        clip.scroll(to: NSPoint(x: clip.bounds.minX, y: toEnd ? max(0, home.frame.height - clip.bounds.height) : 0))
        scroll.reflectScrolledClipView(clip)
    }

    /// Each cell the table has holds the row mounted for its item, or nothing.
    private func fill() {
        table.enumerateAvailableRowViews { view, row in
            guard let cell = view.view(atColumn: 0) as? TableRowCell else { return }
            if let node = mounted[row] { cell.adopt(node, cell: cell.frame) } else { cell.sendHome() }
        }
    }

    func numberOfRows(in tableView: NSTableView) -> Int { heights.count }
    func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat { heights.height(row) }
    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let cell = tableView.makeView(withIdentifier: Self.column, owner: nil) as? TableRowCell ?? TableRowCell(feed: self)
        cell.identifier = Self.column
        if let node = mounted[row] { cell.adopt(node, cell: tableView.frameOfCell(atColumn: 0, row: row)) } else { cell.sendHome() }
        return cell
    }
    func tableView(_ tableView: NSTableView, didAdd rowView: NSTableRowView, forRow row: Int) {
        (rowView.view(atColumn: 0) as? TableRowCell)?.place(cell: nil)
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
        mounted[row].map(\.accessibleName).flatMap { $0.isEmpty ? nil : $0 }
    }
    func tableViewSelectionDidChange(_ notification: Notification) {
        let row = table.selectedRow
        if !syncing { selectedKey = mounted[row]?.props["listItemKey"] }
        if press == .selection, !syncing, NSApp.currentEvent?.type == .leftMouseDown { pressRow(row) }
    }
    @objc private func clicked() {
        if press == .action { pressRow(table.clickedRow) }
    }
    /// The press of the row's authored root, as a click on it was.
    func pressRow(_ row: Int) {
        guard let node = mounted[row], let root = roots[node.id], let target = presenter?.views[root], target.pressable, !target.disabled, !target.inert else { return }
        presenter?.press(root)
    }

    /// The table leaves the list: every row back in the document.
    func remove() {
        table.enumerateAvailableRowViews { view, _ in (view.view(atColumn: 0) as? TableRowCell)?.sendHome() }
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

    /// Whether `node` is a row in a table's cell: a `children` op of its
    /// list leaves it there.
    func owns(_ node: NodeView) -> Bool { node.superview is TableRowCell }
    /// A row in a cell takes layout's frame in the cell's space.
    func frame(_ node: NodeView, _ frame: NSRect) -> Bool {
        guard let cell = node.superview as? TableRowCell else { return false }
        cell.place(node, kernel: frame)
        return true
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

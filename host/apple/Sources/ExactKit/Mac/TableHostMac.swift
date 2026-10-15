// @ref LLP 1115.002 D5, LLP 1115.000 D16 — a vertical `list` is a table
// on the Mac: AppKit's `NSTableView`, with its selection, its separators
// and its keys, holding the runner's rows (`TableFeed`, TableFeedMac.swift).
// Contract marks the lists whose rows are cells; this host reads that mark.
//
// A marked list is a table whether the runner windows its rows or keeps
// them all mounted. Contract excludes carousels, authored arrangements,
// grouped forms and sections (D16). Kept painted too, each said once in the log
// and queued (LLP 1115.002 §3, stage 3b): a list whose rows are in
// `section`s, a list inside another list's row, and a list whose rows are
// dragged to reorder.
//
// Under the agent the rows are painted, as the header and the tablist are,
// and `--chrome platform` shows the table.
#if os(macOS)
import AppKit

final class TableHost {
    private weak var presenter: Presenter?
    private(set) var feeds: [UInt32: TableFeed] = [:]
    /// Painted rows, not AppKit's table: the driver's own chrome
    /// (`--chrome agent`), as it paints the header and the tablist. A test
    /// of the painted list says so.
    var painted = ExactEnv.authoredChrome { didSet { if painted != oldValue { decide() } } }
    /// Why each list that is no table is none, as last said in the log.
    private var said: [UInt32: String] = [:]
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Why `list` is not a table; nil when it is one.
    private func refusal(_ list: NodeView) -> String? {
        guard presenter != nil, list.kind == "list" else { return "it is no list" }
        if list.props["listStyle"] != nil { return "appearance=\"auto\": the grouped form, painted (LLP 1115.002 D6)" }
        if list.style["appearance"]?.string == "none" { return "appearance=\"none\": the author's rows" }
        guard list.props["listRows"] == "true" else { return "Contract leaves its authored rows (LLP 1115.000 D16)" }
        if list.handlers.contains("reorderdrop") { return "its rows are dragged to reorder, which the table does not do yet" }
        var above = list.superview
        while let view = above {
            if view is TableRowCell || (view as? NodeView)?.kind == "list" { return "it is inside another list's row" }
            above = view.superview
        }
        return nil
    }

    /// Which lists are tables, read again by a batch that can change the
    /// answer (a list made or destroyed, restyled, given its window or its
    /// children) and when the driver's chrome changes.
    private func decide() {
        guard let presenter else { return }
        for (id, feed) in feeds where presenter.views[id] == nil || presenter.listViews[id] == nil { feeds[id] = nil; said[id] = nil; feed.remove() }
        for (id, list) in presenter.listViews {
            let why = painted ? "" : refusal(list)
            if let why {
                if let feed = feeds.removeValue(forKey: id) { feed.remove(); presenter.keyViewLoopStale = true }
                if !why.isEmpty, said[id] != why { said[id] = why; presenter.session?.log("table: list #\(id) keeps painted rows: \(why)") }
            } else if feeds[id] == nil {
                feeds[id] = TableFeed(list, presenter: presenter, style: .inset)
                presenter.keyViewLoopStale = true
                if said[id] != "" { said[id] = ""; presenter.session?.log("table: list #\(id) is an NSTableView") }
            }
        }
        said = said.filter { presenter.listViews[$0.key] != nil }
    }
    private static let deciding: Set<BatchOp.Kind> = [.create, .destroy, .style, .props, .children, .collections]

    /// Whether the feed mounts `node`, a child the `children` op of `list`
    /// names. A row in a table's cell stays there. A row the runner has
    /// just made, in no view yet, goes straight into the cell the table has
    /// for it when the batch's ops are done (`sync`). Every child of a list
    /// the runner does not window is the feed's: a row in a cell or in the
    /// document, and a child that is no row in the document.
    func owns(_ node: NodeView, in list: UInt32) -> Bool {
        guard let feed = feeds[list] else { return node.superview is TableRowCell }
        guard node.superview == nil || feed.children != nil else { return node.superview is TableRowCell }
        guard feed.expects(node) else { return false }
        feed.dirty = true
        if node.superview == nil { feed.arrivals += 1 }
        return true
    }
    /// The `children` op of a list: the rows of one the runner does not window.
    func children(_ list: UInt32, _ want: [NodeView]) {
        if let feed = feeds[list], feed.children != nil { feed.children = want }
    }
    /// A row in a cell takes layout's frame in the cell's space. Any other
    /// frame while a table shows may be a row's in the document, a new
    /// row's, or of something inside a row (where its text begins may have moved).
    func frame(_ node: NodeView, _ frame: NSRect) -> Bool {
        guard !feeds.isEmpty else { return false }
        if let cell = node.superview as? TableRowCell {
            cell.place(node, kernel: frame)
            cell.feed?.dirty = true
            return true
        }
        guard var above = node.superview else {
            for feed in feeds.values { feed.dirty = true }
            return false
        }
        for feed in feeds.values where feed.home === above { feed.dirty = true; return false }
        for _ in 0..<24 {
            if let cell = above as? TableRowCell { cell.textMoved = true; cell.feed?.dirty = true; break }
            guard !(above is NSTableView), let next = above.superview else { break }
            above = next
        }
        return false
    }
    /// Whether `node` is what a click on its table's row presses
    /// (`TableFeed.pressing`): the table's own to select, and no Tab stop.
    func pressesRow(_ node: NodeView) -> Bool {
        var above: NSView? = node
        for _ in 0..<24 {
            guard let view = above, !(view is NSTableView) else { return false }
            if let cell = view as? TableRowCell { return cell.row.flatMap { cell.feed?.row(of: $0) }.flatMap { cell.feed?.pressing($0) } === node }
            above = view.superview
        }
        return false
    }

    // MARK: The agent

    /// What AppKit shows for a list that is a table, and for a row of one
    /// (its view or its authored root): the agent's `layout <node>`.
    func observation(_ node: NodeView) -> [String: Any]? {
        if let feed = feeds[node.id] { return feed.summary }
        for feed in feeds.values {
            guard let row = feed.rows.first(where: { $0.node === node || $0.root == node.id || feed.pressing($0) === node }) else { continue }
            return ["view": "NSTableView", "list": Int(feed.id), "row": row.index, "selected": feed.table.selectedRow == row.index,
                    "selectable": feed.tableView(feed.table, shouldSelectRow: row.index), "inCell": row.node.superview is TableRowCell]
        }
        return nil
    }
    /// `state.tables` (the agent): each list that is a table.
    var summary: [[String: Any]] { feeds.values.sorted { $0.id < $1.id }.map(\.summary) }

    /// After a batch's ops (`Presenter.apply`).
    func sync(_ batch: Batch) {
        if !painted || !feeds.isEmpty, batch.ops.contains(where: { Self.deciding.contains($0.op) }) { decide() }
        for feed in feeds.values { feed.sync() }
    }
    func reset() {
        feeds.values.forEach { $0.remove() }
        feeds.removeAll()
        said.removeAll()
    }
}
#endif

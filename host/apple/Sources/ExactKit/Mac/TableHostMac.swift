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
    private var rowChildren: [UInt32: [UInt32]] = [:]
    private var nextRow: [UInt32: [UInt32: UInt32]] = [:]
    private var lines: [UInt32: CALayer] = [:]
    /// Painted rows, not AppKit's table: the driver's own chrome
    /// (`--chrome agent`), as it paints the header and the tablist. A test
    /// of the painted list says so.
    var painted = ExactEnv.authoredChrome { didSet { if painted != oldValue { decide(); syncPainted() } } }
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
    /// The list's logical children, including a windowed list's spacers.
    func children(_ list: UInt32, _ want: [NodeView]) {
        if presenter?.listViews[list] != nil {
            rowChildren[list] = want.map(\.id)
            var next: [UInt32: UInt32] = [:]
            for (row, after) in zip(want, want.dropFirst()) { next[row.id] = after.id }
            nextRow[list] = next
        }
        if let feed = feeds[list], feed.children != nil { feed.children = want }
    }

    /// The Swift projection of `Kernel.row_separator` (LLP 1115.000 D16).
    /// The wrapper is the row; only its authored child's edge intent is read.
    /// Logical siblings include the children AppKit does not make table rows.
    func separates(_ row: NodeView, in list: UInt32) -> Bool {
        guard let nextID = nextRow[list]?[row.id], let next = presenter?.views[nextID],
              row.props["accessibilityElementsHidden"] != "true", !row.wrote("box_shadow") else { return false }
        func authored(_ node: NodeView) -> NodeView? {
            node.props["listItemKey"] == nil ? node : node.container.subviews.compactMap { $0 as? NodeView }.first
        }
        func owns(_ node: NodeView?, _ edge: String) -> Bool {
            node.map { $0.wrote("border_width_" + edge) || $0.wrote("border_style_" + edge) } ?? false
        }
        return !owns(authored(row), "bottom") && !owns(authored(next), "top")
    }

    /// The native cell and the painted row use the same leading paragraph.
    static func textStart(of node: NodeView) -> NodeView? {
        if node.kind == "text" { return node }
        for case let child as NodeView in node.container.subviews where !child.isHidden {
            if let found = textStart(of: child) { return found }
        }
        return nil
    }

    /// A layer on the row follows its transform and opacity and takes no hit.
    /// Ownership is read after every batch, including a written-bit-only one.
    private func syncPainted() {
        guard let presenter else { return }
        for list in rowChildren.keys where presenter.listViews[list] == nil {
            nextRow[list] = nil
            rowChildren[list] = nil
        }
        var needed = Set<UInt32>()
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        for (id, list) in presenter.listViews where list.props["listRows"] == "true" && feeds[id] == nil {
            // Keep logical sibling order; paint rank does not change whose top
            // edge owns the preceding separator. Spacers stay in this order.
            let rows = (rowChildren[id] ?? []).compactMap { presenter.views[$0] }
            for row in rows {
                guard separates(row, in: id) else { continue }
                let lead = Self.textStart(of: row).map { row.convert(NSPoint(x: $0.bounds.minX, y: 0), from: $0).x } ?? 0
                let end = min(row.bounds.maxX, row.convert(NSPoint(x: list.bounds.maxX - 16, y: 0), from: list).x)
                let from = max(row.bounds.minX, lead)
                guard end > from, row.bounds.height >= 1 else { continue }
                row.wantsLayer = true
                guard let layer = row.layer else { continue }
                let line = lines[row.id] ?? CALayer()
                lines[row.id] = line
                needed.insert(row.id)
                if line.superlayer !== layer { layer.addSublayer(line) }
                let scale = row.window?.backingScaleFactor ?? 1
                let bottom = (row.bounds.maxY * scale).rounded() / scale
                // The measured AppKit line is one point, ending at the row's
                // bottom; the trailing cell inset is sixteen points.
                let frame = NSRect(x: from, y: bottom - 1, width: end - from, height: 1)
                if line.frame != frame { line.frame = frame }
                row.effectiveAppearance.performAsCurrentDrawingAppearance {
                    let color = NSColor.separatorColor.cgColor
                    if line.backgroundColor != color { line.backgroundColor = color }
                }
            }
        }
        for (id, line) in lines where !needed.contains(id) { line.removeFromSuperlayer(); lines[id] = nil }
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
        syncPainted()
    }
    func reset() {
        feeds.values.forEach { $0.remove() }
        feeds.removeAll()
        said.removeAll()
        rowChildren.removeAll()
        nextRow.removeAll()
        lines.values.forEach { $0.removeFromSuperlayer() }
        lines.removeAll()
    }
}
#endif

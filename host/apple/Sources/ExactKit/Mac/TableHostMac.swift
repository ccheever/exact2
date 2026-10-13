// @ref LLP 1115.002 D5, LLP 1115.000 D16 — a vertical `list` is a table
// on the Mac: AppKit's `NSTableView`, with its selection, its separators
// and its keys, holding the runner's rows (`TableFeed`, TableFeedMac.swift).
// This host says which lists.
//
// A list is a table when it is a `list` that lays its rows out down the
// page: one the runner windows (`virtualized`), or any other, whose
// children are its rows. Not a list that scrolls sideways (a carousel,
// D16); not `appearance="none"`, the author's opt-out; not
// `appearance="auto"`, the grouped form, which is painted in the Mac's
// metrics (LLP 1115.002 D6). Kept painted too, each said once in the log
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

    /// Whether a child of a list the runner does not window is a row: it
    /// has a box in the list's flow. A child with none (`display: none`), or
    /// a positioned one, stays in the document as the presenter has it. The
    /// host is not told a child's `position`; it is told its paint rank
    /// among its siblings (LLP 1083), which is none for a box in the flow.
    static func isRow(_ node: NodeView) -> Bool { !node.isHidden && node.frame.height > 0 && node.rank == 0 }

    /// Why `list` is not a table; nil when it is one.
    private func refusal(_ list: NodeView) -> String? {
        guard let presenter, list.kind == "list" else { return "it is no list" }
        if list.props["listStyle"] != nil { return "appearance=\"auto\": the grouped form, painted (LLP 1115.002 D6)" }
        if list.style["appearance"]?.string == "none" { return "appearance=\"none\": the author's rows" }
        let windowed = presenter.collections.entries[list.id]?.snapshot
        // Rows go down the page in a block (the list's own layout) and in a
        // flex column; a flex row, a grid and a column written upward are
        // the author's arrangement of items, not rows.
        let display = list.style["display"]?.string ?? "block", direction = list.style["flex_direction"]?.string ?? "row"
        if windowed?.horizontal == true || list.props["estimatedItemWidth"] != nil || (display.hasSuffix("flex") && direction != "column") || display.hasSuffix("grid") {
            return "it lays its items out sideways: a carousel, not rows (LLP 1115.000 D16)"
        }
        if let scroll = list.scroll, !scroll.scrollsY { return "it scrolls sideways only" }
        if list.handlers.contains("reorderdrop") { return "its rows are dragged to reorder, which the table does not do yet" }
        var above = list.superview
        while let view = above {
            if view is TableRowCell || (view as? NodeView)?.kind == "list" { return "it is inside another list's row" }
            above = view.superview
        }
        if windowed == nil, list.container.subviews.contains(where: { ($0 as? NodeView)?.props["semanticTag"] == "section" })
            || feeds[list.id]?.children?.contains(where: { $0.props["semanticTag"] == "section" }) == true {
            return "its rows are in sections, which the table does not show yet"
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
                feeds.removeValue(forKey: id)?.remove()
                if !why.isEmpty, said[id] != why { said[id] = why; presenter.session?.log("table: list #\(id) keeps painted rows: \(why)") }
            } else if feeds[id] == nil {
                feeds[id] = TableFeed(list, presenter: presenter, style: .inset)
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

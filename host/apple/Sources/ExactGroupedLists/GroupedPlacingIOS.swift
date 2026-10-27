// Which batches take a grouped list's carried rows home, and where a row
// goes when it does (`GroupedListView.restore`, `sendHome`); and the rows a
// scroll to a row passes, sized before it aims (`measure`).
import ExactKit
#if os(iOS) || os(tvOS)
import UIKit

extension GroupedListHost {
    func prepare(_ ops: [BatchOp]) { for list in lists.values { list.prepare(ops) } }

    func placing(_ container: UIView, _ want: [NodeView]) -> [NodeView] {
        lists.values.reduce(want) { $1.placing(container, $0) }
    }

    func carried(from container: UIView) -> [NodeView] {
        lists.values.flatMap { $0.carried(from: container) }
    }

    func cellFrame(of view: NodeView, at place: GridPlace) -> CGRect? {
        guard presenter.views[view.id] === view, lists.values.contains(where: { $0.carried[view.id] != nil }) else { return nil }
        return GroupedListView.cellFrame(place, scale: presenter.gridScale)
    }
}

extension GroupedListView {
    /// UIKit's card and cell margins, left then right, inside the list's
    /// content box. The kernel already applies CSS padding; including it
    /// in the card margins would subtract it from a custom row's width twice.
    func margins() -> [CGFloat]? {
        guard let cell = collection.visibleCells.first as? UICollectionViewListCell,
              let path = collection.indexPath(for: cell),
              let frame = collection.layoutAttributesForItem(at: path)?.frame else { return nil }
        let m = cell.directionalLayoutMargins
        let rtl = cell.effectiveUserInterfaceLayoutDirection == .rightToLeft
        return [max(0, frame.minX - reach.left - horizontalPadding.left),
                max(0, collection.bounds.width - frame.maxX - reach.right - horizontalPadding.right),
                rtl ? m.trailing : m.leading, rtl ? m.leading : m.trailing]
    }

    /// Before a batch: the carried rows it must read at home go back. A
    /// carried row's own box (a `frame` op) and the order of the nodes it
    /// was carried from (a `children` op, which `placing` follows) are kept
    /// in its cell, as are those nodes' boxes and their props other than
    /// `inert`: the shown rows of a list two rows are inserted into, or
    /// whose `refreshing` changes, stay in their cells, as a collection
    /// view's cells do under an insert. Any other op on a row takes it home
    /// alone; any other op on a node it was carried from (a style its rows
    /// inherit, an `inert`) takes every row home. Carried home and back,
    /// each shown row's subtree took UIKit's move and trait passes twice.
    func prepare(_ ops: [BatchOp]) {
        guard !carried.isEmpty else { return }
        var above = Set<UInt32>(), walked = Set<ObjectIdentifier>()
        for place in carried.values {
            var at: UIView? = place.parent
            while let view = at, walked.insert(ObjectIdentifier(view)).inserted {
                if let node = view as? NodeView { above.insert(node.id) }
                at = view.superview
            }
        }
        var alone: [NodeView] = []
        for op in ops {
            if [.roots, .flight, .land, .exit].contains(op.op) { return restore() }
            if carried[op.id] != nil {
                if op.op != .frame, let row = host.presenter.views[op.id] { alone.append(row) }
                continue
            }
            guard above.contains(op.id) else { continue }
            switch op.op {
            case .frame, .content, .children: continue
            case .props where op.props["inert"] == nil && !op.clear.contains("inert"): continue
            default: return restore()
            }
        }
        for row in alone where carried[row.id] != nil { sendHome(row) }
    }

    /// A `children` op places `want` in `container`: a row this list
    /// carried from there stays in its cell, and its place among the rest is
    /// noted for its return; one carried from elsewhere is the presenter's
    /// to move now, and one no longer wanted there leaves its cell as it
    /// would leave its parent. The rest, which the presenter places.
    func placing(_ container: UIView, _ want: [NodeView]) -> [NodeView] {
        guard !carried.isEmpty else { return want }
        for row in want where carried[row.id].map({ $0.parent !== container }) == true { forget(row.id) }
        let key = ObjectIdentifier(container)
        guard homes[key] != nil else { return want }
        let wanted = Set(want.map(\.id))
        for (id, place) in carried where place.parent === container && !wanted.contains(id) {
            forget(id)
            host.presenter.views[id]?.removeFromSuperview()
        }
        if homes[key] != nil { homes[key] = want.map(\.id) }
        return want.filter { carried[$0.id] == nil }
    }

    /// A carried row in its cell: at its layout x on the cell's grid, at the
    /// size it is drawn at home. Its children keep their places in it.
    static func cellFrame(_ place: GridPlace, scale: CGFloat) -> CGRect {
        CGRect(x: PixelGrid.round(place.layout.minX, scale: scale), y: 0, width: place.drawn.width, height: place.drawn.height)
    }

    /// The rows carried from `container`, which the presenter places on the
    /// pixel grid from it (`Presenter.regridChildren`).
    func carried(from container: UIView) -> [NodeView] {
        guard homes[ObjectIdentifier(container)] != nil else { return [] }
        return carried.compactMap { $0.value.parent === container ? host.presenter.views[$0.key] : nil }
    }

    /// A row is carried into a cell from `parent`: the order of the
    /// parent's children, all home until one is carried, is noted for its
    /// return.
    func noteHome(_ parent: UIView) {
        let key = ObjectIdentifier(parent)
        if homes[key] == nil { homes[key] = parent.subviews.compactMap { ($0 as? NodeView)?.id } }
    }

    /// The row `id` is carried no longer, wherever its view is.
    func forget(_ id: UInt32) {
        guard let place = carried.removeValue(forKey: id) else { return }
        if !carried.values.contains(where: { $0.parent === place.parent }) { homes[ObjectIdentifier(place.parent)] = nil }
    }

    /// A row whose cell leaves the screen goes home as it leaves, while
    /// both are in the window: one move a cell, spread over the scroll,
    /// where a batch taking every such row home at once moved each back
    /// into the window. Home, a row that does not show has no native cell.
    func collectionView(_ view: UICollectionView, didEndDisplaying cell: UICollectionViewCell, forItemAt path: IndexPath) {
        for case let row as NodeView in cell.contentView.subviews where carried[row.id] != nil { sendHome(row) }
    }

    /// A carried row back where the presenter put it, alone: its cell left
    /// the screen or now holds another row. It goes between the siblings
    /// its parent's last `children` op put either side of it, in the box
    /// layout last gave it there, which a batch may have changed while it
    /// was carried.
    func sendHome(_ row: NodeView) {
        guard host.presenter.views[row.id] === row, let place = carried[row.id] else {
            row.removeFromSuperview()
            return
        }
        let parent = place.parent, order = homes[ObjectIdentifier(parent)] ?? []
        forget(row.id)
        let views = host.presenter.views
        let home = { (id: UInt32) in views[id].flatMap { $0.superview === parent ? $0 : nil } }
        if let i = order.firstIndex(of: row.id), let next = order[(i + 1)...].lazy.compactMap(home).first {
            parent.insertSubview(row, belowSubview: next)
        } else if let i = order.firstIndex(of: row.id), let previous = order[..<i].reversed().lazy.compactMap(home).first {
            parent.insertSubview(row, aboveSubview: previous)
        } else {
            parent.insertSubview(row, at: 0)
        }
        row.frame = row.grid?.drawn ?? place.frame
    }
}
extension GroupedListView {
    /// Every row from what shows to the one holding `node` sized by its
    /// cell, so the layout places that row exactly. A row never shown has
    /// UIKit's estimated height until its cell sizes it: a scroll aimed at a
    /// row hours down a timetable stopped where the estimates put it, an
    /// hour short, and the rows it passed grew under it (r28's timetable).
    /// The list moves a page at a time, unanimated and unreported, to the
    /// row and back, and the first row that showed stays where it was.
    func measure(toward node: NodeView) {
        var at: UIView? = node, path: IndexPath?
        while let view = at, path == nil {
            if let n = view as? NodeView, let row = rows[n.id] { path = source.indexPath(for: row.view) }
            at = view.superview
        }
        guard let path else { return }
        collection.layoutIfNeeded()
        let start = collection.contentOffset
        let anchor = collection.indexPathsForVisibleItems.min()
        let anchorY = anchor.flatMap { collection.layoutAttributesForItem(at: $0)?.frame.minY }
        measuring = true
        defer { measuring = false }
        for _ in 0..<64 {
            guard let frame = collection.layoutAttributesForItem(at: path)?.frame else { break }
            let page = collection.bounds, i = collection.adjustedContentInset
            if frame.minY >= page.minY && frame.maxY <= page.maxY { break }
            let toward = frame.minY < page.minY ? max(frame.minY, page.minY - page.height) : min(frame.minY, page.minY + page.height)
            let y = min(max(toward, -i.top), max(-i.top, collection.contentSize.height + i.bottom - page.height))
            guard abs(y - page.minY) > 0.5 else { break }
            collection.contentOffset.y = y
            collection.layoutIfNeeded()
        }
        guard collection.contentOffset != start else { return }
        collection.contentOffset = start
        collection.layoutIfNeeded()
        // Rows above it measured on the way up move it down by what they grew.
        if let anchor, let anchorY, let y = collection.layoutAttributesForItem(at: anchor)?.frame.minY, abs(y - anchorY) > 0.5 {
            collection.contentOffset.y = start.y + y - anchorY
            collection.layoutIfNeeded()
        }
    }
}
#endif

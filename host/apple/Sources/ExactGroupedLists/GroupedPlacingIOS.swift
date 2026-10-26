// Which batches take a grouped list's carried rows home (`GroupedListView.restore`).
import ExactKit
#if os(iOS) || os(tvOS)
import UIKit

extension GroupedListView {
    /// Whether a batch touching `ids` touches a carried row or a node it
    /// was carried from (its section, the list, any authored ancestor):
    /// those go home for it, as the batch reads them there. A batch that
    /// changes only what is inside rows (a live price) leaves them in
    /// their cells: carried home and back, each shown row's subtree took
    /// UIKit's move and trait passes twice a batch, and its changed text
    /// was painted while hidden.
    func touches(_ ids: Set<UInt32>) -> Bool {
        guard !ids.isEmpty else { return false }
        return carried.contains { id, place in
            if ids.contains(id) { return true }
            var at: UIView? = place.parent
            while let view = at {
                if let node = view as? NodeView, ids.contains(node.id) { return true }
                at = view.superview
            }
            return false
        }
    }

    /// The carried rows whose cells no longer show go home, as every row
    /// did before a batch; the shown ones stay in their cells.
    func restoreUnshown() {
        let shown = Set(collection.visibleCells.map { ObjectIdentifier($0.contentView) })
        for id in carriedOrder.reversed() {
            guard let place = carried[id] else { continue }
            guard let row = host.presenter.views[id] else { carried[id] = nil; continue }
            if let cell = row.superview, shown.contains(ObjectIdentifier(cell)) { continue }
            if row.superview !== place.parent { place.parent.insertSubview(row, at: min(place.index, place.parent.subviews.count)) }
            row.frame = place.frame
            carried[id] = nil
        }
        carriedOrder.removeAll { carried[$0] == nil }
    }
}
#endif

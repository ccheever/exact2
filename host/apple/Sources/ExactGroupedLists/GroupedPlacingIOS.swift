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

    /// A row whose cell leaves the screen goes home as it leaves, while
    /// both are in the window: one move a cell, spread over the scroll,
    /// where a batch taking every such row home at once moved each back
    /// into the window. Home, a row that does not show has no native cell.
    func collectionView(_ view: UICollectionView, didEndDisplaying cell: UICollectionViewCell, forItemAt path: IndexPath) {
        for case let row as NodeView in cell.contentView.subviews where carried[row.id] != nil { sendHome(row) }
    }

    /// A carried row back where the presenter put it, alone: its cell left
    /// the screen or now holds another row.
    func sendHome(_ row: NodeView) {
        guard host.presenter.views[row.id] === row, let place = carried.removeValue(forKey: row.id) else {
            row.removeFromSuperview()
            return
        }
        place.parent.insertSubview(row, at: min(place.index, place.parent.subviews.count))
        row.frame = place.frame
        carriedOrder.removeAll { $0 == row.id }
    }
}
#endif

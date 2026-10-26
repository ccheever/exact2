// Which batches take a grouped list's carried rows home (`GroupedListView.restore`).
import ExactKit
#if os(iOS) || os(tvOS)
import UIKit

extension GroupedListView {
    /// Whether placing `ids` moves a carried row: its own box, or the
    /// children of the node it was carried from. A batch that only changes
    /// what rows show (a live price) leaves them in their cells: carried
    /// home and back, each shown row's subtree took UIKit's move and trait
    /// passes twice a batch, and its changed text was painted while hidden.
    func places(_ ids: Set<UInt32>) -> Bool {
        guard !ids.isEmpty else { return false }
        return carried.contains { id, place in
            if ids.contains(id) { return true }
            var at: UIView? = place.parent
            while let view = at, !(view is NodeView) { at = view.superview }
            return (at as? NodeView).map { ids.contains($0.id) } ?? true
        }
    }
}
#endif

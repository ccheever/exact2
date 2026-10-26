// @ref LLP 1144 D1 — what a held fling reads of its scroller each turn: the
// offset UIKit or AppKit committed, and whether a finger or momentum moves it.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

extension Agent {
    /// A reader of node `id`'s scroller, or nil when it has none.
    func scrollProbe(_ id: Int) -> (() -> (Double, String)?)? {
        guard let node = presenter.views[UInt32(id)] else { return nil }
        #if canImport(UIKit)
        guard let sv = node.scrollView else { return nil }
        return { [weak sv] in
            guard let sv else { return nil }
            let y = Double(sv.contentOffset.y), i = sv.adjustedContentInset
            let past = sv.contentOffset.y < -i.top - 0.5 || sv.contentOffset.y > max(-i.top, sv.contentSize.height + i.bottom - sv.bounds.height) + 0.5
            return (y, past ? "b" : sv.isTracking ? "t" : sv.isDecelerating ? "d" : "-")
        }
        #else
        // The node's own scroller; the page's encloses every node and does not move with a list.
        guard let sv: NSScrollView = node.scroll ?? node.enclosingScrollView else { return nil }
        return { [weak sv] in
            guard let sv else { return nil }
            let clip = sv.contentView.bounds, doc = sv.documentView?.frame.height ?? 0
            return (Double(clip.origin.y), clip.origin.y < -0.5 || clip.origin.y > max(0, doc - clip.height) + 0.5 ? "b" : "-")
        }
        #endif
    }
}

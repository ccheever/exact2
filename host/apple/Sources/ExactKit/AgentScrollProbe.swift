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
        return { [weak sv] in sv?.scrollSample }
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

#if canImport(UIKit)
extension UIScrollView {
    /// The offset UIKit committed, along the axis it scrolls on (x for one
    /// that scrolls only sideways, a row of cards), and what moves it: `b`
    /// past its range, `t` a finger dragging it (`isDragging`: a finger only
    /// touching it is not a drag, as Apple's scroll-dragging metric counts
    /// it), `d` decelerating, `-` still. A held fling and a recording read it.
    var scrollSample: (Double, String) {
        let i = adjustedContentInset
        let sideways = contentSize.width + i.left + i.right > bounds.width + 0.5 && contentSize.height + i.top + i.bottom <= bounds.height + 0.5
        let at = sideways ? contentOffset.x : contentOffset.y, low = sideways ? -i.left : -i.top
        let high = max(low, sideways ? contentSize.width + i.right - bounds.width : contentSize.height + i.bottom - bounds.height)
        return (Double(at), at < low - 0.5 || at > high + 0.5 ? "b" : isTracking && isDragging ? "t" : isDecelerating ? "d" : "-")
    }
}
#endif

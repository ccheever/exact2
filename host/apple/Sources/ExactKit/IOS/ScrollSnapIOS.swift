// CSS scroll snap on UIKit (LLP 1008, "Horizontal scroll snap"): UIKit
// projects where a drag comes to rest; the nearest captured snap position
// replaces it.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    /// CSS's admitted `x mandatory` / `start` and `end` scroll snap. UIKit supplies
    /// the projected resting offset and owns the resulting deceleration.
    package func scrollViewWillEndDragging(_ scrollView: UIScrollView, withVelocity velocity: CGPoint, targetContentOffset: UnsafeMutablePointer<CGPoint>) {
        guard (style["scroll_snap_type"]?.string) == "x mandatory" else { return }
        let maximum = max(0, scrollView.contentSize.width - scrollView.bounds.width)
        var positions: [CGFloat] = []
        func visit(_ view: UIView) {
            for case let node as NodeView in view.subviews where !node.isHidden {
                let align = node.style["scroll_snap_align"]?.string
                if align == "start" || align == "end" {
                    let rect = node.convert(node.bounds, to: scrollView)
                    let width = scrollView.bounds.width
                    // A snap area wider than the viewport can be explored
                    // freely while it covers the viewport (CSS Snap §5.2.2);
                    // a narrower one rests with its start, or its end, at
                    // the viewport's.
                    let wide = rect.width >= width
                    let at = align == "end" && !wide ? rect.maxX - width : rect.minX
                    let start = min(maximum, max(0, at))
                    let end = wide ? min(maximum, max(start, rect.maxX - width)) : start
                    positions.append(min(end, max(start, targetContentOffset.pointee.x)))
                }
                // A nested scroll container captures its own snap areas.
                if node.scroll == nil && !node.scrollDormant && (node.style["scroll_snap_type"]?.string ?? "none") == "none" { visit(node.container) }
            }
        }
        visit(scrollView)
        if let nearest = positions.min(by: { abs($0 - targetContentOffset.pointee.x) < abs($1 - targetContentOffset.pointee.x) }) {
            targetContentOffset.pointee.x = nearest
        }
    }
}
#endif

// CSS scroll snap on UIKit (LLP 1008, "Horizontal scroll snap"): UIKit
// projects where a drag comes to rest; the nearest captured snap position
// replaces it.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    /// CSS's admitted `x mandatory` / `start` and `end` scroll snap. UIKit supplies
    /// the projected resting offset and owns the resulting deceleration.
    package func scrollViewWillEndDragging(_ scrollView: UIScrollView, withVelocity velocity: CGPoint, targetContentOffset: UnsafeMutablePointer<CGPoint>) {
        guard scrollView === self.scrollView else { return }
        #if os(iOS)
        // A painted swipe row's full swipe decides where it rests (PaintedSwipesIOS.swift).
        defer { if swipeOwner { presenter?.swipeActions.painted.willEnd(self, scrollView, target: targetContentOffset) } }
        #endif
        targetContentOffset.pointee = snappedOffset(targetContentOffset.pointee, in: scrollView)
    }

    /// The native resting position, optionally constrained to the snap area
    /// holding a target the agent reveals.
    func snappedOffset(_ proposed: CGPoint, in scrollView: UIScrollView, revealing target: UIView? = nil) -> CGPoint {
        guard (style["scroll_snap_type"]?.string) == "x mandatory" else { return proposed }
        let maximum = max(0, scrollView.contentSize.width - scrollView.bounds.width)
        var positions: [CGFloat] = [], targetPositions: [CGFloat] = []
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
                    let position = min(end, max(start, proposed.x))
                    positions.append(position)
                    if let target, target === node || target.isDescendant(of: node) { targetPositions.append(position) }
                }
                // A nested scroll container captures its own snap areas.
                if node.scrollView == nil && !node.scrollDormant && (node.style["scroll_snap_type"]?.string ?? "none") == "none" { visit(node.container) }
            }
        }
        visit(scrollView)
        let choices = targetPositions.isEmpty ? positions : targetPositions
        var result = proposed
        if let nearest = choices.min(by: { abs($0 - proposed.x) < abs($1 - proposed.x) }) { result.x = nearest }
        return result
    }
}
#endif

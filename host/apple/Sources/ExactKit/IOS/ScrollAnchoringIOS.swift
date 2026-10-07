// CSS scroll anchoring on a plain UIKit scroller (`ScrollAnchoring.swift`,
// #138 X23d). A `scrollFollowEnd` scroller keeps its own reading anchor
// (`NodeViewIOS.captureScrollPosition`); every other one takes the spec's.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    func captureScrollAnchor(_ sv: UIScrollView) {
        // No anchor at the start (spec §2.2): content inserted on top shows.
        guard hasScrollLayoutBox, sv.contentOffset.y > -sv.adjustedContentInset.top else { return }
        // The scrollport: what a bar or the safe area covers is not in view.
        let i = sv.adjustedContentInset
        scrollAnchor = ScrollAnchoring.anchor(in: self, space: sv, port: sv.bounds.inset(by: UIEdgeInsets(top: i.top, left: 0, bottom: i.bottom, right: 0)))
    }

    func restoreScrollAnchor(_ sv: UIScrollView) {
        guard let anchor = scrollAnchor,
              let shift = ScrollAnchoring.shift(of: anchor, in: self, space: sv, changes: presenter?.anchorChanges ?? .init(),
                                                live: { [weak self] in self?.presenter?.views[$0.id] === $0 }),
              shift != 0 else { return }
        // While a finger or a fling moves it, move the offset by the shift
        // only, as UIKit's own content offset adjustment does; at rest, clamp.
        if sv.isTracking || sv.isDecelerating {
            sv.contentOffset.y += shift
            return
        }
        let i = sv.adjustedContentInset
        // From the current offset, as Chromium adjusts it: UIKit does not clamp
        // on a content size change, and a collapsing title may have moved it.
        let y = ScrollAnchoring.adjusted(top: sv.contentOffset.y, before: 0, after: shift, minimum: -i.top,
                                         maximum: sv.contentSize.height + i.bottom - sv.bounds.height)
        if sv.contentOffset.y != y { sv.setContentOffset(CGPoint(x: sv.contentOffset.x, y: y), animated: false) }
    }
}
#endif

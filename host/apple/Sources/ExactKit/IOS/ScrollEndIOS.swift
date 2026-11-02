// `scrollend` on UIKit (`ScrollEnd.swift`): the delegate reports the end of
// a drag that throws nothing, of its deceleration, and of an animation; a
// move none of them holds ends once still. A pooled view's owed end is
// dropped when it goes to another node (`NodePool`).
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    /// The scroll view reported its offset (`scrollViewDidScroll`).
    func scrollEndMoved(_ sv: UIScrollView) {
        // A collapse (`display: none`) clamps the offset and sends no
        // `scroll`: nothing to end, and not a position to compare with.
        guard (handlers.contains("scrollend") || presenter?.session?.timerAdmissions.enabled == true), hasScrollLayoutBox else { return }
        let end = scrollEnd ?? ScrollEnd()
        scrollEnd = end
        guard end.moved(to: [sv.contentOffset.x, sv.contentOffset.y, scrollTopInset(sv)]) else { return }
        // A drag and its deceleration report their end; a touch that only
        // tracks reports none, so the quiet check asks until it lifts.
        if sv.isDragging || sv.isDecelerating { end.hold() } else { settleScrollEnd() }
    }

    /// Once still: ended, unless a touch, a smooth correction or an element
    /// reveal holds it now (asked again then; a touch that never drags
    /// reports no end of its own, a correction's first frame may not have
    /// come yet, and a reveal ends with its animation).
    private func settleScrollEnd() {
        scrollEnd?.settle { [weak self, token = incarnation] in
            guard let self, self.incarnation == token, let sv = self.scrollView else { return }
            let correcting = self.presenter?.collections.offsetDrivers[self.id] != nil || self.presenter?.collections.startOwed.contains(self.id) == true
                || self.presenter?.elementScrolls.revealing(sv) != nil
            if sv.isTracking || sv.isDragging || sv.isDecelerating || correcting { self.settleScrollEnd() } else { self.endScroll() }
        }
    }

    /// A deceleration or an animation stopped: over, unless a finger caught
    /// it (UIKit stops a fling a finger lands on), which ends when it lifts
    /// or once still.
    func scrollMayHaveEnded(_ sv: UIScrollView) {
        if sv.isTracking || sv.isDragging { settleScrollEnd() } else { endScroll() }
    }

    /// The scroll is over: its last `scroll` first, then `scrollend`, once.
    func endScroll() {
        guard (handlers.contains("scrollend") || presenter?.session?.timerAdmissions.enabled == true), presenter?.views[id] === self, let end = scrollEnd, end.take(), hasScrollLayoutBox else { return }
        // The last `scroll` may run its handler at once, and a batch it
        // commits may give this view to another node.
        let node = id, token = incarnation
        sendScrollEvent()
        guard id == node, incarnation == token, presenter?.views[node] === self else { return }
        if handlers.contains("scrollend") { presenter?.scrollend(node) }
        presenter?.session?.timerAdmissions.check()
    }
}
#endif

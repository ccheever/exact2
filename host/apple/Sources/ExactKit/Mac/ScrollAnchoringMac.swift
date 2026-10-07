// A scroll container's place across a batch on AppKit: a `scrollFollowEnd`
// container at its end stays there (LLP 1001); anywhere else above the start,
// CSS scroll anchoring keeps the visible content still (`ScrollAnchoring`).
// The presenter calls `captureScrollPosition` before a batch and
// `restoreScrollPosition` after its layout; an explicit `scrollTop` then wins.
#if os(macOS)
import AppKit

extension NodeView {
    func captureScrollPosition() {
        beforeLayoutScroll = scroll?.contentView.bounds.origin
        followedScroll = nil
        scrollAnchor = nil
        // A hidden (`display: none`) follower is left to `hiddenScroll`, which
        // puts its offset back when the box returns.
        guard let sv = scroll, let doc = sv.documentView, hasScrollLayoutBox else { return }
        let top = sv.contentView.bounds.minY
        if props["scrollFollowEnd"] == "true" {
            let maximum = max(0, doc.bounds.height - sv.contentView.bounds.height)
            followedScroll = (top, top >= maximum - 1)
            if top >= maximum - 1 { return }
        }
        // No anchor at the start (spec §2.2): content inserted on top shows.
        guard top > 0 else { return }
        scrollAnchor = ScrollAnchoring.anchor(in: self, space: doc, port: doc.convert(sv.contentView.bounds, from: sv.contentView))
    }

    func restoreScrollPosition() {
        defer { followedScroll = nil; scrollAnchor = nil }
        guard let sv = scroll, let doc = sv.documentView else { return }
        let maximum = max(0, doc.bounds.height - sv.contentView.bounds.height)
        var y: CGFloat
        if props["scrollFollowEnd"] == "true", followedScroll?.end ?? true {
            y = maximum
        } else {
            let top = followedScroll?.top ?? beforeLayoutScroll?.y ?? sv.contentView.bounds.minY
            let shift = scrollAnchor.flatMap { anchor in
                ScrollAnchoring.shift(of: anchor, in: self, space: doc, changes: presenter?.anchorChanges ?? .init(),
                                      live: { [weak self] in self?.presenter?.views[$0.id] === $0 })
            }
            // A plain scroller is written only when its anchor moved: never in
            // the middle of a rubber band or a smooth scroll it has no reason
            // to touch. Without an anchor it keeps its numeric offset, as
            // AppKit left it; a followed one is put back where it was.
            guard (shift ?? 0) != 0 || followedScroll != nil else { return }
            y = ScrollAnchoring.adjusted(top: top, before: 0, after: shift ?? 0, minimum: 0, maximum: maximum)
        }
        if sv.contentView.bounds.minY != y {
            sv.contentView.scroll(to: NSPoint(x: sv.contentView.bounds.minX, y: y))
            sv.reflectScrolledClipView(sv.contentView)
        }
    }
}
#endif

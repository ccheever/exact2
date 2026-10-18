// A page with no native container drawn past its area (LLP 1130): its
// document scrolls under the status bar and the home indicator, and a
// scroller at the area's top or foot reaches into that band, as a
// hand-built screen's scroll view fills the screen. Layout is the area's,
// unchanged: only where the page is drawn moves.
#if os(iOS) || os(tvOS)
import UIKit

extension UIScrollView {
    /// The content point at the top-left of what the page lays out under:
    /// the scroll, and for a page drawn past its area its margins. A content
    /// point less it is a point of the kernel's viewport (a box, `clientX`);
    /// a viewport point plus it is a content point.
    package var pageOrigin: CGPoint {
        let m = (self as? Viewport)?.pageMargins ?? .zero
        return CGPoint(x: contentOffset.x + m.left, y: contentOffset.y + m.top)
    }
    /// What the page lays out under, in content coordinates: the bounds less
    /// those margins, where something is shown clear of the status bar and
    /// the home indicator.
    package var pageBounds: CGRect { bounds.inset(by: (self as? Viewport)?.pageMargins ?? .zero) }
    /// Its size.
    package var pageSize: CGSize { pageBounds.size }
}

extension Presenter {
    /// The bands past the page's area the viewport takes for a page with no
    /// container (D1): the status bar's and the home indicator's, and the
    /// sides' in landscape. The document sits inside them and its content is
    /// that much larger, so its offsets are the area's. Zero where the
    /// viewport is the area (a container, `viewport-fit=cover`, tvOS, the
    /// agent's explicit size).
    package var pageMargins: UIEdgeInsets {
        get { (viewport as? Viewport)?.pageMargins ?? .zero }
        set { (viewport as? Viewport)?.pageMargins = newValue }
    }
    /// The page's area in the viewport's superview, against which the
    /// keyboard's overlap is measured.
    package var pageFrame: CGRect { viewport.frame.inset(by: pageMargins) }

    /// Each scroller at the area's top edge reaches up through the status
    /// bar's band, and one at its foot down through the home indicator's
    /// (D3); every other scroller has none. After a batch's frames and a
    /// change of the margins.
    func reachPageEdges() {
        let m = pageMargins, area = viewportSize
        for id in scrollers {
            guard let v = views[id] else { continue }
            let band = m.top > 0 || m.bottom > 0 ? pageBand(of: v, margins: m, area: area) : .zero
            if v.pageBand != band { v.setPageBand(band) }
        }
    }

    /// The band `v`'s scroll view takes: an Exact scroller that scrolls down
    /// the page, laid out in the document with nothing between them that
    /// clips, transforms, scrolls or hides, whose box starts at the area's
    /// top (the top band) or ends at its foot in a document no taller than
    /// the area (the bottom band). A list UIKit draws is not one yet.
    private func pageBand(of v: NodeView, margins m: UIEdgeInsets, area: CGSize) -> UIEdgeInsets {
        guard let sv = v.scroll, v.scrollView === sv, sv.scrollsY, v.window != nil else { return .zero }
        var at: UIView? = v
        while let c = at, c !== root {
            if c.isHidden || !c.transform.isIdentity || c.clipsToBounds || (c !== v && c is UIScrollView) { return .zero }
            at = c.superview
        }
        guard at === root, !root.clipsToBounds else { return .zero }
        let r = v.convert(v.bounds, to: root)
        let top = r.minY <= 0.5 ? m.top : 0
        let bottom = r.maxY >= area.height - 0.5 && root.bounds.height <= area.height + 0.5 ? m.bottom : 0
        return UIEdgeInsets(top: top, left: 0, bottom: bottom, right: 0)
    }
}

extension NodeView {
    /// Reach into `band` past the box: the scroll view's frame grows by it
    /// and its inset by as much, so the content stays where it was, and the
    /// logical `scrollTop` (`scrollTopInset`) is kept across the change.
    func setPageBand(_ band: UIEdgeInsets) {
        guard let sv = scroll else { return }
        let old = pageBand, top = sv.contentOffset.y + scrollTopInset(sv)
        more.pageBand = band
        sv.frame = scrollFrame
        var inset = sv.contentInset, bars = sv.verticalScrollIndicatorInsets
        inset.top += band.top - old.top; inset.bottom += band.bottom - old.bottom
        bars.top += band.top - old.top; bars.bottom += band.bottom - old.bottom
        sv.contentInset = inset
        sv.verticalScrollIndicatorInsets = bars
        fitScroll()
        sv.contentOffset.y = top - scrollTopInset(sv)
        // Offsets a layout in flight saved to restore are native: they move with the inset.
        let moved = band.top - old.top
        if var saved = beforeLayoutScroll { saved.offset.y -= moved; saved.topInset += moved; beforeLayoutScroll = saved }
        if var followed = followedScroll { followed.top -= moved; followedScroll = followed }
        presenter?.collections.offsetsMoved(id, by: -moved)
    }
}
#endif

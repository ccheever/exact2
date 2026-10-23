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

    /// Each scroller's bands (LLP 1130 D3, LLP 1131 D2): at the page's area's
    /// edges, through the status bar's and the home indicator's; through the
    /// control bars between it and its page's or route's edge; none for every
    /// other. After a batch's frames and a change of the margins.
    func reachPageEdges() {
        let m = pageMargins, area = viewportSize
        // Control bars are iOS 26's, where UIKit draws the edge effect over them (LLP 1131 D4).
        var judge: ControlBars? = ControlBars(self)
        #if os(tvOS)
        judge = nil
        #else
        if #unavailable(iOS 26.0) { judge = nil }
        #endif
        var bars: [(bar: NodeView, scroller: NodeView, edge: UIRectEdge)] = []
        var routes: [ObjectIdentifier: (top: UIScrollView?, bottom: UIScrollView?)] = [:]
        for id in scrollers {
            guard let v = views[id] else { continue }
            var found = Bands(), own = UIEdgeInsets.zero
            if let sv = v.scrollView, (sv as? ScrollView)?.scrollsY ?? true, v.window != nil {
                let edgeEffect = v.style["scroll_edge_effect"]?.string != "none"
                if let route = ViewPresentation.controllers(for: v).lazy.compactMap({ $0 as? RouteController }).first(where: { $0.host === navigation && v.isDescendant(of: $0.node) }) {
                    if let judge, edgeEffect { found = routeBands(of: v, sv, in: route, judge) }
                    #if os(iOS)
                    if route.edgeScroll === v, let top = keyboardTop,
                       abs(route.node.convert(route.node.bounds, to: nil).maxY - top) < 1 {
                        found.band.bottom += keyboardAccessoryHeight
                    }
                    #endif
                    own = found.band
                    // UIKit clips the inherited safe area at the node's layout box,
                    // even though its scroll view draws the band outside that box.
                    if !route.ridingBars.isEmpty { own.top += min(found.band.top, route.view.safeAreaInsets.top) }
                    var r = routes[ObjectIdentifier(route)] ?? (nil, nil)
                    if !found.top.isEmpty, navigation.contentScroller(of: route) === v { r.top = sv }
                    if !found.bottom.isEmpty { r.bottom = sv }
                    routes[ObjectIdentifier(route)] = r
                } else { found = pageBands(of: v, sv, margins: m, area: area, edgeEffect ? judge : nil); own = found.band }
            }
            if v.pageBand != found.band || v.bandInset != own { v.setPageBand(found.band, inset: own) }
            bars += found.top.map { ($0, v, .top) } + found.bottom.map { ($0, v, .bottom) }
        }
        #if os(iOS)
        // The bar's and the bottom bar's own edge effects follow a banded scroller (D2).
        for c in navigation.controllers.values {
            let r = routes[ObjectIdentifier(c)]
            if c.bandedTop !== r?.top { c.bandedTop = r?.top; c.setContentScrollView(r?.top ?? c.collapseScrollView, for: .top) }
            if c.bandedBottom !== r?.bottom { c.bandedBottom = r?.bottom; c.setContentScrollView(r?.bottom ?? c.edgeScroll?.scrollView, for: .bottom) }
        }
        #endif
        ControlBars.sync(bars, in: self)
    }

    /// A scroller's bands, and the control bars it is drawn through on each edge.
    struct Bands { var band = UIEdgeInsets.zero, top: [NodeView] = [], bottom: [NodeView] = [] }

    /// A page's scroller, laid out in the document with nothing between them
    /// that clips, transforms, scrolls or hides. LLP 1130 D3: an edge at the
    /// area's edge reaches through the margin past it (the bottom one only in
    /// a document no taller than the area). LLP 1131 D2: where only control
    /// bars of its root's are between it and the area's edge, that edge
    /// reaches the area's edge and the margin, provided the other edge is
    /// banded or open too.
    private func pageBands(of v: NodeView, _ sv: UIScrollView, margins m: UIEdgeInsets, area: CGSize, _ judge: ControlBars?) -> Bands {
        var at: UIView? = v, page: NodeView?
        while let c = at, c !== root {
            if c.isHidden || !c.transform.isIdentity || c.clipsToBounds || (c !== v && c is UIScrollView) { return Bands() }
            if let n = c as? NodeView, c.superview === root { page = n }
            at = c.superview
        }
        guard at === root, !root.clipsToBounds, let page else { return Bands() }
        let r = v.convert(v.bounds, to: root), fits = root.bounds.height <= area.height + 0.5
        let atTop = r.minY <= 0.5, atFoot = r.maxY >= area.height - 0.5 && fits
        let margins = Bands(band: UIEdgeInsets(top: atTop ? m.top : 0, left: 0, bottom: atFoot ? m.bottom : 0, right: 0))
        guard let judge else { return margins }
        guard let top = atTop ? [] : judge.bars(of: v, in: page, top: true), let bottom = atFoot ? [] : judge.bars(of: v, in: page, top: false),
              !top.isEmpty || !bottom.isEmpty, page.frame.minY <= 0.5, page.frame.maxY >= area.height - 0.5, fits,
              Self.over(top, v), Self.over(bottom, v) else { return margins }
        // Both edges reach the page's: an open one through the root's padding too (D2).
        let band = UIEdgeInsets(top: max(0, r.minY) + m.top, left: 0, bottom: max(0, area.height - r.maxY) + m.bottom, right: 0)
        guard Self.drawn(band, v, sv) else { return margins }
        return Bands(band: band, top: top, bottom: bottom)
    }

    /// A route's scroller (LLP 1131 D2): when control bars are between it and
    /// the route's edge on one side, and that side's other edge is banded or
    /// open, both reach the route's edges, the covers included; an edge the
    /// bar's collapse or the bottom edge already took is there already. None
    /// under a large title unless the eligible top bars ride its safe area.
    private func routeBands(of v: NodeView, _ sv: UIScrollView, in c: RouteController, _ judge: ControlBars) -> Bands {
        #if os(iOS)
        if let nav = c.navigationController, navigation.routeShowsBar(c, in: nav), c.navigationItem.largeTitleDisplayMode == .always, c.ridingBars.isEmpty { return Bands() }
        #endif
        if !c.ridingBars.isEmpty, case .edges(let edges)? = navigation.covers[c.node.id], edges.top > 0.5 { return Bands() }
        return controlBands(of: v, in: c.node, judge)
    }

    /// Collapse adoption and banding must agree about both edges and paint order.
    func controlBands(of v: NodeView, in route: NodeView, _ judge: ControlBars) -> Bands {
        guard let sv = v.scrollView else { return Bands() }
        let r = v.convert(v.bounds, to: route)
        guard let top = judge.bars(of: v, in: route, top: true), let bottom = judge.bars(of: v, in: route, top: false),
              !top.isEmpty || !bottom.isEmpty, Self.over(top, v), Self.over(bottom, v) else { return Bands() }
        let band = UIEdgeInsets(top: max(0, r.minY), left: 0, bottom: max(0, route.bounds.height - r.maxY), right: 0)
        return Self.drawn(band, v, sv) ? Bands(band: band, top: top, bottom: bottom) : Bands()
    }

    /// A bar the author ranks under the scroller (a `z-index`) cannot be over it (D3).
    private static func over(_ bars: [NodeView], _ v: NodeView) -> Bool {
        bars.allSatisfy { $0.paintRank >= v.paintRank || !$0.wrote("z_index") && !v.wrote("z_index") }
    }

    /// A virtualized list draws rows a port past its own (D5): a longer band is not taken.
    private static func drawn(_ band: UIEdgeInsets, _ v: NodeView, _ sv: UIScrollView) -> Bool {
        guard v.presenter?.collections.entries[v.id] != nil else { return true }
        let i = sv.adjustedContentInset, own = v.pageBand
        let port = v.bounds.height - (i.top - own.top) - (i.bottom - own.bottom)
        return band.top <= port + 0.5 && band.bottom <= port + 0.5
    }
}

extension NodeView {
    /// Reach into `band` past the box: the scroll view's frame grows by it
    /// and its `contentInset` by `inset` (all of it unless said), so the content stays where it was, and
    /// the logical `scrollTop` (`scrollTopInset`) is kept across the change;
    /// a list following its end, or a virtualized list at its end, stays there.
    func setPageBand(_ band: UIEdgeInsets, inset own: UIEdgeInsets? = nil) {
        guard let sv = scrollView else { return }
        let own = own ?? band
        let end = { max(-sv.adjustedContentInset.top, sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height) }
        // A virtualized list at its end stays there too. Its runner follows the end
        // (`paragraph.rs` keeps `scrollFollowEnd` from this view), but cannot see a
        // band handed to UIKit's inset: a list that appears in a route mid-push grows
        // under the bar as the push lands, its port keeps its size, and the kept
        // scrollTop would only move the offset, which reads as the reader leaving
        // the end.
        let follows = props["scrollFollowEnd"] == "true" || presenter?.collections.entries[id] != nil
        let atEnd = follows && sv.contentOffset.y >= end() - 1
        let oldOwn = bandInset, top = sv.contentOffset.y + scrollTopInset(sv)
        more.pageBand = band; more.bandInset = own
        sv.frame = scrollFrame
        var inset = sv.contentInset, bars = sv.verticalScrollIndicatorInsets
        inset.top += own.top - oldOwn.top; inset.bottom += own.bottom - oldOwn.bottom
        bars.top += own.top - oldOwn.top; bars.bottom += own.bottom - oldOwn.bottom
        sv.contentInset = inset
        sv.verticalScrollIndicatorInsets = bars
        fitScroll()
        if atEnd { sv.layoutIfNeeded() }
        sv.contentOffset = PixelGrid.offset(CGPoint(x: sv.contentOffset.x, y: atEnd ? end() : top - scrollTopInset(sv)), in: sv, clamped: !sv.isTracking && !sv.isDragging && !sv.isDecelerating)
        // Offsets a layout in flight saved to restore are native: they move with the inset.
        let moved = own.top - oldOwn.top
        if var saved = beforeLayoutScroll { saved.offset.y -= moved; saved.topInset += moved; beforeLayoutScroll = saved }
        if var followed = followedScroll { followed.top -= moved; followedScroll = followed }
        presenter?.collections.offsetsMoved(id, by: -moved)
    }
}
#endif

// LLP 1115 wave 1: a tap on the status bar scrolls the screen's content to
// its top, as in a hand-built app. UIKit does it only when exactly one scroll
// view in the window has `scrollsToTop`, so every other one is turned off:
// the one left on is the active route's content scroller (the one its
// `navigationScroll` names, else the first vertical scroller under it: the
// bar's scroller too, by one rule, LLP 1115.000 D11.1), else, with no routes,
// the viewport when it scrolls, else the first vertical scroller on the
// page. A grouped list answers with the collection view that shows, never
// the hidden scroll beneath it. Run after every batch and every finished
// navigation.
#if os(iOS) || os(tvOS)
import UIKit

// `scrollsToTop` is iOS's alone; the search below is the bar's rule on tvOS too.
#if os(iOS)
extension Presenter {
    func syncScrollsToTop() {
        #if EXACT_INSPECT
        let chosen = Inspect.on ? nil : scrollsToTopTarget() // (inspect mode: a status-bar tap scrolls nothing)
        #else
        let chosen = scrollsToTopTarget()
        #endif
        if viewport.scrollsToTop != (chosen === viewport) { viewport.scrollsToTop = chosen === viewport }
        for route in navigation.primaryRoutes {
            route.contentViewport?.scrollsToTop = route.contentViewport === chosen
        }
        for id in scrollers {
            guard let sv = views[id]?.scrollView else { continue }
            if sv.scrollsToTop != (sv === chosen) { sv.scrollsToTop = sv === chosen }
        }
    }

    /// The scroll view a status-bar tap scrolls, or nil for none.
    func scrollsToTopTarget() -> UIScrollView? {
        if let route = navigation.activeRoute { return navigation.contentScrollView(of: route) }
        if let modal = modals.routes.last?.node { return navigation.shownScroll(Self.firstVerticalScroller(under: modal)) }
        if viewport.contentSize.height > viewport.bounds.height - viewport.adjustedContentInset.top - viewport.adjustedContentInset.bottom {
            return viewport
        }
        let roots = root.subviews.compactMap { $0 as? NodeView }
        return roots.lazy.compactMap { Self.firstVerticalScroller(under: $0) }.first.flatMap { self.navigation.shownScroll($0) } ?? viewport
    }
}
#endif

extension Presenter {
    /// The shallowest shown scroller under `node` (itself included) that
    /// scrolls vertically, in tree order at each depth; a screen's content
    /// scroller is near its top, so the search stops eight levels down.
    static func firstVerticalScroller(under node: NodeView) -> NodeView? {
        var level: [NodeView] = [node], depth = 0
        while !level.isEmpty, depth < 8 {
            depth += 1
            if let hit = level.first(where: { $0.scrollView != nil && $0.scrollsVertically }) { return hit }
            level = level.flatMap { node -> [NodeView] in
                #if os(iOS)
                if #available(iOS 27.1, *), let panes = node.presenter?.arrangements.panes(in: node) { return panes.filter { !$0.isHidden } }
                #endif
                return node.container.subviews.compactMap { $0 as? NodeView }.filter { !$0.isHidden }
            }
        }
        return nil
    }
}
#endif

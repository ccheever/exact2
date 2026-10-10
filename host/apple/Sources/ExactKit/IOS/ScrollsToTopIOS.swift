// LLP 1115 wave 1: a tap on the status bar scrolls the screen's content to
// its top, as in a hand-built app. UIKit does it only when exactly one scroll
// view in the window has `scrollsToTop`, so every other one is turned off:
// the one left on is the active route's content scroller (its
// `navigationScroll`, else the first vertical scroller under it), else, with
// no routes, the viewport when it scrolls, else the first vertical scroller
// on the page. Run after every batch and every finished navigation.
#if os(iOS)
import UIKit

extension Presenter {
    func syncScrollsToTop() {
        let chosen = scrollsToTopTarget()
        if viewport.scrollsToTop != (chosen === viewport) { viewport.scrollsToTop = chosen === viewport }
        for id in scrollers {
            guard let sv = views[id]?.scroll else { continue }
            if sv.scrollsToTop != (sv === chosen) { sv.scrollsToTop = sv === chosen }
        }
    }

    /// The scroll view a status-bar tap scrolls, or nil for none.
    func scrollsToTopTarget() -> UIScrollView? {
        if let route = navigation.activeRoute {
            return navigation.contentScroll(of: route) ?? Self.firstVerticalScroller(under: route.node)
        }
        if let modal = modals.routes.last?.node { return Self.firstVerticalScroller(under: modal) }
        if viewport.contentSize.height > viewport.bounds.height - viewport.adjustedContentInset.top - viewport.adjustedContentInset.bottom {
            return viewport
        }
        let roots = root.subviews.compactMap { $0 as? NodeView }
        return roots.lazy.compactMap { Self.firstVerticalScroller(under: $0) }.first ?? viewport
    }

    /// The shallowest shown scroller under `node` (itself included) that
    /// scrolls vertically, in tree order at each depth; a screen's content
    /// scroller is near its top, so the search stops eight levels down.
    static func firstVerticalScroller(under node: NodeView) -> UIScrollView? {
        var level: [NodeView] = [node], depth = 0
        while !level.isEmpty, depth < 8 {
            depth += 1
            if let hit = level.first(where: { $0.scroll?.scrollsY == true }) { return hit.scroll }
            level = level.flatMap { $0.container.subviews.compactMap { $0 as? NodeView }.filter { !$0.isHidden } }
        }
        return nil
    }
}
#endif

// LLP 1123 D3–D4: a route can turn off an edge's native clearance without
// taking ownership of its bars, authored padding or keyboard accommodation.
#if os(iOS) || os(tvOS)
import UIKit

final class RouteSafeArea {
    private var top = Edge(), bottom = Edge()
    private var syncing = false

    func sync(_ route: RouteController) {
        guard !syncing else { return }
        syncing = true
        defer { syncing = false }
        top.sync(route.collapseScroll, scroll: route.collapseScrollView, ignored: route.ignoresSafeArea("top"), side: \.top)
        bottom.sync(route.edgeScroll, scroll: route.edgeScroll?.scrollView, ignored: route.ignoresSafeArea("bottom"), side: \.bottom)
    }

    func releaseTop() {
        guard !syncing else { return }
        syncing = true
        defer { syncing = false }
        top.release(side: \.top)
    }
    func releaseBottom() {
        guard !syncing else { return }
        syncing = true
        defer { syncing = false }
        bottom.release(side: \.bottom)
    }

    func insets(for scroll: UIScrollView) -> UIEdgeInsets {
        .init(top: top.scroll === scroll ? top.applied : 0, left: 0,
              bottom: bottom.scroll === scroll ? bottom.applied : 0, right: 0)
    }

    private struct Edge {
        weak var scroll: UIScrollView?
        weak var node: NodeView?
        var applied: CGFloat = 0
        private var ignored = false

        mutating func release(side: WritableKeyPath<UIEdgeInsets, CGFloat>) {
            set(0, ignored: false, side: side)
            applied = 0
            scroll = nil; node = nil
        }

        mutating func sync(_ node: NodeView?, scroll next: UIScrollView?, ignored: Bool, side: WritableKeyPath<UIEdgeInsets, CGFloat>) {
            if scroll !== next { release(side: side); scroll = next }
            self.node = node
            guard let scroll else { return }
            // Read UIKit's actual contribution, not the route's safe area: a
            // scroller inside authored padding overlaps only part of it.
            let automatic = scroll.adjustedContentInset[keyPath: side] - scroll.contentInset[keyPath: side]
            set(ignored ? -automatic : 0, ignored: ignored, side: side)
        }

        private mutating func set(_ amount: CGFloat, ignored: Bool, side: WritableKeyPath<UIEdgeInsets, CGFloat>) {
            defer { self.ignored = ignored }
            guard let scroll, abs(amount - applied) > 0.01 else { return }
            // Keep UIKit's top anchor; elsewhere a bar's changed automatic inset is not content room on an opted-out edge.
            let atStart = abs(scroll.contentOffset.y + scroll.adjustedContentInset.top) < 0.5
            let previousTop = self.ignored && !atStart ? scroll.contentInset.top - applied : scroll.adjustedContentInset.top
            let offset = scroll.contentOffset.y + previousTop
            let delta = amount - applied
            applied = amount
            var inset = scroll.contentInset, indicator = scroll.verticalScrollIndicatorInsets
            inset[keyPath: side] += delta; indicator[keyPath: side] += delta
            scroll.contentInset = inset
            scroll.verticalScrollIndicatorInsets = indicator
            if side == \.top {
                node?.scrollOrigin = scroll.adjustedContentInset.top
                node?.scrollCollapsed = scroll.adjustedContentInset.top
                scroll.contentOffset.y = offset - scroll.adjustedContentInset.top
            }
        }
    }
}
#endif

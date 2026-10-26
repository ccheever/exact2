// LLP 1123 D3–D4: cancel only UIKit's contribution on an opted-out edge.
#if os(iOS) || os(tvOS)
import UIKit

final class RouteSafeArea {
    private var states: [Scroller] = []
    private weak var route: RouteController?
    private var syncing = false, pending = false, ended = false
    private var value: String?, edges: UIRectEdge = []
    private var invalidReported = false

    func ignores(_ edge: String, in route: RouteController) -> Bool {
        let mask = policy(route)
        switch edge {
        case "top": return mask.contains(.top)
        case "right": return mask.contains(.right)
        case "bottom": return mask.contains(.bottom)
        case "left": return mask.contains(.left)
        default: return false
        }
    }

    private func policy(_ route: RouteController) -> UIRectEdge {
        let raw = route.node.props["safeArea"] ?? "auto"
        guard value != raw else { return edges }
        value = raw
        if let parsed = Self.parse(raw) { edges = parsed }
        else {
            edges = []
            if !invalidReported {
                invalidReported = true
                route.host?.presenter.session?.log("route \(route.key): invalid -exact-safe-area value \(raw.debugDescription); using auto")
            }
        }
        return edges
    }

    private static func parse(_ value: String) -> UIRectEdge? {
        if value == "auto" { return [] }
        if value == "none" { return .all }
        let words = value.split(whereSeparator: { $0.isWhitespace })
        guard !words.isEmpty else { return nil }
        var out: UIRectEdge = []
        for word in words {
            let edge: UIRectEdge
            switch word {
            case "top": edge = .top
            case "right": edge = .right
            case "bottom": edge = .bottom
            case "left": edge = .left
            default: return nil
            }
            guard !out.contains(edge) else { return nil }
            out.insert(edge)
        }
        return out
    }

    func sync(_ route: RouteController) {
        guard !ended else { return }
        self.route = route
        pending = true
        drain()
    }

    func end() {
        ended = true; pending = true
        drain()
    }

    func insets(for scroll: UIScrollView) -> UIEdgeInsets {
        states.first { $0.scroll === scroll }?.applied ?? .zero
    }

    private func targets(_ route: RouteController) -> [(NodeView, UIScrollView)] {
        var out: [(NodeView, UIScrollView)] = []
        if let node = route.collapseScroll, let scroll = route.collapseScrollView { out.append((node, scroll)) }
        if let node = route.edgeScroll, let scroll = node.scrollView, !out.contains(where: { $0.1 === scroll }) { out.append((node, scroll)) }
        return out
    }

    private func drain() {
        guard !syncing else { return }
        syncing = true
        defer { syncing = false }
        while pending {
            pending = false
            let mask = ended ? [] : route.map(policy) ?? []
            if !ended, mask.isEmpty, states.isEmpty { continue }
            let wanted = ended ? [] : route.map(targets) ?? []
            let retired = states.filter { old in !wanted.contains { $0.1 === old.scroll } }
            states.removeAll { old in retired.contains { $0 === old } }
            for old in retired { old.release() }
            guard !ended, let route else { continue }
            for (node, scroll) in wanted where !mask.isEmpty && !states.contains(where: { $0.scroll === scroll }) {
                states.append(Scroller(node, scroll, owner: self))
            }
            for state in states {
                state.apply(mask)
                if ended { pending = true; break }
            }
            // A setter can replace a backend before its nested cover report runs.
            if !ended, !mask.isEmpty, targets(route).map({ ObjectIdentifier($0.1) }) != wanted.map({ ObjectIdentifier($0.1) }) { pending = true }
        }
    }

    private final class Scroller {
        weak var scroll: UIScrollView?
        private weak var node: NodeView?
        private weak var owner: RouteSafeArea?
        var applied = UIEdgeInsets.zero
        private var last: UIEdgeInsets, band: UIEdgeInsets
        // UIKit can replay this snapshot while ending refresh, even after the policy returns to auto.
        private var snapshot: (inset: UIEdgeInsets, vertical: UIEdgeInsets, horizontal: UIEdgeInsets, correction: UIEdgeInsets, band: UIEdgeInsets)?
        private var wasRefreshing = false, writing = false
        private var observation: NSKeyValueObservation?
        private static let sides: [(UIRectEdge, WritableKeyPath<UIEdgeInsets, CGFloat>)] = [(.top, \.top), (.right, \.right), (.bottom, \.bottom), (.left, \.left)]

        init(_ node: NodeView, _ scroll: UIScrollView, owner: RouteSafeArea) {
            self.node = node; self.scroll = scroll; self.owner = owner; last = scroll.contentInset; band = node.bandInset
            observation = scroll.observe(\.contentInset) { [weak self] _, _ in
                guard let self, !self.writing, let owner = self.owner, let route = owner.route else { return }
                owner.sync(route)
            }
        }

        func release() {
            observation?.invalidate(); observation = nil
            apply([])
        }

        func apply(_ mask: UIRectEdge) {
            guard let scroll else { return }
            writing = true
            defer { writing = false }
            let current = scroll.contentInset, adjusted = scroll.adjustedContentInset
            let previousVertical = scroll.verticalScrollIndicatorInsets, previousHorizontal = scroll.horizontalScrollIndicatorInsets
            var vertical = previousVertical, horizontal = previousHorizontal
            #if os(tvOS)
            let refreshing = false
            #else
            let refreshing = scroll.refreshControl?.isRefreshing == true
            #endif
            if refreshing && !wasRefreshing { snapshot = (last, vertical, horizontal, applied, band) }
            wasRefreshing = refreshing
            let nextBand = node?.bandInset ?? .zero
            let bandChanged = nextBand != band
            defer { band = nextBand }
            var next = current, wanted = UIEdgeInsets.zero
            let hadTop = applied.top != 0
            for (edge, side) in Self.sides {
                let amount = mask.contains(edge) ? -(adjusted[keyPath: side] - current[keyPath: side]) : 0
                wanted[keyPath: side] = amount
                var contentOwn = applied[keyPath: side], verticalOwn = contentOwn, horizontalOwn = contentOwn
                // Refresh restores the contentInset it captured before a bar's correction changed.
                if !refreshing, !bandChanged, let snapshot, current[keyPath: side] != last[keyPath: side],
                   current[keyPath: side] == snapshot.inset[keyPath: side], contentOwn != snapshot.correction[keyPath: side] {
                    contentOwn = snapshot.correction[keyPath: side]
                    let bandDelta = nextBand[keyPath: side] - snapshot.band[keyPath: side]
                    next[keyPath: side] += bandDelta
                    if vertical[keyPath: side] == snapshot.vertical[keyPath: side] {
                        verticalOwn = contentOwn
                        vertical[keyPath: side] += bandDelta
                    }
                    if horizontal[keyPath: side] == snapshot.horizontal[keyPath: side] { horizontalOwn = contentOwn }
                }
                next[keyPath: side] += amount - contentOwn
                vertical[keyPath: side] += amount - verticalOwn
                horizontal[keyPath: side] += amount - horizontalOwn
            }
            let anchor = CGPoint(x: scroll.contentOffset.x + adjusted.left, y: scroll.contentOffset.y + adjusted.top)
            let changed = current != next
            applied = wanted; last = next
            if changed {
                scroll.contentInset = next
                // A native inset writer can run inside contentInset's setter.
                for (_, side) in Self.sides {
                    vertical[keyPath: side] += scroll.verticalScrollIndicatorInsets[keyPath: side] - previousVertical[keyPath: side]
                    horizontal[keyPath: side] += scroll.horizontalScrollIndicatorInsets[keyPath: side] - previousHorizontal[keyPath: side]
                }
            }
            if scroll.verticalScrollIndicatorInsets != vertical { scroll.verticalScrollIndicatorInsets = vertical }
            if scroll.horizontalScrollIndicatorInsets != horizontal { scroll.horizontalScrollIndicatorInsets = horizontal }
            if node?.scrollView === scroll, mask.contains(.top) || hadTop {
                node?.scrollOrigin = scroll.adjustedContentInset.top
                node?.scrollCollapsed = scroll.adjustedContentInset.top
            }
            if changed {
                let inset = scroll.adjustedContentInset
                let point = PixelGrid.offset(CGPoint(x: anchor.x - inset.left, y: anchor.y - inset.top), in: scroll,
                                             clamped: !scroll.isTracking && !scroll.isDragging && !scroll.isDecelerating)
                if scroll.contentOffset != point { scroll.contentOffset = point }
            }
            if scroll.contentInset != next { owner?.pending = true }
        }
    }
}
#endif

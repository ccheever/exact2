// @ref LLP 1123 D3, D4 — an edge whose clearance is off: UIKit still insets
// the route's primary scrollers by their overlap with the safe area
// (`.automatic`), and Exact adds the negation to the scroller's own insets.
#if os(iOS) || os(tvOS)
import UIKit

/// A route's opted-out edges (`-exact-safe-area`, and the top under a
/// transparent bar) and the corrections they hold on its primary scrollers.
/// A correction is a delta in `contentInset` and the indicator insets, which
/// other writers (the keyboard, control bands, grouped lists) also change by
/// deltas: each pass adds only what UIKit's contribution changed since the last.
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
        if value != raw {
            value = raw
            if let parsed = Self.parse(raw) { edges = parsed } else {
                edges = []
                if !invalidReported {
                    invalidReported = true
                    route.host?.presenter.session?.log("route \(route.key): invalid -exact-safe-area value \(raw.debugDescription); using auto")
                }
            }
        }
        // Under a transparent bar the content starts at the top edge (`NavigationHost.collapse`).
        return NavigationHost.transparentBar(route.node) ? edges.union(.top) : edges
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
        pass()
    }

    func end() {
        ended = true
        pass()
    }

    /// Hands `scroll` back without its correction before its owner stops UIKit
    /// insetting it (`.never`); a later pass corrects it again while it is a target.
    func release(_ scroll: UIScrollView?) {
        guard let scroll, let state = states.first(where: { $0.scroll === scroll }) else { return }
        states.removeAll { $0 === state }
        state.release()
    }

    func insets(for scroll: UIScrollView) -> UIEdgeInsets {
        states.first { $0.scroll === scroll }?.applied ?? .zero
    }

    private func pass() {
        pending = true
        drain()
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
        // A writer inside a setter asks for another pass; one that answers every pass is not followed for good.
        var passes = 0
        while pending, passes < 8 {
            pending = false
            passes += 1
            let mask = ended ? [] : route.map(policy) ?? []
            let wanted = ended || mask.isEmpty ? [] : route.map(targets) ?? []
            let retired = states.filter { old in !wanted.contains { $0.1 === old.scroll } }
            states.removeAll { old in retired.contains { $0 === old } }
            for old in retired { old.release() }
            guard !ended, let route else { continue }
            for (_, scroll) in wanted where !states.contains(where: { $0.scroll === scroll }) {
                states.append(Scroller(scroll, owner: self))
            }
            for state in states {
                state.node = wanted.first { $0.1 === state.scroll }?.0
                state.apply(mask)
                if ended { pending = true; break }
            }
            // A setter can replace a backend before its nested cover report runs.
            if !ended, !mask.isEmpty, targets(route).map({ ObjectIdentifier($0.1) }) != wanted.map({ ObjectIdentifier($0.1) }) { pending = true }
        }
    }

    private final class Scroller {
        weak var scroll: UIScrollView?
        weak var node: NodeView?
        private weak var owner: RouteSafeArea?
        private(set) var applied = UIEdgeInsets.zero
        /// The page's title insets (`scrollOrigin`, `scrollCollapsed`) from before the top was turned off.
        private var origin: (CGFloat, CGFloat)?
        private var writing = false, done = false
        private var observation: NSKeyValueObservation?
        private static let sides: [(UIRectEdge, WritableKeyPath<UIEdgeInsets, CGFloat>)] = [(.top, \.top), (.right, \.right), (.bottom, \.bottom), (.left, \.left)]

        init(_ scroll: UIScrollView, owner: RouteSafeArea) {
            self.scroll = scroll; self.owner = owner
            observation = scroll.observe(\.contentInset) { [weak self] _, _ in
                guard let self, !self.writing else { return }
                self.owner?.pass()
            }
        }

        /// Released inside a pass, a scroller that pass still lists is not corrected again.
        func release() {
            guard !done else { return }
            apply([])
            done = true
            observation?.invalidate(); observation = nil
        }

        /// The scroller's overlap with its superview's safe area: what `.automatic`
        /// adds for the chrome. UIKit adds a refreshing control's room to the
        /// scroller's own safe area (iOS 27), not to its superview's, so the
        /// control keeps its room on an edge turned off.
        private static func overlap(_ scroll: UIScrollView) -> UIEdgeInsets {
            guard let parent = scroll.superview else { return .zero }
            let safe = parent.bounds.inset(by: parent.safeAreaInsets), frame = scroll.frame
            return UIEdgeInsets(top: max(0, safe.minY - frame.minY), left: max(0, safe.minX - frame.minX),
                                bottom: max(0, frame.maxY - safe.maxY), right: max(0, frame.maxX - safe.maxX))
        }

        func apply(_ mask: UIRectEdge) {
            guard !done, let scroll else { return }
            writing = true
            defer { writing = false }
            let current = scroll.contentInset, adjusted = scroll.adjustedContentInset, overlap = Self.overlap(scroll)
            let previousVertical = scroll.verticalScrollIndicatorInsets, previousHorizontal = scroll.horizontalScrollIndicatorInsets
            var next = current, vertical = previousVertical, horizontal = previousHorizontal, wanted = UIEdgeInsets.zero
            let hadTop = applied.top != 0
            for (edge, side) in Self.sides {
                let amount = mask.contains(edge) ? -min(adjusted[keyPath: side] - current[keyPath: side], overlap[keyPath: side]) : 0
                wanted[keyPath: side] = amount
                let delta = amount - applied[keyPath: side]
                next[keyPath: side] += delta
                vertical[keyPath: side] += delta
                horizontal[keyPath: side] += delta
            }
            let anchor = CGPoint(x: scroll.contentOffset.x + adjusted.left, y: scroll.contentOffset.y + adjusted.top)
            let changed = current != next
            let withinRange = anchor.x >= -0.5 && anchor.y >= -0.5 &&
                anchor.x <= max(0, scroll.contentSize.width + adjusted.left + adjusted.right - scroll.bounds.width) + 0.5 &&
                anchor.y <= max(0, scroll.contentSize.height + adjusted.top + adjusted.bottom - scroll.bounds.height) + 0.5
            applied = wanted
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
            if let node, node.scrollView === scroll {
                // CSS scrollTop 0 is the content's top: under the bar while the top is off,
                // the expanded title's inset again once it is back (`reportCovers` samples it at rest).
                if mask.contains(.top) {
                    if origin == nil { origin = (node.scrollOrigin, node.scrollCollapsed) }
                    node.scrollOrigin = scroll.adjustedContentInset.top
                    node.scrollCollapsed = scroll.adjustedContentInset.top
                } else if hadTop {
                    let (expanded, collapsed) = origin ?? (0, 0)
                    origin = nil
                    node.scrollOrigin = expanded > 0 ? expanded : scroll.adjustedContentInset.top
                    node.scrollCollapsed = collapsed > 0 ? collapsed : scroll.adjustedContentInset.top
                }
            }
            if changed {
                let inset = scroll.adjustedContentInset
                let point = PixelGrid.offset(CGPoint(x: anchor.x - inset.left, y: anchor.y - inset.top), in: scroll,
                                             clamped: withinRange && !scroll.isTracking && !scroll.isDragging && !scroll.isDecelerating)
                if scroll.contentOffset != point { scroll.contentOffset = point }
            }
            if scroll.contentInset != next { owner?.pending = true }
        }
    }
}

extension NodeView {
    /// UIKit's inset changed as a bar or the safe area did: a route that turns
    /// an edge off corrects it now, before the next batch's cover report.
    package func scrollViewDidChangeAdjustedContentInset(_ scrollView: UIScrollView) {
        guard scrollView === self.scrollView else { return }
        presenter?.navigation.adjustedInsetChanged(self)
    }
}
#endif

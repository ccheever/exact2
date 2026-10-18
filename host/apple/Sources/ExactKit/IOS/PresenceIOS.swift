// Exit animation on UIKit (LLP 1063). An `exit` op names a view leaving
// with its exit: it and every view under it leave the presenter's maps, so
// no lookup by id, focus or accessibility finds them, but stay in the window
// where they were, above their old siblings, taking no input, until the
// host's `destroy` of the leaving view ends the exit and drops them all.
// Its `present` ops keep coming until then: the engine animates it.
// A view's transform, with a layout transition's box, is in `PressFeedback.swift`.
#if os(iOS) || os(tvOS)
import UIKit

struct Leaving {
    let view: NodeView
    let members: [NodeView]
}

extension Presenter {
    func beginExit(_ id: UInt32) {
        guard let view = views[id] else { return }
        // The routes under it that a native container holds outside its view
        // leave with it: a tab's, which its tab container holds, and a
        // presented route and those pushed over it, which their sheet mounts
        // (LLP 1123 D2), shown or kept by their stack
        let held = navigation.controllers.values.map(\.node).filter { (node: NodeView) in
            views[node.id] === node && !node.isDescendant(of: view) && logicallyWithin(node, view)
        }
        // A flying view leaves with its subtree (LLP 1013.000): landed, it is
        // back in its route, where the walk below finds it
        for leaving in [view] + held { landFlights(inside: leaving) }
        var members: [NodeView] = [], seen = Set<UInt32>()
        var stack: [UIView] = [view] + held
        while let next = stack.popLast() {
            if let node = next as? NodeView, views[node.id] === node {
                guard seen.insert(node.id).inserted else { continue }
                members.append(node)
            }
            stack.append(contentsOf: next.subviews)
        }
        for leaving in [view] + held { _ = leaving.endEditing(true) }
        for member in members { release(member.id) { _ in } }
        view.isUserInteractionEnabled = false
        view.accessibilityElementsHidden = true
        // A leaving modal no longer hides what stays (LLP 1080.003).
        view.accessibilityViewIsModal = false
        view.setGhost(true)
        leaving[id] = Leaving(view: view, members: members)
    }

    /// The host's `destroy` of a leaving view: its exit ended.
    func endExit(_ id: UInt32) -> Bool {
        guard let ended = leaving.removeValue(forKey: id) else { return false }
        ended.view.setGhost(false)
        for member in ended.members { member.forget() }
        ended.view.removeFromSuperview()
        return true
    }

    func isLeaving(_ view: NodeView) -> Bool { leaving[view.id]?.view === view }

    /// Presented surface boxes and opacity, including ghosts absent from `tree`.
    func presenceObservation() -> [[String: Any]] {
        let live = views.values
        return (Array(live) + leaving.values.map(\.view)).filter { $0.window != nil }.map { view in
            let viewport = self.viewport
            let r = view.convert(view.surface?.frame ?? view.bounds, to: viewport)
            return ["id": Int(view.id), "x": r.minX - viewport.contentOffset.x, "y": r.minY - viewport.contentOffset.y,
                    "w": r.width, "h": r.height, "opacity": view.alpha, "exiting": isLeaving(view)]
        }
    }
}
#endif

// Exit animation on UIKit (LLP 1063). An `exit` op names a view leaving
// with its exit: it and every view under it leave the presenter's maps, so
// no lookup by id, focus or accessibility finds them, but stay in the window
// where they were, above their old siblings, taking no input, until the
// host's `destroy` of the leaving view ends the exit and drops them all.
// Its `present` ops keep coming until then: the engine animates it.
// A view's transform, with a layout transition's box, is in `PressFeedback.swift`.
#if os(iOS)
import UIKit

struct Leaving {
    let view: NodeView
    let members: [NodeView]
}

extension Presenter {
    func beginExit(_ id: UInt32) {
        guard let view = views[id] else { return }
        var members: [NodeView] = []
        var stack: [UIView] = [view]
        while let next = stack.popLast() {
            if let node = next as? NodeView, views[node.id] === node { members.append(node) }
            stack.append(contentsOf: next.subviews)
        }
        _ = view.endEditing(true)
        for member in members { release(member.id) { _ in } }
        view.isUserInteractionEnabled = false
        view.accessibilityElementsHidden = true
        view.superview?.bringSubviewToFront(view)
        leaving[id] = Leaving(view: view, members: members)
    }

    /// The host's `destroy` of a leaving view: its exit ended.
    func endExit(_ id: UInt32) -> Bool {
        guard let ended = leaving.removeValue(forKey: id) else { return false }
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

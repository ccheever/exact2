// Exit animation on UIKit (LLP 1063). An `exit` retires the leaving view
// and its authored descendants from every lookup. Its painted subtree stays
// where it was, inert, until `destroy`; lifted presentations close as their
// content retires. The final destroy releases every member of the same walk.
// Its `present` ops keep coming until then: the engine animates it.
// A view's transform, with a layout transition's box, is in `PressFeedback.swift`.
#if os(iOS) || os(tvOS)
import UIKit

struct Leaving {
    let view: NodeView
    let members: [NodeView]
}

extension Presenter {
    /// Retirement follows lifted content as well as routes. Event ancestry
    /// retains its own inertness rules across a modal presentation.
    func retirementParent(of view: UIView) -> UIView? {
        if let node = view as? NodeView, let parent = menus.sheets.authoredParent(of: node) ?? menus.logicalParent(of: node) { return parent }
        return eventParent(of: view)
    }

    func beginExit(_ id: UInt32) {
        guard let view = views[id] else { return }
        landFlights(inside: view)
        var children: [ObjectIdentifier: [NodeView]] = [:]
        var displaced = Set<ObjectIdentifier>()
        for node in views.values {
            var parent = retirementParent(of: node)
            if parent !== node.superview { displaced.insert(ObjectIdentifier(node)) }
            while let current = parent, !(current is NodeView) { parent = retirementParent(of: current) }
            if let parent { children[ObjectIdentifier(parent), default: []].append(node) }
        }
        var members: [NodeView] = [], seen = Set<ObjectIdentifier>()
        var stack = [view]
        while let node = stack.popLast() {
            guard seen.insert(ObjectIdentifier(node)).inserted else { continue }
            members.append(node)
            stack.append(contentsOf: children[ObjectIdentifier(node)] ?? [])
        }
        for member in members where member === view || displaced.contains(ObjectIdentifier(member)) { _ = member.endEditing(true) }
        for member in members { member.holdGone() }
        for member in members {
            release(member.id) { _ in }
            member.isUserInteractionEnabled = false
            member.accessibilityElementsHidden = true
            member.accessibilityViewIsModal = false
        }
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
            return ["id": Int(view.id), "x": r.minX - viewport.pageOrigin.x, "y": r.minY - viewport.pageOrigin.y,
                    "w": r.width, "h": r.height, "opacity": view.alpha, "exiting": isLeaving(view)]
        }
    }
}
#endif

// Exit animation on AppKit (LLP 1063): as on UIKit (`PresenceIOS.swift`), a
// leaving view and everything under it leave the presenter's maps but stay
// in the window where they were, in their place in paint order, inert to
// the pointer and hidden from accessibility, until the host's `destroy` of the
// leaving view ends its exit. A view's transform, with a layout transition's
// box, is in `PressFeedback.swift`.
#if os(macOS)
import AppKit

struct Leaving {
    let view: NodeView
    let members: [NodeView]
}

extension Presenter {
    func beginExit(_ id: UInt32) {
        guard let view = views[id] else { return }
        let held = (blocks.shown.filter { navigation.split.holds($0) } + dialogs.entries.filter(\.route).map(\.dialog)).filter {
            views[$0.id] === $0 && !$0.isDescendant(of: view) && logicallyWithin($0, view)
        }
        // Native containers hold these routes outside their authored ancestors.
        for leaving in [view] + held { landFlights(inside: leaving) }
        var members: [NodeView] = [], seen = Set<UInt32>()
        var stack: [NSView] = [view] + held
        while let next = stack.popLast() {
            if let node = next as? NodeView, views[node.id] === node {
                guard seen.insert(node.id).inserted else { continue }
                members.append(node)
            }
            stack.append(contentsOf: next.subviews)
        }
        for leaving in [view] + held {
            guard let window = leaving.window, let responder = dialogs.focusOwner(window),
                  responder === leaving || responder.isDescendant(of: leaving) else { continue }
            window.makeFirstResponder(nil)
        }
        for member in members { member.holdGone() } // a held node that starts to leave is gone (HeldState.swift)
        // AppKit discards cells on reload. Move only the row root; a nested
        // exit must keep its ancestors' opacity, transforms and clipping. It
        // goes just after the table and keeps its rank, so it is ordered
        // against the table as a whole, not against the rows in its cells
        // (LLP 1083.000 §4, item 4).
        if let cell = view.superview as? TableRowCell, cell.row === view, let home = cell.feed?.home {
            let frame = home.convert(view.frame, from: cell)
            home.addSubview(view, positioned: .above, relativeTo: cell.feed?.table)
            view.frame = frame
        }
        for member in members { release(member.id, forget: false) }
        for root in [view] + held {
            root.routeInert = true
            root.setAccessibilityHidden(true)
            root.setGhost(true)
        }
        leaving[id] = Leaving(view: view, members: members)
    }

    /// The host's `destroy` of a leaving view: its exit ended.
    func endExit(_ id: UInt32) -> Bool {
        guard let ended = leaving.removeValue(forKey: id) else { return false }
        ended.view.setGhost(false)
        for member in ended.members {
            member.setGhost(false)
            if navigation.split.holds(member) { navigation.split.retire(member) }
            member.forget()
        }
        ended.view.removeFromSuperview()
        return true
    }

    func isLeaving(_ view: NSView) -> Bool {
        guard let node = view as? NodeView else { return false }
        return leaving.values.contains { $0.members.contains { $0 === node } }
    }

    /// Presented surface boxes and opacity, including ghosts absent from `tree`.
    func presenceObservation() -> [[String: Any]] {
        let live = views.values
        return (Array(live) + leaving.values.map(\.view)).filter { $0.window != nil }.map { view in
            let clip = viewport.contentView
            let local = view.surface?.frame ?? view.bounds
            let r = view.convert(local.applying(view.layer?.affineTransform() ?? .identity), to: clip)
            return ["id": Int(view.id), "x": r.minX - clip.bounds.minX, "y": r.minY - clip.bounds.minY,
                    "w": r.width, "h": r.height, "opacity": view.alphaValue, "exiting": isLeaving(view)]
        }
    }
}
#endif

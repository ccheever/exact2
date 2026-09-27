// Exit animation on AppKit (LLP 1063): as on UIKit (`PresenceIOS.swift`), a
// leaving view and everything under it leave the presenter's maps but stay
// in the window where they were, above their old siblings, inert to the
// pointer and hidden from accessibility, until the host's `destroy` of the
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
        var members: [NodeView] = []
        var stack: [NSView] = [view]
        while let next = stack.popLast() {
            if let node = next as? NodeView, views[node.id] === node { members.append(node) }
            stack.append(contentsOf: next.subviews)
        }
        if let responder = view.window?.firstResponder as? NSView, responder.isDescendant(of: view) {
            view.window?.makeFirstResponder(nil)
        }
        for member in members { release(member.id, forget: false) }
        view.routeInert = true
        view.setAccessibilityHidden(true)
        if let parent = view.superview { parent.addSubview(view, positioned: .above, relativeTo: nil) }
        leaving[id] = Leaving(view: view, members: members)
    }

    /// The host's `destroy` of a leaving view: its exit ended.
    func endExit(_ id: UInt32) -> Bool {
        guard let ended = leaving.removeValue(forKey: id) else { return false }
        for member in ended.members { member.forget() }
        ended.view.removeFromSuperview()
        return true
    }

    func isLeaving(_ view: NSView) -> Bool {
        guard let node = view as? NodeView else { return false }
        return leaving[node.id]?.view === node
    }
}
#endif

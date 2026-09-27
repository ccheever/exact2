// Exit animation on AppKit (LLP 1063): as on UIKit (`PresenceIOS.swift`), a
// leaving view and everything under it leave the presenter's maps but stay
// in the window where they were, above their old siblings, inert to the
// pointer and hidden from accessibility, until the host's `destroy` of the
// leaving view ends its exit. A view's transform is here too: its own
// transforms, and outermost a layout transition's box.
#if os(macOS)
import AppKit

struct Leaving {
    let view: NodeView
    let members: [NodeView]
}

extension NodeView {
    func applyTransform() {
        // A lifted Arrange row moves by its frame: AppKit paints and culls a
        // view where its frame is, never where its layer was moved.
        let shift = presenter?.reorder?.lifts(id) == true ? translate : .zero
        if shift != arrangeShift {
            setFrameOrigin(NSPoint(x: frame.minX - arrangeShift.x + shift.x, y: frame.minY - arrangeShift.y + shift.y))
            arrangeShift = shift
        }
        let b = bounds
        var t = CGAffineTransform(translationX: translate.x - shift.x, y: translate.y - shift.y)
        t = t.translatedBy(x: b.midX, y: b.midY).rotated(by: rotate * .pi / 180).scaledBy(x: scale, y: scale).translatedBy(x: -b.midX, y: -b.midY)
        // Outermost, a layout transition's offset and scale from the box's
        // top-left corner, as a web FLIP places it (LLP 1063).
        let flip = CGAffineTransform(translationX: layoutOffset.x + b.minX, y: layoutOffset.y + b.minY).scaledBy(x: layoutScale.x, y: layoutScale.y).translatedBy(x: -b.minX, y: -b.minY)
        layer?.setAffineTransform(t.concatenating(flip))
    }
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

// Exit animation on UIKit (LLP 1063). An `exit` op names a view leaving
// with its exit: it and every view under it leave the presenter's maps, so
// no lookup by id, focus or accessibility finds them, but stay in the window
// where they were, above their old siblings, taking no input, until the
// host's `destroy` of the leaving view ends the exit and drops them all.
// Its `present` ops keep coming until then: the engine animates it. A view's
// transform is here too: its own transforms, and outermost a layout
// transition's box (the `layout` present op's offset and scale).
#if os(iOS)
import UIKit

struct Leaving {
    let view: NodeView
    let members: [NodeView]
}

extension NodeView {
    func applyTransform() {
        // CSS's individual transforms: translate, then rotate, then scale,
        // about the center (UIKit's anchor); a press folds into the scale.
        // Outermost, a layout transition's offset and scale from the box's
        // top-left corner, as a web FLIP places it (LLP 1063).
        let own = CGAffineTransform(translationX: translate.x, y: translate.y).rotated(by: rotate * .pi / 180).scaledBy(x: scale * pressFactor, y: scale * pressFactor)
        let half = bounds.size
        let flip = CGAffineTransform(scaleX: layoutScale.x, y: layoutScale.y).concatenating(CGAffineTransform(translationX: layoutOffset.x + (layoutScale.x - 1) * half.width / 2, y: layoutOffset.y + (layoutScale.y - 1) * half.height / 2))
        transform = own.concatenating(flip).concatenating(contextTransform)
    }
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
}
#endif

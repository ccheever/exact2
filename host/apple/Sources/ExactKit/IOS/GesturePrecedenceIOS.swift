#if os(iOS) || os(tvOS)
import UIKit

// LLP 1057.001 §1 on UIKit, with UIKit's own relationships: rule 3's boundary
// (a press handler or text input between the touch and a node keeps the touch
// from that node's drags; a `pan` passes a press) and innermost-first (an
// ancestor's drag waits for a descendant's to fail). A pinch never gates an
// ancestor: with one finger it stays possible until the touch ends.
extension NodeView {
    /// This node's drag-family recognizers: the candidates rules 3 and 4 order.
    var dragRecognizers: [UIGestureRecognizer] {
        [reorderPan, transformRecognizer, heightRecognizer, layoutPanRecognizer, swipeRecognizer].compactMap { $0 }
    }
    /// Whether `gesture` is one of this node's drags, the photo's pinch or the
    /// reorder lift: the recognizers a nested press or editor keeps its touch from.
    func stopsAtPress(_ gesture: UIGestureRecognizer) -> Bool {
        dragRecognizers.contains { $0 === gesture } || gesture === transformContact?.pinch || gesture === reorderPress
    }
    /// A press handler or text input strictly between the touch and this node.
    /// A `pan` passes a press (`presses: false`): it takes the touch once it
    /// begins, cancelling the press, as a draggable element hears a drag that
    /// starts on a button inside it (kanban F6).
    func pressBoundary(_ touch: UITouch, presses: Bool = true) -> Bool {
        var view = touch.view
        while let current = view, current !== self {
            if let node = current as? NodeView, (presses && node.handlers.contains("press")) || node.field != nil || node.textArea != nil { return true }
            view = current.superview
        }
        return false
    }
    package func gestureRecognizer(_ gesture: UIGestureRecognizer, shouldRequireFailureOf other: UIGestureRecognizer) -> Bool {
        // A swipe waits for the screen-edge back gesture to fail, as UIKit's
        // own swipe actions and Signal's swipe to reply do: from the edge, back wins.
        #if os(iOS)
        // Only the screen-edge one: iOS 26's content pop is the same class,
        // and it waits for the swipe in turn, so waiting on it would leave neither.
        if gesture === swipeRecognizer, let nav = other.view?.next as? UINavigationController,
           nav.interactivePopGestureRecognizer === other { return true }
        #endif
        guard dragRecognizers.contains(where: { $0 === gesture }), let owner = other.view as? NodeView,
              owner !== self, owner.isDescendant(of: self) else { return false }
        return owner.dragRecognizers.contains { $0 === other }
    }
}
#endif

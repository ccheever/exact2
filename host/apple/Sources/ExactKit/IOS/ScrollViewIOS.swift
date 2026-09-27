#if os(iOS)
import UIKit

/// A plain container: the document, a canvas's overlay (LLP 1014). Hit-
/// testable at alpha 0 — a canvas's children painted through its surface
/// composite at alpha 0 and must still take a tap, which UIKit's default
/// hit-test refuses below 0.01 — and transparent to a hit on nothing, so
/// the touch reaches what holds it (the canvas, the viewport).
final class PlainView: UIView {
    override func didAddSubview(_ subview: UIView) { super.didAddSubview(subview); FocusSearch.joined(subview) }
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard !isHidden, isUserInteractionEnabled, bounds.contains(point) else { return nil }
        for sub in subviews.reversed() {
            if let hit = sub.hitTest(convert(point, to: sub), with: event) { return hit }
        }
        return nil
    }
}

/// A scroll container — the viewport over the document, and a node whose
/// effective `overflow` scrolls. The platform pans it (LLP 1002 D4: scroll
/// always wins; UIKit does not chain a pan out of a nested scroll view at
/// its edge). Which axes it scrolls comes from the node's rows; a tap's
/// wheel (the agent's) applies the web's chaining rule itself
/// (`AgentIOS.swift`).
class ScrollView: UIScrollView {
    var scrollsX = true
    var scrollsY = true
    override func touchesShouldCancel(in view: UIView) -> Bool {
        !CanvasInput.owns(view) && super.touchesShouldCancel(in: view)
    }
    override func gestureRecognizerShouldBegin(_ gesture: UIGestureRecognizer) -> Bool {
        if gesture === panGestureRecognizer {
            let velocity = panGestureRecognizer.velocity(in: self)
            let location = panGestureRecognizer.location(in: self)
            let translation = panGestureRecognizer.translation(in: self)
            var view = hitTest(CGPoint(x: location.x - translation.x, y: location.y - translation.y), with: nil)
            if CanvasInput.owns(view) { return false }
            // CSS intersects touch-action from the hit element through the
            // scroll container. It governs initial direction, not reversal.
            while let current = view {
                if let node = current as? NodeView, !node.allowsTouchPan(velocity) { return false }
                if current === self { break }
                view = current.superview
            }
            if let owner = superview as? NodeView, !owner.allowsTouchPan(velocity) { return false }
        }
        return super.gestureRecognizerShouldBegin(gesture)
    }
    /// A touch that no node took — nothing focusable, nothing pressable —
    /// ends the editing, as a tap on a page's blank ground blurs the field
    /// and sends the keyboard away (LLP 1008 §9).
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        // An inert button can still retain focus, for example between the
        // two taps of a double-tap recognizer. Its unhandled touch is not
        // blank ground. Check before forwarding to enclosing scroll views.
        var target = touches.first?.view
        while let view = target {
            if let node = view as? NodeView {
                if node.presenter?.contextRetainsFocus(node) == true { return }
                break
            }
            target = view.superview
        }
        super.touchesEnded(touches, with: event)
        // A blur is the session's (LLP 1035.001 D5): its viewport's, never the
        // window's — found through the nearest node above a nested scroller;
        // the viewport itself has none above it and is its own.
        if let t = touches.first, bounds.contains(t.location(in: self)) {
            var viewport: UIView = self
            var above = superview
            while let current = above {
                if let node = current as? NodeView, let owned = node.presenter?.viewport { viewport = owned; break }
                above = current.superview
            }
            viewport.endEditing(true)
        }
    }
}

#endif

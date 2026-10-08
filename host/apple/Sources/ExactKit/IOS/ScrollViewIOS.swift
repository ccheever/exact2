#if os(iOS) || os(tvOS)
import UIKit

/// A plain container: the document, a canvas's overlay (LLP 1014). Hit-
/// testable at alpha 0 — a canvas's children painted through its surface
/// composite at alpha 0 and must still take a tap, which UIKit's default
/// hit-test refuses below 0.01 — and transparent to a hit on nothing, so
/// the touch reaches what holds it (the canvas, the viewport).
package final class PlainView: UIView {
    package override func didAddSubview(_ subview: UIView) { super.didAddSubview(subview); FocusSearch.joined(subview) }
    package override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard !isHidden, isUserInteractionEnabled, bounds.contains(point) else { return nil }
        return NodeView.hitChildren(in: self, at: point, with: event)
    }
}

/// A scroll container — the viewport over the document, and a node whose
/// effective `overflow` scrolls. The platform pans it (LLP 1002 D4: scroll
/// always wins; UIKit does not chain a pan out of a nested scroll view at
/// its edge). Which axes it scrolls comes from the node's rows; a tap's
/// wheel (the agent's) applies the web's chaining rule itself
/// (`AgentIOS.swift`).
package class ScrollView: UIScrollView {
    package override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard let hit = super.hitTest(point, with: event) else { return nil }
        return NodeView.hitChildren(in: self, at: point, with: event) ?? hit
    }

    var scrollsX = true
    var scrollsY = true
    #if os(tvOS)
    /// The offset the remote's last step scrolls to, and when it began
    /// (`RemoteTVOS.swift`): a step pressed during that animation goes on
    /// from there.
    var remoteStepTarget: (offset: CGPoint, at: CFTimeInterval)?
    #endif
    /// A pan cancels a touch in progress, as it does a custom button's; UIKit
    /// would leave a `UIControl` its touch, so a native button (LLP 1069.011
    /// D4) is named. A canvas that owns its input keeps it.
    package override func touchesShouldCancel(in view: UIView) -> Bool {
        !CanvasInputs.owns(view) && (view is NativeButtonIOS || super.touchesShouldCancel(in: view))
    }
    package override func gestureRecognizerShouldBegin(_ gesture: UIGestureRecognizer) -> Bool {
        if gesture === panGestureRecognizer {
            let velocity = panGestureRecognizer.velocity(in: self)
            let location = panGestureRecognizer.location(in: self)
            let translation = panGestureRecognizer.translation(in: self)
            if !admitsPan(velocity: velocity, translation: translation, start: CGPoint(x: location.x - translation.x, y: location.y - translation.y)) { return false }
        }
        return super.gestureRecognizerShouldBegin(gesture)
    }
    /// The pan's own check, apart from UIKit's: `touch-action` intersected
    /// from the node hit at `start` through this container, then chaining.
    /// The direction is the velocity, or the movement that crossed the slop
    /// while UIKit has none yet, as a photo's yield reads it (LLP 1057.001
    /// rule 2), so the drag it steps aside from is taken here.
    func admitsPan(velocity: CGPoint, translation: CGPoint, start: CGPoint) -> Bool {
        let direction = velocity == .zero ? translation : velocity
        var view = hitTest(start, with: nil)
        if CanvasInputs.owns(view) { return false }
        // CSS intersects touch-action from the hit element through the
        // scroll container. It governs initial direction, not reversal.
        while let current = view {
            if let node = current as? NodeView, !node.allowsTouchPan(direction) { return false }
            if current === self { break }
            view = current.superview
        }
        if let owner = superview as? NodeView, !owner.allowsTouchPan(direction) { return false }
        return !handsOff(direction)
    }
    /// CSS's scroll chaining at a gesture's start (LLP 1070 G1, Q4 as ruled
    /// provisionally): under `overscroll-behavior: auto`, a drag that begins
    /// at this view's edge and pushes outward belongs to the enclosing
    /// scroller that can take it, as Chrome and Safari chain it, instead of
    /// UIKit's rubber band here. Only for a known direction, on an axis this
    /// view actually scrolls; `contain` keeps the band, `none` keeps the
    /// gesture with no band. Never mid-gesture: UIKit asks once, at begin.
    func handsOff(_ velocity: CGPoint) -> Bool {
        guard velocity != .zero, let owner = superview as? NodeView else { return false }
        let horizontal = abs(velocity.x) > abs(velocity.y)
        let behavior = owner.style[horizontal ? "overscroll_behavior_x" : "overscroll_behavior_y"]?.string ?? "auto"
        // One flag for both axes, written each gesture so a style that leaves
        // `none` gets its band back: off for this drag's `none`, or for an
        // axis this view scrolls under `none`.
        let still = { (axis: String) in owner.style["overscroll_behavior_\(axis)"]?.string == "none" }
        bounces = !(behavior == "none" || (scrollsX && still("x")) || (scrollsY && still("y")))
        return chains(velocity)
    }
    /// `handsOff`'s answer without its side effect: whether a drag in
    /// `velocity` begun now chains to an enclosing scroller (a descendant's
    /// recognizer asks it, LLP 1057.001 rule 2).
    func chains(_ velocity: CGPoint) -> Bool {
        guard velocity != .zero, let owner = superview as? NodeView else { return false }
        let horizontal = abs(velocity.x) > abs(velocity.y)
        guard (owner.style[horizontal ? "overscroll_behavior_x" : "overscroll_behavior_y"]?.string ?? "auto") == "auto" else { return false }
        let i = adjustedContentInset
        let (at, low, high, scrolls) = horizontal
            ? (contentOffset.x, -i.left, contentSize.width + i.right - bounds.width, scrollsX)
            : (contentOffset.y, -i.top, contentSize.height + i.bottom - bounds.height, scrollsY)
        guard scrolls, high > low else { return false }
        // A finger moving toward +x pulls the content toward its start.
        let toward = horizontal ? velocity.x : velocity.y
        let atEdge = toward > 0 ? at <= low + 0.5 : at >= high - 0.5
        guard atEdge else { return false }
        // Only when an enclosing scroller can take that direction; with none,
        // the band here is all a finger can have.
        var up = superview
        while let current = up {
            if let outer = current as? ScrollView {
                let o = outer.adjustedContentInset
                let (at, low, high, scrolls) = horizontal
                    ? (outer.contentOffset.x, -o.left, outer.contentSize.width + o.right - outer.bounds.width, outer.scrollsX)
                    : (outer.contentOffset.y, -o.top, outer.contentSize.height + o.bottom - outer.bounds.height, outer.scrollsY)
                if scrolls && high > low && (toward > 0 ? at > low + 0.5 : at < high - 0.5) { return true }
            }
            up = current.superview
        }
        return false
    }
    /// A touch that no node took — nothing focusable, nothing pressable —
    /// ends the editing, as a tap on a page's blank ground blurs the field
    /// and sends the keyboard away (LLP 1008 §9).
    package override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
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

#if !os(tvOS)
/// UIKit's pull-to-refresh control, below the scroller's `padding-top`
/// (@ref LLP 1010 §6.9, the one rule a padded list takes that CSS has no
/// word for: React Native's `progressViewOffset`, placed as its
/// `RCTRefreshControl` places it). The padding is in the content, so a header
/// laid over it would hide the spinner; here the control's top sits that far
/// down the scroller's port, behind the content, and the pull uncovers it
/// between the header and the first row, where it stays while the app
/// refreshes. UIKit lays the control out as the pull goes; after each
/// layout its frame moves to that place, which is a no-op once it is there.
/// With no padding, UIKit's own place.
final class PaddedRefreshControl: UIRefreshControl {
    override func layoutSubviews() {
        super.layoutSubviews()
        guard let owner = superview?.superview as? NodeView else { return }
        let top = owner.number("padding_top")
        guard top != 0 else { return }
        let gap = convert(CGPoint(x: 0, y: top), from: owner).y
        if gap != 0 { frame = frame.offsetBy(dx: 0, dy: gap) }
    }
}
#endif

extension NodeView {
    /// A `refresh` handler on a scroll container is UIKit's pull-to-refresh:
    /// the control fires the event; the app's `refreshing` going false ends it.
    func updateRefresh() {
        // tvOS has no refresh control.
        #if !os(tvOS)
        guard let sv = scroll else { return }
        if handlers.contains("refresh") {
            if sv.refreshControl == nil {
                let control = PaddedRefreshControl()
                control.addTarget(self, action: #selector(pulledToRefresh), for: .valueChanged)
                sv.refreshControl = control
            }
            if props["refreshing"] != "true", let control = sv.refreshControl, control.isRefreshing {
                control.endRefreshing()
            }
        } else if sv.refreshControl != nil {
            sv.refreshControl = nil
        }
        #endif
    }
    #if !os(tvOS)
    @objc func pulledToRefresh() {
        presenter?.refresh(id)
        // An app that starts nothing leaves `refreshing` false: end promptly.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) { [weak self, token = incarnation] in if self?.incarnation == token { self?.updateRefresh() } }
    }
    #endif
}
#endif

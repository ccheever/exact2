import ExactKit
import CExact
#if os(iOS) || os(tvOS)
import UIKit
import QuartzCore

extension NodeView {
    func transformDragModel() -> TransformDragPosition? {
        TransformDragPosition(x: Double(translate.x), y: Double(translate.y), scale: Double(scale))
    }
    func transformDragPresentation() -> TransformDragPosition? {
        guard rotate == 0, contextTransform.isIdentity else { return nil }
        // UIView applies its anchor separately; this matrix already excludes
        // the center translation unlike the AppKit layer mapping.
        // The render tree's copy only while Core Animation runs a curve on it.
        let source = layer.animationKeys()?.isEmpty == false ? layer.presentation() ?? layer : layer
        return TransformDragPosition(matrix: source.affineTransform(), center: .zero)
    }
    func updateTransformDragGesture() {
        // tvOS has no multi-finger pan or pinch.
        #if !os(tvOS)
        if presenter?.transformBindings[id]?.target != nil, transformRecognizer == nil {
            let pan = UIPanGestureRecognizer(target: self, action: #selector(transformDragging(_:)))
            pan.maximumNumberOfTouches = 2; pan.delegate = self
            let pinch = UIPinchGestureRecognizer(target: self, action: #selector(transformDragging(_:)))
            pinch.delegate = self
            addGestureRecognizer(pan); addGestureRecognizer(pinch)
            transformRecognizer = pan; transformContact = TransformContact(pinch)
        } else if presenter?.transformBindings[id]?.target == nil, let pan = transformRecognizer {
            let previous = transformHold; transformHold = nil
            DispatchQueue.main.async { previous?.cancel() }
            removeGestureRecognizer(pan); transformRecognizer = nil
            if let pinch = transformContact?.pinch { removeGestureRecognizer(pinch) }
            transformContact = nil
        }
        #endif
    }
    /// The binding's recognizers: eligible handles only, and the pinch only
    /// where the platform would not zoom — a node from here up whose
    /// `touch-action` excludes `pinch-zoom`, as the browser decides (§2).
    /// The pan leaves the platform an axis the handle's own `touch-action`
    /// names when a scroller would take it (rule 2: `pan-x` on a pager's
    /// photo pages it sideways), as a browser cancels the pointer; `auto`
    /// and `manipulation` keep every drag the binding's, and a pan joining
    /// the binding's pinch is always admitted (rule 6).
    func transformShouldBegin(_ gesture: UIGestureRecognizer) -> Bool? {
        guard gesture === transformRecognizer || gesture === transformContact?.pinch else { return nil }
        guard SwipeInput.allows(self), presenter?.transformBindings[id]?.target != nil else { return false }
        guard gesture === transformRecognizer else { return !allowsPinchZoom }
        let action = style["touch_action"]?.string ?? "auto"
        guard action != "auto", action != "manipulation", let pan = gesture as? UIPanGestureRecognizer else { return true }
        if let pinch = transformContact?.pinch, pinch.state == .began || pinch.state == .changed { return true }
        let velocity = pan.velocity(in: self), translation = pan.translation(in: self)
        let direction = velocity == .zero ? translation : velocity
        guard direction != .zero else { return true }
        let location = pan.location(in: self)
        return !platformPans(direction, from: CGPoint(x: location.x - translation.x, y: location.y - translation.y))
    }
    /// Whether the platform takes a drag in `direction` begun at `start`
    /// (this view's points): `touch-action` intersected from the node hit
    /// there up through the scroller that would move — the nearest enabled
    /// one that can on that axis (its insets count), or past one at its edge to the
    /// scroller it chains to — its owner included, as CSS and
    /// `ScrollView.gestureRecognizerShouldBegin` decide. With no such
    /// scroller nothing would take it.
    func platformPans(_ direction: CGPoint, from start: CGPoint) -> Bool {
        let horizontal = abs(direction.x) > abs(direction.y)
        var view: UIView? = hitTest(start, with: nil) ?? self
        while let current = view {
            if let scroll = current as? ScrollView {
                let i = scroll.adjustedContentInset
                let room = horizontal ? scroll.contentSize.width + i.left + i.right - scroll.bounds.width
                                      : scroll.contentSize.height + i.top + i.bottom - scroll.bounds.height
                if scroll.isScrollEnabled, (horizontal ? scroll.scrollsX : scroll.scrollsY) && room > 0.5 {
                    if let owner = scroll.superview as? NodeView, !owner.allowsTouchPan(direction) { return false }
                    if !scroll.chains(direction) { return true }
                }
            } else if let node = current as? NodeView, !node.allowsTouchPan(direction) {
                return false
            }
            view = current.superview
        }
        return false
    }
    var allowsPinchZoom: Bool {
        var view: UIView? = self
        while let current = view {
            if let node = current as? NodeView {
                let action = node.style["touch_action"]?.string ?? "auto"
                if !(action == "auto" || action == "manipulation" || action.hasSuffix("pinch-zoom")) { return false }
            }
            view = current.superview
        }
        return true
    }
    package func gestureRecognizer(_ gesture: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
        let pair: [UIGestureRecognizer?] = [transformRecognizer, transformContact?.pinch]
        return pair.contains { $0 === gesture } && pair.contains { $0 === other }
    }
    /// The clip's centre in window points: the origin of the focal arithmetic.
    private var transformCenter: CGPoint? {
        guard let clipID = presenter?.transformBindings[id]?.clip, let clip = presenter?.views[clipID], let window else { return nil }
        return clip.convert(CGPoint(x: clip.bounds.midX, y: clip.bounds.midY), to: window)
    }
    #if !os(tvOS)
    @objc func transformDragging(_ gesture: UIGestureRecognizer) {
        guard let contact = transformContact, let pan = transformRecognizer else { return }
        let time = CACurrentMediaTime(), isPan = gesture === pan
        func focal() -> CGPoint { (contact.panning ? pan : contact.pinch).location(in: window) }
        func anchor(_ from: CGPoint? = nil) {
            guard let hold = transformHold as? TransformDragHold, let value = TransformDragPosition(hold.current) else { return }
            contact.anchor = (value, from ?? focal(), contact.pinch.scale, gesture.numberOfTouches)
        }
        func current() -> [Double]? {
            guard let anchor = contact.anchor, let center = transformCenter else { return nil }
            let from = anchor.focal, to = focal()
            let factor = contact.pinching ? Double(contact.pinch.scale / anchor.scale) : 1
            return anchor.value.focused(from: CGPoint(x: from.x - center.x, y: from.y - center.y),
                                        to: CGPoint(x: to.x - center.x, y: to.y - center.y), factor: factor)?.values
        }
        switch gesture.state {
        case .began:
            if transformHold == nil { transformHold = TransformDragHold(self, time: time) }
            guard transformHold != nil else { return }
            if isPan { contact.panning = true } else { contact.pinching = true }
            // UIKit measures a pan from touch-down: keep the movement that
            // recognized it (a coalesced drag may begin and end at once).
            let point = focal(), t = pan.translation(in: window)
            anchor(isPan && !contact.pinching ? CGPoint(x: point.x - t.x, y: point.y - t.y) : nil)
            if let values = current(), (transformHold as? TransformDragHold)?.move(to: values, time: time) != true { transformHold?.cancel(); transformHold = nil }
        case .changed:
            guard let hold = transformHold as? TransformDragHold else { return }
            if contact.anchor?.touches != gesture.numberOfTouches, isPan || !contact.panning { anchor() }
            guard let values = current(), hold.move(to: values, time: time) else {
                hold.cancel(); transformHold = nil; contact.panning = false; contact.pinching = false; return
            }
        case .ended, .cancelled, .failed:
            if isPan { contact.panning = false } else { contact.pinching = false }
            guard let hold = transformHold as? TransformDragHold else { return }
            if contact.panning || contact.pinching { anchor(); return }
            // The last of the pair ends: one release while both tokens are live.
            transformHold = nil
            hold.finish(to: hold.current, time: time, cancel: gesture.state != .ended)
        default: break
        }
    }
    #endif
}
#endif

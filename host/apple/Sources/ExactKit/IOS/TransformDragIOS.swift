#if os(iOS)
import UIKit
import QuartzCore

/// The photo handle's one contact (LLP 1057.001 §4): the binding's pan, now up
/// to two fingers, and a `UIPinchGestureRecognizer`, simultaneous with it —
/// the one built-in simultaneity (precedence rule 6). Both feed the same paired
/// `TransformDragHold`. Every sample is the pair anchored at the contact's
/// focal point; the anchor moves whenever a recognizer starts or stops or the
/// finger count changes, which also absorbs UIKit's centroid jump.
final class TransformContact {
    let pinch: UIPinchGestureRecognizer
    var panning = false, pinching = false
    /// The pair's value, the focal point (window points) and pinch scale at the anchor.
    var anchor: (value: TransformDragPosition, focal: CGPoint, scale: CGFloat, touches: Int)?
    init(_ pinch: UIPinchGestureRecognizer) { self.pinch = pinch }
}

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
    }
    /// The binding's recognizers: eligible handles only, and the pinch only
    /// where the platform would not zoom — a node from here up whose
    /// `touch-action` excludes `pinch-zoom`, as the browser decides (§2).
    func transformShouldBegin(_ gesture: UIGestureRecognizer) -> Bool? {
        guard gesture === transformRecognizer || gesture === transformContact?.pinch else { return nil }
        guard SwipeInput.allows(self), presenter?.transformBindings[id]?.target != nil else { return false }
        return gesture === transformRecognizer || !allowsPinchZoom
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
    func gestureRecognizer(_ gesture: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
        let pair: [UIGestureRecognizer?] = [transformRecognizer, transformContact?.pinch]
        return pair.contains { $0 === gesture } && pair.contains { $0 === other }
    }
    /// The clip's centre in window points: the origin of the focal arithmetic.
    private var transformCenter: CGPoint? {
        guard let clipID = presenter?.transformBindings[id]?.clip, let clip = presenter?.views[clipID], let window else { return nil }
        return clip.convert(CGPoint(x: clip.bounds.midX, y: clip.bounds.midY), to: window)
    }
    @objc func transformDragging(_ gesture: UIGestureRecognizer) {
        guard let contact = transformContact, let pan = transformRecognizer else { return }
        let time = CACurrentMediaTime(), isPan = gesture === pan
        func focal() -> CGPoint { (contact.panning ? pan : contact.pinch).location(in: window) }
        func anchor(_ from: CGPoint? = nil) {
            guard let hold = transformHold, let value = TransformDragPosition(hold.current) else { return }
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
            if let values = current(), transformHold?.move(to: values, time: time) != true { transformHold?.cancel(); transformHold = nil }
        case .changed:
            guard let hold = transformHold else { return }
            if contact.anchor?.touches != gesture.numberOfTouches, isPan || !contact.panning { anchor() }
            guard let values = current(), hold.move(to: values, time: time) else {
                hold.cancel(); transformHold = nil; contact.panning = false; contact.pinching = false; return
            }
        case .ended, .cancelled, .failed:
            if isPan { contact.panning = false } else { contact.pinching = false }
            guard let hold = transformHold else { return }
            if contact.panning || contact.pinching { anchor(); return }
            // The last of the pair ends: one release while both tokens are live.
            transformHold = nil
            hold.finish(to: hold.current, time: time, cancel: gesture.state != .ended)
        default: break
        }
    }
}
extension Presenter {
    func transformFacts(_ binding: TransformDragBinding) -> TransformGeometryFacts? {
        guard let handle = views[binding.id], let targetID = binding.target, let clipID = binding.clip,
              let target = views[targetID], let clip = views[clipID], target.superview === clip,
              let window = clip.window, handle.window === window, target.window === window,
              SwipeInput.allows(handle), SwipeInput.allows(target), SwipeInput.allows(clip) else { return nil }
        var ancestor: UIView? = handle
        while let view = ancestor {
            if view !== target {
                if !view.transform.isIdentity || !view.layer.affineTransform().isIdentity
                    || !(view.layer.presentation()?.affineTransform().isIdentity ?? true) { return nil }
                if (view.layer.animationKeys() ?? []).contains(where: { $0.contains("transform") || $0.contains("position") || $0.contains("bounds") }) { return nil }
            } else if target.rotate != 0 || !target.contextTransform.isIdentity { return nil }
            ancestor = view.superview
        }
        // UIView.frame is undefined under a nonidentity transform. Bounds and
        // center retain the untransformed layout box in its parent's coordinates.
        let frame = CGRect(x: target.center.x - target.bounds.width / 2, y: target.center.y - target.bounds.height / 2,
            width: target.bounds.width, height: target.bounds.height)
        return TransformGeometryFacts(targetBounds: target.bounds, targetFrame: frame,
            clipBounds: clip.bounds, windowOrigin: clip.convert(.zero, to: window), supportedAncestors: true)
    }
}
#endif

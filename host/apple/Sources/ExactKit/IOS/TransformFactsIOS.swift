// A transform drag's geometry facts on UIKit (`TransformGeometry`): the
// core's, which reports them whether or not the Drag module is linked
// (LLP 1047.001 D4).
#if os(iOS) || os(tvOS)
import UIKit

/// The photo handle's one contact (LLP 1057.001 §4): the binding's pan, now up
/// to two fingers, and a `UIPinchGestureRecognizer`, simultaneous with it —
/// the one built-in simultaneity (precedence rule 6). Both feed the same paired
/// `TransformDragHold`. Every sample is the pair anchored at the contact's
/// focal point; the anchor moves whenever a recognizer starts or stops or the
/// finger count changes, which also absorbs UIKit's centroid jump.
package final class TransformContact {
    #if os(tvOS)
    // tvOS has no pinch; nothing makes a contact there.
    package let pinch: UIGestureRecognizer
    #else
    package let pinch: UIPinchGestureRecognizer
    #endif
    package var panning = false, pinching = false
    /// The pair's value, the focal point (window points) and pinch scale at the anchor.
    package var anchor: (value: TransformDragPosition, focal: CGPoint, scale: CGFloat, touches: Int)?
    #if os(tvOS)
    package init(_ pinch: UIGestureRecognizer) { self.pinch = pinch }
    #else
    package init(_ pinch: UIPinchGestureRecognizer) { self.pinch = pinch }
    #endif
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

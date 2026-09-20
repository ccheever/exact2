#if os(iOS)
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
        return TransformDragPosition(matrix: (layer.presentation() ?? layer).affineTransform(), center: .zero)
    }
    func updateTransformDragGesture() {
        if presenter?.transformBindings[id]?.target != nil, transformRecognizer == nil {
            let pan = UIPanGestureRecognizer(target: self, action: #selector(transformDragging(_:)))
            pan.maximumNumberOfTouches = 1; pan.delegate = self
            addGestureRecognizer(pan); transformRecognizer = pan
        } else if presenter?.transformBindings[id]?.target == nil, let pan = transformRecognizer {
            let previous = transformHold; transformHold = nil
            DispatchQueue.main.async { previous?.cancel() }
            removeGestureRecognizer(pan); transformRecognizer = nil
        }
    }
    @objc func transformDragging(_ pan: UIPanGestureRecognizer) {
        let point = pan.translation(in: window), time = CACurrentMediaTime()
        switch pan.state {
        case .began:
            // UIKit measures translation from touch-down. Preserve the movement
            // that recognized this pan: a coalesced drag can deliver began/end
            // at the same nonzero translation, without any changed callback.
            transformHold?.cancel(); transformOrigin = .zero
            transformHold = TransformDragHold(self, time: time)
        case .changed:
            guard let hold = transformHold else { return }
            if !hold.move(dx: Double(point.x - transformOrigin.x), dy: Double(point.y - transformOrigin.y), time: time) {
                hold.cancel(); transformHold = nil
            }
        case .ended, .cancelled, .failed:
            let previous = transformHold; transformHold = nil
            previous?.finish(dx: Double(point.x - transformOrigin.x), dy: Double(point.y - transformOrigin.y),
                time: time, cancel: pan.state != .ended)
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

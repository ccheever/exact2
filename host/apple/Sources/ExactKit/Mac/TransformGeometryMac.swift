#if os(macOS)
import AppKit

extension NodeView {
    package func transformDragModel() -> TransformDragPosition? {
        TransformDragPosition(x: Double(translate.x), y: Double(translate.y), scale: Double(scale))
    }
    package func transformDragPresentation() -> TransformDragPosition? {
        guard rotate == 0, let layer else { return nil }
        // The render tree's copy only while Core Animation runs a curve on it;
        // otherwise it may not have caught up with the last presented frame
        // (a pinch caught right after another read the one before).
        let matrix = (layer.animationKeys()?.isEmpty == false ? layer.presentation() ?? layer : layer).affineTransform()
        return TransformDragPosition(matrix: matrix, center: CGPoint(x: bounds.midX, y: bounds.midY))
    }
}
extension Presenter {
    func transformFacts(_ binding: TransformDragBinding) -> TransformGeometryFacts? {
        guard let handle = views[binding.id], let targetID = binding.target, let clipID = binding.clip,
              let target = views[targetID], let clip = views[clipID], target.superview === clip,
              let window = clip.window, handle.window === window, target.window === window,
              SwipeInput.allows(handle), SwipeInput.allows(target), SwipeInput.allows(clip) else { return nil }
        var ancestor: NSView? = handle
        while let view = ancestor {
            if view !== target {
                if let node = view as? NodeView, node.translate != .zero || node.scale != 1 || node.rotate != 0 { return nil }
                if let layer = view.layer {
                    if !layer.affineTransform().isIdentity || !(layer.presentation()?.affineTransform().isIdentity ?? true) { return nil }
                    if (layer.animationKeys() ?? []).contains(where: { $0.contains("transform") || $0.contains("position") || $0.contains("bounds") }) { return nil }
                }
            } else if target.rotate != 0 { return nil }
            ancestor = view.superview
        }
        return TransformGeometryFacts(targetBounds: target.bounds, targetFrame: target.frame,
            clipBounds: clip.bounds, windowOrigin: clip.convert(.zero, to: nil), supportedAncestors: true)
    }
}
#endif

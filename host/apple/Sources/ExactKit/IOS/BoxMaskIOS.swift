// What masks a node's layer on UIKit: CSS `clip-path`, an overflow clip
// with shaped corners (LLP 1076 D1: the corners Core Animation's radius
// cannot say), and CSS `mask-image` (LLP 1076 D2: a gradient's alpha over
// the border box). A filtered box's mask is its picture's (`BoxFilter`).
//
// A material's effect view takes the gradient on its own `mask` rather than
// under its node's layer, which UIKit requires of a `UIVisualEffectView`
// (masking an ancestor draws the effect wrong): the blur fades and the
// node's children, unlike CSS's, do not.
#if os(iOS)
import UIKit

extension NodeView {
    /// The layer's mask, rebuilt where the box's size is known (`display`).
    func applyBoxMask() {
        guard !hasBoxFilter else { return }
        let gradient = Gradient(style["mask_image"])
        let clip: CALayer? = clipPath != nil ? ClipPath.mask(clipPath, clipRule) : shapedClip()
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        if let effect = materialView {
            if let gradient {
                let view = (effect.mask as? GradientMaskView) ?? GradientMaskView()
                if view.frame != effect.bounds { view.frame = effect.bounds }
                gradient.apply(view.gradient, bounds: view.bounds, box: view.bounds, dark: drawsDark)
                if effect.mask !== view { effect.mask = view }
            } else if effect.mask is GradientMaskView {
                effect.mask = nil
            }
            if layer.mask !== clip { layer.mask = clip }
            return
        }
        guard let gradient else {
            if layer.mask !== clip { layer.mask = clip }
            return
        }
        let g = (layer.mask as? CAGradientLayer) ?? CAGradientLayer()
        if g.frame != layer.bounds { g.frame = layer.bounds }
        gradient.apply(g, bounds: g.bounds, box: g.bounds, dark: drawsDark)
        // Both: the clip masks the gradient that masks the layer.
        g.mask = clip
        if layer.mask !== g { layer.mask = g }
    }

    /// The border box's shaped outline when the node clips its overflow and
    /// its corners are shapes the layer's radius cannot say.
    private func shapedClip() -> CALayer? {
        guard clipsToBounds || clipBox != nil, let shape = CornerShape(style["corner_shape"]),
              !(shape.isAppleContinuous && layer.cornerRadius > 0) else { return nil }
        return ClipPath.mask(roundedPath(in: bounds).cgPath)
    }
}

/// A view whose layer is a gradient: a `UIVisualEffectView`'s mask.
final class GradientMaskView: UIView {
    override class var layerClass: AnyClass { CAGradientLayer.self }
    var gradient: CAGradientLayer { layer as! CAGradientLayer }
}
#endif

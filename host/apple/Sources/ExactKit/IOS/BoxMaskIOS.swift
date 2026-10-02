// What masks a node's layer on UIKit: CSS `clip-path`, an overflow clip
// with shaped corners (LLP 1076 D1: the corners Core Animation's radius
// cannot say), and CSS `mask-image` (LLP 1076 D2: a gradient's alpha over
// the border box). They compose: the gradient is masked by the clip, which
// is masked by the shaped corners. A filtered box's picture takes the same
// composition (`renderFilter`); its layer's own mask is the picture's hide.
//
// The shaped clip goes where the children are clipped: on their clip box
// when the node has one (an outer shadow lives outside it), else on the
// node's layer.
//
// A material's effect view takes the gradient on its own `mask` rather than
// under its node's layer, which UIKit requires of a `UIVisualEffectView`
// (masking an ancestor draws the effect wrong): the blur fades and the
// node's children, unlike CSS's, do not (declared in LLP 1001).
#if os(iOS)
import UIKit

extension NodeView {
    /// The layer's mask, rebuilt where the box's size is known (`display`).
    func applyBoxMask() {
        let shaped = shapedClip()
        if let box = clipBox?.layer, box.mask !== shaped { box.mask = clipBox != nil ? shaped : nil }
        guard !hasBoxFilter else { return }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        if let effect = materialView {
            if let gradient = Gradient(style["mask_image"]) {
                if gradient.isConic {
                    let view = UIImageView(frame: effect.bounds)
                    view.image = gradient.image(size: effect.bounds.size, scale: traitCollection.displayScale, dark: drawsDark).map { UIImage(cgImage: $0) }
                    effect.mask = view
                } else {
                    let view = (effect.mask as? GradientMaskView) ?? GradientMaskView()
                    if view.frame != effect.bounds { view.frame = effect.bounds }
                    gradient.apply(view.gradient, bounds: view.bounds, box: view.bounds, dark: drawsDark)
                    if effect.mask !== view { effect.mask = view }
                }
            } else if effect.mask != nil {
                effect.mask = nil
            }
            let clip = clipMask(shaped: clipBox == nil ? shaped : nil)
            if layer.mask !== clip { layer.mask = clip }
            return
        }
        let mask = composedMask(shaped: clipBox == nil ? shaped : nil)
        if layer.mask !== mask { layer.mask = mask }
    }

    /// `mask-image` over `clip-path` over the shaped corners, as one mask
    /// for the layer or a filtered box's picture.
    func composedMask(shaped: CALayer?) -> CALayer? {
        let clip = clipMask(shaped: shaped)
        guard let gradient = Gradient(style["mask_image"]) else { return clip }
        if gradient.isConic {
            // A conic mask is pixels (LLP 1076 D5).
            let m = ConicMaskLayer()
            m.frame = layer.bounds
            m.contents = gradient.image(size: layer.bounds.size, scale: traitCollection.displayScale, dark: drawsDark)
            m.mask = clip
            return m
        }
        let g = CAGradientLayer()
        g.frame = layer.bounds
        gradient.apply(g, bounds: g.bounds, box: g.bounds, dark: drawsDark)
        g.mask = clip
        return g
    }

    /// `clip-path`, masked by the shaped corners when both are there.
    private func clipMask(shaped: CALayer?) -> CALayer? {
        guard let path = clipPath else { return shaped }
        let clip = ClipPath.mask(path, clipRule)
        clip?.mask = shaped
        return clip
    }

    /// The border box's shaped outline when the node clips its overflow and
    /// its corners are shapes the layer's radius cannot say.
    func shapedClip() -> CALayer? {
        guard clipsToBounds || clipBox != nil, let shape = CornerShape(style["corner_shape"]),
              !(shape.isAppleContinuous && (clipBox?.layer.cornerRadius ?? layer.cornerRadius) > 0) else { return nil }
        return ClipPath.mask(roundedPath(in: bounds).cgPath)
    }
}

/// A view whose layer is a gradient: a `UIVisualEffectView`'s mask.
final class GradientMaskView: UIView {
    override class var layerClass: AnyClass { CAGradientLayer.self }
    var gradient: CAGradientLayer { layer as! CAGradientLayer }
}
#endif

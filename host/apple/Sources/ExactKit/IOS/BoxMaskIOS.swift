// What masks a node's layer on UIKit: CSS `clip-path`, an overflow clip
// with shaped corners (LLP 1077 D1: the corners Core Animation's radius
// cannot say), and CSS `mask-image` (LLP 1077 D2: a gradient's alpha over
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
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    /// The layer's mask, rebuilt where the box's size is known (`display`).
    func applyBoxMask() {
        // A layout transition's surface clips with a mask of its own
        // (`Surface.swift`) until it ends.
        guard surface == nil else { return }
        let outline = shapedOutline(), effect = materialView, image = style["mask_image"]
        // A material's mask view replaces the elliptical outline its radius
        // put on (`BorderPaint.clip`), so it carries that outline too.
        let effectOutline = effect == nil || image == nil ? nil : BorderPaint.uncircular(in: bounds, radii: cornerSizes(in: bounds))
        let key = BoxMaskState.Key(image: image, clip: style["clip_path"], outline: outline, size: bounds.size, dark: drawsDark,
                                   clipBox: clipBox != nil, filter: hasBoxFilter, material: effect != nil, materialOutline: effectOutline)
        let state = BoxMaskState.of(self)
        if state.key == key, clipBox?.layer.mask === state.boxMask, hasBoxFilter || layer.mask === state.layerMask,
           image == nil || effect?.layer.mask === (state.effectMask as? UIView)?.layer { return }
        state.key = key
        let shaped = ClipPath.mask(outline)
        if let box = clipBox?.layer { box.mask = shaped }
        state.boxMask = clipBox?.layer.mask
        guard !hasBoxFilter else { return }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        if let effect {
            if let gradient = Gradient(image) {
                let view: UIView
                if gradient.isConic {
                    let pixels = UIImageView(frame: effect.bounds)
                    pixels.image = gradient.image(size: effect.bounds.size, scale: traitCollection.displayScale, dark: drawsDark).map { UIImage(cgImage: $0) }
                    view = pixels
                } else {
                    let g = (effect.mask as? GradientMaskView) ?? GradientMaskView()
                    if g.frame != effect.bounds { g.frame = effect.bounds }
                    gradient.apply(g.gradient, bounds: g.bounds, box: g.bounds, dark: drawsDark)
                    view = g
                }
                view.layer.mask = ClipPath.mask(effectOutline)
                // `BorderPaint.clip` sets the layer's mask, not the view's.
                if effect.layer.mask !== view.layer { effect.mask = nil; effect.mask = view }
            } else if state.effectMask != nil {
                if effect.mask != nil { effect.mask = nil }
                applyMaterialRadius()
            }
            state.effectMask = effect.mask
            layer.mask = clipMask(shaped: clipBox == nil ? shaped : nil)
            state.layerMask = layer.mask
            return
        }
        state.effectMask = nil
        layer.mask = composedMask(shaped: clipBox == nil ? shaped : nil)
        state.layerMask = layer.mask
    }

    /// `mask-image` over `clip-path` over the shaped corners, as one mask
    /// for the layer or a filtered box's picture.
    func composedMask(shaped: CALayer?) -> CALayer? {
        let clip = clipMask(shaped: shaped)
        guard let gradient = Gradient(style["mask_image"]) else { return clip }
        if gradient.isConic {
            // A conic mask is pixels (LLP 1077 D5).
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

    /// The shaped corners masked by `clip-path` when both are there.
    private func clipMask(shaped: CALayer?) -> CALayer? {
        let authored = ClipPath.mask(clipPath, clipRule)
        guard let shaped else { return authored }
        shaped.mask = authored
        return shaped
    }

    /// The border box's outline when the node clips its overflow and the
    /// layer's radius cannot say it: shaped corners, or four equal elliptical
    /// ones (`ClipPath.swift`).
    func shapedClip() -> CALayer? { ClipPath.mask(shapedOutline()) }
    func shapedOutline() -> CGPath? {
        if clipsToBounds || clipBox != nil, let shape = CornerShape(style["corner_shape"]),
           !(shape.isAppleContinuous && (clipBox?.layer.cornerRadius ?? layer.cornerRadius) > 0) {
            return roundedPath(in: bounds).cgPath
        }
        return ellipticalClip
    }
}

/// A view whose layer is a gradient: a `UIVisualEffectView`'s mask.
final class GradientMaskView: UIView {
    override class var layerClass: AnyClass { CAGradientLayer.self }
    var gradient: CAGradientLayer { layer as! CAGradientLayer }
}
#endif

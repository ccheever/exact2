// What masks a node's layer on AppKit, as on UIKit (`IOS/BoxMaskIOS.swift`):
// CSS `clip-path`, an overflow clip with shaped corners (LLP 1076 D1), and
// CSS `mask-image` (LLP 1076 D2), composed, the shaped clip on the children's
// clip box when there is one. A filtered box's picture takes the same
// composition. A material's effect view takes the gradient as its
// `maskImage`, which AppKit composites with the effect: the blur fades and
// the node's children, unlike CSS's, do not.
#if os(macOS)
import AppKit

extension NodeView {
    /// The layer's mask, rebuilt where the box's size is known.
    func applyBoxMask() {
        // A layout transition's surface clips with a mask of its own
        // (`Surface.swift`) until it ends.
        guard surface == nil else { return }
        let shaped = shapedClip()
        if let box = clipBox?.layer, box.mask !== shaped { box.mask = shaped }
        guard !hasBoxFilter, let layer else { return }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        if let effect = materialView as? NSVisualEffectView {
            if let gradient = Gradient(style["mask_image"]) {
                let size = effect.bounds.size, dark = drawsDark
                effect.maskImage = NSImage(size: size, flipped: true) { rect in
                    if let ctx = NSGraphicsContext.current?.cgContext {
                        gradient.paint(ctx, clip: CGPath(rect: rect, transform: nil), box: rect, dark: dark)
                    }
                    return true
                }
            } else if effect.maskImage != nil {
                effect.maskImage = nil
            }
            let clip = clipMask(shaped: clipBox == nil ? shaped : nil)
            if layer.mask !== clip { layer.mask = clip }
            return
        }
        let mask = composedMask(shaped: clipBox == nil ? shaped : nil)
        if layer.mask !== mask { layer.mask = mask }
    }

    /// `mask-image` over `clip-path` over the shaped corners, as one mask.
    func composedMask(shaped: CALayer?) -> CALayer? {
        let clip = clipMask(shaped: shaped)
        guard let layer, let gradient = Gradient(style["mask_image"]) else { return clip }
        if gradient.isConic {
            // A conic mask is pixels (LLP 1076 D5).
            let m = ConicMaskLayer()
            m.frame = layer.bounds
            m.contents = gradient.image(size: layer.bounds.size, scale: window?.backingScaleFactor ?? 2, dark: drawsDark)
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
    func shapedClip() -> CALayer? {
        if clipsToBounds || clipBox != nil, let shape = CornerShape(style["corner_shape"]),
           !(shape.isAppleContinuous && ((clipBox?.layer ?? layer)?.cornerRadius ?? 0) > 0) {
            return ClipPath.mask(roundedPath(in: bounds).cgPath)
        }
        return ClipPath.mask(ellipticalClip)
    }
}
#endif

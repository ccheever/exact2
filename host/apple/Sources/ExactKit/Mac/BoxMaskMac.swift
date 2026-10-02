// What masks a node's layer on AppKit, as on UIKit (`IOS/BoxMaskIOS.swift`):
// CSS `clip-path`, an overflow clip with shaped corners (LLP 1076 D1), and
// CSS `mask-image` (LLP 1076 D2). A filtered box's mask is its picture's.
// A material's effect view takes the gradient as its `maskImage`, which
// AppKit composites with the effect: the blur fades and the node's
// children, unlike CSS's, do not.
#if os(macOS)
import AppKit

extension NodeView {
    /// The layer's mask, rebuilt where the box's size is known.
    func applyBoxMask() {
        guard !hasBoxFilter, let layer else { return }
        let gradient = Gradient(style["mask_image"])
        let clip: CALayer? = clipPath != nil ? ClipPath.mask(clipPath, clipRule) : shapedClip()
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        if let effect = materialView as? NSVisualEffectView {
            if let gradient {
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
            if layer.mask !== clip { layer.mask = clip }
            return
        }
        guard let gradient else {
            if layer.mask !== clip { layer.mask = clip }
            return
        }
        if gradient.isConic {
            // A conic mask is pixels (LLP 1076 D5).
            let m = (layer.mask as? ConicMaskLayer) ?? ConicMaskLayer()
            m.frame = layer.bounds
            m.contents = gradient.image(size: layer.bounds.size, scale: window?.backingScaleFactor ?? 2, dark: drawsDark)
            m.mask = clip
            if layer.mask !== m { layer.mask = m }
            return
        }
        let g = (layer.mask as? CAGradientLayer) ?? CAGradientLayer()
        if g.frame != layer.bounds { g.frame = layer.bounds }
        gradient.apply(g, bounds: g.bounds, box: g.bounds, dark: drawsDark)
        g.mask = clip
        if layer.mask !== g { layer.mask = g }
    }

    /// The border box's shaped outline when the node clips its overflow and
    /// its corners are shapes the layer's radius cannot say.
    private func shapedClip() -> CALayer? {
        guard clipsToBounds || clipBox != nil, let shape = CornerShape(style["corner_shape"]),
              !(shape.isAppleContinuous && (layer?.cornerRadius ?? 0) > 0) else { return nil }
        return ClipPath.mask(roundedPath(in: bounds).cgPath)
    }
}
#endif

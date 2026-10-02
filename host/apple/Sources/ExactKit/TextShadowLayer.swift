// CSS `text-shadow` on a paragraph's raster layer (LLP 1076 D3): Core
// Animation casts the glyphs' own alpha, past the layer's bounds as CSS's
// shadow reaches past the box. Without a `shadowPath` that is an offscreen
// pass per frame, so the layer is rasterized: the shadowed glyphs are kept
// until the raster changes.
import QuartzCore

enum TextShadowLayer {
    /// `shadow`: offset x, y, blur, r g b a (0–255), as `Spec.shadow`.
    static func apply(_ shadow: [Double]?, to layer: CALayer) {
        guard let s = shadow, s.count == 7 else {
            if layer.shadowOpacity != 0 { layer.shadowOpacity = 0; layer.shouldRasterize = false }
            return
        }
        layer.shadowColor = CGColor(srgbRed: s[3] / 255, green: s[4] / 255, blue: s[5] / 255, alpha: s[6] / 255)
        layer.shadowOpacity = 1
        layer.shadowOffset = CGSize(width: s[0], height: s[1])
        // Core Animation's radius is the Gaussian's σ; CSS's is twice it.
        layer.shadowRadius = s[2] / 2
        layer.shouldRasterize = true
        layer.rasterizationScale = layer.contentsScale
    }
}

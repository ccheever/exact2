// CSS `text-shadow` on a paragraph's raster layer (LLP 1077 D3): Core
// Animation casts the glyphs' own alpha, past the layer's bounds as CSS's
// shadow reaches past the box. Without a `shadowPath` that is an offscreen
// pass per frame, so the layer is rasterized: the shadowed glyphs are kept
// until the raster changes.
import QuartzCore

enum TextShadowLayer {
    /// `shadow`: offset x, y, blur, r g b a (0–255), as `Spec.shadow`.
    static func apply(_ shadow: [Double]?, to layer: CALayer) {
        guard let s = shadow, TextEngine.isShadow(s) else {
            if layer.shadowOpacity != 0 { layer.shadowOpacity = 0; layer.shouldRasterize = false }
            return
        }
        layer.shadowColor = TextEngine.shadowColor(s)
        layer.shadowOpacity = 1
        layer.shadowOffset = CGSize(width: s[0], height: s[1])
        // Core Animation's radius is the Gaussian's σ; CSS's is twice it.
        layer.shadowRadius = s[2] / 2
        layer.shouldRasterize = true
        layer.rasterizationScale = layer.contentsScale
    }
}

/// An HDR `text-shadow`'s bitmap, under its ink layer. Each layer is mapped to
/// the limit on its own, so under `standard` SDR ink stays white (LLP 1100 D8).
final class TextCastLayer: CALayer {
    override func action(forKey event: String) -> (any CAAction)? { nil }
}

extension CALayer {
    private static let castKey = "exactTextCast"
    var textCast: CALayer? { value(forKey: Self.castKey) as? CALayer }

    /// Shows `contents` under this ink layer, or removes the cast for nil.
    /// `rolling`: the transition the ink's new contents arrive with
    /// (`NumeralRoll`), which the shadow's then arrive with too.
    func applyTextCast(_ contents: Any?, headroom: Float, limit: String?, rolling: CAAnimation? = nil) {
        guard let contents, let superlayer else { dropTextCast(); return }
        let cast = textCast ?? TextCastLayer()
        if textCast == nil { setValue(cast, forKey: Self.castKey) }
        if cast.superlayer !== superlayer { superlayer.insertSublayer(cast, below: self) }
        cast.frame = frame
        cast.contentsScale = contentsScale
        cast.contentsGravity = contentsGravity
        if let rolling { cast.add(rolling, forKey: kCATransition) }
        cast.contents = contents
        cast.applyTextRange(headroom: headroom, limit: limit)
    }

    func dropTextCast() {
        textCast?.removeFromSuperlayer()
        setValue(nil, forKey: Self.castKey)
    }
}

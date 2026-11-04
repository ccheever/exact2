// A new layer with another's own values, for Core Animation to composite
// again: a capture draws it (`PaintCapture.swift`), a lifted run's picture
// commits it (`ContextMenusIOS.swift`). Each adds what its use needs.
import QuartzCore

extension CALayer {
    /// A new layer with this one's geometry, look, contents (shared) and
    /// shadow: a shape, gradient or transform layer with its own values where
    /// this is one, else a plain layer. Not its sublayers, mask, `zPosition`,
    /// visibility or animations.
    /// `CALayer(layer:)` is the subclass copy initializer Core Animation uses;
    /// called directly, it gives an empty layer, so the values go one by one.
    func valueCopy() -> CALayer {
        let layer: CALayer
        if let shape = self as? CAShapeLayer {
            let out = CAShapeLayer()
            out.path = shape.path; out.fillColor = shape.fillColor; out.fillRule = shape.fillRule
            out.strokeColor = shape.strokeColor; out.lineWidth = shape.lineWidth
            out.lineCap = shape.lineCap; out.lineJoin = shape.lineJoin; out.miterLimit = shape.miterLimit
            out.lineDashPattern = shape.lineDashPattern; out.lineDashPhase = shape.lineDashPhase
            out.strokeStart = shape.strokeStart; out.strokeEnd = shape.strokeEnd
            layer = out
        } else if let gradient = self as? CAGradientLayer {
            let out = CAGradientLayer()
            out.colors = gradient.colors; out.locations = gradient.locations; out.type = gradient.type
            out.startPoint = gradient.startPoint; out.endPoint = gradient.endPoint
            layer = out
        } else { layer = self is CATransformLayer ? CATransformLayer() : CALayer() }
        layer.bounds = bounds; layer.position = position
        layer.anchorPoint = anchorPoint; layer.anchorPointZ = anchorPointZ
        layer.transform = transform; layer.sublayerTransform = sublayerTransform
        layer.opacity = opacity
        layer.isDoubleSided = isDoubleSided; layer.masksToBounds = masksToBounds
        layer.cornerRadius = cornerRadius; layer.maskedCorners = maskedCorners
        layer.cornerCurve = cornerCurve
        layer.backgroundColor = backgroundColor
        layer.borderColor = borderColor; layer.borderWidth = borderWidth
        layer.contents = contents; layer.contentsScale = contentsScale
        layer.contentsRect = contentsRect; layer.contentsCenter = contentsCenter
        layer.contentsGravity = contentsGravity
        layer.minificationFilter = minificationFilter; layer.magnificationFilter = magnificationFilter
        layer.shadowPath = shadowPath; layer.shadowColor = shadowColor
        layer.shadowOffset = shadowOffset; layer.shadowRadius = shadowRadius
        layer.shadowOpacity = shadowOpacity
        layer.allowsGroupOpacity = allowsGroupOpacity
        return layer
    }
}

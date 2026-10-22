#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    /// Mask only the fill: CSS's visible overflow and outer shadows extend beyond the corners.
    func applyCornerFill(_ color: CGColor?) {
        guard let color else {
            boxFill?.removeFromSuperlayer()
            boxFill = nil
            return
        }
        let fill = boxFill ?? CALayer()
        if boxFill == nil {
            boxFill = fill
            fill.mask = CAShapeLayer()
            insertBoxSublayer(fill)
        }
        if fill.frame != bounds { fill.frame = bounds }
        if fill.backgroundColor != color { fill.backgroundColor = color }
        if let mask = fill.mask as? CAShapeLayer {
            let path = roundedPath(in: bounds).cgPath
            if mask.path != path { mask.path = path }
        }
    }
}
#endif

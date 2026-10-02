// CSS's 3D transforms on both Apple platforms (LLP 1076 D8): `rotate` about
// an axis, `translate`'s z, a parent's `perspective` about its
// `perspective-origin`, and `backface-visibility`. Core Animation's layer
// transform is CSS's matrix (WebKit draws CSS 3D with it); its sublayers
// flatten into its plane, which is CSS's initial `transform-style: flat`.
import QuartzCore
#if os(iOS)
import UIKit
#else
import AppKit
#endif

extension NodeView {
    /// The node's transform in space when it has one — a rotation out of the
    /// screen's plane or a z translation — about `origin` (from the layer's
    /// anchor), else nil and the 2D path applies. `shift` is what a lifted
    /// Arrange row already moved by its frame (macOS).
    func spaceTransform(origin d: CGPoint, scale s: CGFloat, shift: CGPoint = .zero) -> CATransform3D? {
        let axis = style["rotate_axis"]?.numbers ?? [0, 0, 1]
        let z = number("translate_z")
        guard axis.count == 3, (axis[0] != 0 || axis[1] != 0 || z != 0) else { return nil }
        var t = CATransform3DMakeTranslation(translate.x - shift.x + d.x, translate.y - shift.y + d.y, z)
        t = CATransform3DRotate(t, rotate * .pi / 180, axis[0], axis[1], axis[2])
        t = CATransform3DScale(t, s, s, 1)
        return CATransform3DTranslate(t, -d.x, -d.y, 0)
    }

    /// The 3D rows (LLP 1076 D8) go on again when any of them changed.
    func applySpace(changedFrom old: NodeStyle) {
        let rows = ["rotate_axis", "translate_z", "perspective", "perspective_origin", "backface_visibility"]
        if rows.contains(where: { style[$0] != old[$0] }) { applyTransform(); applyPerspective() }
    }

    /// `perspective` on the box that holds the children, about
    /// `perspective-origin`, and `backface-visibility` on the box itself.
    func applyPerspective() {
        #if os(iOS)
        let own: CALayer? = layer
        let holder: CALayer? = container.layer
        #else
        let own = layer
        let holder = container.layer
        #endif
        let hidden = style["backface_visibility"]?.string == "hidden"
        if own?.isDoubleSided == hidden { own?.isDoubleSided = !hidden }
        guard let holder else { return }
        let d = number("perspective")
        guard d > 0 else {
            if !CATransform3DIsIdentity(holder.sublayerTransform) { holder.sublayerTransform = CATransform3DIdentity }
            return
        }
        // The vanishing point, from the holder's anchor.
        let axes = style["perspective_origin"]?.array ?? []
        func axis(_ i: Int, _ size: CGFloat) -> CGFloat {
            guard axes.count == 2 else { return size / 2 }
            if let points = axes[i].number { return points }
            if case .object(let o) = axes[i], let pct = o["pct"]?.number { return size * pct / 100 }
            return size / 2
        }
        let b = holder.bounds
        let anchor = CGPoint(x: b.minX + b.width * holder.anchorPoint.x, y: b.minY + b.height * holder.anchorPoint.y)
        let o = CGPoint(x: axis(0, bounds.width) - anchor.x, y: axis(1, bounds.height) - anchor.y)
        var p = CATransform3DIdentity
        p.m34 = -1 / d
        let t = CATransform3DConcat(CATransform3DConcat(CATransform3DMakeTranslation(-o.x, -o.y, 0), p), CATransform3DMakeTranslation(o.x, o.y, 0))
        if !CATransform3DEqualToTransform(holder.sublayerTransform, t) { holder.sublayerTransform = t }
    }
}

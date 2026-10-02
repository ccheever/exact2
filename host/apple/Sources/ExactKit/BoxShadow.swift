// CSS `box-shadow`, one outer shadow, on both Apple platforms (LLP 1064 D2).
// A layer with no contents casts the border box's rounded outline through
// `shadowPath` (no offscreen pass to find a shape), masked to what lies
// outside that outline: CSS clips an outer shadow to outside the border box,
// so it never shows through a translucent background. It goes in as the
// node's lowest sublayer, under its children, as CSS paints it; what else
// the node draws is inside the outline. A node that clips its overflow
// clips its children in a box of their own instead (`clipBox`), since
// clipping the node's layer would clip the shadow too. Core Animation's
// `shadowRadius` is a Gaussian's standard deviation; CSS's blur radius is
// twice that.
import CoreGraphics
import QuartzCore
#if os(iOS)
import UIKit
#else
import AppKit
#endif

final class ShadowCaster: CALayer {
    private let outside = CAShapeLayer()

    override init() {
        super.init()
        outside.fillRule = .evenOdd
        mask = outside
    }
    override init(layer: Any) { super.init(layer: layer) }
    required init?(coder: NSCoder) { nil }

    /// `box` and `outline` in the host layer's coordinates.
    func cast(box: CGRect, outline: CGPath, color: CGColor, offset: CGSize, blur: CGFloat) {
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        // Three deviations past the offset box: all the shadow there is.
        let reach = 1.5 * blur + max(abs(offset.width), abs(offset.height)) + 1
        let extent = box.insetBy(dx: -reach, dy: -reach)
        if frame != extent { frame = extent }
        var local = CGAffineTransform(translationX: -extent.minX, y: -extent.minY)
        let path = outline.copy(using: &local)
        shadowPath = path
        if shadowColor != color { shadowColor = color }
        if shadowOffset != offset { shadowOffset = offset }
        if shadowRadius != blur / 2 { shadowRadius = blur / 2 }
        if shadowOpacity != 1 { shadowOpacity = 1 }
        let hole = CGMutablePath()
        hole.addRect(CGRect(origin: .zero, size: extent.size))
        if let path { hole.addPath(path) }
        outside.frame = CGRect(origin: .zero, size: extent.size)
        outside.path = hole
    }
}

extension NodeView {
    /// The shadow's colour with its opacity row folded in, resolved for the
    /// view's appearance; nil when nothing would show.
    var shadowColor: CGColor? {
        // Paint motion's colour has the opacity in its alpha already (LLP 1062).
        let opacity = min(number("shadow_opacity"), 1)
        guard opacity > 0, let c = channels("shadow_color"), c[3] > 0 else { return nil }
        return CGColor(srgbRed: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255 * opacity)
    }

    /// The offset and CSS blur radius.
    var shadowGeometry: (x: CGFloat, y: CGFloat, blur: CGFloat) {
        let o = style["shadow_offset"]?.numbers ?? []
        return (o.first ?? 0, o.count > 1 ? o[1] : 0, max(0, number("shadow_radius")))
    }

    /// The caster onto the layer's bottom, cast from the border box, or gone.
    func applyShadow(outline: CGPath) {
        #if os(iOS)
        let host: CALayer? = layer
        #else
        let host = layer
        #endif
        guard let host, surface == nil, let color = shadowColor, bounds.width > 0, bounds.height > 0 else {
            shadowCaster?.removeFromSuperlayer(); shadowCaster = nil; return
        }
        let caster = shadowCaster ?? ShadowCaster()
        if caster.superlayer !== host { host.insertSublayer(caster, at: 0) }
        shadowCaster = caster
        let g = shadowGeometry
        caster.cast(box: bounds, outline: outline, color: color, offset: CGSize(width: g.x, height: g.y), blur: g.blur)
    }
}

extension NodeView {
    #if os(iOS)
    private typealias ClipBox = PlainView
    #else
    /// The shadow at the node's current size.
    func applyShadow() {
        applyShadow(outline: roundedPath(in: bounds).cgPath)
    }

    /// A capture (`cacheDisplay`, the agent's screenshot and a canvas's
    /// surface) draws views, not sublayers: the shadow drawn as the caster
    /// shows it, before the box. Core Graphics' blur is CSS's radius; its
    /// offset is in the unflipped base space.
    func drawCapturedShadow(_ ctx: CGContext) {
        guard Capture.capturing, shadowCaster != nil, let color = shadowColor else { return }
        let outline = roundedPath(in: bounds).cgPath
        let (x, y, blur) = shadowGeometry
        let outside = CGMutablePath()
        outside.addRect(bounds.insetBy(dx: -(1.5 * blur + abs(x) + abs(y) + 1), dy: -(1.5 * blur + abs(x) + abs(y) + 1)))
        outside.addPath(outline)
        ctx.saveGState()
        ctx.addPath(outside)
        ctx.clip(using: .evenOdd)
        ctx.setShadow(offset: CGSize(width: x, height: -y), blur: blur, color: color)
        ctx.addPath(outline)
        ctx.setFillColor(CGColor(gray: 0, alpha: 1))
        ctx.fillPath()
        ctx.restoreGState()
    }

    /// Flipped, and never a hit target of its own.
    private final class ClipBox: NSView {
        override var isFlipped: Bool { true }
        override func hitTest(_ point: NSPoint) -> NSView? {
            let hit = super.hitTest(point)
            return hit === self ? nil : hit
        }
    }
    #endif

    /// The children into a clipping box of their own, or back out of it, as
    /// a scroll view takes them.
    func syncClipBox(_ wanted: Bool) {
        if wanted, clipBox == nil {
            let box = ClipBox(frame: bounds)
            #if os(iOS)
            box.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            #else
            box.autoresizingMask = [.width, .height]
            box.wantsLayer = true
            #endif
            box.clipsToBounds = true
            GlassGroups.moving(in: self) {
                for child in subviews where child is NodeView || child === glassGroupView { child.removeFromSuperview(); box.addSubview(child) }
                addSubview(box)
            }
            clipBox = box
        } else if !wanted, let box = clipBox {
            GlassGroups.moving(in: self) {
                for child in box.subviews where child is NodeView || child === glassGroupView { child.removeFromSuperview(); addSubview(child) }
                box.removeFromSuperview()
            }
            clipBox = nil
        }
    }
}

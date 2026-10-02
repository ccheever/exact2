// A layout transition's size (LLP 1063) is the box's surface alone, as a
// UIKit or SwiftUI frame animation shows it: the background, border, corner
// radius, shadow and the clip it puts on its children take the size the
// engine shows, from the box's top-left corner, while the view keeps its
// laid-out frame, so its content and children stay at their final geometry
// and move only by their own transitions. A growing card reveals its title;
// it never squashes it.
//
// While the size differs from the laid-out one the node paints no surface of
// its own (`surface != nil` turns its box, gradient and shadow off); a
// `SurfaceLayer` under everything the node holds paints it at the shown size
// — Core Animation's own properties when it can say the box (one border, one
// radius, no gradient), else drawn, as the node's `draw(_:)` draws it — and a
// node that clips its children clips them to the shown box through a mask.
#if os(iOS)
import UIKit
#else
import AppKit
#endif

final class SurfaceLayer: CALayer {
    /// A box Core Animation cannot say, drawn at the layer's size.
    var drawn: ((CGContext, CGRect) -> Void)?
    let caster = ShadowCaster()
    /// The children's clip at the shown size, and the layer it masks.
    let clip = CAShapeLayer()
    weak var clipped: CALayer?
    /// The node's own clip, turned off while this one stands in for it.
    var clippedSelf = false

    override init() {
        super.init()
        needsDisplayOnBoundsChange = true
        addSublayer(caster)
    }
    override init(layer: Any) { super.init(layer: layer) }
    required init?(coder: NSCoder) { nil }

    override func draw(in ctx: CGContext) {
        // The node's painters draw from the top-left corner.
        if ctx.ctm.d > 0 { ctx.translateBy(x: 0, y: bounds.height); ctx.scaleBy(x: 1, y: -1) }
        drawn?(ctx, bounds)
    }
}

extension NodeView {
    private var hostLayer: CALayer? { layer }

    /// The surface at `layoutScale` of the laid-out size; none at one.
    func applySurface() {
        guard layoutScale != CGPoint(x: 1, y: 1), let host = hostLayer else { return endSurface() }
        let started = surface == nil
        let s = surface ?? SurfaceLayer()
        surface = s
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        if host.sublayers?.first !== s { host.insertSublayer(s, at: 0) }
        let box = CGRect(x: 0, y: 0, width: max(0, bounds.width * layoutScale.x), height: max(0, bounds.height * layoutScale.y))
        if s.frame != box { s.frame = box }
        #if os(iOS)
        s.contentsScale = traitCollection.displayScale
        #else
        s.contentsScale = window?.backingScaleFactor ?? 2
        #endif
        paintSurface(s, box)
        let outline = roundedPath(in: box).cgPath
        clipSurface(s, outline)
        if started { surfaceChanged() }
    }

    /// The node paints its own surface again at its laid-out size.
    func endSurface() {
        guard let s = surface else { return }
        surface = nil
        CATransaction.begin(); CATransaction.setDisableActions(true)
        if s.clipped?.mask === s.clip { s.clipped?.mask = nil }
        if s.clippedSelf { clipsToBounds = true }
        s.removeFromSuperlayer()
        CATransaction.commit()
        surfaceChanged()
    }

    private func surfaceChanged() {
        applyShadow(outline: roundedPath(in: bounds).cgPath)
        #if os(iOS)
        applyBoxLayer()
        applyGradientLayer()
        setNeedsDisplay()
        #else
        needsDisplay = true
        #endif
    }

    /// The box as the node paints it, at `box`: paint motion's values over
    /// its rows, read afresh on every frame the size moves.
    private func paintSurface(_ s: SurfaceLayer, _ box: CGRect) {
        let sides = ["top", "right", "bottom", "left"]
        let uniform = number("border_width")
        let widths = sides.map { number("border_width_" + $0, uniform) }
        let top = color("border_color_top", .clear)
        let colors = sides.map { color("border_color_" + $0, top).cgColor }
        let sizes = BorderPaint.radii(style, in: box)
        let radii = sizes.map { $0.width }
        let fill = channels("background_color").map { CGColor(srgbRed: $0[0] / 255, green: $0[1] / 255, blue: $0[2] / 255, alpha: $0[3] / 255) }
        let gradient = Gradient(style["background_image"])
        let radius = radii.max() ?? 0
        let said = gradient == nil && sizes.allSatisfy { $0.width == $0.height }
            && widths.allSatisfy { $0 == widths[0] } && (widths[0] == 0 || colors.allSatisfy { $0 == colors[0] })
            && radii.allSatisfy { $0 == 0 || $0 == radius } && radius <= min(box.width, box.height) / 2
        if said {
            var corners: CACornerMask = []
            let masks: [CACornerMask] = [.layerMinXMinYCorner, .layerMaxXMinYCorner, .layerMaxXMaxYCorner, .layerMinXMaxYCorner]
            for (r, mask) in zip(radii, masks) where r > 0 { corners.insert(mask) }
            s.drawn = nil; s.contents = nil
            s.backgroundColor = fill
            s.cornerRadius = radius
            if radius > 0 { s.maskedCorners = corners }
            s.borderWidth = widths[0]
            s.borderColor = colors[0]
        } else {
            s.backgroundColor = nil; s.cornerRadius = 0; s.borderWidth = 0
            let dark = drawsDark
            s.drawn = { [weak self] ctx, rect in
                guard let self else { return }
                let path = self.roundedPath(in: rect).cgPath
                if let fill { ctx.addPath(path); ctx.setFillColor(fill); ctx.fillPath() }
                gradient?.paint(ctx, clip: path, box: rect.insetBy(left: widths[3], top: widths[0], right: widths[1], bottom: widths[2]), dark: dark)
                BorderPaint.paint(ctx, box: rect, widths: widths, colors: colors, radii: sizes)
            }
            s.setNeedsDisplay()
        }
        if let color = shadowColor {
            let g = shadowGeometry
            s.caster.isHidden = false
            s.caster.cast(box: box, outline: roundedPath(in: box).cgPath, color: color, offset: CGSize(width: g.x, height: g.y), blur: g.blur)
        } else {
            s.caster.isHidden = true
        }
    }

    /// A node that clips its children clips them to the shown box: its own
    /// clip is lifted (the surface may be larger than its frame as it
    /// shrinks) and a mask stands in; a box of its own that holds them is
    /// masked instead, keeping the node's shadow unclipped.
    private func clipSurface(_ s: SurfaceLayer, _ outline: CGPath) {
        s.clip.path = outline
        let scroller: CALayer? = scroll?.layer
        let target: CALayer?
        if let box = clipBox?.layer { target = box }
        else if let scroller { target = scroller }
        else if clipsToBounds, hostLayer?.mask == nil || hostLayer?.mask === s.clip {
            clipsToBounds = false
            s.clippedSelf = true
            target = hostLayer
        } else if s.clippedSelf { target = hostLayer } else { target = nil }
        guard let target else { return }
        // In the masked layer's own coordinates: a scroll view's are scrolled.
        s.clip.frame = CGRect(origin: target === hostLayer ? .zero : CGPoint(x: -target.frame.minX + target.bounds.minX, y: -target.frame.minY + target.bounds.minY), size: bounds.size)
        if target.mask !== s.clip { target.mask = s.clip }
        s.clipped = target
    }
}

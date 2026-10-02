// CSS `box-shadow` on both Apple platforms (LLP 1064 D2, LLP 1077 D4): a
// list of outer and inset shadows with spread, the first on top. An outer
// shadow is a layer with no contents casting the border box's outline,
// grown by the spread, through `shadowPath` (no offscreen pass to find a
// shape); they share one container masked to what lies outside the border
// box, since CSS clips an outer shadow there, so it never shows through a
// translucent background. The container is the node's lowest sublayer,
// under its children, as CSS paints it. An inset shadow casts a frame
// whose hole is the padding box shrunk by the spread, inward; they share a
// container masked to the padding box, over the background and under the
// border and children. A node that clips its overflow clips its children
// in a box of their own instead (`clipBox`), since clipping the node's
// layer would clip the shadow too. Core Animation's `shadowRadius` is a
// Gaussian's standard deviation; CSS's blur radius is twice that.
import CoreGraphics
import QuartzCore
#if os(iOS)
import UIKit
#else
import AppKit
#endif

/// One shadow of a `box-shadow` list, resolved for an appearance.
struct BoxShadowSpec: Equatable {
    var color: CGColor
    var offset: CGSize
    var blur: CGFloat
    var spread: CGFloat
    var inset: Bool

    /// The row's list (`style.rs`), the colours for `dark`; transparent
    /// shadows left out.
    static func list(_ value: BatchValue?, dark: Bool) -> [BoxShadowSpec] {
        (value?.array ?? []).compactMap { item in
            guard case .object(let o) = item, let off = o["o"]?.numbers, off.count == 2 else { return nil }
            var c = o["c"]?.numbers
            if c == nil, let pair = o["c"]?.array, pair.count == 2 { c = pair[dark ? 1 : 0].numbers }
            guard let c, c.count == 4, c[3] > 0 else { return nil }
            return BoxShadowSpec(color: CGColor(srgbRed: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255),
                                 offset: CGSize(width: off[0], height: off[1]), blur: max(0, CGFloat(o["b"]?.number ?? 0)),
                                 spread: CGFloat(o["s"]?.number ?? 0), inset: o["i"] != nil)
        }
    }

    /// How far past the shape it reaches: three deviations, the offset and
    /// the spread.
    var reach: CGFloat { 1.5 * blur + max(abs(offset.width), abs(offset.height)) + abs(spread) + 1 }
}

/// The outer shadows: a container masked to outside the border box, one
/// casting sublayer per shadow.
final class ShadowCaster: CALayer {
    private let outside = CAShapeLayer()

    override init() {
        super.init()
        outside.fillRule = .evenOdd
        mask = outside
    }
    override init(layer: Any) { super.init(layer: layer) }
    required init?(coder: NSCoder) { nil }

    /// One outer shadow (the GPU capture and the tests read the first).
    func cast(box: CGRect, outline: CGPath, color: CGColor, offset: CGSize, blur: CGFloat) {
        cast(box: box, outline: outline, shadows: [(outline, BoxShadowSpec(color: color, offset: offset, blur: blur, spread: 0, inset: false))])
    }

    /// `box` and the paths in the host layer's coordinates: `outline` the
    /// border box's, each shadow's shape its outline grown by its spread.
    func cast(box: CGRect, outline: CGPath, shadows: [(shape: CGPath, spec: BoxShadowSpec)]) {
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let reach = shadows.map(\.spec.reach).max() ?? 1
        let extent = box.insetBy(dx: -reach, dy: -reach)
        if frame != extent { frame = extent }
        var local = CGAffineTransform(translationX: -extent.minX, y: -extent.minY)
        // The first shadow on top: the last sublayer.
        var casts = sublayers ?? []
        while casts.count > shadows.count { casts.removeLast().removeFromSuperlayer() }
        while casts.count < shadows.count { let l = CALayer(); addSublayer(l); casts.append(l) }
        for (layer, (shape, spec)) in zip(casts, shadows.reversed()) {
            if layer.frame != bounds { layer.frame = bounds }
            layer.shadowPath = shape.copy(using: &local)
            if layer.shadowColor != spec.color { layer.shadowColor = spec.color }
            if layer.shadowOffset != spec.offset { layer.shadowOffset = spec.offset }
            if layer.shadowRadius != spec.blur / 2 { layer.shadowRadius = spec.blur / 2 }
            if layer.shadowOpacity != 1 { layer.shadowOpacity = 1 }
        }
        let hole = CGMutablePath()
        hole.addRect(CGRect(origin: .zero, size: extent.size))
        if let path = outline.copy(using: &local) { hole.addPath(path) }
        outside.frame = CGRect(origin: .zero, size: extent.size)
        outside.path = hole
    }

    /// The first shadow's colour, as a single caster's was (tests, captures).
    var firstColor: CGColor? { sublayers?.last?.shadowColor }
}

/// The inset shadows: a container masked to the padding box, one sublayer
/// per shadow casting a frame inward (LLP 1077 D4).
final class InsetShadowCaster: CALayer {
    private let inside = CAShapeLayer()

    override init() {
        super.init()
        mask = inside
    }
    override init(layer: Any) { super.init(layer: layer) }
    required init?(coder: NSCoder) { nil }

    /// `padding` the padding box's outline; each shadow's hole the padding
    /// box shrunk by its spread, all in the host layer's coordinates.
    func cast(box: CGRect, padding: CGPath, shadows: [(hole: CGPath, spec: BoxShadowSpec)]) {
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        if frame != box { frame = box }
        var local = CGAffineTransform(translationX: -box.minX, y: -box.minY)
        inside.frame = bounds
        inside.path = padding.copy(using: &local)
        var casts = sublayers ?? []
        while casts.count > shadows.count { casts.removeLast().removeFromSuperlayer() }
        while casts.count < shadows.count { let l = CALayer(); addSublayer(l); casts.append(l) }
        for (layer, (hole, spec)) in zip(casts, shadows.reversed()) {
            if layer.frame != bounds { layer.frame = bounds }
            // A frame around the hole, far enough out that its blur never
            // shows an outer edge; Core Animation fills a shadow path by the
            // non-zero rule, so the hole runs the other way round.
            let frame = CGMutablePath()
            frame.addRect(bounds.insetBy(dx: -spec.reach - bounds.width, dy: -spec.reach - bounds.height))
            if let h = hole.copy(using: &local) { frame.addPath(BoxShadowSpec.reversed(h)) }
            layer.shadowPath = frame
            if layer.shadowColor != spec.color { layer.shadowColor = spec.color }
            if layer.shadowOffset != spec.offset { layer.shadowOffset = spec.offset }
            if layer.shadowRadius != spec.blur / 2 { layer.shadowRadius = spec.blur / 2 }
            if layer.shadowOpacity != 1 { layer.shadowOpacity = 1 }
        }
    }
}

extension BoxShadowSpec {
    /// A closed path of lines and cubics, traced the other way round.
    static func reversed(_ path: CGPath) -> CGPath {
        // Each segment as its start, controls and end.
        var segments: [[CGPoint]] = [], start = CGPoint.zero, current = CGPoint.zero
        path.applyWithBlock { e in
            let p = e.pointee.points
            switch e.pointee.type {
            case .moveToPoint: start = p[0]; current = p[0]
            case .addLineToPoint: segments.append([current, p[0]]); current = p[0]
            case .addQuadCurveToPoint: segments.append([current, p[0], p[1]]); current = p[1]
            case .addCurveToPoint: segments.append([current, p[0], p[1], p[2]]); current = p[2]
            case .closeSubpath: if current != start { segments.append([current, start]) }; current = start
            @unknown default: break
            }
        }
        let out = CGMutablePath()
        guard let last = segments.last?.last else { return out }
        out.move(to: last)
        for s in segments.reversed() {
            switch s.count {
            case 2: out.addLine(to: s[0])
            case 3: out.addQuadCurve(to: s[0], control: s[1])
            default: out.addCurve(to: s[0], control1: s[2], control2: s[1])
            }
        }
        out.closeSubpath()
        return out
    }

    /// A corner radius grown by a spread (CSS Backgrounds 3 §7.1.1): a square
    /// corner stays square, and a radius smaller than the spread grows less.
    static func spreadRadius(_ r: CGFloat, _ spread: CGFloat) -> CGFloat {
        if r <= 0 { return 0 }
        if spread < 0 { return max(0, r + spread) }
        let ratio = r / spread
        return r + spread * (ratio >= 1 ? 1 : 1 + pow(ratio - 1, 3))
    }
}

extension NodeView {
    /// The `box-shadow` list resolved for the view's appearance.
    var boxShadows: [BoxShadowSpec] { BoxShadowSpec.list(style["box_shadow"], dark: drawsDark) }

    /// The first outer shadow's colour; nil when there is none.
    var shadowColor: CGColor? { boxShadows.first { !$0.inset }?.color }

    /// The first outer shadow's offset and CSS blur radius.
    var shadowGeometry: (x: CGFloat, y: CGFloat, blur: CGFloat) {
        guard let s = boxShadows.first(where: { !$0.inset }) else { return (0, 0, 0) }
        return (s.offset.width, s.offset.height, s.blur)
    }

    /// The casters onto the layer: the outer ones at its bottom, cast from
    /// the border box; the inset ones over the box's paint; or gone.
    func applyShadow(outline: CGPath) {
        #if os(iOS)
        let host: CALayer? = layer
        #else
        let host = layer
        #endif
        let all = surface == nil && bounds.width > 0 && bounds.height > 0 ? boxShadows : []
        let sizes = BorderPaint.reduced(BorderPaint.radii(style, in: bounds), in: bounds)
        let shape = CornerShape(style["corner_shape"])
        let outer = all.filter { !$0.inset }
        if let host, !outer.isEmpty {
            let caster = shadowCaster ?? ShadowCaster()
            if caster.superlayer !== host { host.insertSublayer(caster, at: 0) }
            shadowCaster = caster
            caster.cast(box: bounds, outline: outline, shadows: outer.map { spec in
                guard spec.spread != 0 else { return (outline, spec) }
                let grown = bounds.insetBy(dx: -spec.spread, dy: -spec.spread)
                let radii = sizes.map { CGSize(width: BoxShadowSpec.spreadRadius($0.width, spec.spread),
                                                height: BoxShadowSpec.spreadRadius($0.height, spec.spread)) }
                return (BorderPaint.roundedRect(grown, radii, shape: shape), spec)
            })
        } else {
            shadowCaster?.removeFromSuperlayer(); shadowCaster = nil
        }
        let inner = all.filter(\.inset)
        guard let host, !inner.isEmpty else { insetCaster?.removeFromSuperlayer(); insetCaster = nil; return }
        let uniform = number("border_width")
        let w = ["top", "right", "bottom", "left"].map { number("border_width_" + $0, uniform) }
        let padding = CGRect(x: w[3], y: w[0], width: max(0, bounds.width - w[1] - w[3]), height: max(0, bounds.height - w[0] - w[2]))
        let padRadii = [CGSize(width: sizes[0].width - w[3], height: sizes[0].height - w[0]),
                        CGSize(width: sizes[1].width - w[1], height: sizes[1].height - w[0]),
                        CGSize(width: sizes[2].width - w[1], height: sizes[2].height - w[2]),
                        CGSize(width: sizes[3].width - w[3], height: sizes[3].height - w[2])]
            .map { CGSize(width: max(0, $0.width), height: max(0, $0.height)) }
        let caster = insetCaster ?? InsetShadowCaster()
        insetCaster = caster
        if caster.superlayer !== host { insertInsetCaster(caster, into: host) }
        caster.cast(box: bounds, padding: BorderPaint.roundedRect(padding, padRadii, shape: shape), shadows: inner.map { spec in
            let hole = padding.insetBy(dx: spec.spread, dy: spec.spread)
            let radii = padRadii.map { CGSize(width: max(0, $0.width - spec.spread), height: max(0, $0.height - spec.spread)) }
            return (hole.width > 0 && hole.height > 0 ? BorderPaint.roundedRect(hole, radii, shape: shape) : CGMutablePath(), spec)
        })
    }

    /// The inset casters over the box's background and gradient, under its
    /// border, image, text and children.
    private func insertInsetCaster(_ caster: CALayer, into host: CALayer) {
        #if os(macOS)
        insertBoxSublayer(caster)
        #else
        let below = [boxGradient, shadowCaster].compactMap { $0 }.first { $0.superlayer === host }
        if let below { host.insertSublayer(caster, above: below) } else { host.insertSublayer(caster, at: 0) }
        #endif
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
        guard Capture.capturing, shadowCaster != nil || insetCaster != nil else { return }
        let outline = roundedPath(in: bounds).cgPath
        let sizes = BorderPaint.reduced(BorderPaint.radii(style, in: bounds), in: bounds)
        let shape = CornerShape(style["corner_shape"])
        // The last first, so the first is on top, as the live casters stack.
        for spec in boxShadows.reversed() where !spec.inset {
            let (x, y, blur) = (spec.offset.width, spec.offset.height, spec.blur)
            let grown = bounds.insetBy(dx: -spec.spread, dy: -spec.spread)
            let radii = sizes.map { CGSize(width: BoxShadowSpec.spreadRadius($0.width, spec.spread),
                                            height: BoxShadowSpec.spreadRadius($0.height, spec.spread)) }
            let cast = spec.spread == 0 ? outline : BorderPaint.roundedRect(grown, radii, shape: shape)
            let outside = CGMutablePath()
            outside.addRect(bounds.insetBy(dx: -spec.reach, dy: -spec.reach))
            outside.addPath(outline)
            ctx.saveGState()
            ctx.addPath(outside)
            ctx.clip(using: .evenOdd)
            ctx.setShadow(offset: CGSize(width: x, height: -y), blur: blur, color: spec.color)
            ctx.addPath(cast)
            ctx.setFillColor(CGColor(gray: 0, alpha: 1))
            ctx.fillPath()
            ctx.restoreGState()
        }
    }

    /// The inset shadows into a capture, over the box's paint (LLP 1077 D4):
    /// a frame around the shrunk padding box casts inward, clipped to it.
    func drawCapturedInsetShadow(_ ctx: CGContext) {
        guard Capture.capturing, insetCaster != nil else { return }
        let uniform = number("border_width")
        let w = ["top", "right", "bottom", "left"].map { number("border_width_" + $0, uniform) }
        let padding = CGRect(x: w[3], y: w[0], width: max(0, bounds.width - w[1] - w[3]), height: max(0, bounds.height - w[0] - w[2]))
        let sizes = BorderPaint.reduced(BorderPaint.radii(style, in: bounds), in: bounds)
        let shape = CornerShape(style["corner_shape"])
        let padRadii = [CGSize(width: sizes[0].width - w[3], height: sizes[0].height - w[0]),
                        CGSize(width: sizes[1].width - w[1], height: sizes[1].height - w[0]),
                        CGSize(width: sizes[2].width - w[1], height: sizes[2].height - w[2]),
                        CGSize(width: sizes[3].width - w[3], height: sizes[3].height - w[2])]
            .map { CGSize(width: max(0, $0.width), height: max(0, $0.height)) }
        let clip = BorderPaint.roundedRect(padding, padRadii, shape: shape)
        for spec in boxShadows.reversed() where spec.inset {
            let hole = padding.insetBy(dx: spec.spread, dy: spec.spread)
            let frame = CGMutablePath()
            frame.addRect(padding.insetBy(dx: -spec.reach - padding.width, dy: -spec.reach - padding.height))
            if hole.width > 0, hole.height > 0 {
                let radii = padRadii.map { CGSize(width: max(0, $0.width - spec.spread), height: max(0, $0.height - spec.spread)) }
                frame.addPath(BorderPaint.roundedRect(hole, radii, shape: shape))
            }
            ctx.saveGState()
            ctx.addPath(clip)
            ctx.clip()
            ctx.setShadow(offset: CGSize(width: spec.offset.width, height: -spec.offset.height), blur: spec.blur, color: spec.color)
            ctx.addPath(frame)
            ctx.setFillColor(spec.color)
            ctx.fillPath(using: .evenOdd)
            ctx.restoreGState()
        }
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

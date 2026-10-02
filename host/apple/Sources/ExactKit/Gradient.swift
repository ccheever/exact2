// CSS `background-image` gradients (LLP 1066), shared by UIKit and AppKit.
// The host's Rust side sends the shape and stops (`style.rs` gradient_json),
// stops already expanded to mix as CSS's premultiplied ones do; placement
// needs the box, so it is computed here — the kernel's
// `Gradient::geometry` again, in the language that has the box.
import CoreGraphics
import Foundation
import QuartzCore

struct Gradient {
    enum Shape: Equatable {
        case angle(CGFloat)
        case corner(right: Bool, bottom: Bool)
        /// `at`: x%, xpx, y%, ypx. `extent`: closest-side, closest-corner,
        /// farthest-side, farthest-corner.
        case radial(circle: Bool, extent: Int, at: [CGFloat])
        /// `conic-gradient()` (LLP 1076 D5): `from` CSS degrees (0 up,
        /// clockwise), `at` x%, xpx, y%, ypx.
        case conic(from: CGFloat, at: [CGFloat])
    }
    enum Placement: Equatable {
        /// Stop 0 at the first point, stop 1 at the second.
        case axial(CGPoint, CGPoint)
        /// Stop 0 at the centre, stop 1 on the ellipse of these radii.
        case radial(CGPoint, CGSize)
        /// Stops around the centre, from this many CSS degrees.
        case conic(CGPoint, CGFloat)
    }

    let shape: Shape
    private let light: [CGFloat]
    private let dark: [CGFloat]?

    init?(_ value: BatchValue?) {
        guard case .object(let o)? = value, let light = o["stops"]?.numbers,
              light.count >= 10, light.count % 5 == 0 else { return nil }
        if let deg = o["linear"]?.number {
            shape = .angle(CGFloat(deg))
        } else if let c = o["corner"]?.numbers, c.count == 2 {
            shape = .corner(right: c[0] != 0, bottom: c[1] != 0)
        } else if let r = o["radial"]?.numbers, r.count == 6 {
            shape = .radial(circle: r[0] != 0, extent: Int(r[1]), at: r[2...].map { CGFloat($0) })
        } else if let c = o["conic"]?.numbers, c.count == 5 {
            shape = .conic(from: CGFloat(c[0]), at: c[1...].map { CGFloat($0) })
        } else {
            return nil
        }
        self.light = light.map { CGFloat($0) }
        dark = o["dark"]?.numbers.map { $0.map { CGFloat($0) } }
    }

    /// A `background-image`'s layers (LLP 1076 D5): one gradient's object,
    /// or an array of them, the first on top.
    static func layers(_ value: BatchValue?) -> [Gradient] {
        if let list = value?.array { return list.compactMap { Gradient($0) } }
        return Gradient(value).map { [$0] } ?? []
    }

    /// The gradient's alpha as pixels `size` points at `scale`, placed in
    /// the whole picture: a conic mask (LLP 1076 D2, D5), which Core
    /// Animation's gradient layer cannot say.
    func image(size: CGSize, scale: CGFloat, dark: Bool) -> CGImage? {
        let w = Int((size.width * scale).rounded(.up)), h = Int((size.height * scale).rounded(.up))
        guard w > 0, h > 0, let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                                                 space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                                 bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        // y down, as the layer the mask sits on.
        ctx.translateBy(x: 0, y: CGFloat(h)); ctx.scaleBy(x: scale, y: -scale)
        let rect = CGRect(origin: .zero, size: size)
        paint(ctx, clip: CGPath(rect: rect, transform: nil), box: rect, dark: dark)
        return ctx.makeImage()
    }

    /// Whether Core Animation's gradient layer can say it: a conic one is
    /// drawn (its unit space would bend CSS's angles in a box that is not
    /// square).
    var isConic: Bool { if case .conic = shape { return true }; return false }

    /// Locations 0–1 and their sRGB colours under an appearance. `dense`
    /// samples each stretch between two hues sixteen times: Core Animation
    /// mixes stops in a space of its own, not sRGB (measured against Chrome:
    /// red into blue a mean 5.5/255 off, three hues 12), and short sRGB
    /// steps leave it nothing to disagree about.
    func stops(dark isDark: Bool, dense: Bool = false) -> ([CGFloat], [CGColor]) {
        var s = isDark ? dark ?? light : light
        if dense {
            var out: [CGFloat] = []
            for i in stride(from: 0, to: s.count, by: 5) {
                out += s[i..<i + 5]
                guard i + 9 < s.count, s[i] < s[i + 5], s[i + 1..<i + 4] != s[i + 6..<i + 9] else { continue }
                for k in 1..<16 {
                    let t = CGFloat(k) / 16
                    out += (0..<5).map { s[i + $0] + (s[i + 5 + $0] - s[i + $0]) * t }
                }
            }
            s = out
        }
        var locations: [CGFloat] = [], colors: [CGColor] = []
        for i in stride(from: 0, to: s.count, by: 5) {
            locations.append(s[i])
            colors.append(CGColor(srgbRed: s[i + 1] / 255, green: s[i + 2] / 255, blue: s[i + 3] / 255, alpha: s[i + 4] / 255))
        }
        return (locations, colors)
    }

    /// Where the gradient falls for a padding box `box` (CSS's gradient box).
    func placement(in box: CGRect) -> Placement {
        let (w, h) = (max(box.width, 0), max(box.height, 0))
        switch shape {
        case .angle, .corner:
            var (sin, cos): (CGFloat, CGFloat)
            if case .angle(let deg) = shape {
                (sin, cos) = (Foundation.sin(deg * .pi / 180), Foundation.cos(deg * .pi / 180))
            } else if case .corner(let right, let bottom) = shape {
                // The 50% line joins the other two corners.
                let (x, y) = (right ? h : -h, bottom ? w : -w), n = hypot(x, y)
                (sin, cos) = n == 0 ? (0, 1) : (x / n, -y / n)
            } else {
                (sin, cos) = (0, 1)
            }
            let half = (w * abs(sin) + h * abs(cos)) / 2
            let (cx, cy) = (box.midX, box.midY)
            return .axial(CGPoint(x: cx - sin * half, y: cy + cos * half), CGPoint(x: cx + sin * half, y: cy - cos * half))
        case .conic(let from, let at):
            return .conic(CGPoint(x: box.minX + w * at[0] / 100 + at[1], y: box.minY + h * at[2] / 100 + at[3]), from)
        case .radial(let circle, let extent, let at):
            let c = CGPoint(x: box.minX + w * at[0] / 100 + at[1], y: box.minY + h * at[2] / 100 + at[3])
            let dx = [abs(c.x - box.minX), abs(box.minX + w - c.x)], dy = [abs(c.y - box.minY), abs(box.minY + h - c.y)]
            let near = CGSize(width: dx.min()!, height: dy.min()!), far = CGSize(width: dx.max()!, height: dy.max()!)
            let corners = dx.flatMap { x in dy.map { hypot(x, $0) } }
            let r: CGSize
            switch (circle, extent) {
            case (true, 0): let n = min(near.width, near.height); r = CGSize(width: n, height: n)
            case (true, 1): let n = corners.min()!; r = CGSize(width: n, height: n)
            case (true, 2): let n = max(far.width, far.height); r = CGSize(width: n, height: n)
            case (true, _): let n = corners.max()!; r = CGSize(width: n, height: n)
            case (false, 0): r = near
            case (false, 1): r = CGSize(width: near.width * 2.squareRoot(), height: near.height * 2.squareRoot())
            case (false, 2): r = far
            default: r = CGSize(width: far.width * 2.squareRoot(), height: far.height * 2.squareRoot())
            }
            return .radial(c, r)
        }
    }

    /// The colour a radial gradient with no area is everywhere (CSS).
    private func degenerate(_ p: Placement, dark: Bool) -> CGColor? {
        guard case .radial(_, let r) = p, r.width <= 0 || r.height <= 0 else { return nil }
        return stops(dark: dark).1.last
    }

    /// Paint inside `clip` (the border box's rounded outline), placed in
    /// `box`: over whatever was drawn before, under what is drawn after.
    func paint(_ ctx: CGContext, clip: CGPath, box: CGRect, dark: Bool) {
        let place = placement(in: box)
        ctx.saveGState(); defer { ctx.restoreGState() }
        ctx.addPath(clip); ctx.clip()
        if let solid = degenerate(place, dark: dark) {
            ctx.setFillColor(solid); ctx.fill(clip.boundingBox); return
        }
        if case .conic(let c, let from) = place {
            paintConic(ctx, center: c, from: from, reach: clip.boundingBox, dark: dark)
            return
        }
        let (locations, colors) = stops(dark: dark)
        guard let g = CGGradient(colorsSpace: CGColorSpace(name: CGColorSpace.sRGB), colors: colors as CFArray, locations: locations) else { return }
        switch place {
        case .axial(let a, let b):
            ctx.drawLinearGradient(g, start: a, end: b, options: [.drawsBeforeStartLocation, .drawsAfterEndLocation])
        case .radial(let c, let r):
            ctx.translateBy(x: c.x, y: c.y); ctx.scaleBy(x: r.width, y: r.height)
            ctx.drawRadialGradient(g, startCenter: .zero, startRadius: 0, endCenter: .zero, endRadius: 1, options: [.drawsAfterEndLocation])
        case .conic:
            break
        }
    }

    /// A conic gradient as wedges about its centre, each the colour at its
    /// middle angle: Core Graphics has no conic gradient. A wedge a fifth of
    /// a degree wide is under a device pixel at the edge of any box a phone
    /// shows; they overlap by half a wedge and are not antialiased, so no
    /// seam shows, and the clip's own edge stays smooth. Each replaces what
    /// it overlaps (a translucent colour would otherwise be laid twice), in
    /// a layer of their own over what was drawn before.
    private func paintConic(_ ctx: CGContext, center c: CGPoint, from: CGFloat, reach: CGRect, dark: Bool) {
        let corners = [CGPoint(x: reach.minX, y: reach.minY), CGPoint(x: reach.maxX, y: reach.minY),
                       CGPoint(x: reach.minX, y: reach.maxY), CGPoint(x: reach.maxX, y: reach.maxY)]
        let r = corners.map { hypot($0.x - c.x, $0.y - c.y) }.max()! + 2
        let (locations, colors) = stops(dark: dark, dense: true)
        let wedges = 1800
        ctx.beginTransparencyLayer(auxiliaryInfo: nil)
        ctx.setShouldAntialias(false)
        ctx.setBlendMode(.copy)
        defer { ctx.setBlendMode(.normal); ctx.setShouldAntialias(true); ctx.endTransparencyLayer() }
        func point(_ t: CGFloat) -> CGPoint {
            let a = (from + 360 * t) * .pi / 180
            return CGPoint(x: c.x + r * Foundation.sin(a), y: c.y - r * Foundation.cos(a))
        }
        var k = 0
        for i in 0..<wedges {
            let t0 = CGFloat(i) / CGFloat(wedges), t1 = CGFloat(i + 1) / CGFloat(wedges), mid = (t0 + t1) / 2
            while k + 1 < locations.count - 1 && locations[k + 1] < mid { k += 1 }
            let a = locations[k], b = locations[min(k + 1, locations.count - 1)]
            let f = b > a ? min(max((mid - a) / (b - a), 0), 1) : 0
            ctx.setFillColor(Gradient.mix(colors[k], colors[min(k + 1, colors.count - 1)], f))
            ctx.move(to: c)
            ctx.addLine(to: point(max(t0 - 0.5 / CGFloat(wedges), 0)))
            ctx.addLine(to: point(min(t1 + 0.5 / CGFloat(wedges), 1)))
            ctx.closePath(); ctx.fillPath()
        }
    }

    /// Straight sRGB mixing, as the stops are already laid out to look as
    /// CSS's premultiplied mix does.
    private static func mix(_ a: CGColor, _ b: CGColor, _ t: CGFloat) -> CGColor {
        let x = a.components ?? [0, 0, 0, 0], y = b.components ?? [0, 0, 0, 0]
        guard x.count == 4, y.count == 4 else { return t < 0.5 ? a : b }
        return CGColor(srgbRed: x[0] + (y[0] - x[0]) * t, green: x[1] + (y[1] - x[1]) * t,
                       blue: x[2] + (y[2] - x[2]) * t, alpha: x[3] + (y[3] - x[3]) * t)
    }

    /// A gradient layer covering `bounds`, placed in `box` (same space).
    func apply(_ layer: CAGradientLayer, bounds: CGRect, box: CGRect, dark: Bool) {
        let place = placement(in: box)
        var (locations, colors) = stops(dark: dark, dense: true)
        let unit = { (p: CGPoint) in
            CGPoint(x: bounds.width > 0 ? (p.x - bounds.minX) / bounds.width : 0, y: bounds.height > 0 ? (p.y - bounds.minY) / bounds.height : 0)
        }
        var type = CAGradientLayerType.axial, start = CGPoint(x: 0.5, y: 0), end = CGPoint(x: 0.5, y: 1)
        if let solid = degenerate(place, dark: dark) {
            (locations, colors) = ([0, 1], [solid, solid])
        } else if case .axial(let a, let b) = place {
            // Core Animation draws its bands perpendicular to the line in the
            // unit square, which a box that is not square skews. Aim the unit
            // line along S·d instead (S the box's scale, d the CSS direction)
            // and size it so the end band still passes through `b`.
            let (dx, dy) = (b.x - a.x, b.y - a.y), length = hypot(dx, dy)
            let sd = CGPoint(x: bounds.width * dx / max(length, .ulpOfOne), y: bounds.height * dy / max(length, .ulpOfOne))
            let k = length / max(sd.x * sd.x + sd.y * sd.y, .ulpOfOne)
            start = unit(a)
            end = CGPoint(x: start.x + k * sd.x, y: start.y + k * sd.y)
        } else if case .radial(let c, let r) = place {
            type = .radial
            (start, end) = (unit(c), unit(CGPoint(x: c.x + r.width, y: c.y + r.height)))
        }
        if layer.type != type { layer.type = type }
        if layer.startPoint != start { layer.startPoint = start }
        if layer.endPoint != end { layer.endPoint = end }
        let numbers = locations.map { NSNumber(value: Double($0)) }
        if layer.locations != numbers { layer.locations = numbers }
        if (layer.colors as? [CGColor]) != colors { layer.colors = colors }
    }
}

/// A conic `mask-image`'s pixels as a layer's mask (LLP 1076 D5).
final class ConicMaskLayer: CALayer {}

extension BatchValue {
    /// A gradient that carries a `light-dark()` stop: an appearance change
    /// is something to the view that shows it.
    var isSchemeGradient: Bool {
        if case .object(let o) = self { return o["dark"] != nil }
        return false
    }
}

extension NodeView {
    /// The padding box: CSS's gradient box under the initial `background-origin`.
    var gradientBox: CGRect {
        let uniform = number("border_width")
        return bounds.insetBy(left: number("border_width_left", uniform), top: number("border_width_top", uniform),
                              right: number("border_width_right", uniform), bottom: number("border_width_bottom", uniform))
    }

    /// The gradient painted by `draw(_:)`, over the background and under
    /// the border, inside the border box's outline.
    func paintGradient(_ ctx: CGContext, clip: CGPath) {
        guard surface == nil else { return }
        // The last layer first, so the first is on top (LLP 1076 D5).
        for g in Gradient.layers(style["background_image"]).reversed() {
            g.paint(ctx, clip: clip, box: gradientBox, dark: drawsDark)
        }
    }

    /// Whether the background's gradients need `draw(_:)`: several layers,
    /// or a conic one (LLP 1076 D5).
    var gradientDraws: Bool {
        let layers = Gradient.layers(style["background_image"])
        return layers.count > 1 || layers.contains { $0.isConic }
    }
}

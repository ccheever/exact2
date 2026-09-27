// @ref LLP 1055.000 D7 — a shape's paint when it is more than one solid
// colour, or not in the normal order: a part layer per paint, in
// `paint-order`. A solid part is a `CAShapeLayer`; a gradient part is a
// layer holding the gradient drawn by Core Graphics (the two-circle radial
// is SVG's focal model exactly) at the scale the shape shows at, masked by
// the shape's fill or stroke. The Rust host resolved everything: stops,
// spread, and the gradient's transform to the shape's user space.
import CoreGraphics
import Foundation
import QuartzCore

private final class PartAction: NSObject, CAAction {
    static let shared = PartAction()
    func run(forKey event: String, object anObject: Any, arguments dict: [AnyHashable: Any]?) {}
}
private final class PartDelegate: NSObject, CALayerDelegate {
    static let shared = PartDelegate()
    func action(for layer: CALayer, forKey event: String) -> CAAction? { PartAction.shared }
}
private func quiet<L: CALayer>(_ layer: L) -> L { layer.delegate = PartDelegate.shared; return layer }

private func numbers(_ v: Any?) -> [Double] { (v as? [Any])?.map { ($0 as? NSNumber)?.doubleValue ?? 0 } ?? [] }

enum SvgPaint {
    /// Whether an element needs part layers: a gradient paint, or a
    /// paint order other than fill then stroke.
    static func needsParts(_ e: [String: Any]) -> Bool {
        e["f"] is [String: Any] || e["s"] is [String: Any] || numbers(e["po"]).count == 3
    }

    /// Rebuild `shape`'s part layers from the element: fill and stroke in
    /// paint order (markers have no part yet). The shape layer itself then
    /// paints nothing.
    static func parts(_ shape: CAShapeLayer, _ e: [String: Any], scale: CGFloat, dark: Bool, color: (Any?, Bool) -> CGColor?) {
        shape.sublayers?.forEach { $0.removeFromSuperlayer() }
        guard let path = shape.path else { return }
        let order = numbers(e["po"]).count == 3 ? numbers(e["po"]).map(Int.init) : [0, 1, 2]
        let stroke = path.copy(strokingWithWidth: shape.lineWidth, lineCap: cgCap(shape.lineCap), lineJoin: cgJoin(shape.lineJoin), miterLimit: shape.miterLimit)
        let shows = CGFloat(numbers([e["cs"] ?? 1]).first ?? 1) * scale
        for part in order {
            switch part {
            case 0:
                guard let spec = e["f"], !(spec is NSNull) else { continue }
                if let g = spec as? [String: Any] {
                    if let layer = gradient(g, rect: path.boundingBoxOfPath, scale: shows, dark: dark, color: color) {
                        layer.mask = mask(path, of: layer, fill: true, like: shape)
                        shape.addSublayer(layer)
                    }
                } else {
                    let fill = quiet(CAShapeLayer())
                    fill.path = path; fill.fillRule = shape.fillRule
                    fill.fillColor = color(spec, dark); fill.strokeColor = nil
                    shape.addSublayer(fill)
                }
            case 1:
                guard let spec = e["s"], !(spec is NSNull), shape.lineWidth > 0 else { continue }
                if let g = spec as? [String: Any] {
                    if let layer = gradient(g, rect: stroke.boundingBoxOfPath, scale: shows, dark: dark, color: color) {
                        layer.mask = mask(path, of: layer, fill: false, like: shape)
                        shape.addSublayer(layer)
                    }
                } else {
                    let line = strokeLayer(path, like: shape)
                    line.strokeColor = color(spec, dark)
                    shape.addSublayer(line)
                }
            default: continue
            }
        }
    }

    private static func strokeLayer(_ path: CGPath, like shape: CAShapeLayer) -> CAShapeLayer {
        let line = quiet(CAShapeLayer())
        line.path = path; line.fillColor = nil
        line.lineWidth = shape.lineWidth; line.lineCap = shape.lineCap; line.lineJoin = shape.lineJoin
        line.miterLimit = shape.miterLimit; line.lineDashPattern = shape.lineDashPattern; line.lineDashPhase = shape.lineDashPhase
        return line
    }

    /// A mask in the gradient layer's space, which is the shape's user
    /// space (the layer's bounds start at its rect's origin).
    private static func mask(_ path: CGPath, of layer: CALayer, fill: Bool, like shape: CAShapeLayer) -> CAShapeLayer {
        let m = fill ? quiet(CAShapeLayer()) : strokeLayer(path, like: shape)
        m.path = path
        if fill { m.fillColor = CGColor(gray: 0, alpha: 1); m.fillRule = shape.fillRule } else { m.strokeColor = CGColor(gray: 0, alpha: 1) }
        m.anchorPoint = .zero
        m.bounds = layer.bounds
        m.position = layer.bounds.origin
        return m
    }

    private static func cgCap(_ c: CAShapeLayerLineCap) -> CGLineCap { c == .round ? .round : c == .square ? .square : .butt }
    private static func cgJoin(_ j: CAShapeLayerLineJoin) -> CGLineJoin { j == .round ? .round : j == .bevel ? .bevel : .miter }

    /// A layer covering `rect` (user units) with the gradient drawn in it.
    private static func gradient(_ g: [String: Any], rect: CGRect, scale: CGFloat, dark: Bool, color: (Any?, Bool) -> CGColor?) -> CALayer? {
        let rect = rect.integral.insetBy(dx: -1, dy: -1)
        let w = Int((rect.width * scale).rounded(.up)), h = Int((rect.height * scale).rounded(.up))
        guard w > 0, h > 0, w * h <= 16_777_216,
              let space = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0, space: space,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        // Top-left origin, points to pixels, the rect's origin at zero,
        // then gradient space.
        ctx.translateBy(x: 0, y: CGFloat(h)); ctx.scaleBy(x: 1, y: -1)
        ctx.scaleBy(x: CGFloat(w) / rect.width, y: CGFloat(h) / rect.height)
        ctx.translateBy(x: -rect.minX, y: -rect.minY)
        let t = numbers(g["t"])
        let m = t.count == 6 ? CGAffineTransform(a: t[0], b: t[1], c: t[2], d: t[3], tx: t[4], ty: t[5]) : .identity
        ctx.concatenate(m)
        var stops: [(CGFloat, CGColor)] = (g["st"] as? [Any] ?? []).compactMap { s in
            guard let s = s as? [Any], s.count == 2, let c = color(s[1], dark) else { return nil }
            return (CGFloat((s[0] as? NSNumber)?.doubleValue ?? 0), c)
        }
        guard stops.count >= 2 else { return nil }
        // The rect's corners in gradient space: how far `reflect` and
        // `repeat` must reach.
        let inverse = m.inverted()
        let corners = [CGPoint(x: rect.minX, y: rect.minY), CGPoint(x: rect.maxX, y: rect.minY),
                       CGPoint(x: rect.minX, y: rect.maxY), CGPoint(x: rect.maxX, y: rect.maxY)].map { $0.applying(inverse) }
        let spread = Int(numbers([g["sp"] ?? 0]).first ?? 0)
        let pad: CGGradientDrawingOptions = [.drawsBeforeStartLocation, .drawsAfterEndLocation]
        if let lg = Optional(numbers(g["lg"])), lg.count == 4 {
            var p1 = CGPoint(x: lg[0], y: lg[1]), p2 = CGPoint(x: lg[2], y: lg[3])
            let d = CGPoint(x: p2.x - p1.x, y: p2.y - p1.y), len2 = d.x * d.x + d.y * d.y
            if spread != 0, len2 > 0 {
                let ts = corners.map { (($0.x - p1.x) * d.x + ($0.y - p1.y) * d.y) / len2 }
                let lo = floor(ts.min() ?? 0), hi = ceil(ts.max() ?? 1)
                stops = repeated(stops, from: Int(lo), to: Int(hi), reflect: spread == 1)
                (p1, p2) = (CGPoint(x: p1.x + d.x * lo, y: p1.y + d.y * lo), CGPoint(x: p1.x + d.x * hi, y: p1.y + d.y * hi))
            }
            guard let grad = make(stops, space) else { return nil }
            ctx.drawLinearGradient(grad, start: p1, end: p2, options: pad)
        } else if let rg = Optional(numbers(g["rg"])), rg.count == 6 {
            let c = CGPoint(x: rg[0], y: rg[1]), f = CGPoint(x: rg[3], y: rg[4])
            var r = CGFloat(rg[2])
            let fr = CGFloat(rg[5])
            if spread != 0, f == c, fr == 0, r > 0 {
                // Concentric: repeating the stops is exact. A moving focus
                // pads until the island draws it (LLP 1055.000 §0).
                let far = corners.map { hypot($0.x - c.x, $0.y - c.y) }.max() ?? r
                let n = max(1, Int(ceil(far / r)))
                stops = repeated(stops, from: 0, to: n, reflect: spread == 1)
                r *= CGFloat(n)
            }
            guard let grad = make(stops, space) else { return nil }
            ctx.drawRadialGradient(grad, startCenter: f, startRadius: fr, endCenter: c, endRadius: r, options: pad)
        } else { return nil }
        guard let image = ctx.makeImage() else { return nil }
        let layer = quiet(CALayer())
        layer.contents = image
        layer.anchorPoint = .zero
        layer.bounds = rect
        layer.position = rect.origin
        return layer
    }

    /// The stops over periods `from..<to` of the gradient vector, in one
    /// 0–1 range; `reflect` mirrors every other period.
    private static func repeated(_ stops: [(CGFloat, CGColor)], from: Int, to: Int, reflect: Bool) -> [(CGFloat, CGColor)] {
        let periods = max(1, to - from)
        guard periods <= 256 else { return stops }
        var out: [(CGFloat, CGColor)] = []
        for k in from..<(from + periods) {
            let flip = reflect && (k % 2 != 0)
            let period = flip ? stops.reversed().map { (1 - $0.0, $0.1) } : stops
            for (o, c) in period { out.append(((CGFloat(k - from) + o) / CGFloat(periods), c)) }
        }
        return out
    }

    private static func make(_ stops: [(CGFloat, CGColor)], _ space: CGColorSpace) -> CGGradient? {
        CGGradient(colorsSpace: space, colors: stops.map(\.1) as CFArray, locations: stops.map(\.0))
    }
}

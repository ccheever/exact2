// @ref LLP 1056 D7 — Canvas 2D on Apple: the recorded lists replayed into a
// Core Graphics bitmap per canvas, on the main thread, and shown as a layer's
// contents in the canvas's content box, under its children. The list's
// geometry is resolved (canvas/src/list.rs): paths arrive in canvas
// coordinates, so fills and clips draw them under the base transform (the
// flip and the device scale), and a stroke takes them back to user space and
// strokes under base ∘ author, so its width follows the author's matrix.
// iOS and macOS share this file.
import CoreGraphics
import Foundation
import QuartzCore

/// No implicit animations: new pixels appear with the batch that drew them.
private final class Instant: NSObject, CALayerDelegate {
    static let shared = Instant()
    func action(for layer: CALayer, forKey event: String) -> CAAction? { NSNull() }
}

/// The list's opcodes (canvas/src/list.rs `Op`), by number.
private enum Op: UInt32 {
    case save = 1, restore = 2, reset = 3, setTransform = 4
    case fillColor = 10, fillGradient = 11, strokeColor = 12, strokeGradient = 13
    case lineWidth = 14, lineCap = 15, lineJoin = 16, miterLimit = 17, lineDash = 18, lineDashOffset = 19
    case globalAlpha = 20, composite = 21
    case linearGradient = 30, radialGradient = 31, colorStop = 32
    case beginPath = 40, moveTo = 41, lineTo = 42, quadTo = 43, cubicTo = 44, closePath = 45
    case fill = 50, stroke = 51, clip = 52, fillRect = 53, strokeRect = 54, clearRect = 55
}

/// `globalCompositeOperation`, by the list's index (`exact_canvas::COMPOSITE`).
private let blends: [CGBlendMode] = [
    .normal, .sourceIn, .sourceOut, .sourceAtop, .destinationOver, .destinationIn,
    .destinationOut, .destinationAtop, .plusLighter, .copy, .xor, .multiply, .screen, .overlay,
    .darken, .lighten, .colorDodge, .colorBurn, .hardLight, .softLight, .difference, .exclusion,
    .hue, .saturation, .color, .luminosity,
]

private enum Style: Equatable { case color(CGColor), gradient(UInt32) }

private struct Gradient {
    var linear: [Double]?   // x0 y0 x1 y1
    var radial: [Double]?   // x0 y0 r0 x1 y1 r1
    var stops: [(Double, CGColor)] = []
}

/// What the replayer keeps beside Core Graphics' own state stack.
private struct State {
    var author = CGAffineTransform.identity
    var fill: Style = .color(CGColor(srgbRed: 0, green: 0, blue: 0, alpha: 1))
    var stroke: Style = .color(CGColor(srgbRed: 0, green: 0, blue: 0, alpha: 1))
    var dash: [CGFloat] = []
    var dashOffset = 0.0
}

private let sRGB = CGColorSpace(name: CGColorSpace.sRGB)!

/// One canvas's bitmap and replayer.
final class Canvas2DReplayer {
    let lifetime: UInt64
    let generation: UInt32
    let context: CGContext?
    let base: CGAffineTransform
    private var state = State()
    private var stack: [State] = []
    /// The current path, in canvas coordinates; it survives painting.
    private var path = CGMutablePath()
    private var gradients: [UInt32: Gradient] = [:]

    init(width: Int, height: Int, scale: Double, lifetime: UInt64, generation: UInt32) {
        self.lifetime = lifetime; self.generation = generation
        let info = CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        context = width > 0 && height > 0
            ? CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: sRGB, bitmapInfo: info)
            : nil
        // Flip once to the canvas's y-down space, then the device scale.
        base = CGAffineTransform(a: CGFloat(scale), b: 0, c: 0, d: -CGFloat(scale), tx: 0, ty: CGFloat(height))
        if let c = context {
            c.concatenate(base)
            // The root state: `reset` restores to it, dropping every clip.
            c.saveGState()
            c.clear(CGRect(x: 0, y: 0, width: CGFloat(width) / CGFloat(scale), height: CGFloat(height) / CGFloat(scale)))
        }
    }

    private func color(_ n: [Double], _ i: Int) -> CGColor {
        CGColor(srgbRed: n[i] / 255, green: n[i + 1] / 255, blue: n[i + 2] / 255, alpha: n[i + 3])
    }

    /// Paint the clip-bounded area with a gradient under the author matrix.
    private func drawGradient(_ c: CGContext, _ id: UInt32) {
        guard let g = gradients[id], !g.stops.isEmpty else { return }
        c.concatenate(state.author)
        if g.stops.count == 1 {
            c.setFillColor(g.stops[0].1)
            c.fill(CGRect(x: -1e7, y: -1e7, width: 2e7, height: 2e7))
            return
        }
        guard let gradient = CGGradient(colorsSpace: sRGB, colors: g.stops.map(\.1) as CFArray,
                                        locations: g.stops.map { CGFloat($0.0) }) else { return }
        let extend: CGGradientDrawingOptions = [.drawsBeforeStartLocation, .drawsAfterEndLocation]
        if let l = g.linear {
            if l[0] == l[2] && l[1] == l[3] { return }
            c.drawLinearGradient(gradient, start: CGPoint(x: l[0], y: l[1]), end: CGPoint(x: l[2], y: l[3]), options: extend)
        } else if let r = g.radial {
            if r[0] == r[3] && r[1] == r[4] && r[2] == r[5] { return }
            c.drawRadialGradient(gradient, startCenter: CGPoint(x: r[0], y: r[1]), startRadius: CGFloat(r[2]),
                                 endCenter: CGPoint(x: r[3], y: r[4]), endRadius: CGFloat(r[5]), options: extend)
        }
    }

    /// Fill `canvasPath` (canvas coordinates) with the fill style.
    private func fill(_ c: CGContext, _ canvasPath: CGPath, rule: CGPathFillRule) {
        switch state.fill {
        case .color(let color):
            c.setFillColor(color)
            c.addPath(canvasPath)
            c.fillPath(using: rule)
        case .gradient(let id):
            c.saveGState()
            c.addPath(canvasPath)
            c.clip(using: rule)
            drawGradient(c, id)
            c.restoreGState()
        }
    }

    /// Stroke `userPath` (user space) under base ∘ author.
    private func stroke(_ c: CGContext, _ userPath: CGPath) {
        c.saveGState()
        c.concatenate(state.author)
        c.addPath(userPath)
        switch state.stroke {
        case .color(let color):
            c.setStrokeColor(color)
            c.strokePath()
        case .gradient(let id):
            c.replacePathWithStrokedPath()
            c.clip()
            c.concatenate(state.author.inverted())
            drawGradient(c, id)
        }
        c.restoreGState()
    }

    private func rect(_ n: [Double]) -> CGRect { CGRect(x: n[0], y: n[1], width: n[2], height: n[3]) }

    /// Apply one list; false when it is not one this reader can read.
    func apply(_ data: Data) -> Bool {
        guard let c = context else { return true }
        return data.withUnsafeBytes { (raw: UnsafeRawBufferPointer) -> Bool in
            guard raw.count >= 8, raw.loadUnaligned(fromByteOffset: 0, as: UInt32.self).littleEndian == 0x4432_4345,
                  raw.loadUnaligned(fromByteOffset: 4, as: UInt32.self).littleEndian == 1 else { return false }
            var at = 8
            var n = [Double](repeating: 0, count: 8)
            while at + 8 <= raw.count {
                let code = raw.loadUnaligned(fromByteOffset: at, as: UInt32.self).littleEndian
                let count = Int(raw.loadUnaligned(fromByteOffset: at + 4, as: UInt32.self).littleEndian)
                at += 8
                guard at + count * 8 <= raw.count, let op = Op(rawValue: code) else { return false }
                if n.count < count { n = [Double](repeating: 0, count: count) }
                for i in 0..<count { n[i] = Double(bitPattern: raw.loadUnaligned(fromByteOffset: at + i * 8, as: UInt64.self).littleEndian) }
                at += count * 8
                switch op {
                case .save: stack.append(state); c.saveGState()
                case .restore:
                    if let s = stack.popLast() { state = s; c.restoreGState() }
                case .reset:
                    while stack.popLast() != nil { c.restoreGState() }
                    c.restoreGState(); c.saveGState()
                    state = State(); path = CGMutablePath()
                    c.clear(CGRect(x: -1e7, y: -1e7, width: 2e7, height: 2e7))
                case .setTransform: state.author = CGAffineTransform(a: n[0], b: n[1], c: n[2], d: n[3], tx: n[4], ty: n[5])
                case .fillColor: state.fill = .color(color(n, 0))
                case .fillGradient: state.fill = .gradient(UInt32(n[0]))
                case .strokeColor: state.stroke = .color(color(n, 0))
                case .strokeGradient: state.stroke = .gradient(UInt32(n[0]))
                case .lineWidth: c.setLineWidth(n[0])
                case .lineCap: c.setLineCap([CGLineCap.butt, .round, .square][Int(n[0])])
                case .lineJoin: c.setLineJoin([CGLineJoin.miter, .round, .bevel][Int(n[0])])
                case .miterLimit: c.setMiterLimit(n[0])
                case .lineDash:
                    state.dash = (0..<count).map { CGFloat(n[$0]) }
                    c.setLineDash(phase: state.dashOffset, lengths: state.dash)
                case .lineDashOffset:
                    state.dashOffset = n[0]
                    c.setLineDash(phase: state.dashOffset, lengths: state.dash)
                case .globalAlpha: c.setAlpha(n[0])
                case .composite: c.setBlendMode(blends[min(Int(n[0]), blends.count - 1)])
                case .linearGradient: gradients[UInt32(n[0])] = Gradient(linear: Array(n[1...4]))
                case .radialGradient: gradients[UInt32(n[0])] = Gradient(radial: Array(n[1...6]))
                case .colorStop:
                    let id = UInt32(n[0])
                    if var g = gradients[id] {
                        let i = g.stops.firstIndex { $0.0 > n[1] } ?? g.stops.count
                        g.stops.insert((n[1], color(n, 2)), at: i)
                        gradients[id] = g
                    }
                case .beginPath: path = CGMutablePath()
                case .moveTo: path.move(to: CGPoint(x: n[0], y: n[1]))
                case .lineTo: path.isEmpty ? path.move(to: CGPoint(x: n[0], y: n[1])) : path.addLine(to: CGPoint(x: n[0], y: n[1]))
                case .quadTo: path.addQuadCurve(to: CGPoint(x: n[2], y: n[3]), control: CGPoint(x: n[0], y: n[1]))
                case .cubicTo:
                    path.addCurve(to: CGPoint(x: n[4], y: n[5]), control1: CGPoint(x: n[0], y: n[1]), control2: CGPoint(x: n[2], y: n[3]))
                case .closePath: if !path.isEmpty { path.closeSubpath() }
                case .fill: if !path.isEmpty { fill(c, path, rule: n[0] == 1 ? .evenOdd : .winding) }
                case .stroke:
                    if !path.isEmpty {
                        var inverse = state.author.inverted()
                        if let user = path.copy(using: &inverse) { stroke(c, user) }
                    }
                case .clip:
                    if path.isEmpty { c.clip(to: CGRect.zero) } else { c.addPath(path); c.clip(using: n[0] == 1 ? .evenOdd : .winding) }
                case .fillRect:
                    var author = state.author
                    fill(c, CGPath(rect: rect(n), transform: &author), rule: .winding)
                case .strokeRect:
                    let r = rect(n)
                    if r.width == 0 && r.height == 0 { break }
                    let p = CGMutablePath()
                    if r.width == 0 || r.height == 0 {
                        p.move(to: r.origin); p.addLine(to: CGPoint(x: r.maxX, y: r.maxY))
                    } else {
                        p.addRect(r)
                    }
                    stroke(c, p)
                case .clearRect:
                    c.saveGState()
                    c.concatenate(state.author)
                    c.setBlendMode(.clear)
                    c.setAlpha(1)
                    c.addRect(rect(n))
                    c.fillPath()
                    c.restoreGState()
                }
            }
            return at == raw.count
        }
    }

    func image() -> CGImage? { context?.makeImage() }
}

/// A presenter's 2D canvases, by view id: the `canvas2d` op, and cleanup
/// when a view goes. The bitmap shows in a sublayer below the view's
/// children, framed to the content box.
final class Canvas2DHost {
    private var replayers: [UInt32: Canvas2DReplayer] = [:]
    private var layers: [UInt32: CALayer] = [:]
    /// Lists that could not be read, for `logs`.
    var errors: [String] = []
    /// The views' real scale differs from what the canvases were drawn at.
    var onScale: ((CGFloat) -> Void)?
    private var reported: CGFloat = 0

    func apply(_ id: UInt32, _ payload: [String: Any], layer parent: CALayer?) {
        guard let parent else { return }
        let num = { (key: String) -> Double in (payload[key] as? NSNumber)?.doubleValue ?? 0 }
        let lifetime = UInt64(num("lifetime")), generation = UInt32(num("generation"))
        if (payload["fresh"] as? NSNumber)?.boolValue == true {
            replayers[id] = Canvas2DReplayer(width: Int(num("w")), height: Int(num("h")), scale: num("scale"),
                                             lifetime: lifetime, generation: generation)
        }
        guard let r = replayers[id], r.lifetime == lifetime, r.generation == generation else { return }
        for case let text as String in payload["lists"] as? [Any] ?? [] {
            guard let data = Data(base64Encoded: text), r.apply(data) else {
                errors.append("canvas \(id): unreadable list"); continue
            }
        }
        let layer = layers[id] ?? {
            let l = CALayer(); l.delegate = Instant.shared; l.contentsGravity = .resize
            l.magnificationFilter = .linear; l.minificationFilter = .linear
            layers[id] = l; return l
        }()
        if layer.superlayer !== parent { parent.insertSublayer(layer, at: 0) }
        let box = (payload["box"] as? [Any])?.compactMap { ($0 as? NSNumber)?.doubleValue } ?? []
        if box.count == 4 { layer.frame = CGRect(x: box[0], y: box[1], width: box[2], height: box[3]) }
        // A rounded canvas clips its bitmap to the content edge's curve, as
        // the web clips replaced content.
        let radii = (payload["radii"] as? [Any])?.compactMap { ($0 as? NSNumber).map { CGFloat($0.doubleValue) } } ?? []
        if radii.count == 4, radii.contains(where: { $0 > 0 }) {
            let mask = (layer.mask as? CAShapeLayer) ?? CAShapeLayer()
            mask.path = Canvas2DHost.rounded(CGRect(origin: .zero, size: layer.bounds.size), radii)
            layer.mask = mask
        } else {
            layer.mask = nil
        }
        layer.contentsScale = max(1, num("scale"))
        layer.contents = r.image()
        let actual = parent.contentsScale
        if (payload["stretch"] as? NSNumber)?.boolValue != true, actual >= 1, abs(actual - num("scale")) > 0.01, actual != reported {
            reported = actual
            onScale?(actual)
        }
    }

    /// A rectangle with four corner radii (top-left, top-right, bottom-right,
    /// bottom-left), in a y-down layer.
    static func rounded(_ r: CGRect, _ radii: [CGFloat]) -> CGPath {
        let p = CGMutablePath(), (x0, y0, x1, y1) = (r.minX, r.minY, r.maxX, r.maxY)
        let f = min(1, r.width / max(radii[0] + radii[1], radii[2] + radii[3], 1e-9), r.height / max(radii[0] + radii[3], radii[1] + radii[2], 1e-9))
        let (tl, tr, br, bl) = (radii[0] * f, radii[1] * f, radii[2] * f, radii[3] * f)
        p.move(to: CGPoint(x: x0 + tl, y: y0))
        p.addLine(to: CGPoint(x: x1 - tr, y: y0))
        p.addArc(tangent1End: CGPoint(x: x1, y: y0), tangent2End: CGPoint(x: x1, y: y0 + tr), radius: tr)
        p.addLine(to: CGPoint(x: x1, y: y1 - br))
        p.addArc(tangent1End: CGPoint(x: x1, y: y1), tangent2End: CGPoint(x: x1 - br, y: y1), radius: br)
        p.addLine(to: CGPoint(x: x0 + bl, y: y1))
        p.addArc(tangent1End: CGPoint(x: x0, y: y1), tangent2End: CGPoint(x: x0, y: y1 - bl), radius: bl)
        p.addLine(to: CGPoint(x: x0, y: y0 + tl))
        p.addArc(tangent1End: CGPoint(x: x0, y: y0), tangent2End: CGPoint(x: x0 + tl, y: y0), radius: tl)
        p.closeSubpath()
        return p
    }

    func forget(_ id: UInt32) {
        replayers.removeValue(forKey: id)
        layers.removeValue(forKey: id)?.removeFromSuperlayer()
    }
}

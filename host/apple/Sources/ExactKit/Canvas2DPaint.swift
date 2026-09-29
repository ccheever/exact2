// @ref LLP 1056 §1, §3 stage 2 — how every Canvas 2D paint reaches the
// bitmap: `render` applies global alpha, the compositing operator and the
// shadow around one paint. Shadow offsets and blur are canvas pixels × the
// device scale, never × the author's matrix, in Core Graphics' base space
// (y up, so the offset's y is negated). The five operators that reach
// outside the shape (source-in, source-out, destination-in,
// destination-atop, copy) draw the shape into a transparent bitmap the size
// of the canvas and composite that bitmap over the whole canvas, bounded by
// the clip, as WebKit does. Paints with a gradient, pattern or conic
// gradient clip to the shape and fill the clip.
import CoreGraphics
import Foundation

extension Canvas2DReplayer {
    /// The line style, which a paint applies to whichever context it uses.
    func configureLines(_ c: CGContext) {
        c.setLineWidth(state.lineWidth)
        c.setLineCap(state.cap)
        c.setLineJoin(state.join)
        c.setMiterLimit(state.miter)
        c.setLineDash(phase: state.dashOffset, lengths: state.dash)
        c.interpolationQuality = !state.smoothing ? .none : [CGInterpolationQuality.low, .medium, .high][min(2, max(0, state.quality))]
    }

    /// Back to the base transform (flip and device scale), whatever the
    /// paint concatenated; the clip is untouched.
    func resetToBase(_ c: CGContext) {
        c.concatenate(c.ctm.inverted())
        c.concatenate(base)
    }

    var shadows: Bool {
        state.shadowColor.alpha > 0 && (state.shadowBlur > 0 || state.shadowOffset != .zero)
    }

    private func setShadow(_ c: CGContext) {
        // Base space: device pixels, y up. WebKit hands `shadowBlur` to
        // Core Graphics as its blur unchanged (canvas σ = blur / 2).
        let s = scale
        c.setShadow(offset: CGSize(width: state.shadowOffset.width * s, height: -state.shadowOffset.height * s),
                    blur: state.shadowBlur * s, color: state.shadowColor)
    }

    /// One paint, drawn by `draw` into a context at the base transform.
    func render(_ c: CGContext, _ draw: (CGContext) -> Void) {
        c.saveGState()
        configureLines(c)
        if canvas2DClipsExtent(state.composite) {
            if let off = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
                                   space: canvas2DSRGB, bitmapInfo: canvas2DBitmapInfo) {
                off.concatenate(base)
                configureLines(off)
                off.setAlpha(state.alpha)
                draw(off)
                if let layer = off.makeImage() {
                    c.setAlpha(1)
                    c.setBlendMode(canvas2DBlends[state.composite])
                    if shadows { setShadow(c) }
                    c.concatenate(c.ctm.inverted())
                    c.interpolationQuality = .none
                    c.draw(layer, in: CGRect(x: 0, y: 0, width: width, height: height))
                }
            }
        } else {
            c.setAlpha(state.alpha)
            c.setBlendMode(canvas2DBlends[state.composite])
            if shadows {
                setShadow(c)
                c.beginTransparencyLayer(auxiliaryInfo: nil)
                draw(c)
                c.endTransparencyLayer()
            } else {
                draw(c)
            }
        }
        c.restoreGState()
    }

    /// Fill `canvasPath` (canvas coordinates) with the fill style.
    func fillPath(_ c: CGContext, _ canvasPath: CGPath, rule: CGPathFillRule) {
        guard !canvasPath.isEmpty else { return }
        // The common paint, a colour under source-over or a blend with no
        // shadow, needs no saved state: every paint sets the alpha, the
        // operator and the colour it uses. Five calls where `render` makes
        // thirteen, which a recording context records one by one.
        if case .color(let color) = state.fill, !shadows, !canvas2DClipsExtent(state.composite) {
            c.setAlpha(state.alpha)
            c.setBlendMode(canvas2DBlends[state.composite])
            c.setFillColor(color)
            c.addPath(canvasPath)
            c.fillPath(using: rule)
            return
        }
        render(c) { c in
            if case .color(let color) = state.fill {
                c.setFillColor(color)
                c.addPath(canvasPath)
                c.fillPath(using: rule)
            } else {
                c.addPath(canvasPath)
                c.clip(using: rule)
                paintArea(c, state.fill)
            }
        }
    }

    /// Stroke a canvas-space path: back to user space, stroked there.
    func strokeCanvasPath(_ c: CGContext, _ canvasPath: CGPath) {
        guard !canvasPath.isEmpty else { return }
        var inverse = state.author.inverted()
        if let user = canvasPath.copy(using: &inverse) { strokeUserPath(c, user) }
    }

    /// Stroke `userPath` (user space) under base ∘ author.
    func strokeUserPath(_ c: CGContext, _ userPath: CGPath) {
        render(c) { c in
            c.concatenate(state.author)
            c.addPath(userPath)
            if case .color(let color) = state.stroke {
                c.setStrokeColor(color)
                c.strokePath()
            } else {
                c.replacePathWithStrokedPath()
                c.clip()
                resetToBase(c)
                paintArea(c, state.stroke)
            }
        }
    }

    /// Fill the whole clip with a style. The context is at the base
    /// transform; gradients and patterns are in the author's space.
    func paintArea(_ c: CGContext, _ style: Canvas2DStyle) {
        switch style {
        case .color(let color):
            c.setFillColor(color)
            c.fill(CGRect(x: -1e7, y: -1e7, width: 2e7, height: 2e7))
        case .gradient(let id):
            guard let g = gradients[id], !g.stops.isEmpty else { return }
            if case .conic(let p) = g.kind { drawConic(c, g, p); return }
            drawGradient(c, g)
        case .pattern(let id):
            guard let p = patterns[id] else { return }
            drawPattern(c, p)
        }
    }

    private func drawGradient(_ c: CGContext, _ g: Canvas2DGradient) {
        c.concatenate(state.author)
        if g.stops.count == 1 {
            c.setFillColor(g.stops[0].1)
            c.fill(CGRect(x: -1e7, y: -1e7, width: 2e7, height: 2e7))
            return
        }
        guard let gradient = CGGradient(colorsSpace: canvas2DSRGB, colors: g.stops.map(\.1) as CFArray,
                                        locations: g.stops.map { CGFloat($0.0) }) else { return }
        let extend: CGGradientDrawingOptions = [.drawsBeforeStartLocation, .drawsAfterEndLocation]
        switch g.kind {
        case .linear(let l):
            if l[0] == l[2] && l[1] == l[3] { return }
            c.drawLinearGradient(gradient, start: CGPoint(x: l[0], y: l[1]), end: CGPoint(x: l[2], y: l[3]), options: extend)
        case .radial(let r):
            if r[0] == r[3] && r[1] == r[4] && r[2] == r[5] { return }
            c.drawRadialGradient(gradient, startCenter: CGPoint(x: r[0], y: r[1]), startRadius: CGFloat(r[2]),
                                 endCenter: CGPoint(x: r[3], y: r[4]), endRadius: CGFloat(r[5]), options: extend)
        case .conic: break
        }
    }

    /// A conic gradient: Core Graphics has none public, so each device pixel
    /// is shaded by its angle about the centre in gradient space, and the
    /// image fills the clip.
    private func drawConic(_ c: CGContext, _ g: Canvas2DGradient, _ p: [Double]) {
        let toDevice = state.author.concatenating(base)
        let inverse = toDevice.inverted()
        let (w, h) = (width, height)
        let stops = g.stops.map { stop -> (Double, [Double]) in
            let comps = stop.1.converted(to: canvas2DSRGB, intent: .defaultIntent, options: nil)?.components ?? [0, 0, 0, 1]
            return (stop.0, comps.map(Double.init))
        }
        let tau = 2 * Double.pi
        var bytes = [UInt8](repeating: 0, count: w * h * 4)
        for row in 0..<h {
            let dy = Double(h - row) - 0.5
            for col in 0..<w {
                let q = CGPoint(x: Double(col) + 0.5, y: dy).applying(inverse)
                var t = (atan2(Double(q.y) - p[2], Double(q.x) - p[1]) - p[0]).truncatingRemainder(dividingBy: tau)
                if t < 0 { t += tau }
                t /= tau
                var rgba = stops.last!.1
                if t <= stops[0].0 { rgba = stops[0].1 } else if let k = stops.firstIndex(where: { $0.0 >= t }), k > 0 {
                    let (a, b) = (stops[k - 1], stops[k])
                    let f = b.0 > a.0 ? (t - a.0) / (b.0 - a.0) : 1
                    rgba = (0..<4).map { a.1[$0] + (b.1[$0] - a.1[$0]) * f }
                }
                let i = (row * w + col) * 4
                for k in 0..<4 { bytes[i + k] = UInt8(max(0, min(255, (rgba[k] * 255).rounded()))) }
            }
        }
        guard let provider = CGDataProvider(data: Data(bytes) as CFData),
              let image = CGImage(width: w, height: h, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: w * 4, space: canvas2DSRGB,
                                  bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.last.rawValue), provider: provider,
                                  decode: nil, shouldInterpolate: false, intent: .defaultIntent) else { return }
        c.concatenate(c.ctm.inverted())
        c.interpolationQuality = .none
        c.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
    }

    /// A pattern: the image tiled in pattern space (author ∘ the pattern's
    /// transform), limited to one row, one column or one tile by its
    /// repetition, filling the clip.
    private func drawPattern(_ c: CGContext, _ p: Canvas2DPattern) {
        guard let src = imageSources[p.image], let image = env?.canvasImage(src) else { return }
        let (iw, ih) = (CGFloat(image.width), CGFloat(image.height))
        c.concatenate(state.author)
        c.concatenate(p.transform)
        let far: CGFloat = 1e7
        switch p.repetition {
        case 1: c.clip(to: CGRect(x: -far, y: 0, width: 2 * far, height: ih))
        case 2: c.clip(to: CGRect(x: 0, y: -far, width: iw, height: 2 * far))
        case 3: c.clip(to: CGRect(x: 0, y: 0, width: iw, height: ih))
        default: break
        }
        // Upright tiles in the y-down space: flip each about its own row.
        c.translateBy(x: 0, y: ih)
        c.scaleBy(x: 1, y: -1)
        c.draw(image, in: CGRect(x: 0, y: 0, width: iw, height: ih), byTiling: true)
    }
}

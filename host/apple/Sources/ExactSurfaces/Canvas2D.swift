// @ref LLP 1056 D7 — Canvas 2D on Apple: the recorded lists replayed into a
// Core Graphics bitmap per canvas, on the main thread, and shown as a layer's
// contents in the canvas's content box, under its children. The list's
// geometry is resolved (canvas/src/list.rs): paths arrive in canvas
// coordinates, so fills and clips draw them under the base transform (the
// flip and the device scale), and a stroke takes them back to user space and
// strokes under base ∘ author, so its width follows the author's matrix.
// Every paint goes through `render` (Canvas2DPaint.swift): shadows, global
// alpha and the compositing operators, including the five that reach
// outside the shape. Text is Canvas2DText.swift, images and pixels
// Canvas2DImage.swift. iOS and macOS share these files.
import CoreGraphics
import CoreText
import Foundation
import QuartzCore
import ExactKit

/// The list's opcodes (canvas/src/list.rs `Op`), by number.
enum Canvas2DOp: UInt32 {
    case save = 1, restore = 2, reset = 3, setTransform = 4
    case fillColor = 10, fillGradient = 11, strokeColor = 12, strokeGradient = 13
    case lineWidth = 14, lineCap = 15, lineJoin = 16, miterLimit = 17, lineDash = 18, lineDashOffset = 19
    case globalAlpha = 20, composite = 21, shadowColor = 22, shadowBlur = 23, shadowOffset = 24, imageSmoothing = 25
    case linearGradient = 30, radialGradient = 31, colorStop = 32, conicGradient = 33
    case pattern = 34, patternTransform = 35, fillPattern = 36, strokePattern = 37
    case beginPath = 40, moveTo = 41, lineTo = 42, quadTo = 43, cubicTo = 44, closePath = 45
    case fill = 50, stroke = 51, clip = 52, fillRect = 53, strokeRect = 54, clearRect = 55
    case font = 60, fillText = 61, strokeText = 62
    case image = 70, drawImage = 71, putImageData = 72
    case pathMoveTo = 80, pathLineTo = 81, pathQuadTo = 82, pathCubicTo = 83, pathClose = 84
    case fillPath = 85, strokePath = 86, clipPath = 87
}

/// `globalCompositeOperation`, by the list's index (`exact_canvas::COMPOSITE`).
let canvas2DBlends: [CGBlendMode] = [
    .normal, .sourceIn, .sourceOut, .sourceAtop, .destinationOver, .destinationIn,
    .destinationOut, .destinationAtop, .plusLighter, .copy, .xor, .multiply, .screen, .overlay,
    .darken, .lighten, .colorDodge, .colorBurn, .hardLight, .softLight, .difference, .exclusion,
    .hue, .saturation, .color, .luminosity,
]

/// The operators that change pixels outside the shape, within the clip
/// (`exact_canvas::composite_clips_extent`).
func canvas2DClipsExtent(_ k: Int) -> Bool { [1, 2, 5, 7, 9].contains(k) }

enum Canvas2DStyle { case color(CGColor), gradient(UInt32), pattern(UInt32) }

struct Canvas2DGradient {
    enum Kind { case linear([Double]), radial([Double]), conic([Double]) }
    var kind: Kind
    var stops: [(Double, CGColor)] = []
}

struct Canvas2DPattern {
    var image: UInt32
    var repetition: Int // 0 repeat, 1 repeat-x, 2 repeat-y, 3 no-repeat
    var transform = CGAffineTransform.identity
}

/// The text style a `Font` record sets.
struct Canvas2DFont: Hashable {
    var size = 10.0, weight = 400, style = 0, stretch = 100.0, caps = 0, kerning = 0, rendering = 0
    var letterSpacing = 0.0, wordSpacing = 0.0
    var families = ["sans-serif"]
}

extension Canvas2DFont {
    /// A `Font` record's style.
    init(record n: [Double], count: Int) {
        self.init()
        size = n[0]; weight = Int(n[1]); style = Int(n[2]); stretch = n[3]; caps = Int(n[4])
        kerning = Int(n[5]); rendering = Int(n[6]); letterSpacing = n[7]; wordSpacing = n[8]
        families = canvas2DText(n, from: 9, count: count).split(separator: ",").map(String.init)
    }
}

/// A record's code points from `from` on, as a string.
func canvas2DText(_ n: [Double], from: Int, count: Int) -> String {
    var s = String.UnicodeScalarView()
    for i in from..<max(from, count) { if let u = Unicode.Scalar(UInt32(max(0, n[i]))) { s.append(u) } }
    return String(s)
}

/// What the replayer keeps beside Core Graphics' own state stack (which
/// holds the clip): everything a paint applies at the paint.
struct Canvas2DState {
    var author = CGAffineTransform.identity
    var fill: Canvas2DStyle = .color(CGColor(srgbRed: 0, green: 0, blue: 0, alpha: 1))
    var stroke: Canvas2DStyle = .color(CGColor(srgbRed: 0, green: 0, blue: 0, alpha: 1))
    var lineWidth: CGFloat = 1, cap = CGLineCap.butt, join = CGLineJoin.miter, miter: CGFloat = 10
    var dash: [CGFloat] = [], dashOffset: CGFloat = 0
    var alpha: CGFloat = 1, composite = 0
    var shadowColor = CGColor(srgbRed: 0, green: 0, blue: 0, alpha: 0), shadowBlur: CGFloat = 0, shadowOffset = CGSize.zero
    var smoothing = true, quality = 0
    var font = Canvas2DFont()
}

let canvas2DSRGB = CGColorSpace(name: CGColorSpace.sRGB)!
let canvas2DBitmapInfo = CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue

/// A canvas's getContext settings (LLP 1100 D12a): `colorSpace:
/// "display-p3"` and `colorType: "float16"`.
struct Canvas2DSpace: Equatable {
    var p3 = false, float16 = false
    static let srgb8 = Canvas2DSpace()
    private static let p3Space = CGColorSpace(name: CGColorSpace.displayP3)!
    private static let extended = (srgb: CGColorSpace(name: CGColorSpace.extendedSRGB)!, p3: CGColorSpace(name: CGColorSpace.extendedDisplayP3)!)

    /// The space a list's colour and pixel bytes are in.
    var space: CGColorSpace { p3 ? Canvas2DSpace.p3Space : canvas2DSRGB }

    /// A zeroed bitmap; `float16` is half floats in the extended space.
    func context(width: Int, height: Int) -> CGContext? {
        guard float16 else {
            return CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: space, bitmapInfo: canvas2DBitmapInfo)
        }
        return CGContext(data: nil, width: width, height: height, bitsPerComponent: 16, bytesPerRow: 0,
                         space: p3 ? Canvas2DSpace.extended.p3 : Canvas2DSpace.extended.srgb,
                         bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.floatComponents.rawValue
                             | CGBitmapInfo.byteOrder16Little.rawValue)
    }

    /// 8-bit RGBA bytes in this space (non-premultiplied, alpha last) as an image.
    func image(_ bytes: [UInt8], width: Int, height: Int) -> CGImage? {
        guard let provider = CGDataProvider(data: Data(bytes) as CFData) else { return nil }
        return CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4, space: space,
                       bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.last.rawValue), provider: provider,
                       decode: nil, shouldInterpolate: false, intent: .defaultIntent)
    }
}

/// What a replayer draws with beyond its list: the session's fonts and its
/// decoded image handles (the presenter's `Canvas2DHost`).
protocol Canvas2DEnv: AnyObject {
    /// The Core Text font a `font` record resolves to.
    func canvasFont(_ f: Canvas2DFont) -> CTFont?
    func canvasImage(_ src: String) -> CGImage?
}

/// One canvas's bitmap and replayer.
final class Canvas2DReplayer {
    let lifetime: UInt64
    let generation: UInt32
    let context: CGContext?
    let base: CGAffineTransform
    let width: Int, height: Int
    var state = Canvas2DState()
    var stack: [Canvas2DState] = []
    /// The current path, in canvas coordinates; it survives painting.
    var path = CGMutablePath()
    /// A `Path2D`'s segments for the next path paint.
    var scratch = CGMutablePath()
    /// What a tracker (`Canvas2DRecord.swift`) has seen; nil for a replayer
    /// that paints.
    var tracking: Canvas2DTracking?
    var gradients: [UInt32: Canvas2DGradient] = [:]
    var patterns: [UInt32: Canvas2DPattern] = [:]
    var imageSources: [UInt32: String] = [:]
    weak var env: Canvas2DEnv?
    var space = Canvas2DSpace.srgb8
    /// Nothing has painted since the bitmap was made or last cleared whole,
    /// so `reset` has nothing to clear. A draw that starts with
    /// `ctx.reset()` (the idiom for a canvas that redraws) otherwise writes
    /// every pixel of a fresh bitmap once for nothing: a third of a list
    /// row's canvas replay at 3x (LLP 1056 D10).
    private var blank = true

    convenience init(width: Int, height: Int, scale: Double, lifetime: UInt64, generation: UInt32, space: Canvas2DSpace = .srgb8) {
        let context = width > 0 && height > 0 ? space.context(width: width, height: height) : nil
        // Flip once to the canvas's y-down space, then the device scale.
        let base = CGAffineTransform(a: CGFloat(scale), b: 0, c: 0, d: -CGFloat(scale), tx: 0, ty: CGFloat(height))
        self.init(context: context, base: base, width: width, height: height, lifetime: lifetime, generation: generation)
        self.space = space
    }

    /// A replayer into `context`, whose user space `base` maps from canvas
    /// coordinates.
    init(context: CGContext?, base: CGAffineTransform, width: Int, height: Int, lifetime: UInt64, generation: UInt32) {
        self.lifetime = lifetime; self.generation = generation
        self.width = width; self.height = height
        self.context = context
        self.base = base
        if let c = context {
            c.concatenate(base)
            // The root state: `reset` restores to it, dropping every clip.
            // A bitmap context Core Graphics allocates starts zeroed, which is
            // the transparent black a new canvas is: no clear here.
            c.saveGState()
        }
    }

    /// The device scale the base transform applies.
    var scale: CGFloat { base.a }

    func color(_ n: [Double], _ i: Int) -> CGColor {
        guard space.p3 else { return CGColor(srgbRed: n[i] / 255, green: n[i + 1] / 255, blue: n[i + 2] / 255, alpha: n[i + 3]) }
        return CGColor(colorSpace: space.space, components: [n[i] / 255, n[i + 1] / 255, n[i + 2] / 255, n[i + 3]])
            ?? CGColor(srgbRed: 0, green: 0, blue: 0, alpha: 1)
    }

    private func rect(_ n: [Double]) -> CGRect { CGRect(x: n[0], y: n[1], width: n[2], height: n[3]) }

    private func text(_ n: [Double], from: Int, count: Int) -> String { canvas2DText(n, from: from, count: count) }

    /// Read one list's records in order; false when it is not one this
    /// reader can read.
    static func read(_ data: Data, _ record: (Canvas2DOp, [Double], Int) -> Void) -> Bool {
        data.withUnsafeBytes { (raw: UnsafeRawBufferPointer) -> Bool in
            guard raw.count >= 8, raw.loadUnaligned(fromByteOffset: 0, as: UInt32.self).littleEndian == 0x4432_4345,
                  raw.loadUnaligned(fromByteOffset: 4, as: UInt32.self).littleEndian == 1 else { return false }
            var at = 8
            var n = [Double](repeating: 0, count: 16)
            while at + 8 <= raw.count {
                let code = raw.loadUnaligned(fromByteOffset: at, as: UInt32.self).littleEndian
                let count = Int(raw.loadUnaligned(fromByteOffset: at + 4, as: UInt32.self).littleEndian)
                at += 8
                guard at + count * 8 <= raw.count, let op = Canvas2DOp(rawValue: code) else { return false }
                if n.count < count { n = [Double](repeating: 0, count: count) }
                for i in 0..<count { n[i] = Double(bitPattern: raw.loadUnaligned(fromByteOffset: at + i * 8, as: UInt64.self).littleEndian) }
                at += count * 8
                record(op, n, count)
            }
            return at == raw.count
        }
    }

    /// The fonts a list sets, in order: what its text needs resolved.
    /// It runs on the main thread for every list, so it walks the record
    /// headers and decodes only `font` records' operands: decoding every
    /// record was most of `apply`'s main-thread time for a full-screen
    /// canvas drawn every frame (3,000 arcs a list, F2).
    static func fonts(in data: Data) -> [Canvas2DFont] {
        var out: [Canvas2DFont] = []
        data.withUnsafeBytes { (raw: UnsafeRawBufferPointer) in
            guard raw.count >= 8, raw.loadUnaligned(fromByteOffset: 0, as: UInt32.self).littleEndian == 0x4432_4345,
                  raw.loadUnaligned(fromByteOffset: 4, as: UInt32.self).littleEndian == 1 else { return }
            var at = 8
            while at + 8 <= raw.count {
                let code = raw.loadUnaligned(fromByteOffset: at, as: UInt32.self).littleEndian
                let count = Int(raw.loadUnaligned(fromByteOffset: at + 4, as: UInt32.self).littleEndian)
                at += 8
                guard count >= 0, at + count * 8 <= raw.count else { return }
                if code == Canvas2DOp.font.rawValue {
                    let n = (0..<count).map { Double(bitPattern: raw.loadUnaligned(fromByteOffset: at + $0 * 8, as: UInt64.self).littleEndian) }
                    out.append(Canvas2DFont(record: n, count: count))
                }
                at += count * 8
            }
        }
        return out
    }

    /// Apply one list; false when it is not one this reader can read.
    func apply(_ data: Data) -> Bool {
        guard let c = context else { return true }
        return Canvas2DReplayer.read(data) { op, n, count in step(c, op, n, count) }
    }

    // swiftlint:disable:next cyclomatic_complexity function_body_length
    private func step(_ c: CGContext, _ op: Canvas2DOp, _ n: [Double], _ count: Int) {
        if tracking != nil, track(op, n) { return }
        switch op {
        case .fill, .fillPath, .stroke, .strokePath, .fillRect, .strokeRect, .fillText, .strokeText, .drawImage, .putImageData:
            blank = false
        default: break
        }
        switch op {
        case .save: stack.append(state); c.saveGState()
        case .restore:
            if let s = stack.popLast() { state = s; c.restoreGState() }
        case .reset:
            while stack.popLast() != nil { c.restoreGState() }
            c.restoreGState(); c.saveGState()
            state = Canvas2DState(); path = CGMutablePath(); scratch = CGMutablePath()
            if !blank { c.clear(CGRect(x: -1e7, y: -1e7, width: 2e7, height: 2e7)); blank = true }
        case .setTransform: state.author = CGAffineTransform(a: n[0], b: n[1], c: n[2], d: n[3], tx: n[4], ty: n[5])
        case .fillColor: state.fill = .color(color(n, 0))
        case .fillGradient: state.fill = .gradient(UInt32(n[0]))
        case .fillPattern: state.fill = .pattern(UInt32(n[0]))
        case .strokeColor: state.stroke = .color(color(n, 0))
        case .strokeGradient: state.stroke = .gradient(UInt32(n[0]))
        case .strokePattern: state.stroke = .pattern(UInt32(n[0]))
        case .lineWidth: state.lineWidth = n[0]
        case .lineCap: state.cap = [CGLineCap.butt, .round, .square][Int(n[0])]
        case .lineJoin: state.join = [CGLineJoin.miter, .round, .bevel][Int(n[0])]
        case .miterLimit: state.miter = n[0]
        case .lineDash: state.dash = (0..<count).map { CGFloat(n[$0]) }
        case .lineDashOffset: state.dashOffset = n[0]
        case .globalAlpha: state.alpha = n[0]
        case .composite: state.composite = min(Int(n[0]), canvas2DBlends.count - 1)
        case .shadowColor: state.shadowColor = color(n, 0)
        case .shadowBlur: state.shadowBlur = n[0]
        case .shadowOffset: state.shadowOffset = CGSize(width: n[0], height: n[1])
        case .imageSmoothing: state.smoothing = n[0] != 0; state.quality = Int(n[1])
        case .linearGradient: gradients[UInt32(n[0])] = Canvas2DGradient(kind: .linear(Array(n[1...4])))
        case .radialGradient: gradients[UInt32(n[0])] = Canvas2DGradient(kind: .radial(Array(n[1...6])))
        case .conicGradient: gradients[UInt32(n[0])] = Canvas2DGradient(kind: .conic(Array(n[1...3])))
        case .colorStop:
            let id = UInt32(n[0])
            if var g = gradients[id] {
                let i = g.stops.firstIndex { $0.0 > n[1] } ?? g.stops.count
                g.stops.insert((n[1], color(n, 2)), at: i)
                gradients[id] = g
            }
        case .pattern: patterns[UInt32(n[0])] = Canvas2DPattern(image: UInt32(n[1]), repetition: Int(n[2]))
        case .patternTransform:
            patterns[UInt32(n[0])]?.transform = CGAffineTransform(a: n[1], b: n[2], c: n[3], d: n[4], tx: n[5], ty: n[6])
        case .beginPath: path = CGMutablePath()
        case .moveTo: path.move(to: CGPoint(x: n[0], y: n[1]))
        case .lineTo: Canvas2DReplayer.line(path, n)
        case .quadTo: path.addQuadCurve(to: CGPoint(x: n[2], y: n[3]), control: CGPoint(x: n[0], y: n[1]))
        case .cubicTo:
            path.addCurve(to: CGPoint(x: n[4], y: n[5]), control1: CGPoint(x: n[0], y: n[1]), control2: CGPoint(x: n[2], y: n[3]))
        case .closePath: if !path.isEmpty { path.closeSubpath() }
        case .pathMoveTo: scratch.move(to: CGPoint(x: n[0], y: n[1]))
        case .pathLineTo: Canvas2DReplayer.line(scratch, n)
        case .pathQuadTo: scratch.addQuadCurve(to: CGPoint(x: n[2], y: n[3]), control: CGPoint(x: n[0], y: n[1]))
        case .pathCubicTo:
            scratch.addCurve(to: CGPoint(x: n[4], y: n[5]), control1: CGPoint(x: n[0], y: n[1]), control2: CGPoint(x: n[2], y: n[3]))
        case .pathClose: if !scratch.isEmpty { scratch.closeSubpath() }
        case .fill: fillPath(c, path, rule: n[0] == 1 ? .evenOdd : .winding)
        case .fillPath: fillPath(c, scratch, rule: n[0] == 1 ? .evenOdd : .winding); scratch = CGMutablePath()
        case .stroke: strokeCanvasPath(c, path)
        case .strokePath: strokeCanvasPath(c, scratch); scratch = CGMutablePath()
        case .clip: clip(c, path, rule: n[0] == 1 ? .evenOdd : .winding)
        case .clipPath: clip(c, scratch, rule: n[0] == 1 ? .evenOdd : .winding); scratch = CGMutablePath()
        case .fillRect:
            var author = state.author
            fillPath(c, CGPath(rect: rect(n), transform: &author), rule: .winding)
        case .strokeRect:
            let r = rect(n)
            if r.width == 0 && r.height == 0 { break }
            let p = CGMutablePath()
            if r.width == 0 || r.height == 0 {
                p.move(to: r.origin); p.addLine(to: CGPoint(x: r.maxX, y: r.maxY))
            } else {
                p.addRect(r)
            }
            strokeUserPath(c, p)
        case .clearRect:
            c.saveGState()
            c.concatenate(state.author)
            c.setBlendMode(.clear)
            c.setAlpha(1)
            c.addRect(rect(n))
            c.fillPath()
            c.restoreGState()
        case .font: state.font = Canvas2DFont(record: n, count: count)
        case .fillText, .strokeText:
            guard count >= 4 else { break }
            drawText(c, fill: op == .fillText, x: n[0], y: n[1], scaleX: n[2], rtl: n[3] != 0, text: text(n, from: 4, count: count))
        case .image: imageSources[UInt32(n[0])] = text(n, from: 1, count: count)
        case .drawImage: drawImage(c, n)
        case .putImageData: putImageData(c, n, count)
        }
    }

    private static func line(_ p: CGMutablePath, _ n: [Double]) {
        p.isEmpty ? p.move(to: CGPoint(x: n[0], y: n[1])) : p.addLine(to: CGPoint(x: n[0], y: n[1]))
    }

    private func clip(_ c: CGContext, _ p: CGPath, rule: CGPathFillRule) {
        if p.isEmpty { c.clip(to: CGRect.zero) } else { c.addPath(p); c.clip(using: rule) }
    }

    func image() -> CGImage? { context?.makeImage() }
}

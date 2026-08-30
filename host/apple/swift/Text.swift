// Text: CoreText, one engine for measuring and painting (LLP 1008 §3; the
// lesson of exact1's LLP 0418/0430 — TextKit is ~5× slower per wrap, and
// measuring with one engine while painting with another is a correctness
// tax). A Paragraph is a width-specific snapshot: the wrapped CTLines, their
// baselines, and the size — the object that answers the kernel's measure
// and the object `draw` paints, cached by (spec, width). Fonts are cached.
//
// Shared by the AppKit and UIKit presenters (host/apple/swift): the two
// differ only in the font and color classes, and the same CoreText answers
// the kernel on both, so a paragraph wraps the same way on macOS and iOS.
#if canImport(UIKit)
import UIKit
typealias PlatformFont = UIFont
typealias PlatformColor = UIColor
#else
import AppKit
typealias PlatformFont = NSFont
typealias PlatformColor = NSColor
#endif
import CExact
import CoreText

/// One styled run: what changes glyph metrics.
struct Run: Hashable {
    var text: String
    var size: CGFloat
    var weight: Int
    var italic: Bool
    var lineHeight: CGFloat
    var letterSpacing: CGFloat
}

/// A paragraph's specification: runs plus paragraph style.
struct Spec: Hashable {
    var runs: [Run]
    var align: Int // 0 left, 1 center, 2 right, 3 justify
    var lineClamp: Int
    var color: [Double] // r g b a, 0–255
}

/// A wrapped paragraph at one width: what is measured is what is painted.
final class Paragraph {
    let lines: [CTLine]
    /// Baseline of each line, measured from the top.
    let baselines: [CGFloat]
    let width: CGFloat
    let height: CGFloat
    var firstBaseline: CGFloat { baselines.first ?? 0 }
    init(lines: [CTLine], baselines: [CGFloat], width: CGFloat, height: CGFloat) {
        self.lines = lines
        self.baselines = baselines
        self.width = width
        self.height = height
    }
}

/// How many times the kernel asked, how many were answered from cache, and
/// how long the misses took, since launch.
nonisolated(unsafe) var measureCount = 0
nonisolated(unsafe) var measureHits = 0
nonisolated(unsafe) var measureSeconds = 0.0

enum Text {
    nonisolated(unsafe) static var fonts: [String: PlatformFont] = [:]
    nonisolated(unsafe) static var paragraphs: [Int: Paragraph] = [:]

    static func font(size: CGFloat, weight: Int, italic: Bool) -> PlatformFont {
        let key = "\(size)/\(weight)/\(italic)"
        if let f = fonts[key] { return f }
        let w: PlatformFont.Weight
        switch weight {
        case ..<200: w = .ultraLight
        case 200..<300: w = .thin
        case 300..<400: w = .light
        case 400..<500: w = .regular
        case 500..<600: w = .medium
        case 600..<700: w = .semibold
        case 700..<800: w = .bold
        case 800..<900: w = .heavy
        default: w = .black
        }
        var f = PlatformFont.systemFont(ofSize: size, weight: w)
        if italic {
            #if canImport(UIKit)
            if let d = f.fontDescriptor.withSymbolicTraits(.traitItalic) { f = UIFont(descriptor: d, size: size) }
            #else
            f = NSFontManager.shared.convert(f, toHaveTrait: .italicFontMask)
            #endif
        }
        fonts[key] = f
        return f
    }

    /// A color from the style dictionary's `[r,g,b,a]` bytes (sRGB).
    static func color(_ c: [Double]) -> PlatformColor {
        #if canImport(UIKit)
        UIColor(red: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
        #else
        NSColor(srgbRed: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
        #endif
    }

    static func attributed(_ spec: Spec) -> NSAttributedString {
        let s = NSMutableAttributedString()
        let color = Text.color(spec.color)
        for r in spec.runs {
            var a: [NSAttributedString.Key: Any] = [.font: font(size: r.size, weight: r.weight, italic: r.italic), .foregroundColor: color]
            if r.letterSpacing != 0 { a[.kern] = r.letterSpacing }
            s.append(NSAttributedString(string: r.text, attributes: a))
        }
        return s
    }

    /// Wrap `spec` at `width` (infinite = max-content). Cached.
    static func paragraph(_ spec: Spec, width: CGFloat) -> Paragraph {
        var h = Hasher()
        h.combine(spec)
        h.combine(width)
        let key = h.finalize()
        if let p = paragraphs[key] { return p }
        if paragraphs.count > 4096 { paragraphs.removeAll(keepingCapacity: true) }
        let p = layout(spec, width: width)
        paragraphs[key] = p
        return p
    }

    private static func layout(_ spec: Spec, width: CGFloat) -> Paragraph {
        let s = attributed(spec)
        let typesetter = CTTypesetterCreateWithAttributedString(s)
        let length = s.length
        let lineHeight = spec.runs.map(\.lineHeight).max() ?? 0
        var lines: [CTLine] = []
        var baselines: [CGFloat] = []
        var maxWidth: CGFloat = 0
        var y: CGFloat = 0
        var start = 0
        let limit = width.isFinite ? Double(width) : Double.greatestFiniteMagnitude
        while start < length {
            if spec.lineClamp > 0 && lines.count == spec.lineClamp { break }
            var count = CTTypesetterSuggestLineBreak(typesetter, start, limit)
            if count <= 0 { count = length - start }
            var line = CTTypesetterCreateLine(typesetter, CFRangeMake(start, count))
            if spec.lineClamp > 0 && lines.count + 1 == spec.lineClamp && start + count < length,
               let token = CTLineCreateWithAttributedString(NSAttributedString(string: "…", attributes: s.attributes(at: start, effectiveRange: nil))) as CTLine?,
               let truncated = CTLineCreateTruncatedLine(line, limit, .end, token) {
                line = truncated
            }
            var ascent: CGFloat = 0, descent: CGFloat = 0, leading: CGFloat = 0
            let w = CGFloat(CTLineGetTypographicBounds(line, &ascent, &descent, &leading))
            // CSS: `line-height: normal` is the font's ascent + descent +
            // line gap; a set line-height centers the glyphs in the box.
            let natural = ascent + descent + leading
            let box = lineHeight > 0 ? lineHeight : natural
            let half = lineHeight > 0 ? (lineHeight - natural) / 2 : 0
            baselines.append(y + half + ascent)
            y += box
            maxWidth = max(maxWidth, w)
            lines.append(line)
            start += count
        }
        if lines.isEmpty {
            // Empty text still has a line box.
            let f0 = spec.runs.first.map { Text.font(size: $0.size, weight: $0.weight, italic: $0.italic) } ?? PlatformFont.systemFont(ofSize: 16)
            let natural = f0.ascender - f0.descender + f0.leading
            let box = lineHeight > 0 ? lineHeight : natural
            baselines.append(f0.ascender + (lineHeight > 0 ? (lineHeight - natural) / 2 : 0))
            y = box
        }
        return Paragraph(lines: lines, baselines: baselines, width: ceil(maxWidth), height: ceil(y))
    }

    /// As narrow as the content can be: the longest unbreakable piece.
    static func minContentWidth(_ spec: Spec) -> CGFloat {
        var widest: CGFloat = 0
        for r in spec.runs {
            for word in r.text.split(whereSeparator: { $0.isWhitespace }) {
                var one = spec
                one.runs = [Run(text: String(word), size: r.size, weight: r.weight, italic: r.italic, lineHeight: r.lineHeight, letterSpacing: r.letterSpacing)]
                widest = max(widest, paragraph(one, width: .infinity).width)
            }
        }
        return widest
    }

    /// Paint a paragraph into a y-down context (a flipped NSView's, a
    /// UIView's): one CTLineDraw per line, baselines snapped to device
    /// pixels, flush by alignment.
    static func draw(_ p: Paragraph, spec: Spec, in bounds: CGRect, context ctx: CGContext) {
        ctx.saveGState()
        ctx.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
        let flush: CGFloat = spec.align == 1 ? 0.5 : spec.align == 2 ? 1 : 0
        for (line, baseline) in zip(p.lines, p.baselines) {
            let x = CGFloat(CTLineGetPenOffsetForFlush(line, flush, Double(bounds.width)))
            ctx.textPosition = CGPoint(x: bounds.minX + x, y: bounds.minY + baseline.rounded())
            CTLineDraw(line, ctx)
        }
        ctx.restoreGState()
    }
}

/// The kernel's text measurer: called for every paragraph it lays out. The
/// paragraph it wraps to answer is the one the presenter paints.
let measureText: ExactMeasureFn = { _, request in
    measureCount += 1
    guard let request = request?.pointee else { return ExactMetrics(width: 0, height: 0, baseline: -1) }
    let runs = UnsafeBufferPointer(start: request.runs, count: request.count).map { run in
        Run(text: String(decoding: UnsafeBufferPointer(start: run.text, count: run.len), as: UTF8.self), size: CGFloat(run.font_size), weight: Int(run.font_weight), italic: run.italic != 0, lineHeight: CGFloat(run.line_height), letterSpacing: CGFloat(run.letter_spacing))
    }
    // Color does not change metrics; measure everything as black so the
    // cache is shared with the painted paragraph (which re-keys by color).
    let spec = Spec(runs: runs, align: Int(request.align), lineClamp: Int(request.line_clamp), color: [0, 0, 0, 255])
    let started = CACurrentMediaTime()
    let width: CGFloat = request.width == EXACT_MIN_CONTENT ? Text.minContentWidth(spec) : request.width < 0 ? .infinity : CGFloat(request.width)
    let before = Text.paragraphs.count
    let p = Text.paragraph(spec, width: width)
    if Text.paragraphs.count == before { measureHits += 1 } else { measureSeconds += CACurrentMediaTime() - started }
    return ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline))
}

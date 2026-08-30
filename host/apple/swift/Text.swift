// Text: CoreText, one engine for measuring and painting (LLP 1008 §3; the
// lesson of exact1's LLP 0418/0430 — TextKit is ~5× slower per wrap, and
// measuring with one engine while painting with another is a correctness
// tax). A Paragraph is a width-specific, thread-neutral snapshot: CoreText
// lays it out on the runtime thread, then the glyphs, positions, fonts,
// baselines, and size are copied out. CTLine/CTRun never cross queues; the
// snapshot answers the kernel and is what `draw` paints on main.
//
// @ref LLP 1008 §3 (CoreText on the runtime thread; immutable glyph snapshot)
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
}

/// One shaped run copied from a CoreText line. CTFont is explicitly safe to
/// share across queues; glyph and position arrays are immutable values.
struct GlyphRun: @unchecked Sendable {
    let font: CTFont
    let glyphs: [CGGlyph]
    let positions: [CGPoint]
}

/// One shaped line, independent of the CTLine that produced it.
struct Line: @unchecked Sendable {
    let runs: [GlyphRun]
    let width: CGFloat
}

/// A wrapped paragraph at one width: what is measured is what is painted.
final class Paragraph: @unchecked Sendable {
    let lines: [Line]
    /// Baseline of each line, measured from the top.
    let baselines: [CGFloat]
    let width: CGFloat
    let height: CGFloat
    var firstBaseline: CGFloat { baselines.first ?? 0 }
    init(lines: [Line], baselines: [CGFloat], width: CGFloat, height: CGFloat) {
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
    private struct ParagraphKey: Hashable {
        let spec: Spec
        let width: CGFloat
    }

    nonisolated(unsafe) private static var fonts: [String: PlatformFont] = [:]
    nonisolated(unsafe) private static var coreFonts: [String: CTFont] = [:]
    nonisolated(unsafe) private static var paragraphs: [ParagraphKey: Paragraph] = [:]
    private static let fontLock = NSLock()
    private static let paragraphLock = NSLock()

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

    /// The same system face as `font`, as CoreText's thread-safe font object.
    static func coreFont(size: CGFloat, weight: Int, italic: Bool) -> CTFont {
        let key = "\(size)/\(weight)/\(italic)"
        fontLock.lock()
        if let f = coreFonts[key] { fontLock.unlock(); return f }
        fontLock.unlock()
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
        var traits: [CFString: Any] = [kCTFontWeightTrait: w.rawValue]
        if italic { traits[kCTFontSymbolicTrait] = CTFontSymbolicTraits.traitItalic.rawValue }
        let descriptor = CTFontDescriptorCreateWithAttributes([kCTFontTraitsAttribute: traits] as CFDictionary)
        let base = CTFontCreateUIFontForLanguage(.system, size, nil)
            ?? CTFontCreateWithName("Helvetica" as CFString, size, nil)
        let made = CTFontCreateCopyWithAttributes(base, size, nil, descriptor)
        fontLock.lock()
        let result = coreFonts[key] ?? made
        coreFonts[key] = result
        fontLock.unlock()
        return result
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
        for r in spec.runs {
            var a: [NSAttributedString.Key: Any] = [NSAttributedString.Key(kCTFontAttributeName as String): coreFont(size: r.size, weight: r.weight, italic: r.italic)]
            if r.letterSpacing != 0 { a[.kern] = r.letterSpacing }
            s.append(NSAttributedString(string: r.text, attributes: a))
        }
        return s
    }

    /// Wrap `spec` at `width` (infinite = max-content). Cached.
    static func paragraph(_ spec: Spec, width: CGFloat) -> Paragraph {
        resolve(spec, width: width).0
    }

    /// The measurer also needs to report whether the shared snapshot existed.
    static func measuredParagraph(_ spec: Spec, width: CGFloat) -> (Paragraph, Bool) {
        resolve(spec, width: width)
    }

    private static func resolve(_ spec: Spec, width: CGFloat) -> (Paragraph, Bool) {
        let key = ParagraphKey(spec: spec, width: width)
        paragraphLock.lock()
        if let p = paragraphs[key] { paragraphLock.unlock(); return (p, true) }
        paragraphLock.unlock()
        let made = layout(spec, width: width)
        paragraphLock.lock()
        if let p = paragraphs[key] { paragraphLock.unlock(); return (p, true) }
        if paragraphs.count > 4096 { paragraphs.removeAll(keepingCapacity: true) }
        paragraphs[key] = made
        paragraphLock.unlock()
        return (made, false)
    }

    private static func layout(_ spec: Spec, width: CGFloat) -> Paragraph {
        let s = attributed(spec)
        let typesetter = CTTypesetterCreateWithAttributedString(s)
        let length = s.length
        let lineHeight = spec.runs.map(\.lineHeight).max() ?? 0
        var lines: [Line] = []
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
            lines.append(snapshot(line))
            start += count
        }
        if lines.isEmpty {
            // Empty text still has a line box.
            let f0 = spec.runs.first.map { Text.coreFont(size: $0.size, weight: $0.weight, italic: $0.italic) } ?? Text.coreFont(size: 16, weight: 400, italic: false)
            let ascent = CTFontGetAscent(f0), descent = CTFontGetDescent(f0), leading = CTFontGetLeading(f0)
            let natural = ascent + descent + leading
            let box = lineHeight > 0 ? lineHeight : natural
            baselines.append(ascent + (lineHeight > 0 ? (lineHeight - natural) / 2 : 0))
            y = box
        }
        return Paragraph(lines: lines, baselines: baselines, width: ceil(maxWidth), height: ceil(y))
    }

    /// Copy everything drawing needs out of CoreText's queue-confined layout
    /// objects. The CTLine and its CTRuns die before this operation returns.
    private static func snapshot(_ line: CTLine) -> Line {
        var copied: [GlyphRun] = []
        for value in CTLineGetGlyphRuns(line) as NSArray {
            let run = value as! CTRun
            let count = CTRunGetGlyphCount(run)
            guard count > 0 else { continue }
            var glyphs = [CGGlyph](repeating: 0, count: count)
            var positions = [CGPoint](repeating: .zero, count: count)
            glyphs.withUnsafeMutableBufferPointer { CTRunGetGlyphs(run, CFRangeMake(0, 0), $0.baseAddress!) }
            positions.withUnsafeMutableBufferPointer { CTRunGetPositions(run, CFRangeMake(0, 0), $0.baseAddress!) }
            let attributes = CTRunGetAttributes(run) as NSDictionary
            let font = attributes[kCTFontAttributeName] as! CTFont
            copied.append(GlyphRun(font: font, glyphs: glyphs, positions: positions))
        }
        var ascent: CGFloat = 0, descent: CGFloat = 0, leading: CGFloat = 0
        let width = CGFloat(CTLineGetTypographicBounds(line, &ascent, &descent, &leading))
        return Line(runs: copied, width: width)
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
    /// UIView's): copied glyph runs per line, baselines snapped to device
    /// pixels, flush by alignment.
    static func draw(_ p: Paragraph, spec: Spec, color: PlatformColor, in bounds: CGRect, context ctx: CGContext) {
        let flush: CGFloat = spec.align == 1 ? 0.5 : spec.align == 2 ? 1 : 0
        for (line, baseline) in zip(p.lines, p.baselines) {
            let x = flush * max(0, bounds.width - line.width)
            ctx.saveGState()
            ctx.translateBy(x: bounds.minX + x, y: bounds.minY + baseline.rounded())
            ctx.scaleBy(x: 1, y: -1)
            ctx.textMatrix = .identity
            ctx.setFillColor(color.cgColor)
            for run in line.runs {
                run.glyphs.withUnsafeBufferPointer { glyphs in
                    run.positions.withUnsafeBufferPointer { positions in
                        CTFontDrawGlyphs(run.font, glyphs.baseAddress!, positions.baseAddress!, glyphs.count, ctx)
                    }
                }
            }
            ctx.restoreGState()
        }
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
    // Color is applied by the presenter and does not enter shaping, so this
    // exact snapshot is shared with paint.
    let spec = Spec(runs: runs, align: Int(request.align), lineClamp: Int(request.line_clamp))
    let started = CACurrentMediaTime()
    let width: CGFloat = request.width == EXACT_MIN_CONTENT ? Text.minContentWidth(spec) : request.width < 0 ? .infinity : CGFloat(request.width)
    let (p, hit) = Text.measuredParagraph(spec, width: width)
    if hit { measureHits += 1 } else { measureSeconds += CACurrentMediaTime() - started }
    return ExactMetrics(width: Float(p.width), height: Float(p.height), baseline: Float(p.firstBaseline))
}

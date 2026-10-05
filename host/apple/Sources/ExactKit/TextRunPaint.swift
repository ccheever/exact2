// Per-run `text-shadow` and `-webkit-text-stroke` (LLP 1077 D3, D7): CSS
// paints both per inline box, so each run carries its own computed values
// (a run's style crosses with every inherited row resolved). A paragraph
// whose runs all share one shadow keeps it on the paragraph: one layer
// shadow over the raster, or one transparency layer in `draw`. Runs that
// differ carry theirs as an attribute the line painter draws with Core
// Graphics around those glyph runs; Core Text ignores an `NSShadow`.
import CoreText
import Foundation

/// A text node's or run's shadow and stroke rows as they crossed, colours
/// still light/dark pairs: resolved per appearance by `resolve`.
struct RunPaintRows: Equatable {
    /// Offset x, y and blur in points; nil is `none`.
    var shadow: [Double]?
    /// The shadow's colour; nil is `currentcolor`.
    var shadowColor: BatchValue?
    var strokeWidth: Double = 0
    /// The stroke's colour; nil is `currentcolor`.
    var strokeColor: BatchValue?

    init() {}
    init(_ style: NodeStyle) {
        for key in ["text_shadow", "text_stroke_width", "text_stroke_color"] {
            if let value = style[key] { set(key, value) }
        }
    }

    /// Whether a colour here is a `light-dark()` pair.
    var paired: Bool { shadowColor?.isSchemeColor == true || strokeColor?.isSchemeColor == true }
    /// Whether a colour here is the view's tint (LLP 1095 D8).
    var namesTint: Bool { shadowColor?.namesTint == true || strokeColor?.namesTint == true }

    /// One row off the wire; false when `key` is not one of these rows.
    @discardableResult
    mutating func set(_ key: String, _ value: BatchValue) -> Bool {
        switch key {
        case "text_shadow":
            guard case .object(let o) = value, let offset = o["o"]?.numbers, offset.count == 2 else { shadow = nil; return true }
            shadow = offset + [max(0, o["b"]?.number ?? 0)]
            shadowColor = o["c"]
        case "text_stroke_width": strokeWidth = value.number ?? 0
        case "text_stroke_color": strokeColor = value.string == nil ? value : nil
        default: return false
        }
        return true
    }

    /// The shadow (offset x, y, blur, r g b a) and stroke (width, r g b a)
    /// for an appearance, `currentcolor` being `color`, the text's own.
    func resolve(dark: Bool, contrast: Bool? = nil, elevated: Bool = false, tint: PlatformColor? = nil, color: [Double]) -> (shadow: [Double]?, stroke: [Double]?) {
        let shade = shadow.map { $0 + (shadowColor?.textChannels(dark: dark, contrast: contrast, elevated: elevated, tint: tint) ?? color) }
        let stroke = strokeWidth > 0 ? [strokeWidth] + (strokeColor?.textChannels(dark: dark, contrast: contrast, elevated: elevated, tint: tint) ?? color) : nil
        return (shade, stroke)
    }
}

extension Spec {
    /// Each run's shadow is the paragraph's when they all agree (the cheap
    /// path, unchanged); otherwise each run keeps its own and the paragraph
    /// has none. Runs that paint nothing do not count.
    mutating func gatherShadows() {
        let painted = runs.filter { !$0.text.isEmpty }
        guard let first = painted.first else { shadow = nil; return }
        if painted.allSatisfy({ $0.shadow == first.shadow }) {
            shadow = first.shadow
            for i in runs.indices { runs[i].shadow = nil }
        } else {
            shadow = nil
        }
    }
}

extension Spec {
    /// How far runs' own shadows reach past the box to the left and right,
    /// each at most `TextRasterJob.maxShadowReach`: what a band's clip must
    /// admit for them (a paragraph-wide shadow is the layer's, outside it).
    var runShadowReach: (left: CGFloat, right: CGFloat) {
        var left: CGFloat = 0, right: CGFloat = 0
        for run in runs {
            guard let s = run.shadow, TextEngine.isShadow(s) else { continue }
            // As `TextRunShadow.reach`: a Gaussian of σ = blur / 2 is spent by 3σ.
            let spread = s[2] * 1.5 + 1
            left = max(left, spread - s[0]); right = max(right, spread + s[0])
        }
        let cap = TextRasterJob.maxShadowReach
        return (min(left, cap), min(right, cap))
    }
}

extension TextEngine {
    /// A `text-shadow`: offset x, y, blur, then a colour as `color` takes it.
    static func isShadow(_ s: [Double]) -> Bool { s.count == 7 || s.count == 12 }

    static func shadowColor(_ s: [Double]) -> CGColor { color(Array(s[3...])).cgColor }
}

/// A run's own shadow as a Core Text attribute: offset x, y and blur in
/// points (CSS's radius), and its colour.
final class TextRunShadow: NSObject {
    let offset: CGSize
    let blur: CGFloat
    let color: CGColor
    /// `s`: offset x, y, blur, then the colour, as `Run.shadow`.
    init(_ s: [Double]) {
        offset = CGSize(width: s[0], height: s[1]); blur = s[2]
        color = TextEngine.shadowColor(s)
    }
    override func isEqual(_ object: Any?) -> Bool {
        guard let other = object as? TextRunShadow else { return false }
        return offset == other.offset && blur == other.blur && color == other.color
    }
    override var hash: Int { offset.width.hashValue ^ offset.height.hashValue ^ blur.hashValue ^ color.hashValue }

    static func of(_ run: CTRun) -> TextRunShadow? {
        (CTRunGetAttributes(run) as NSDictionary)[NSAttributedString.Key.exactShadow] as? TextRunShadow
    }

    /// Core Graphics' shadow is in base space, y up: points in a view's
    /// context, pixels (`scale`) in a bitmap the host made.
    func set(on ctx: CGContext, scale: CGFloat) {
        ctx.setShadow(offset: CGSize(width: offset.width * scale, height: -offset.height * scale), blur: blur * scale, color: color)
    }

    /// The ink a line's run shadows add past its glyphs' (`ink`, y down).
    static func reach(_ line: CTLine, ink: CGRect) -> CGRect {
        var out = CGRect.null
        var seen: [TextRunShadow] = []
        for run in CTLineGetGlyphRuns(line) as! [CTRun] {
            guard let s = of(run), !seen.contains(s) else { continue }
            seen.append(s)
            // A Gaussian of σ = blur / 2 is spent by 3σ.
            let spread = s.blur * 1.5 + 1
            out = out.union(ink.offsetBy(dx: s.offset.width, dy: s.offset.height).insetBy(dx: -spread, dy: -spread))
        }
        return out
    }
}

extension NSAttributedString.Key {
    static let exactHidden = NSAttributedString.Key("ExactRunHidden")
    static let exactShadow = NSAttributedString.Key("ExactRunShadow")
}

extension TextLinePaint {
    /// A line whose glyph runs carry their own shadows, in a context already
    /// at the line's origin, y up: each stretch of runs sharing one shadow
    /// drawn in a transparency layer that casts it, under those glyphs, as
    /// Chrome paints each inline box's shadow with its text.
    static func drawShadowed(_ line: CTLine, in ctx: CGContext, scale: CGFloat) -> Bool {
        let runs = CTLineGetGlyphRuns(line) as! [CTRun]
        let shadows = runs.map(TextRunShadow.of)
        func hidden(_ run: CTRun) -> Bool { (CTRunGetAttributes(run) as NSDictionary)[NSAttributedString.Key.exactHidden] as? Bool == true }
        guard shadows.contains(where: { $0 != nil }) || runs.contains(where: hidden) else { return false }
        var i = 0
        while i < runs.count {
            var j = i + 1
            while j < runs.count, shadows[j] == shadows[i] { j += 1 }
            if let s = shadows[i] {
                ctx.saveGState()
                s.set(on: ctx, scale: scale)
                ctx.beginTransparencyLayer(auxiliaryInfo: nil)
            }
            for k in i..<j where !hidden(runs[k]) {
                ctx.textPosition = .zero
                CTRunDraw(runs[k], ctx, CFRange())
            }
            if shadows[i] != nil { ctx.endTransparencyLayer(); ctx.restoreGState() }
            i = j
        }
        return true
    }
}

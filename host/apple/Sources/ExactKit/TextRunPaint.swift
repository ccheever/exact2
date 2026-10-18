// Per-run `text-shadow` and `-webkit-text-stroke` (LLP 1077 D3, D7): CSS
// paints both per inline box, so each run carries its own computed values
// (a run's style crosses with every inherited row resolved). A paragraph
// whose runs all share one shadow keeps it on the paragraph: one layer
// shadow over the raster, or one transparency layer in `draw`. Runs that
// differ carry theirs as an attribute the line painter draws with Core
// Graphics around those glyph runs; Core Text ignores an `NSShadow`.
import CoreText
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// A text node's or run's shadow and stroke rows as they crossed, colours
/// still light/dark pairs: resolved per appearance by `resolve`.


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
    /// How far the shadows a raster paints reach past the box to the left
    /// and right, each at most `TextRasterJob.maxShadowReach`: what a band's
    /// clip must admit for them. Runs' own, and the paragraph's when it is
    /// HDR (`hdrShadow`); an SDR paragraph-wide shadow is the layer's,
    /// outside the pixels.
    var shadowReach: (left: CGFloat, right: CGFloat) {
        var left: CGFloat = 0, right: CGFloat = 0
        for shadow in runs.map(\.shadow) + [hdrShadow] {
            guard let s = shadow, TextEngine.isShadow(s) else { continue }
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
    /// `Run.fixedInk`: the run's colour is one its author fixed.
    static let exactFixedInk = NSAttributedString.Key("ExactRunFixedInk")
}

extension TextLinePaint {
    /// A line's ink in one colour, into a y-down context: how selected text
    /// is drawn over its selection's fill (LLP 1115.000 D7.9). Each glyph at
    /// its own coverage whatever its ink's alpha, so a label colour, which is
    /// part transparent, is solid when selected, as AppKit's is. An underline,
    /// a line-through and a stroke are CoreText's own, recoloured where it
    /// draws them. Emoji and other colour glyphs stay as they are. A hidden
    /// run paints nothing, nor does one whose ink is fully transparent,
    /// unless `transparentInk` (`background-clip: text`, whose glyphs show).
    /// With `keepsFixed` (the light appearance), a run whose colour its
    /// author fixed (`Run.fixedInk`) is drawn as it was, its own colour at
    /// its own alpha and its decorations with it, as an `NSTextView` leaves it.
    static func draw(_ line: CTLine, at origin: CGPoint, in ctx: CGContext, ink: CGColor, keepsFixed: Bool = false, transparentInk: Bool = false) {
        enum Paint { case nothing, own, ink, decorated, keptLined }
        let runs = CTLineGetGlyphRuns(line) as! [CTRun]
        var solid = [Bool](repeating: true, count: runs.count)
        let paints: [Paint] = runs.enumerated().map { i, run in
            let a = CTRunGetAttributes(run) as NSDictionary
            if a[NSAttributedString.Key.exactHidden] as? Bool == true { return .nothing }
            if let font = a[kCTFontAttributeName], CTFontGetSymbolicTraits(font as! CTFont).contains(.traitColorGlyphs) { return .own }
            let alpha = (a[NSAttributedString.Key.foregroundColor] as? PlatformColor)?.cgColor.alpha ?? 1
            if alpha == 0, !transparentInk { return .nothing }
            solid[i] = alpha == 1
            let lined = { (key: NSAttributedString.Key) in (a[key] as? NSNumber)?.doubleValue ?? 0 != 0 }
            let decorated = lined(.underlineStyle) || lined(.strikethroughStyle) || lined(.strokeWidth)
            if keepsFixed, alpha > 0, a[NSAttributedString.Key.exactFixedInk] as? Bool == true { return decorated ? .keptLined : .own }
            return alpha > 0 && decorated ? .decorated : .ink
        }
        ctx.saveGState()
        defer { ctx.restoreGState() }
        // As `draw(_:at:in:)`: the CTM flips y, and the caller's text matrix is put back.
        let matrix = ctx.textMatrix
        defer { ctx.textMatrix = matrix }
        ctx.translateBy(x: origin.x, y: origin.y)
        ctx.scaleBy(x: 1, y: -1)
        ctx.textMatrix = .identity
        let decorated = zip(runs, paints).filter { $0.1 == .decorated }.map { extent($0.0) }
        if !decorated.isEmpty {
            // CoreText draws a decoration only with its whole line
            // (`CTRunDraw` draws none): the line, clipped to the runs that
            // have one, with every painted pixel's colour replaced.
            ctx.saveGState()
            ctx.clip(to: decorated)
            ctx.beginTransparencyLayer(auxiliaryInfo: nil)
            ctx.textPosition = .zero
            CTLineDraw(line, ctx)
            ctx.setBlendMode(.sourceIn)
            ctx.setFillColor(ink)
            ctx.fill(decorated)
            ctx.endTransparencyLayer()
            ctx.restoreGState()
        }
        // A kept run's decoration is CoreText's too, in the run's own colour:
        // the line again, clipped to those runs and left as it draws.
        let kept = zip(runs, paints).filter { $0.1 == .keptLined }.map { extent($0.0) }
        if !kept.isEmpty {
            ctx.saveGState()
            ctx.clip(to: kept)
            ctx.textPosition = .zero
            CTLineDraw(line, ctx)
            ctx.restoreGState()
        }
        ctx.setFillColor(ink)
        for (i, run) in runs.enumerated() {
            switch paints[i] {
            case .nothing, .keptLined: continue
            case .own:
                ctx.textPosition = .zero
                CTRunDraw(run, ctx, CFRange())
            case .decorated where solid[i]: continue
            case .ink, .decorated:
                let count = CTRunGetGlyphCount(run)
                guard count > 0, let font = (CTRunGetAttributes(run) as NSDictionary)[kCTFontAttributeName] else { continue }
                var glyphs = [CGGlyph](repeating: 0, count: count), positions = [CGPoint](repeating: .zero, count: count)
                CTRunGetGlyphs(run, CFRange(), &glyphs)
                CTRunGetPositions(run, CFRange(), &positions)
                ctx.textMatrix = CTRunGetTextMatrix(run)
                CTFontDrawGlyphs(font as! CTFont, glyphs, positions, count, ctx)
            }
        }
    }

    /// Where a glyph run lies along its line, from the line's origin, y up:
    /// its glyphs' advance across, and room above and below for any ink.
    private static func extent(_ run: CTRun) -> CGRect {
        let count = CTRunGetGlyphCount(run)
        var positions = [CGPoint](repeating: .zero, count: count), advances = [CGSize](repeating: .zero, count: count)
        CTRunGetPositions(run, CFRange(), &positions)
        CTRunGetAdvances(run, CFRange(), &advances)
        var lo = CGFloat.infinity, hi = -CGFloat.infinity
        for i in 0..<count {
            lo = min(lo, positions[i].x, positions[i].x + advances[i].width)
            hi = max(hi, positions[i].x, positions[i].x + advances[i].width)
        }
        var ascent: CGFloat = 0, descent: CGFloat = 0
        _ = CTRunGetTypographicBounds(run, CFRange(), &ascent, &descent, nil)
        guard lo <= hi else { return .zero }
        return CGRect(x: lo, y: -descent * 2 - 4, width: hi - lo, height: (ascent + descent) * 2 + 8)
    }

    /// Which of a line's paint a pass makes. A raster paints an HDR
    /// `text-shadow` apart from the ink, for a layer of its own (LLP 1100 D8).
    enum Pass {
        /// Everything: a view's own drawing.
        case all
        /// A raster's ink: all but runs' HDR shadows.
        case ink
        /// Only the glyphs of the runs that cast this shadow, with no shadow.
        case caster(TextRunShadow)

        /// An inline background casts no run's shadow.
        var paintsBackgrounds: Bool { if case .caster = self { false } else { true } }
    }

    /// A line whose glyph runs carry their own shadows, in a context already
    /// at the line's origin, y up: each stretch of runs sharing one shadow
    /// drawn in a transparency layer that casts it, under those glyphs, as
    /// Chrome paints each inline box's shadow with its text.
    static func drawShadowed(_ line: CTLine, in ctx: CGContext, scale: CGFloat, pass: Pass = .all) -> Bool {
        let runs = CTLineGetGlyphRuns(line) as! [CTRun]
        var shadows = runs.map(TextRunShadow.of)
        func hidden(_ run: CTRun) -> Bool { (CTRunGetAttributes(run) as NSDictionary)[NSAttributedString.Key.exactHidden] as? Bool == true }
        switch pass {
        case .all: break
        case .ink: shadows = shadows.map { $0.flatMap { ColorRange.isHDR($0.color) ? nil : $0 } }
        case .caster(let cast):
            for (run, shadow) in zip(runs, shadows) where shadow == cast && !hidden(run) {
                ctx.textPosition = .zero
                CTRunDraw(run, ctx, CFRange())
            }
            return true
        }
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

extension RunPaintRows {
    var namesTint: Bool { shadowColor?.namesTint == true || strokeColor?.namesTint == true }
    func resolve(dark: Bool, contrast: Bool? = nil, elevated: Bool = false, tint: PlatformColor? = nil, color: [Double]) -> (shadow: [Double]?, stroke: [Double]?) {
        let shade = shadow.map { $0 + (shadowColor?.textChannels(dark: dark, contrast: contrast, elevated: elevated, tint: tint) ?? color) }
        let stroke = strokeWidth > 0 ? [strokeWidth] + (strokeColor?.textChannels(dark: dark, contrast: contrast, elevated: elevated, tint: tint) ?? color) : nil
        return (shade, stroke)
    }
}

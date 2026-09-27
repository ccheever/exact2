// @ref LLP 1055.000 D11 — SVG text on Apple: each run shaped by Core Text
// with the presenter's own fonts (LLP 1019), its glyphs' outlines one path
// in a `CAShapeLayer`, so fill, stroke and dashes paint text as they paint
// any shape. A chunk is anchored by its runs' total advance and set on the
// baseline `dominant-baseline` names, with Chrome's arithmetic: integer
// ascent and descent, the x-height from the font.
import CoreGraphics
import CoreText
import Foundation
import QuartzCore

enum SvgText {
    /// A font for a run: size, CSS weight, family index, italic.
    typealias Fonts = (CGFloat, Int, Int, Bool) -> CTFont

    /// Rebuild `group`'s sublayers from a text item's chunks.
    static func build(_ group: CALayer, chunks: [Any], dark: Bool, fonts: Fonts, color: (Any?, Bool) -> CGColor?) {
        group.sublayers?.forEach { $0.removeFromSuperlayer() }
        var pen = CGPoint.zero
        for case let chunk as [String: Any] in chunks {
            let runs = (chunk["runs"] as? [Any] ?? []).compactMap { $0 as? [String: Any] }
            let shaped: [(CTLine, CTFont, [String: Any], CGFloat)] = runs.map { r in
                let font = fonts(CGFloat(number(r["fs"])), Int(number(r["fw"])), Int(number(r["ff"])), number(r["it"]) != 0)
                var attrs: [NSAttributedString.Key: Any] = [.init(kCTFontAttributeName as String): font]
                let spacing = number(r["ls"])
                if spacing != 0 { attrs[.init(kCTKernAttributeName as String)] = spacing }
                let line = CTLineCreateWithAttributedString(NSAttributedString(string: r["t"] as? String ?? "", attributes: attrs))
                let width = CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil))
                return (line, font, r, width)
            }
            let total = shaped.reduce(CGFloat(0)) { $0 + $1.3 + CGFloat(number($1.2["dx"])) }
            let anchor: CGFloat = [0, total / 2, total][min(max(Int(number(chunk["an"])), 0), 2)]
            var x = (chunk["x"] as? NSNumber).map { CGFloat($0.doubleValue) } ?? pen.x
            var y = (chunk["y"] as? NSNumber).map { CGFloat($0.doubleValue) } ?? pen.y
            x -= anchor
            // The first run's font sets the chunk's baseline (LLP 1055.000 D11).
            var shift: CGFloat = 0
            if let font = shaped.first?.1 {
                let ascent = CTFontGetAscent(font).rounded(), descent = CTFontGetDescent(font).rounded()
                switch Int(number(chunk["bl"])) {
                case 2: shift = CTFontGetXHeight(font) / 2 // middle
                case 3: shift = (ascent - descent) / 2 // central
                case 4: shift = 0.8 * ascent // hanging
                case 7: shift = -descent // ideographic
                case 8: shift = ascent / 2 // mathematical
                default: shift = 0
                }
            }
            for (line, _, r, width) in shaped {
                x += CGFloat(number(r["dx"]))
                y += CGFloat(number(r["dy"]))
                let layer = quietShape()
                layer.path = outline(line, at: CGPoint(x: x, y: y + shift))
                layer.fillColor = color(r["f"], dark)
                layer.strokeColor = color(r["s"], dark)
                layer.lineWidth = CGFloat(number(r["w"]))
                layer.lineCap = [CAShapeLayerLineCap.butt, .round, .square][min(Int(number(r["cap"])), 2)]
                layer.lineJoin = [CAShapeLayerLineJoin.miter, .round, .bevel][min(Int(number(r["join"])), 2)]
                layer.miterLimit = CGFloat(number(r["ml"]))
                group.addSublayer(layer)
                x += width
            }
            pen = CGPoint(x: x, y: y)
        }
    }

    /// Every glyph of `line` as one path, its baseline origin at `origin`,
    /// in SVG's y-down space.
    private static func outline(_ line: CTLine, at origin: CGPoint) -> CGPath {
        let path = CGMutablePath()
        for case let run as CTRun in CTLineGetGlyphRuns(line) as? [Any] ?? [] {
            let attributes = CTRunGetAttributes(run) as NSDictionary
            guard let value = attributes[kCTFontAttributeName as String] else { continue }
            let font = value as! CTFont
            let count = CTRunGetGlyphCount(run)
            var glyphs = [CGGlyph](repeating: 0, count: count)
            var positions = [CGPoint](repeating: .zero, count: count)
            CTRunGetGlyphs(run, CFRange(location: 0, length: count), &glyphs)
            CTRunGetPositions(run, CFRange(location: 0, length: count), &positions)
            for i in 0..<count {
                guard let glyph = CTFontCreatePathForGlyph(font, glyphs[i], nil) else { continue }
                // Glyph outlines are y-up: flip them onto the baseline.
                let t = CGAffineTransform(a: 1, b: 0, c: 0, d: -1, tx: origin.x + positions[i].x, ty: origin.y - positions[i].y)
                path.addPath(glyph, transform: t)
            }
        }
        return path
    }

    private static func number(_ v: Any?) -> Double { (v as? NSNumber)?.doubleValue ?? 0 }

    private final class Quiet: NSObject, CALayerDelegate, CAAction {
        static let shared = Quiet()
        func action(for layer: CALayer, forKey event: String) -> CAAction? { self }
        func run(forKey event: String, object anObject: Any, arguments dict: [AnyHashable: Any]?) {}
    }
    private static func quietShape() -> CAShapeLayer { let l = CAShapeLayer(); l.delegate = Quiet.shared; return l }
}

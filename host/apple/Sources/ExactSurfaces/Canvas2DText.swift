// @ref LLP 1056 D8 — Canvas 2D text on Apple: one Core Text line builder
// for measuring (the runner's draw, through `exact_set_canvas_text`, on the
// thread the draw runs on) and for drawing (the replayer), so what was
// measured is what is drawn. A family list resolves as box text resolves
// families: the generic names to the text engine's platform stacks, a
// declared family (LLP 1019) to its registered faces, then any installed
// family by name; none is Chrome's default, serif.
import CExact
import CoreGraphics
import CoreText
import Foundation
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import ExactKit

/// The session's Canvas 2D fonts and lines, over its text engine.
final class CanvasText {
    private unowned let engine: TextEngine
    private var fonts: [String: CTFont] = [:]

    init(engine: TextEngine) { self.engine = engine }

    private static let generics: [String: Int] = [
        "system-ui": 0, "ui-sans-serif": 1, "sans-serif": 2, "ui-serif": 3, "serif": 4,
        "ui-monospace": 5, "monospace": 6, "ui-rounded": 7,
    ]

    /// The font a family list resolves to.
    func font(_ f: Canvas2DFont) -> CTFont {
        let key = "\(f.families.joined(separator: ","))/\(f.size)/\(f.weight)/\(f.style)/\(f.stretch)/\(f.caps)"
        if let hit = fonts[key] { return hit }
        let size = CGFloat(max(0, f.size))
        let italic = f.style != 0
        var chosen: CTFont?
        for name in f.families {
            let trimmed = name.trimmingCharacters(in: .whitespaces)
            if let id = CanvasText.generics[trimmed.lowercased()] ?? engine.stack(named: trimmed) {
                chosen = engine.font(size: size, weight: f.weight, family: id, italic: italic) as CTFont
                break
            }
            if let installed = CanvasText.installed(trimmed, size: size, weight: f.weight, italic: italic) {
                chosen = installed
                break
            }
        }
        var font = chosen ?? engine.font(size: size, weight: f.weight, family: 4, italic: italic) as CTFont
        var features: [[CFString: Any]] = []
        if f.caps == 1 || f.caps == 2 { features.append([kCTFontOpenTypeFeatureTag: "smcp", kCTFontOpenTypeFeatureValue: 1]) }
        if f.caps == 2 { features.append([kCTFontOpenTypeFeatureTag: "c2sc", kCTFontOpenTypeFeatureValue: 1]) }
        if !features.isEmpty {
            let d = CTFontDescriptorCreateWithAttributes([kCTFontFeatureSettingsAttribute: features] as CFDictionary)
            font = CTFontCreateCopyWithAttributes(font, size, nil, d)
        }
        fonts[key] = font
        return font
    }

    /// An installed family by name, or nil when Core Text substitutes.
    private static func installed(_ name: String, size: CGFloat, weight: Int, italic: Bool) -> CTFont? {
        var traits: [CFString: Any] = [kCTFontWeightTrait: weight >= 600 ? 0.4 : 0.0]
        if italic { traits[kCTFontSymbolicTrait] = CTFontSymbolicTraits.traitItalic.rawValue }
        let d = CTFontDescriptorCreateWithAttributes([kCTFontFamilyNameAttribute: name, kCTFontTraitsAttribute: traits] as CFDictionary)
        let font = CTFontCreateWithFontDescriptor(d, size, nil)
        let family = CTFontCopyFamilyName(font) as String
        return family.caseInsensitiveCompare(name) == .orderedSame ? font : nil
    }

    /// One run as a Core Text line, with the canvas's base direction.
    func line(_ f: Canvas2DFont, _ text: String, rtl: Bool) -> CTLine { CanvasText.line(font(f), f, text, rtl: rtl) }

    /// One run in a resolved font. Pure Core Text: a replay off the main
    /// thread calls it with a font resolved on the main thread.
    static func line(_ font: CTFont, _ f: Canvas2DFont, _ text: String, rtl: Bool) -> CTLine {
        let s = NSMutableAttributedString(string: text)
        let all = NSRange(location: 0, length: s.length)
        var attrs: [NSAttributedString.Key: Any] = [
            NSAttributedString.Key(kCTFontAttributeName as String): font,
            NSAttributedString.Key(kCTForegroundColorFromContextAttributeName as String): true,
        ]
        // The paragraph's base direction; the runs inside keep their own.
        var direction: CTWritingDirection = rtl ? .rightToLeft : .leftToRight
        let paragraph = withUnsafeBytes(of: &direction) { raw in
            var setting = CTParagraphStyleSetting(spec: .baseWritingDirection, valueSize: raw.count, value: raw.baseAddress!)
            return CTParagraphStyleCreate(&setting, 1)
        }
        attrs[NSAttributedString.Key(kCTParagraphStyleAttributeName as String)] = paragraph
        if f.kerning == 2 { attrs[NSAttributedString.Key(kCTKernAttributeName as String)] = 0 }
        if f.letterSpacing != 0 { attrs[NSAttributedString.Key(kCTTrackingAttributeName as String)] = f.letterSpacing }
        s.setAttributes(attrs, range: all)
        if f.wordSpacing != 0 {
            let ns = text as NSString
            for i in 0..<ns.length where ns.character(at: i) == 0x20 {
                s.addAttribute(NSAttributedString.Key(kCTKernAttributeName as String), value: f.wordSpacing + f.letterSpacing,
                               range: NSRange(location: i, length: 1))
            }
        }
        return CTLineCreateWithAttributedString(s)
    }

    /// The eleven raw metrics (`exact_canvas::RawMetrics`) of one run.
    func measure(_ f: Canvas2DFont, _ text: String, rtl: Bool) -> [Double] {
        let font = self.font(f)
        let line = self.line(f, text, rtl: rtl)
        let width = CTLineGetTypographicBounds(line, nil, nil, nil)
        let ink = text.isEmpty ? .zero : CTLineGetBoundsWithOptions(line, .useGlyphPathBounds)
        let ascent = Double(CTFontGetAscent(font)), descent = Double(CTFontGetDescent(font))
        var emAscent = ascent, emDescent = descent
        if let table = CTFontCopyTable(font, CTFontTableTag(kCTFontTableOS2), []) as Data?, table.count >= 72 {
            let a = Double(Int16(bitPattern: UInt16(table[68]) << 8 | UInt16(table[69])))
            let d = Double(Int16(bitPattern: UInt16(table[70]) << 8 | UInt16(table[71])))
            if a - d > 0 { emAscent = a; emDescent = -d }
        }
        let sum = emAscent + emDescent
        let size = f.size
        let (ea, ed) = sum > 0 ? (size * emAscent / sum, size * emDescent / sum) : (size * 0.8, size * 0.2)
        let empty = ink.isNull || ink.isEmpty && ink.width == 0 && ink.height == 0
        return [
            width,
            empty ? 0 : -Double(ink.minX), empty ? 0 : Double(ink.maxX),
            empty ? 0 : Double(ink.maxY), empty ? 0 : -Double(ink.minY),
            ascent, descent, ea, ed, 0.8 * ascent, -descent,
        ]
    }

    /// The run's glyph outlines in line space (y up), for a stroked run
    /// with a gradient or pattern.
    static func outline(_ line: CTLine) -> CGPath {
        let path = CGMutablePath()
        for case let run as CTRun in CTLineGetGlyphRuns(line) as [AnyObject] {
            let attrs = CTRunGetAttributes(run) as NSDictionary
            guard let font = attrs[kCTFontAttributeName] as! CTFont? else { continue }
            let n = CTRunGetGlyphCount(run)
            var glyphs = [CGGlyph](repeating: 0, count: n), positions = [CGPoint](repeating: .zero, count: n)
            CTRunGetGlyphs(run, CFRange(location: 0, length: 0), &glyphs)
            CTRunGetPositions(run, CFRange(location: 0, length: 0), &positions)
            for i in 0..<n {
                guard let g = CTFontCreatePathForGlyph(font, glyphs[i], nil) else { continue }
                path.addPath(g, transform: CGAffineTransform(translationX: positions[i].x, y: positions[i].y))
            }
        }
        return path
    }

    /// The Canvas 2D measurer (`exact_set_canvas_text`); `ctx` is the
    /// session's text engine, the one `exact_set_measure` was given.
    static let measureRun: ExactCanvasTextFn = { ctx, run in
        var out = ExactCanvasMetrics()
        guard let ctx, let r = run?.pointee else { return out }
        let engine = Unmanaged<TextEngine>.fromOpaque(ctx).takeUnretainedValue()
        let text = r.len > 0 && r.text != nil ? String(decoding: UnsafeBufferPointer(start: r.text, count: r.len), as: UTF8.self) : ""
        let families = r.families_len > 0 && r.families != nil
            ? String(decoding: UnsafeBufferPointer(start: r.families, count: r.families_len), as: UTF8.self) : "sans-serif"
        var f = Canvas2DFont()
        f.size = r.size; f.weight = Int(r.weight); f.style = Int(r.style); f.stretch = r.stretch; f.caps = Int(r.caps)
        f.kerning = Int(r.kerning); f.letterSpacing = r.letter_spacing; f.wordSpacing = r.word_spacing
        f.families = families.split(separator: ",").map(String.init)
        let m = engine.canvasText.measure(f, text, rtl: r.rtl != 0)
        withUnsafeMutableBytes(of: &out.v) { raw in
            let v = raw.bindMemory(to: Double.self)
            for i in 0..<min(11, m.count) { v[i] = m[i] }
        }
        return out
    }
}

extension Canvas2DReplayer {
    /// `fillText`/`strokeText`: the run from its left end on its alphabetic
    /// baseline (`x`, `y` in user space), squeezed by `scaleX` about it.
    func drawText(_ c: CGContext, fill: Bool, x: Double, y: Double, scaleX: Double, rtl: Bool, text: String) {
        guard let resolved = env?.canvasFont(state.font) else { return }
        let line = CanvasText.line(resolved, state.font, text, rtl: rtl)
        let style = fill ? state.fill : state.stroke
        render(c) { c in
            c.concatenate(state.author)
            c.translateBy(x: x, y: y)
            c.scaleBy(x: scaleX, y: 1)
            c.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
            c.textPosition = .zero
            switch (style, fill) {
            case (.color(let color), true):
                c.setFillColor(color)
                c.setTextDrawingMode(.fill)
                CTLineDraw(line, c)
            case (.color(let color), false):
                // The outlines stroked as a path is: Core Text's own stroke
                // mode draws heavier than the canvas's line width.
                c.scaleBy(x: 1, y: -1)
                c.addPath(CanvasText.outline(line))
                c.setStrokeColor(color)
                c.strokePath()
            case (_, true):
                c.scaleBy(x: 1, y: -1)
                c.addPath(CanvasText.outline(line))
                c.clip()
                resetToBase(c)
                paintArea(c, style)
            case (_, false):
                c.scaleBy(x: 1, y: -1)
                c.addPath(CanvasText.outline(line))
                c.replacePathWithStrokedPath()
                c.clip()
                resetToBase(c)
                paintArea(c, style)
            }
        }
    }
}

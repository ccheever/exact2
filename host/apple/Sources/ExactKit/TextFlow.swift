// @ref LLP 1043.000 §3 D4–D7 — the shared walker breaks; CoreText shapes/draws.
import Foundation
import CoreText
import CExact

/// Owned Swift geometry; counted C pointers exist only inside withCShapes.
struct TextFlowShape: Equatable {
    var kind: UInt32
    var x: Float = 0, y: Float = 0, a: Float = 0, b: Float = 0, radius: Float = 0
    var pairs: [SIMD2<Float>] = []
    init(kind: UInt32, x: Float = 0, y: Float = 0, a: Float = 0, b: Float = 0, radius: Float = 0,
         pairs: [SIMD2<Float>] = []) {
        self.kind = kind; self.x = x; self.y = y; self.a = a; self.b = b; self.radius = radius; self.pairs = pairs
    }
    init?(_ wire: [String: Any]) {
        func n(_ name: String) -> Float { (wire[name] as? NSNumber)?.floatValue ?? 0 }
        switch wire["kind"] as? String {
        case "Circle": self.init(kind: 0, x: n("cx"), y: n("cy"), a: n("r"))
        case "Ellipse": self.init(kind: 1, x: n("cx"), y: n("cy"), a: n("rx"), b: n("ry"))
        case "RoundRect": self.init(kind: 2, x: n("x"), y: n("y"), a: n("width"), b: n("height"), radius: n("radius"))
        case "Polygon", "Spans":
            let polygon = wire["kind"] as? String == "Polygon"
            let rows = (wire[polygon ? "points" : "rows"] as? [[NSNumber]] ?? []).filter { $0.count == 2 }
            self.init(kind: polygon ? (wire["fill_rule"] as? String == "evenodd" ? 5 : 3) : 4, x: n("x"), y: n("y"), a: n("row_height"),
                      pairs: rows.map { SIMD2($0[0].floatValue, $0[1].floatValue) })
        default: return nil
        }
    }
    init(_ c: ExactFlowShape) {
        self.init(kind: c.kind, x: c.x, y: c.y, a: c.a, b: c.b, radius: c.radius,
                  pairs: c.pairs.map { p in UnsafeBufferPointer(start: p, count: c.count).map { SIMD2($0.x, $0.y) } } ?? [])
    }
    func translated(_ offset: CGPoint) -> TextFlowShape {
        var copy = self
        if kind == 3 || kind == 5 { copy.pairs = pairs.map { $0 + SIMD2(Float(offset.x), Float(offset.y)) } }
        else { copy.x += Float(offset.x); copy.y += Float(offset.y) }
        return copy
    }
    static func withCShapes<T>(_ shapes: [TextFlowShape], _ body: (UnsafeBufferPointer<ExactFlowShape>) -> T) -> T {
        let pairs = shapes.flatMap { $0.pairs.map { ExactFlowPair(x: $0.x, y: $0.y) } }
        return pairs.withUnsafeBufferPointer { buffer in
            var offset = 0
            let flat = shapes.map { s -> ExactFlowShape in
                defer { offset += s.pairs.count }
                return ExactFlowShape(kind: s.kind, x: s.x, y: s.y, a: s.a, b: s.b, radius: s.radius,
                                      pairs: s.pairs.isEmpty ? nil : buffer.baseAddress?.advanced(by: offset), count: s.pairs.count)
            }
            return flat.withUnsafeBufferPointer(body)
        }
    }
}

/// The only full unwrapped CTLine and Rust prepare per TextShape. O(UTF16 + glyphs)
/// storage; glyph indices are logical even when a CTRun's visual order is RTL.
final class TextFlowSource {
    let line: CTLine
    let typesetter: CTTypesetter
    let attributed: NSAttributedString
    let advances: [Float]
    let handle: UInt64
    let utf8Count: Int
    let utf16Count: Int
    private(set) var rustBytes = 0
    var box: (above: CGFloat, below: CGFloat, explicit: Bool)?
    var ownedBytes: Int { advances.count * MemoryLayout<Float>.stride + rustBytes }
    init(_ shape: TextShape) {
        // Preserve original UTF16 positions while matching the walker's CSS
        // ASCII space/tab collapsing. Continuations become zero-width controls.
        let original = shape.attributed.string as NSString
        var replacements: [(Int, String)] = []
        var space = false
        for i in 0..<original.length {
            let ch = original.character(at: i)
            // `pre-line` keeps a line feed, a forced break (LLP 1053 G5).
            if shape.spec.whiteSpace != 1 && !(shape.spec.whiteSpace == 3 && ch == 10)
                && [32, 9, 10, 13, 12, 0x85, 0x2028, 0x2029].contains(ch) {
                if space { replacements.append((i, "\u{200b}")) }
                else if ch != 32 { replacements.append((i, " ")) }
                space = true
            } else { space = false }
        }
        if replacements.isEmpty { attributed = shape.attributed; typesetter = shape.typesetter }
        else {
            let value = NSMutableAttributedString(attributedString: shape.attributed)
            for (index, text) in replacements { value.replaceCharacters(in: NSRange(location: index, length: 1), with: text) }
            attributed = value; typesetter = CTTypesetterCreateWithAttributedString(value)
        }
        line = CTTypesetterCreateLine(typesetter, CFRange(location: 0, length: 0))
        let text = shape.attributed.string
        utf8Count = text.utf8.count; utf16Count = (text as NSString).length
        advances = Self.advances(line, text: text)
        // Hyphen uses the paragraph strut's font (or first run's resolved font).
        let attrs = shape.attributed.length > 0 ? shape.attributed.attributes(at: 0, effectiveRange: nil) : [:]
        let hyphen = CTLineCreateWithAttributedString(NSAttributedString(string: "-", attributes: attrs))
        let width = Float(CTLineGetTypographicBounds(hyphen, nil, nil, nil))
        let table = advances, words = Self.complexWords(text)
        handle = Array(text.utf8).withUnsafeBufferPointer { bytes in
            table.withUnsafeBufferPointer { table in
                words.withUnsafeBufferPointer { words in
                    exact_textflow_prepare(bytes.baseAddress, bytes.count, table.baseAddress, table.count, words.baseAddress, words.count, UInt32(shape.spec.overflowWrap), UInt32(shape.spec.whiteSpace), width)
                }
            }
        }
    }
    /// CoreFoundation's line-break units that start between two Thai, Lao, Khmer
    /// or Myanmar letters: the walker has no dictionary (LLP 1043 §4 C).
    static func complexWords(_ text: String) -> [UInt32] {
        let string = text as NSString, length = string.length
        func complex(_ i: Int) -> Bool {
            switch string.character(at: i) {
            case 0x0E00...0x0EFF, 0x1000...0x109F, 0x1780...0x17FF, 0x1980...0x19FF, 0x1A20...0x1AAF, 0xA9E0...0xA9FF, 0xAA60...0xAADF: true
            default: false
            }
        }
        guard (0..<length).contains(where: complex) else { return [] }
        let tokenizer = CFStringTokenizerCreate(nil, string, CFRange(location: 0, length: length), kCFStringTokenizerUnitLineBreak, nil)
        var words: [UInt32] = []
        while CFStringTokenizerAdvanceToNextToken(tokenizer).rawValue != 0 {
            let start = CFStringTokenizerGetCurrentTokenRange(tokenizer).location
            if start > 0 && complex(start - 1) && complex(start) { words.append(UInt32(start)) }
        }
        return words
    }
    deinit { exact_textflow_free(handle) }
    static func advances(_ line: CTLine, text: String) -> [Float] {
        let string = text as NSString, length = string.length
        var first = [Int](repeating: 0, count: length)
        var offset = 0
        while offset < length {
            let range = string.rangeOfComposedCharacterSequence(at: offset)
            for i in range.location..<NSMaxRange(range) { first[i] = range.location }
            offset = NSMaxRange(range)
        }
        var table = [Float](repeating: 0, count: length)
        for run in CTLineGetGlyphRuns(line) as! [CTRun] {
            let count = CTRunGetGlyphCount(run)
            var advances = [CGSize](repeating: .zero, count: count)
            var indices = [CFIndex](repeating: 0, count: count)
            CTRunGetAdvances(run, CFRange(location: 0, length: 0), &advances)
            CTRunGetStringIndices(run, CFRange(location: 0, length: 0), &indices)
            // Reordered marks may have their own (higher) string index. Fold
            // the composed cluster into its lowest unit, independently of visual
            // run order. Keep signed positioning backsteps inside a run: dropping
            // them overmeasures Arabic. A leading reordered mark can also start
            // the pen away from the run origin, so reconcile that initial offset
            // once against the run's typographic advance (e.g. lam + fatha + alef).
            for i in indices.indices where table.indices.contains(indices[i]) {
                table[first[indices[i]]] += Float(advances[i].width)
            }
            if let index = indices.first, table.indices.contains(index) {
                let total = CTRunGetTypographicBounds(run, CFRange(location: 0, length: 0), nil, nil, nil)
                table[first[index]] += Float(total - advances.reduce(0) { $0 + $1.width })
            }
        }
        return table
    }
    func flow(_ shapes: [TextFlowShape], width: CGFloat, height: CGFloat, fontSize: CGFloat, clamp: Int, direction: Int) -> ([ExactFlowFragment], CGFloat, Bool) {
        // Each fragment consumes source; at most one per UTF16 unit. Rust owns
        // reusable walker scratch, the view owns only this frame's final CTLines.
        var fragments = [ExactFlowFragment](repeating: ExactFlowFragment(), count: utf16Count + 1)
        let result = TextFlowShape.withCShapes(shapes) { shapes in
            fragments.withUnsafeMutableBufferPointer { out in
                exact_textflow_flow(handle, shapes.baseAddress, shapes.count, Float(width), Float(height), Float(fontSize),
                                    UInt32(clamping: clamp), UInt32(direction), out.baseAddress, out.count)
            }
        }
        rustBytes = result.bytes
        fragments.removeSubrange(minInt(result.count, fragments.count)..<fragments.count)
        return (fragments, CGFloat(result.height), result.complete != 0 || result.clamped != 0)
    }
    private func minInt(_ a: Int, _ b: Int) -> Int { Swift.min(a, b) }
}

extension TextEngine {
    /// Fixed band takes the maximum inline/strut extents across the paragraph.
    /// Explicit CSS line boxes permit fallback ink to overflow without growing.
    func lineBox(_ spec: Spec, line: CTLine) -> (above: CGFloat, below: CGFloat, explicit: Bool) {
        let strut = spec.strut ?? spec.runs.first
        func extents(_ run: Run) -> (CGFloat, CGFloat) { CSSLineBox.extents(font(run) as CTFont, height: run.lineHeight) }
        let minimum = strut.map(extents) ?? (0, 0)
        var above = minimum.0, below = minimum.1
        var aboveExplicit = strut?.lineHeight != nil, belowExplicit = aboveExplicit
        func include(_ a: CGFloat, _ b: CGFloat, explicit: Bool) {
            if a > above { above = a; aboveExplicit = explicit }
            else if a == above { aboveExplicit = aboveExplicit && explicit }
            if b > below { below = b; belowExplicit = explicit }
            else if b == below { belowExplicit = belowExplicit && explicit }
        }
        for glyphRun in CTLineGetGlyphRuns(line) as! [CTRun] {
            let range = CTRunGetStringRange(glyphRun)
            var offset = 0
            var matched = false, includesNormal = false
            for authored in spec.runs {
                let end = offset + (authored.text as NSString).length
                defer { offset = end }
                guard offset < range.location + range.length && end > range.location else { continue }
                matched = true
                if authored.lineHeight != nil {
                    let (a, b) = extents(authored); include(a, b, explicit: true)
                } else { includesNormal = true }
            }
            if !matched, let strut, strut.lineHeight != nil {
                let (a, b) = extents(strut); include(a, b, explicit: true); continue
            }
            if matched && !includesNormal { continue }
            let attrs = CTRunGetAttributes(glyphRun) as NSDictionary
            let font = attrs[kCTFontAttributeName] as! CTFont
            let (a, d) = CSSLineBox.extents(font, height: nil)
            include(a, d, explicit: false)
        }
        return (above, below, aboveExplicit || belowExplicit)
    }

    // The walker stops at max(1024, 8*prepared work units) bands (or clamp).
    // F fragments consume source, so F <= UTF16 count + 1. At most 2F + 3
    // CTLine constructions follow (selected soft hyphens and final ellipsis
    // can replace a line). Soft-hyphen prefix copies cost O(F * UTF16) in the
    // worst case. CT's internal shaping cost is opaque; the 600-frame test
    // measures it. No shape/frame history enters paragraph residency.
    func layoutFlow(_ shape: TextShape, width: CGFloat, flow: [TextFlowShape]) -> Paragraph {
        let source = shape.preparedFlow(), spec = shape.spec
        let box = source.box ?? lineBox(spec, line: source.line)
        source.box = box
        let lineHeight = box.above + box.below
        let (fragments, height, accepted) = source.flow(flow, width: width, height: lineHeight,
            fontSize: (spec.strut ?? spec.runs.first)?.size ?? 0, clamp: spec.lineClamp, direction: spec.direction)
        guard source.handle != 0 && accepted else {
            let ordinary = layout(shape, width: width)
            ordinary.flowIncomplete = true
            return ordinary
        }
        var lines: [CTLine] = [], origins: [CGFloat] = [], baselines: [CGFloat] = [], bottoms: [CGFloat] = []
        var glyphs = 0, maxWidth: CGFloat = 0
        for (i, fragment) in fragments.enumerated() {
            let range = CFRange(location: fragment.paint_start, length: fragment.paint_end - fragment.paint_start)
            var line = range.length > 0 ? CTTypesetterCreateLine(source.typesetter, range)
                : CTLineCreateWithAttributedString(NSAttributedString(string: ""))
            // The walker consumes a selected SHY; CoreText otherwise keeps it
            // invisible even at a manually supplied line boundary. Retain global
            // UTF16 indices when replacing that single code unit with its ink.
            if range.length > 0, fragment.hyphenated != 0 {
                let visible = NSMutableAttributedString(attributedString: source.attributed.attributedSubstring(
                    from: NSRange(location: 0, length: fragment.paint_end)))
                visible.replaceCharacters(in: NSRange(location: fragment.paint_end - 1, length: 1), with: "-")
                line = CTTypesetterCreateLine(CTTypesetterCreateWithAttributedString(visible), range)
            }
            if i == fragments.count - 1, spec.lineClamp > 0, fragment.end < source.utf8Count {
                line = ellipsizedLine(spec, range: NSRange(location: range.location, length: range.length), width: Double(fragment.available), source: source.attributed) ?? line
            }
            let flush: CGFloat = spec.align == 1 ? 0.5 : spec.align == 2 ? 1 : 0
            let x = CGFloat(fragment.x) + CGFloat(CTLineGetPenOffsetForFlush(line, flush, Double(fragment.available)))
            origins.append(x); baselines.append(CGFloat(fragment.y) + box.above)
            bottoms.append(CGFloat(fragment.y) + lineHeight)
            glyphs += CTLineGetGlyphCount(line); lines.append(line)
            maxWidth = max(maxWidth, x + CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)))
        }
        return Paragraph(lines: lines, baselines: baselines, width: CSSLineBox.layoutWidth(maxWidth), height: height,
                         lineBottoms: bottoms, shape: shape, glyphCount: glyphs, origins: origins,
                         fragments: fragments, flowLineHeight: lineHeight)
    }
}

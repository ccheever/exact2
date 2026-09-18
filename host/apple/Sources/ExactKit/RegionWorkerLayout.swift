import Foundation
import CoreText
import CryptoKit
// Only live layouts own this preparation. It never crosses the worker queue or
// attaches to Sendable source/metadata. Attributed and opaque typesetter storage
// now live as long as their last layout; pixel/index budgets do not cover them.
final class RegionPreparedSource {
    let source: RegionTextSource
    let sourceSHA256: String
    let attributed: NSAttributedString
    let typesetter: CTTypesetter
    init(_ source: RegionTextSource) {
        precondition(!Thread.isMainThread, "region preparation must be worker-owned")
        self.source = source
        // Diagnostic identity belongs to this live preparation, not UI capture
        // or a width/history cache. Metadata retains only the immutable string.
        sourceSHA256 = SHA256.hash(data: Data(source.text.utf8)).map { String(format: "%02x", $0) }.joined()
        attributed = source.attributed()
        typesetter = CTTypesetterCreateWithAttributedString(attributed)
    }
}
// This record is deliberately NOT Sendable. CTLines and index survive all
// viewport queries on one queue, then die there before metadata publication.
final class RegionWorkerLayout {
    let source: RegionTextSource
    let lines: [CTLine]
    let baselines: [CGFloat]
    let metadata: RegionParagraph
    let preparation: RegionPreparedSource
    private init(source: RegionTextSource, lines: [CTLine], baselines: [CGFloat], metadata: RegionParagraph,
                 preparation: RegionPreparedSource) {
        self.source = source; self.lines = lines; self.baselines = baselines; self.metadata = metadata
        self.preparation = preparation
    }
    static func shape(_ source: RegionTextSource, width: CGFloat, retainHits: Bool = true,
                      preparation: RegionPreparedSource? = nil) -> RegionWorkerLayout {
        precondition(!Thread.isMainThread, "region shape must be worker-owned")
        let spec = source
        let preparation = preparation ?? RegionPreparedSource(source)
        precondition(preparation.source === source, "preparation belongs to this exact captured source")
        let attributed = preparation.attributed
        let typesetter = preparation.typesetter
        let length = source.utf16Count
        func extents(_ run: RegionTextRun) -> (CGFloat, CGFloat) {
            (run.font.above, run.font.below)
        }
        let minimum = source.strut.map { ($0.above, $0.below) } ?? (0, 0)
        var explicit = false
        var lineBottoms: [CGFloat] = []
        var lines: [CTLine] = []
        var baselines: [CGFloat] = []
        var maxWidth: CGFloat = 0
        var y: CGFloat = 0
        var start = 0
        var lineCount = 0
        let limit = width.isFinite ? Double(width) : Double.greatestFiniteMagnitude
        // CoreText breaks a word when it cannot fit; CSS normal instead lets
        // that word overflow. Public Unicode line boundaries distinguish those
        // emergency breaks from ordinary opportunities (including CJK).
        var boundaries: [Int] = []
        var boundaryIndex = 0
        if spec.overflowWrap == 0 && width.isFinite {
            let text = spec.runs.map(\.text).joined() as NSString
            let tokenizer = CFStringTokenizerCreate(nil, text as CFString, CFRange(location: 0, length: length), kCFStringTokenizerUnitLineBreak, nil)!
            while CFStringTokenizerAdvanceToNextToken(tokenizer).rawValue != 0 {
                let range = CFStringTokenizerGetCurrentTokenRange(tokenizer)
                boundaries.append(range.location + range.length)
            }
            if boundaries.last != length { boundaries.append(length) }
        }
        while start < length {
            if spec.lineClamp > 0 && lineCount == spec.lineClamp { break }
            var count: Int
            count = CTTypesetterSuggestLineBreak(typesetter, start, limit)
            while boundaryIndex < boundaries.count && boundaries[boundaryIndex] < start + count {
                boundaryIndex += 1
            }
            if boundaryIndex < boundaries.count {
                count = boundaries[boundaryIndex] - start
            }
            if count <= 0 { count = length - start }
            var line = CTTypesetterCreateLine(typesetter, CFRangeMake(start, count))
            if spec.lineClamp > 0 && lineCount + 1 == spec.lineClamp && start + count < length {
                line = ellipsizedLine(attributed, range: NSRange(location: start, length: count), width: limit) ?? line
            }
            var ascent: CGFloat = 0, descent: CGFloat = 0, leading: CGFloat = 0
            let w = CGFloat(CTLineGetTypographicBounds(line, &ascent, &descent, &leading))
            // CSS inline boxes share a baseline. Include the paragraph strut
            // and only the runs on this line, preserving each font's half-leading.
            var above = minimum.0, below = minimum.1
            var aboveExplicit = source.strutExplicit, belowExplicit = aboveExplicit
            func include(_ a: CGFloat, _ b: CGFloat, explicit: Bool) {
                if a > above { above = a; aboveExplicit = explicit }
                else if a == above { aboveExplicit = aboveExplicit && explicit }
                if b > below { below = b; belowExplicit = explicit }
                else if b == below { belowExplicit = belowExplicit && explicit }
            }
            for glyphRun in CTLineGetGlyphRuns(line) as! [CTRun] {
                let range = CTRunGetStringRange(glyphRun)
                var matched = false, includesNormal = false
                // CoreText can coalesce adjacent spans with the same glyph
                // attributes even when their authored line heights differ.
                for authored in spec.runs {
                    guard authored.range.location < range.location + range.length &&
                          NSMaxRange(authored.range) > range.location else { continue }
                    matched = true
                    if authored.lineHeight != nil {
                        // Explicit boxes use authored metrics; fallback ink
                        // can overflow without enlarging the inline box.
                        let (a, b) = extents(authored)
                        include(a, b, explicit: true)
                    } else {
                        includesNormal = true
                    }
                }
                if !matched, source.strutExplicit {
                    let (a, b) = minimum
                    include(a, b, explicit: true)
                    continue
                }
                if matched && !includesNormal { continue }
                let attributes = CTRunGetAttributes(glyphRun) as NSDictionary
                let shapedFont = attributes[kCTFontAttributeName] as! CTFont
                let a = CTFontGetAscent(shapedFont), d = CTFontGetDescent(shapedFont), l = CTFontGetLeading(shapedFont)
                // Normal line height includes the actual emoji/fallback face's
                // metrics, as CTLine measurement did before typed line heights.
                include(a, d + l, explicit: false)
            }
            explicit = explicit || aboveExplicit || belowExplicit
            if retainHits || lineCount == 0 { baselines.append(y + above) }
            y += above + below
            if retainHits { lineBottoms.append(y) }
            maxWidth = max(maxWidth, w)
            if retainHits { lines.append(line) }
            lineCount += 1
            start += count
        }
        if lineCount == 0 {
            // Empty editors retain the paragraph's own line box.
            baselines.append(minimum.0)
            y = minimum.0 + minimum.1
            explicit = source.strutExplicit
        }
        // An authored CSS line height fixes the line box, including fractions.
        // Keep intrinsic width and `normal` height measurement separate: changing
        // their rounding also changes wrapping and the established host parity.
        let metadata = RegionParagraph(source: source, sourceSHA256: preparation.sourceSHA256,
                                       lines: lines, baselines: baselines,
                                       width: ceil(maxWidth), height: explicit ? y : ceil(y),
                                       lineBottoms: lineBottoms, offeredWidth: width, retainHits: retainHits, captureHits: false)
        return RegionWorkerLayout(source: source, lines: retainHits ? lines : [], baselines: retainHits ? baselines : [],
                                  metadata: metadata, preparation: preparation)
    }

    func exactIndex(at point: CGPoint, in bounds: CGRect) -> Int {
        precondition(!Thread.isMainThread)
        if point.y < bounds.minY { return 0 }
        if point.y > bounds.maxY { return source.utf16Count }
        guard let i = metadata.lineIndex(at: point.y - bounds.minY) else { return 0 }
        let value = CTLineGetStringIndexForPosition(lines[i],CGPoint(
            x: point.x - bounds.minX - metadata.lines[i].flushOffset,y: 0))
        return value == kCFNotFound ? source.utf16Count : min(max(0,value),source.utf16Count)
    }

    static func minimumWidth(_ source: RegionTextSource) -> CGFloat {
        precondition(!Thread.isMainThread)
        var widest: CGFloat = 0
        if source.overflowWrap == 2 {
            let attributed = source.attributed(), text = attributed.string as NSString
            var start = 0
            while start < text.length {
                let range = text.rangeOfComposedCharacterSequence(at: start)
                let line = CTLineCreateWithAttributedString(attributed.attributedSubstring(from: range))
                widest = max(widest, CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)))
                start = NSMaxRange(range)
            }
            return ceil(widest)
        }
        for run in source.runs {
            var words: [String: CGFloat] = [:]
            var bytes = 0
            for slice in run.text.split(whereSeparator: { $0.isWhitespace }) {
                let word = String(slice)
                if let value = words[word] { widest = max(widest, value); continue }
                var attrs: [NSAttributedString.Key: Any] = [NSAttributedString.Key(kCTFontAttributeName as String): run.font.value]
                if run.letterSpacing != 0 { attrs[.kern] = run.letterSpacing }
                let line = CTLineCreateWithAttributedString(NSAttributedString(string: word, attributes: attrs))
                let value = ceil(CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)))
                widest = max(widest, value)
                let cost = word.utf8.count + 64
                if cost <= 1024 * 1024 - bytes { words[word] = value; bytes += cost }
            }
        }
        return widest
    }

    private static func ellipsizedLine(_ source: NSAttributedString, range: NSRange, width: Double) -> CTLine? {
        let string = source.string as NSString
        var end = NSMaxRange(range)
        // A wrapped line already fits. Include the ellipsis before asking
        // CoreText to make room for it, removing the line's trailing break/space.
        while end > range.location && [9, 10, 13, 32, 0x2028, 0x2029].contains(Int(string.character(at: end - 1))) { end -= 1 }
        let candidate = NSMutableAttributedString(attributedString: source.attributedSubstring(from: NSRange(location: 0, length: end)))
        candidate.append(NSAttributedString(string: "…", attributes: source.attributes(at: max(range.location, end - 1), effectiveRange: nil)))
        let typesetter = CTTypesetterCreateWithAttributedString(candidate)
        let line = CTTypesetterCreateLine(typesetter, CFRange(location: range.location, length: end - range.location + 1))
        // Keep paragraph-global indices, including the token, for AppKit hits.
        let token = CTTypesetterCreateLine(typesetter, CFRange(location: end, length: 1))
        // If even the token cannot fit, retain the first clipped character,
        // as the browser does, rather than replacing it with a partial ellipsis.
        return CTLineCreateTruncatedLine(line, width, .end, token)
    }

}

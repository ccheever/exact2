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
    // Unicode opportunities depend on this exact source, never its width.
    // Lazy so intrinsic and emergency-wrap layouts keep no boundary array.
    // Like the typesetter, this storage is confined to the serial worker and
    // released with its last live preparation owner; there is no width history.
    lazy var lineBreakBoundaries: [Int] = {
        let text = attributed.string as NSString
        let length = source.utf16Count
        let tokenizer = CFStringTokenizerCreate(nil, text as CFString,
            CFRange(location: 0, length: length), kCFStringTokenizerUnitLineBreak, nil)!
        var boundaries: [Int] = []
        while CFStringTokenizerAdvanceToNextToken(tokenizer).rawValue != 0 {
            let range = CFStringTokenizerGetCurrentTokenRange(tokenizer)
            boundaries.append(range.location + range.length)
        }
        if boundaries.last != length { boundaries.append(length) }
        return boundaries
    }()
    /// CoreText's word-break iterator can rescan the whole prefix on every
    /// line of a giant paragraph. Cluster fitting plus the already indexed
    /// Unicode opportunities avoids that quadratic search. Trailing whitespace
    /// hangs at a break, just as in CTTypesetterSuggestLineBreak.
    lazy var breakContentEnds: [Int] = {
        let text = attributed.string as NSString
        return lineBreakBoundaries.map { boundary in
            var end = boundary
            while end > 0, let scalar = UnicodeScalar(text.character(at: end - 1)),
                  CharacterSet.whitespaces.contains(scalar) { end -= 1 }
            return end
        }
    }()
    func suggestBreak(at start: Int, width: Double, cursor: inout Int) -> Int {
        let fitted = CTTypesetterSuggestClusterBreak(typesetter, start, width)
        let ends = lineBreakBoundaries, content = breakContentEnds
        while cursor < ends.count && ends[cursor] <= start { cursor += 1 }
        let first = cursor
        while cursor < ends.count && content[cursor] <= start + fitted { cursor += 1 }
        if cursor > first { return ends[cursor - 1] - start }
        if source.overflowWrap == 0, cursor < ends.count {
            defer { cursor += 1 }
            return ends[cursor] - start
        }
        if fitted > 0 { return fitted }
        return (attributed.string as NSString).rangeOfComposedCharacterSequence(at: start).length
    }

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
    private var viewportLines: [Int: CTLine] = [:]
    var lineCount: Int { metadata.lines.count }
    func beginViewport() { viewportLines.removeAll(keepingCapacity: true) }
    func line(at index: Int) -> CTLine {
        if !lines.isEmpty { return lines[index] }
        if let cached = viewportLines[index] { return cached }
        let r = metadata.lines[index].range
        let value = CTTypesetterCreateLine(preparation.typesetter, CFRange(location: r.location, length: r.length))
        if viewportLines.count < 512 { viewportLines[index] = value }
        return value
    }
    private init(source: RegionTextSource, lines: [CTLine], baselines: [CGFloat], metadata: RegionParagraph,
                 preparation: RegionPreparedSource) {
        self.source = source; self.lines = lines; self.baselines = baselines; self.metadata = metadata
        self.preparation = preparation
    }
    static func shape(_ source: RegionTextSource, width: CGFloat, retainHits: Bool = true,
                      preparation: RegionPreparedSource? = nil, compact: Bool = false) -> RegionWorkerLayout {
        shape(source, width: width, retainHits: retainHits, preparation: preparation, compact: compact, beforeMetadata: {})
    }
    static func shape(_ source: RegionTextSource, width: CGFloat, retainHits: Bool = true,
                      preparation: RegionPreparedSource? = nil, compact: Bool = false,
                      beforeMetadata: () throws -> Void) rethrows -> RegionWorkerLayout {
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
        var lineBottoms: [CGFloat] = []
        var lines: [CTLine] = []
        var summaries: [RegionLine] = []
        let fontInk = source.runs.reduce(CGRect.null) { $0.union(CTFontGetBoundingBox($1.font.value)) }
        let flush: CGFloat = source.align == 1 ? 0.5 : source.align == 2 ? 1 : 0
        var baselines: [CGFloat] = []
        var maxWidth: CGFloat = 0
        var y: CGFloat = 0
        var start = 0
        var lineCount = 0
        let limit = width.isFinite ? Double(width) : Double.greatestFiniteMagnitude
        // CoreText breaks a word when it cannot fit; CSS normal instead lets
        // that word overflow. Public Unicode line boundaries distinguish those
        // emergency breaks from ordinary opportunities (including CJK).
        let boundaries = spec.overflowWrap == 0 && width.isFinite ? preparation.lineBreakBoundaries : []
        var boundaryIndex = 0
        while start < length {
            if lineCount % 128 == 0 { try beforeMetadata() }
            if spec.lineClamp > 0 && lineCount == spec.lineClamp { break }
            var count: Int
            // Line breaking depends on source/wrap, not compact glyph retention.
            // Zero-width offers retain CoreText's degenerate newline handling.
            if width.isFinite && width > 0 && spec.overflowWrap == 0 {
                count = preparation.suggestBreak(at: start, width: limit, cursor: &boundaryIndex)
            } else {
                count = CTTypesetterSuggestLineBreak(typesetter, start, limit)
                while boundaryIndex < boundaries.count && boundaries[boundaryIndex] < start + count {
                    boundaryIndex += 1
                }
                if boundaryIndex < boundaries.count { count = boundaries[boundaryIndex] - start }
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
            var lineInk = fontInk
            func include(_ a: CGFloat, _ b: CGFloat) { above = max(above, a); below = max(below, b) }
            for glyphRun in CTLineGetGlyphRuns(line) as! [CTRun] {
                let range = CTRunGetStringRange(glyphRun)
                let attributes = CTRunGetAttributes(glyphRun) as NSDictionary
                let shapedFont = attributes[kCTFontAttributeName] as! CTFont
                if compact { lineInk = lineInk.union(CTFontGetBoundingBox(shapedFont)) }
                var matched = false, includesNormal = false
                // CoreText can coalesce adjacent spans with the same glyph
                // attributes even when their authored line heights differ.
                for authored in spec.runs(overlapping: NSRange(location: range.location, length: range.length)) {
                    matched = true
                    if authored.lineHeight != nil {
                        // Explicit boxes use authored metrics; fallback ink
                        // can overflow without enlarging the inline box.
                        let (a, b) = extents(authored)
                        include(a, b)
                    } else {
                        includesNormal = true
                    }
                }
                if !matched, source.strutExplicit {
                    let (a, b) = minimum
                    include(a, b)
                    continue
                }
                if matched && !includesNormal { continue }
                // Normal line height includes the actual emoji/fallback face's
                // line box, as the browser's does.
                let (a, d) = CSSLineBox.extents(shapedFont, height: nil)
                include(a, d)
            }
            if retainHits || lineCount == 0 { baselines.append(y + above) }
            y += above + below
            if retainHits { lineBottoms.append(y) }
            maxWidth = max(maxWidth, w)
            if retainHits {
                if compact {
                    summaries.append(RegionLine(line, flush: flush, width: width, captureHits: false,
                                                conservativeInk: lineInk.isNull ? .zero : lineInk))
                } else { lines.append(line) }
            }
            lineCount += 1
            start += count
        }
        if lineCount == 0 {
            // Empty editors retain the paragraph's own line box.
            baselines.append(minimum.0)
            y = minimum.0 + minimum.1
        }
        // The line boxes are already the browser's (CSSLineBox): the height is
        // their sum; a width rounds up to the 1/64 layout unit.
        // Check before metadata and at its coarse line boundaries. No partial
        // metrics or layout binding escape if the authoritative owner changed.
        try beforeMetadata()
        let metadata: RegionParagraph
        if compact {
            metadata = RegionParagraph(source: source, sourceSHA256: preparation.sourceSHA256,
                lines: summaries, baselines: baselines, lineBottoms: lineBottoms,
                width: CSSLineBox.layoutWidth(maxWidth), height: y, offeredWidth: width)
        } else {
            metadata = try RegionParagraph(source: source, sourceSHA256: preparation.sourceSHA256,
                lines: lines, baselines: baselines, width: CSSLineBox.layoutWidth(maxWidth), height: y,
                lineBottoms: lineBottoms, offeredWidth: width, retainHits: retainHits, captureHits: false,
                metadataCheckpoint: beforeMetadata)
        }
        return RegionWorkerLayout(source: source, lines: retainHits ? lines : [], baselines: retainHits ? baselines : [],
                                  metadata: metadata, preparation: preparation)
    }

    func exactIndex(at point: CGPoint, in bounds: CGRect) -> Int {
        precondition(!Thread.isMainThread)
        if point.y < bounds.minY { return 0 }
        if point.y > bounds.maxY { return source.utf16Count }
        guard let i = metadata.lineIndex(at: point.y - bounds.minY) else { return 0 }
        let value = CTLineGetStringIndexForPosition(line(at: i),CGPoint(
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
            return CSSLineBox.layoutWidth(widest)
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
                let value = CSSLineBox.layoutWidth(CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)))
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

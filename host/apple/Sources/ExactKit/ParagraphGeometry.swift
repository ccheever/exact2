// @ref LLP 1043.000 §3 D7 — every consumer uses fragment origins and band bounds.
import Foundation
import CoreText

extension TextEngine {
    /// CSS Text 3: an over-wide line is start-aligned, even under center/right.
    /// Exclude hanging whitespace as CoreText's flush alignment does. Every
    /// Apple paint and hit path uses the paragraph's authored direction.
    static func lineOffset(_ line: CTLine, flush: CGFloat, width: CGFloat, rtl: Bool) -> CGFloat {
        let advance = CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)) - CTLineGetTrailingWhitespaceWidth(line)
        let resolved: CGFloat = advance > width ? (rtl ? 1 : 0) : flush
        return CGFloat(CTLineGetPenOffsetForFlush(line, resolved, Double(width)))
    }

    /// A broken line as CSS finishes it, for every Apple path that makes one
    /// from a range (layout, rasters, regions). A line that ends at a soft
    /// hyphen (U+00AD) shows a hyphen there, as the browser does; CoreText
    /// breaks there but keeps the character invisible. With `justify` (CSS
    /// `text-align: justify` at that width) every line but the paragraph's
    /// last and one ending in a forced break fills the width
    /// (`text-align-last: auto`). Indices stay the source's, so hit testing,
    /// selection and later layouts read the same ranges. The reader diary
    /// found both missing only on Apple.
    static func finishedLine(_ line: CTLine, source: NSAttributedString, range: CFRange, justify width: Double?) -> CTLine {
        let line = inkedSoftHyphen(line, source: source, range: range)
        return width.map { justified(line, source: source, range: range, width: $0) } ?? line
    }

    static func inkedSoftHyphen(_ line: CTLine, source: NSAttributedString, range: CFRange) -> CTLine {
        let end = range.location + range.length
        guard range.length > 0, end <= source.length,
              (source.string as NSString).character(at: end - 1) == 0xAD else { return line }
        // The prefix keeps every UTF-16 index where it was; only the chosen
        // SHY becomes ink (the exclusion path's rule, TextFlow).
        let visible = NSMutableAttributedString(attributedString: source.attributedSubstring(from: NSRange(location: 0, length: end)))
        visible.replaceCharacters(in: NSRange(location: end - 1, length: 1), with: "-")
        return CTTypesetterCreateLine(CTTypesetterCreateWithAttributedString(visible), range)
    }

    static func justified(_ line: CTLine, source: NSAttributedString, range: CFRange, width: Double) -> CTLine {
        let end = range.location + range.length
        let text = source.string as NSString
        guard width.isFinite, range.length > 0, end < source.length,
              ![0x0A, 0x0D, 0x2028, 0x2029].contains(text.character(at: end - 1)) else { return line }
        // A line with no justification opportunity (a word separator, or
        // ideographs) stays at the start, as Chrome's does: CoreText would
        // otherwise spread a lone word's letters.
        let trimmed = text.substring(with: NSRange(location: range.location, length: range.length))
            .trimmingCharacters(in: .whitespaces)
        guard trimmed.unicodeScalars.contains(where: {
            $0.properties.generalCategory == .spaceSeparator || $0.properties.isIdeographic
        }) else { return line }
        return CTLineCreateJustifiedLine(line, 1.0, width) ?? line
    }
}

/// Where each line of a paragraph starts, from its start edge (which `rtl`
/// puts on the right): CSS `text-indent` on the first line, and a Markdown
/// list item's head indent on each of its lines, the first one's less the
/// marker hung before it — the `padding-left` and outside marker the web
/// lays the same item out with (LLP 1045 D4). Measure, layout, paint, hits
/// and raster workers all start lines here.
struct LineInsets {
    var textIndent: CGFloat = 0
    var rtl = false
    /// The indented paragraphs (each ends after its newline), UTF-16
    /// [start, end), in order: the first line's inset and the others'.
    var heads: [(start: Int, end: Int, first: CGFloat, rest: CGFloat)] = []

    init() {}
    /// `spec`'s; `marker` is the width of a hung run's UTF-16 range, as
    /// the paragraph's own source shapes it.
    init(_ spec: Spec, marker: (CFRange) -> CGFloat) {
        textIndent = spec.textIndent; rtl = spec.direction == 1
        guard spec.runs.contains(where: { $0.indent != 0 }) else { return }
        var open: (start: Int, first: CGFloat, rest: CGFloat)?
        var at = 0
        func close(_ end: Int) {
            if let p = open, p.first != 0 || p.rest != 0 { heads.append((p.start, end, p.first, p.rest)) }
            open = nil
        }
        for run in spec.runs {
            var offset = at
            for unit in run.text.utf16 {
                if open == nil {
                    // A paragraph takes its indent from its first run.
                    let hung = run.hang && offset == at ? marker(CFRange(location: at, length: run.text.utf16.count)) : 0
                    open = (offset, run.indent - hung, run.indent)
                }
                offset += 1
                if unit == 0x0A { close(offset) }
            }
            at = offset
        }
        close(at)
    }

    /// The inset of the line that starts at UTF-16 `location`: how far
    /// right its start moves (`left`) and how much room it gives (`width`).
    func at(_ location: Int) -> (left: CGFloat, width: CGFloat) {
        var width: CGFloat = location == 0 ? textIndent : 0
        var lo = 0, hi = heads.count
        while lo < hi { let mid = (lo + hi) / 2; if heads[mid].start <= location { lo = mid + 1 } else { hi = mid } }
        if lo > 0, location < heads[lo - 1].end {
            width += location == heads[lo - 1].start ? heads[lo - 1].first : heads[lo - 1].rest
        }
        return (rtl ? 0 : width, width)
    }
}

extension TextShape {
    /// This source's `LineInsets`, its markers shaped by its own typesetter.
    var lineInsets: LineInsets {
        if let insets { return insets }
        let made = LineInsets(spec) { CGFloat(CTLineGetTypographicBounds(CTTypesetterCreateLine(self.typesetter, $0), nil, nil, nil)) }
        insets = made
        return made
    }
}

extension LineInsets {
    /// `spec`'s, its markers shaped from `source` (a raster worker's copy).
    init(_ spec: Spec, source: NSAttributedString) {
        self.init(spec) { range in
            let marker = source.attributedSubstring(from: NSRange(location: range.location, length: range.length))
            return CGFloat(CTLineGetTypographicBounds(CTLineCreateWithAttributedString(marker), nil, nil, nil))
        }
    }
}

extension Paragraph {
    /// Ellipsis changes visible glyphs, never measurement or logical ownership.
    /// Paint, selection and hits all consume this same visible line.
    func visibleLine(_ index: Int, width: CGFloat) -> CTLine {
        guard let spec = shape?.spec, spec.ellipsis else { return lines[index] }
        return ellipsized(index, spec: spec, width: width)
    }

    func origin(_ index: Int, align: Int, width: CGFloat) -> CGFloat {
        if origins.indices.contains(index) { return origins[index] }
        var flush: CGFloat = align == 1 ? 0.5 : align == 2 ? 1 : 0
        // CSS `text-indent` and a list item's indent: the line aligns in
        // what its inset leaves.
        let inset = insets.at(CTLineGetStringRange(lines[index]).location)
        let available = width - inset.width
        let original = CGFloat(CTLineGetTypographicBounds(lines[index], nil, nil, nil)) - CTLineGetTrailingWhitespaceWidth(lines[index])
        if original > available { flush = insets.rtl ? 1 : 0 }
        return inset.left + TextEngine.lineOffset(visibleLine(index, width: width), flush: flush, width: available, rtl: insets.rtl)
    }

    /// First select the band, then the closest painted fragment in that band.
    /// A click in an exclusion chooses the adjacent caret, never a different row.
    func lineIndex(at point: CGPoint, align: Int, width: CGFloat) -> Int? {
        guard !lines.isEmpty else { return nil }
        var first = lines.count - 1
        for i in lines.indices where point.y < (lineBottoms.indices.contains(i) ? lineBottoms[i] : height) {
            first = i; break
        }
        let baseline = baselines[first]
        var best = first, distance = CGFloat.infinity
        for i in lines.indices where baselines[i] == baseline {
            let x = origin(i, align: align, width: width)
            let advance = CGFloat(CTLineGetTypographicBounds(visibleLine(i, width: width), nil, nil, nil))
            let d = max(0, max(x - point.x, point.x - x - advance))
            if d < distance { best = i; distance = d }
        }
        return best
    }

    func stringIndex(in line: Int, at x: CGFloat, width: CGFloat? = nil) -> Int {
        if fragments.indices.contains(line), CTLineGetGlyphCount(lines[line]) == 0 {
            return fragments[line].utf16_start
        }
        let visible = width.map { visibleLine(line, width: $0) } ?? lines[line]
        for run in CTLineGetGlyphRuns(visible) as! [CTRun] {
            guard (CTRunGetAttributes(run) as NSDictionary)[TextEngine.overflowToken] as? Bool == true else { continue }
            var position = CGPoint.zero
            CTRunGetPositions(run, CFRange(location: 0, length: 1), &position)
            let advance = CGFloat(CTRunGetTypographicBounds(run, CFRange(), nil, nil, nil))
            if x >= position.x, x <= position.x + advance { return kCFNotFound }
        }
        return CTLineGetStringIndexForPosition(visible, CGPoint(x: x, y: 0))
    }

    func selectionRects(_ range: NSRange, align: Int, in bounds: CGRect, dirty: CGRect) -> [CGRect] {
        guard range.location >= 0, range.length > 0 else { return [] }
        var rects: [CGRect] = []
        for i in lines.indices {
            let line = visibleLine(i, width: bounds.width)
            var above: CGFloat = 0, below: CGFloat = 0
            _ = CTLineGetTypographicBounds(line, &above, &below, nil)
            let y = bounds.minY + baselines[i].rounded()
            guard y + below >= dirty.minY, y - above <= dirty.maxY else { continue }
            let x = bounds.minX + origin(i, align: align, width: bounds.width)
            if shape?.spec.ellipsis != true {
                let r = CTLineGetStringRange(line)
                let lo = max(range.location, r.location), hi = min(NSMaxRange(range), r.location + r.length)
                guard hi > lo else { continue }
                let x0 = CTLineGetOffsetForStringIndex(line, lo, nil), x1 = CTLineGetOffsetForStringIndex(line, hi, nil)
                rects.append(CGRect(x: x + min(x0, x1), y: y - above, width: max(1, abs(x1 - x0)), height: above + below))
                continue
            }
            for run in CTLineGetGlyphRuns(line) as! [CTRun] {
                if (CTRunGetAttributes(run) as NSDictionary)[TextEngine.overflowToken] as? Bool == true { continue }
                let r = CTRunGetStringRange(run)
                let lo = max(range.location, r.location), hi = min(NSMaxRange(range), r.location + r.length)
                guard hi > lo else { continue }
                let x0 = CTLineGetOffsetForStringIndex(line, lo, nil), x1 = CTLineGetOffsetForStringIndex(line, hi, nil)
                rects.append(CGRect(x: x + min(x0, x1), y: y - above, width: max(1, abs(x1 - x0)), height: above + below))
            }
        }
        return rects
    }

    /// Full logical coverage and visible ranges are separate: trailing whitespace
    /// belongs to a fragment even though CoreText never paints it. Work is O(F).
    var flowFacts: [String: Any] {
        ["line_height": flowLineHeight, "height": height,
         "prepare_count": shape?.prepareCount ?? 0,
         "utf8_length": shape?.flow?.utf8Count ?? 0,
         "complete": (fragments.last?.end ?? 0) == shape?.flow?.utf8Count,
         "fragments": fragments.enumerated().map { i, f -> [String: Any] in
             ["start": f.start, "end": f.end, "utf16_start": f.utf16_start, "utf16_end": f.utf16_end,
              "paint_start": f.paint_start, "paint_end": f.paint_end, "line": f.line,
              "x": origins[i], "y": f.y, "width": f.width, "available": f.available,
              "paint_width": CTLineGetTypographicBounds(lines[i], nil, nil, nil),
              "baseline": baselines[i], "bottom": lineBottoms[i]]
         }]
    }
}

extension Spec {
    /// CSS `hyphens: auto`: a soft hyphen at each of the language's own
    /// hyphenation points (CoreFoundation's dictionaries, as Safari's), which
    /// then break and show as authored ones do. Chrome's limits: a word of 5
    /// letters or more, 2 before a point and 2 after; a word with an authored
    /// soft hyphen keeps only those. An unknown language is the web page's
    /// default `lang="en"`; one CoreFoundation cannot hyphenate is left as
    /// `manual`. The source map takes the inserted characters back out.
    mutating func hyphenateAuto() {
        guard hyphens == 2 else { return }
        let locale = CFLocaleCreate(nil, CFLocaleCreateCanonicalLocaleIdentifierFromString(nil, (language.isEmpty ? "en" : language) as CFString))
        guard CFStringIsHyphenationAvailableForLocale(locale) else { return }
        var shaped = 0
        var inserted: [Int] = []
        for i in runs.indices {
            let text = runs[i].text as NSString
            var points: [Int] = []
            var at = 0
            while at < text.length {
                guard let scalar = UnicodeScalar(text.character(at: at)), CharacterSet.letters.contains(scalar) else { at += 1; continue }
                var end = at
                while end < text.length, let s = UnicodeScalar(text.character(at: end)), CharacterSet.letters.contains(s) { end += 1 }
                let word = CFRange(location: at, length: end - at)
                if word.length >= 5, text.range(of: "\u{AD}", range: NSRange(location: at, length: end - at)).location == NSNotFound {
                    var limit = end - 2
                    while limit > at + 2 {
                        let point = CFStringGetHyphenationLocationBeforeIndex(text, limit, word, 0, locale, nil)
                        guard point != kCFNotFound, point >= at + 2, point <= end - 2 else { break }
                        points.append(point)
                        limit = point
                    }
                }
                at = end
            }
            guard !points.isEmpty else { shaped += text.length; continue }
            points.sort()
            let value = NSMutableString(string: text)
            for p in points.reversed() { value.insert("\u{AD}", at: p) } // last first, so earlier points hold
            for (k, p) in points.enumerated() { inserted.append(shaped + p + k) }
            runs[i].text = value as String
            shaped += value.length
        }
        source.inserted = inserted
    }
}

// @ref LLP 1043.000 §3 D7 — every consumer uses fragment origins and band bounds.
import Foundation
import CoreText

extension TextEngine {
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

extension Paragraph {
    func origin(_ index: Int, align: Int, width: CGFloat) -> CGFloat {
        if origins.indices.contains(index) { return origins[index] }
        let flush: CGFloat = align == 1 ? 0.5 : align == 2 ? 1 : 0
        // CSS `text-indent`: the first line aligns in what the indent leaves.
        let inset: (left: CGFloat, width: CGFloat) = index == 0 && lines.first.map({ CTLineGetStringRange($0).location == 0 }) == true ? firstLineInset : (0, 0)
        return inset.left + CGFloat(CTLineGetPenOffsetForFlush(lines[index], flush, Double(width - inset.width)))
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
            let advance = CGFloat(CTLineGetTypographicBounds(lines[i], nil, nil, nil))
            let d = max(0, max(x - point.x, point.x - x - advance))
            if d < distance { best = i; distance = d }
        }
        return best
    }

    func stringIndex(in line: Int, at x: CGFloat) -> Int {
        if fragments.indices.contains(line), CTLineGetGlyphCount(lines[line]) == 0 {
            return fragments[line].utf16_start
        }
        return CTLineGetStringIndexForPosition(lines[line], CGPoint(x: x, y: 0))
    }

    func selectionRects(_ range: NSRange, align: Int, in bounds: CGRect, dirty: CGRect) -> [CGRect] {
        guard range.location >= 0, range.length > 0 else { return [] }
        var rects: [CGRect] = []
        for i in lines.indices {
            let line = lines[i], r = CTLineGetStringRange(line)
            let lo = max(range.location, r.location), hi = min(NSMaxRange(range), r.location + r.length)
            guard hi > lo else { continue }
            var above: CGFloat = 0, below: CGFloat = 0
            _ = CTLineGetTypographicBounds(line, &above, &below, nil)
            let y = bounds.minY + baselines[i].rounded()
            guard y + below >= dirty.minY, y - above <= dirty.maxY else { continue }
            let x = bounds.minX + origin(i, align: align, width: bounds.width)
            let x0 = CTLineGetOffsetForStringIndex(line, lo, nil), x1 = CTLineGetOffsetForStringIndex(line, hi, nil)
            rects.append(CGRect(x: x + min(x0, x1), y: y - above, width: max(1, abs(x1 - x0)), height: above + below))
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

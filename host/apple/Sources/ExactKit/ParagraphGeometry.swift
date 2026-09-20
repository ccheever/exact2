// @ref LLP 1043.000 §3 D7 — every consumer uses fragment origins and band bounds.
import Foundation
import CoreText

extension Paragraph {
    func origin(_ index: Int, align: Int, width: CGFloat) -> CGFloat {
        if origins.indices.contains(index) { return origins[index] }
        let flush: CGFloat = align == 1 ? 0.5 : align == 2 ? 1 : 0
        return CGFloat(CTLineGetPenOffsetForFlush(lines[index], flush, Double(width)))
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

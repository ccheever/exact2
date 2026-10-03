// The device's posture and the viewport segments a fold makes (LLP 1078
// D4, D5, D7): the web's vocabulary and nothing native — `device-posture`
// (`continuous` | `folded`) and the segment grid CSS-ENV-1 §2.3 names,
// `cols × rows` rects in the viewport's points, row-major, none for one
// segment. `exact_segments` is told this; `layout.env` reports it. The
// split is a pure function of a viewport and its divider bands, so the
// XCTest holds it without a 27.1 simulator.
import CoreGraphics
import Foundation

struct ViewportFold: Equatable {
    var posture = "continuous"
    var cols = 1
    var rows = 1
    var rects: [CGRect] = []
    static let flat = ViewportFold()

    /// `layout.env`'s four names (LLP 1012 §1; LLP 1078 D7).
    var env: [String: Any] {
        ["device-posture": posture, "horizontal-viewport-segments": cols, "vertical-viewport-segments": rows,
         "viewport-segments": rects.map { [Agent.r2($0.minX), Agent.r2($0.minY), Agent.r2($0.width), Agent.r2($0.height)] }]
    }
}

enum Segments {
    /// Split a viewport along its divider bands, each in the viewport's own
    /// coordinates and including its margins (UIKit's division frame, LLP
    /// 1078 Q5): a band taller than wide cuts columns, a wider one rows; the
    /// band belongs to no segment; a band outside the viewport, or one that
    /// leaves a segment no room, cuts nothing. `folded` while any band is
    /// present, `continuous` otherwise.
    static func split(viewport: CGSize, dividers: [CGRect]) -> ViewportFold {
        let bounds = CGRect(origin: .zero, size: viewport)
        var xCuts: [(CGFloat, CGFloat)] = [], yCuts: [(CGFloat, CGFloat)] = []
        for band in dividers {
            let clipped = band.intersection(bounds)
            guard !clipped.isNull, !clipped.isEmpty else { continue }
            if band.height >= band.width { xCuts.append((clipped.minX, clipped.maxX)) } else { yCuts.append((clipped.minY, clipped.maxY)) }
        }
        let columns = spans(xCuts, viewport.width), rows = spans(yCuts, viewport.height)
        var fold = ViewportFold(posture: dividers.isEmpty ? "continuous" : "folded", cols: columns.count, rows: rows.count)
        if fold.cols * fold.rows > 1 {
            for (y, h) in rows { for (x, w) in columns { fold.rects.append(CGRect(x: x, y: y, width: w, height: h)) } }
        }
        return fold
    }

    /// The segments left between the cuts, as (start, length), at least one.
    private static func spans(_ cuts: [(CGFloat, CGFloat)], _ total: CGFloat) -> [(CGFloat, CGFloat)] {
        var out: [(CGFloat, CGFloat)] = []
        var position: CGFloat = 0
        for (start, end) in cuts.sorted(by: { $0.0 < $1.0 }) {
            if start > position { out.append((position, start - position)) }
            position = max(position, end)
        }
        if total > position { out.append((position, total - position)) }
        return out.isEmpty ? [(0, total)] : out
    }

    /// The grid a host without a fold makes for `prefer segments
    /// <cols>x<rows> [gap <points>]`: the viewport split evenly, the gap
    /// centred on each divider. Refused by name: a zero count, a negative
    /// gap, a gap wider than the viewport.
    static func even(viewport: CGSize, cols: Int, rows: Int, gap: CGFloat) throws -> [CGRect] {
        struct Refused: Error, CustomStringConvertible { let description: String }
        guard cols >= 1, rows >= 1 else { throw Refused(description: "segments \(cols)x\(rows): each count is at least 1") }
        guard gap.isFinite, gap >= 0 else { throw Refused(description: "segments: gap \(gap) is not a non-negative length") }
        if cols * rows == 1 { return [] }
        func span(_ total: CGFloat, _ n: Int) throws -> CGFloat {
            let bands = CGFloat(n - 1) * gap
            guard bands < total else { throw Refused(description: "segments \(cols)x\(rows) gap \(gap): the gap is wider than the viewport (\(viewport.width) × \(viewport.height))") }
            return (total - bands) / CGFloat(n)
        }
        let w = try span(viewport.width, cols), h = try span(viewport.height, rows)
        var out: [CGRect] = []
        for y in 0..<rows { for x in 0..<cols { out.append(CGRect(x: CGFloat(x) * (w + gap), y: CGFloat(y) * (h + gap), width: w, height: h)) } }
        return out
    }
}

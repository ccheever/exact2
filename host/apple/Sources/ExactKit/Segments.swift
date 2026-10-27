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

    /// The fold under a keyboard that ends the viewport at `viewport` (LLP
    /// 1078, "As built (the keyboard)"): the posture, the count and each
    /// segment's top and left stay the window's, and each segment (with
    /// `lastRow`, each of the last row) ends where the viewport does if it
    /// ends first, never below zero, as Chrome ends its segments at the
    /// visible viewport. Under `resizes-content`
    /// `viewport` is the layout viewport and every segment ends there; Chrome
    /// also clips the band to it, moving the lower segment up to its end or
    /// dropping the fold, where this keeps the band at the hinge (LLP 1001).
    /// Under `resizes-visual` it is the visual viewport and, as in Chrome,
    /// only the last row ends there (`lastRow`).
    func ending(at viewport: CGSize, lastRow: Bool = false) -> ViewportFold {
        var fold = self
        let first = lastRow ? max(0, rects.count - cols) : 0
        fold.rects = rects.enumerated().map { i, r in
            i < first ? r : CGRect(x: r.minX, y: r.minY, width: max(0, min(r.maxX, viewport.width) - r.minX),
                                   height: max(0, min(r.maxY, viewport.height) - r.minY))
        }
        return fold
    }

    /// The fold whose segments are `rects`, the parts of a grid's segments
    /// a box inside it holds (LLP 1123 D9), when they make a whole grid;
    /// flat otherwise.
    static func grid(_ rects: [CGRect]) -> ViewportFold {
        let cols = Set(rects.map(\.minX)).count, rows = Set(rects.map(\.minY)).count
        guard rects.count > 1, cols * rows == rects.count else { return .flat }
        return ViewportFold(posture: "folded", cols: cols, rows: rows, rects: rects.sorted { ($0.minY, $0.minX) < ($1.minY, $1.minX) })
    }

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
    /// present or the hinge itself says it is bent (`hingeBent`: UIKit's
    /// `partiallyOpen`, the Device Posture API's own definition of
    /// `folded`), `continuous` otherwise: the posture never waits for the
    /// regions, which can trail a hinge update by a frame or stop being
    /// reported for a scene altogether (LLP 1078 §As built, the deaf session).
    static func split(viewport: CGSize, dividers: [CGRect], hingeBent: Bool = false) -> ViewportFold {
        let bounds = CGRect(origin: .zero, size: viewport)
        var xCuts: [(CGFloat, CGFloat)] = [], yCuts: [(CGFloat, CGFloat)] = []
        for band in dividers {
            let clipped = band.intersection(bounds)
            guard !clipped.isNull, !clipped.isEmpty else { continue }
            if band.height >= band.width { xCuts.append((clipped.minX, clipped.maxX)) } else { yCuts.append((clipped.minY, clipped.maxY)) }
        }
        let columns = spans(xCuts, viewport.width), rows = spans(yCuts, viewport.height)
        var fold = ViewportFold(posture: dividers.isEmpty && !hingeBent ? "continuous" : "folded", cols: columns.count, rows: rows.count)
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

extension ExactSession {
    /// The device's posture and the viewport segments a fold makes (LLP 1078 D4, D5): the view's reading,
    /// kept for the agent's `layout.env` and told to the kernel and the runner in one batch.
    /// Returns the batch's error, when the runtime refused the grid; the fold is kept only when it took it.
    @discardableResult func segments(_ fold: ViewportFold) -> String? {
        guard booted, state != .destroyed else { return nil }
        // The routes are told their own segments first (LLP 1123 D9), so the batch lays them out once
        let before = presenter.fold
        presenter.fold = fold
        let batch = runtime.segments(fold)
        if batch.error != nil { presenter.fold = before }
        apply(batch)
        return batch.error
    }
}

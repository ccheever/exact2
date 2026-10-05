// @ref LLP 1093 D6–D8: multi-column layout on Apple. The kernel cuts each
// flow into columns and publishes every box at its column's place; a box
// that straddles columns arrives with its union frame and, in a `fragments`
// op, the pieces it was cut into. A paragraph is laid out once, at its own
// unfragmented width, and painted once per fragment, clipped to it and moved
// by its offset, so the lines each column shows are the ones the kernel cut
// there. Hits, selection and link offsets map a point through the fragment
// it lands in; a point in the union's gap is no hit. `column-rule` is drawn
// centred in each gap between two columns that both hold a box.
import Foundation
import CoreGraphics
import CExact
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// A box's fragments and a container's columns, as the kernel last cut them.
struct ColumnRecord: Equatable {
    struct Fragment: Equatable {
        /// In the box's own (published, union) frame.
        var rect: CGRect
        var lines: Range<Int>
        /// What moves a point of the unfragmented box into this fragment.
        var dx: CGFloat
        var dy: CGFloat
    }
    struct Column: Equatable {
        /// In the container's border box.
        var rect: CGRect
        var holds: Bool
    }
    var fragments: [Fragment] = []
    var columns: [Column] = []

    init?(_ payload: [String: Any]) {
        func rows(_ key: String) -> [[CGFloat]] {
            (payload[key] as? [Any] ?? []).map { row in
                (row as? [Any] ?? []).map { CGFloat(($0 as? NSNumber)?.doubleValue ?? 0) }
            }
        }
        fragments = rows("fragments").filter { $0.count == 8 }.map {
            Fragment(rect: CGRect(x: $0[0], y: $0[1], width: $0[2], height: $0[3]),
                     lines: Int($0[4])..<max(Int($0[4]), Int($0[5])), dx: $0[6], dy: $0[7])
        }
        columns = rows("columns").filter { $0.count == 5 }.map {
            Column(rect: CGRect(x: $0[0], y: $0[1], width: $0[2], height: $0[3]), holds: $0[4] != 0)
        }
        if fragments.isEmpty && columns.isEmpty { return nil }
    }
}

extension NodeView {
    /// The `fragments` op: this box's fragments, or a container's columns.
    func applyColumns(_ payload: [String: Any]) {
        let record = ColumnRecord(payload)
        if record == columnRecord { return }
        dropTextRaster()
        columnRecord = record
        cachedTextLayout = nil
        layoutColumnRules()
        #if os(macOS)
        needsDisplay = true
        layer?.setNeedsDisplay()
        #else
        setNeedsDisplay()
        layer.setNeedsDisplay()
        #endif
    }

    /// The paragraph's content box as one column laid it out: `contentBox()`,
    /// or for a fragmented box the same insets at its unfragmented width (a
    /// multi-column `text`'s: its column width). Its lines break at this
    /// width; points mapped by `paragraphPoint` are in its space.
    func paragraphBox() -> CGRect {
        let box = contentBox()
        guard let record = columnRecord, let first = record.fragments.first else { return box }
        let width = record.columns.isEmpty ? first.rect.width - (bounds.width - box.width) : first.rect.width
        return CGRect(x: box.minX, y: box.minY, width: max(0, width), height: 1e7)
    }

    /// Where `point`, in this view's space, lies in the unfragmented
    /// paragraph: inside a fragment, less its offset. `nil` in a union's gap.
    func paragraphPoint(_ point: CGPoint) -> CGPoint? {
        guard let frags = columnRecord?.fragments, !frags.isEmpty else { return point }
        guard let f = frags.first(where: { $0.rect.contains(point) }) else { return nil }
        return CGPoint(x: point.x - f.dx, y: point.y - f.dy)
    }

    /// `paragraphPoint` for a caret: a point in the union's gap goes to the
    /// nearest fragment's edge, so a selection dragged across a gap keeps
    /// its focus.
    func caretPoint(_ point: CGPoint) -> CGPoint {
        guard let frags = columnRecord?.fragments, !frags.isEmpty else { return point }
        func gap(_ r: CGRect) -> CGFloat { max(r.minX - point.x, 0, point.x - r.maxX) + max(r.minY - point.y, 0, point.y - r.maxY) }
        guard let f = frags.min(by: { gap($0.rect) < gap($1.rect) }) else { return point }
        let p = CGPoint(x: min(max(point.x, f.rect.minX), f.rect.maxX - 0.5), y: min(max(point.y, f.rect.minY), f.rect.maxY - 0.5))
        return CGPoint(x: p.x - f.dx, y: p.y - f.dy)
    }

    /// Whether a point in this view's space hits a fragmented box: only
    /// inside a fragment, never in its union's gap (LLP 1093 D8).
    func fragmentHit(_ point: CGPoint) -> Bool {
        columnRecord?.fragments.isEmpty != false || paragraphPoint(point) != nil
    }

    /// `body` once in the content box, or once per fragment: clipped to it,
    /// moved by its offset, with the unfragmented box and that offset (what a
    /// dirty rect in this view's space must be moved back by).
    func eachFragment(_ ctx: CGContext, _ body: (CGRect, CGPoint) -> Void) {
        guard let frags = columnRecord?.fragments, !frags.isEmpty else { return body(contentBox(), .zero) }
        let box = paragraphBox()
        for f in frags {
            ctx.saveGState()
            ctx.clip(to: f.rect)
            ctx.translateBy(x: f.dx, y: f.dy)
            body(box, CGPoint(x: f.dx, y: f.dy))
            ctx.restoreGState()
        }
    }

    /// Where the agent's `tap` aims, as `box` (the view's box as seen): a
    /// fragmented box's first fragment, never its union's middle, which may
    /// be a gap (LLP 1093 D12).
    func tapBox(_ box: CGRect) -> CGRect {
        guard let f = columnRecord?.fragments.first, bounds.width > 0 else { return box }
        let scale = box.width / bounds.width
        return CGRect(x: box.minX + f.rect.minX * scale, y: box.minY + f.rect.minY * scale,
                      width: f.rect.width * scale, height: f.rect.height * scale)
    }

    /// A multi-column container's `column-rule`: a layer under its children
    /// holding one rectangle per gap between two columns that both hold a
    /// box, as tall as the columns (D7). A rule wider than its gap moves no
    /// column.
    func layoutColumnRules() {
        #if os(macOS)
        guard let host = layer else { return }
        #else
        let host = layer
        #endif
        let old = host.sublayers?.first { $0.name == "exact-column-rules" }
        let width = CGFloat(number("column_rule_width", 3))
        guard let columns = columnRecord?.columns, columns.count > 1, width > 0,
              style["column_rule_style"]?.string == "solid" else { old?.removeFromSuperlayer(); return }
        let rules = old ?? CALayer()
        if old == nil { rules.name = "exact-column-rules"; host.insertSublayer(rules, at: 0) }
        rules.frame = host.bounds
        let c = channels("column_rule_color") ?? channels("text_color") ?? [0, 0, 0, 255]
        rules.sublayers = zip(columns, columns.dropFirst()).compactMap { a, b in
            guard a.holds && b.holds else { return nil }
            // The gap's middle, whichever side the next column is on.
            let (left, right) = b.rect.minX >= a.rect.minX ? (a.rect.maxX, b.rect.minX) : (b.rect.maxX, a.rect.minX)
            let h = max(a.rect.height, b.rect.height)
            let y = rules.contentsAreFlipped() ? a.rect.minY : host.bounds.height - a.rect.minY - h
            let rule = CALayer()
            rule.frame = CGRect(x: (left + right - width) / 2, y: y, width: width, height: h)
            rule.backgroundColor = TextEngine.color(c).cgColor
            return rule
        }
    }
}

extension TextEngine {
    /// @ref LLP 1093 D6 — each line box's bottom, in content coordinates, of
    /// the paragraph `request` answers: the one the presenter paints.
    func lineBottoms(_ request: ExactMeasureRequest) -> [CGFloat] {
        guard request.width >= 0, request.exclusion_count == 0, request.markup == 0 else { return [] }
        return paragraph(requestSpec(request), width: CGFloat(request.width)).lineBottoms
    }

    /// The kernel's line-box hook (`exact_set_lines`); `ctx` is the engine.
    static let linesText: ExactLinesFn = { ctx, request, out, cap in
        guard let ctx, let request = request?.pointee else { return 0 }
        let bottoms = Unmanaged<TextEngine>.fromOpaque(ctx).takeUnretainedValue().lineBottoms(request)
        if let out { for (i, b) in bottoms.prefix(cap).enumerated() { out[i] = Float(b) } }
        return bottoms.count
    }
}

// Markdown source into the text engine's runs (LLP 1045 D3, D4).
//
// A `markup="markdown"` node reaches the engine as one run of source. The
// archive's `exact_markup_pieces` turns it into display pieces — the same
// function for measure and paint, so the two agree — and this file turns
// those into `Run`s against the node's own font: headings are bigger and
// bolder, code is the monospace family, links carry their target, and the
// quieter roles (markers, quotes) take the ink at reduced opacity.
import Foundation
import CoreGraphics
import CExact

enum MarkupRuns {
    /// The monospace system family, as the kernel's font table numbers it.
    static let monospaceFamily = 5

    /// Expand `source` into runs over `base` (the node's own run style).
    /// `color` is the node's ink; nil keeps the paragraph colour.
    static func expand(_ source: String, base: Run, color: [Double]?) -> [Run] {
        var out: [Run] = []
        var text = source
        text.withUTF8 { bytes in
            var pieces: UnsafePointer<ExactMarkupPiece>? = nil
            var count = 0
            let handle = exact_markup_pieces(bytes.baseAddress, bytes.count, &pieces, &count)
            defer { exact_markup_free(handle) }
            guard handle != 0, let pieces else { return }
            out.reserveCapacity(count)
            for p in UnsafeBufferPointer(start: pieces, count: count) {
                var run = base
                run.text = String(decoding: UnsafeBufferPointer(start: p.text, count: p.len), as: UTF8.self)
                run.size = (base.size * CGFloat(p.scale)).rounded(.toNearestOrEven)
                if p.font_weight != 0 { run.weight = Int(p.font_weight) }
                if p.italic != 0 { run.italic = true }
                if p.mono != 0 { run.family = monospaceFamily }
                if let href = p.href, p.href_len > 0 {
                    run.href = String(decoding: UnsafeBufferPointer(start: href, count: p.href_len), as: UTF8.self)
                }
                var decoration = p.strike != 0 ? "line-through" : ""
                if p.role == 2 { decoration = decoration.isEmpty ? "underline" : decoration + " underline" }
                run.decoration = decoration
                // A short line between blocks keeps the node's line height
                // from stretching it back to a full line.
                if p.scale < 1 && run.text == "\n" { run.lineHeight = nil }
                switch p.role {
                case 3, 4: run.color = color.map { c in [c[0], c[1], c[2], (c.count > 3 ? c[3] : 255) * 0.62] }
                default: run.color = color
                }
                out.append(run)
            }
        }
        return out
    }
}

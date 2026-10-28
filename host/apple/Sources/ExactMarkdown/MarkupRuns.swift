// Markdown source into the text engine's runs (LLP 1045 D3, D4).
//
// A `markup="markdown"` node reaches the engine as one run of source. The
// archive's `exact_markup_pieces` turns it into display pieces — the same
// function for measure and paint, so the two agree — and this file turns
// those into `Run`s against the node's own font: headings are bigger and
// bolder, code is the monospace family, links carry their target, a list
// item's runs its indent, and the quieter roles (markers, quotes) take the
// ink at reduced opacity.
import Foundation
import CoreGraphics
import CExact
import ExactKit

enum MarkupRuns {
    /// The monospace system family, as the kernel's font table numbers it.
    static let monospaceFamily = 5

    /// Native readers have no document base for relative destinations. Parse
    /// absolute targets and gate their scheme at the navigation boundary.
    static func navigationURL(_ href: String) -> URL? {
        guard let url = URL(string: href), let scheme = url.scheme?.lowercased(),
              ["http", "https", "mailto", "tel"].contains(scheme) else { return nil }
        return url.absoluteURL
    }

    /// A link's target as the reader follows it: an absolute path is a
    /// location in the app (`ExactSession.follow`, LLP 1038 §7), which needs
    /// no document base, as the web's `[Ideas](/note/3)` needs none (notes
    /// diary); otherwise `navigationURL`'s, or nothing.
    static func target(_ href: String) -> String? {
        if href.hasPrefix("/"), !href.hasPrefix("//") { return href }
        return navigationURL(href)?.absoluteString
    }

    /// The `href` an atom's run carries: the press it reports, never a
    /// destination (`NodeView.activateLink` takes it, LLP 1045.000 D7).
    static let atomScheme = "exact-atom:"

    /// Expand `source` into runs over `base` (the node's own run style).
    /// `color` is the node's ink; nil keeps the paragraph colour. `linkColor`
    /// is a link's and `atomColor` an atom's, where it is not `color`. `boldText`:
    /// Bold Text weights `base`, and a piece's own weight takes its step.
    /// An atom (LLP 1045.000 D6) is its label on a rounded inline box, a
    /// narrow no-break space each side its padding, so a line breaks around
    /// it and never inside its ends.
    static func expand(_ source: String, base: Run, color: [Double]?, linkColor: [Double]? = nil, atomColor: [Double]? = nil, boldText: Bool = false, atoms: MarkdownAtoms = MarkdownAtoms()) -> [Run] {
        var out: [Run] = []
        var text = source
        let handle = AtomHandle.of(atoms), paint = AtomPaint.resolve(atoms)
        var boxes: UInt32 = 0
        text.withUTF8 { bytes in
            var pieces: UnsafePointer<ExactMarkupPiece>? = nil
            var count = 0
            let handle = exact_markup_pieces(bytes.baseAddress, bytes.count, handle, &pieces, &count)
            defer { exact_markup_free(handle) }
            guard handle != 0, let pieces else { return }
            out.reserveCapacity(count)
            for p in UnsafeBufferPointer(start: pieces, count: count) {
                var run = base
                run.text = String(decoding: UnsafeBufferPointer(start: p.text, count: p.len), as: UTF8.self)
                run.size = (base.size * CGFloat(p.scale)).rounded(.toNearestOrEven)
                if p.font_weight != 0 { run.weight = boldText ? TextEngine.bolder(Int(p.font_weight)) : Int(p.font_weight) }
                if p.italic != 0 { run.italic = true }
                if p.mono != 0 { run.family = monospaceFamily }
                // A list item's indent and hung marker (LLP 1045 D4, `LineInsets`).
                run.indent = CGFloat(p.indent); run.hang = p.hang != 0
                if let href = p.href, p.href_len > 0 {
                    let target = String(decoding: UnsafeBufferPointer(start: href, count: p.href_len), as: UTF8.self)
                    run.href = MarkupRuns.target(target) ?? ""
                }
                var decoration = p.strike != 0 ? "line-through" : ""
                if p.role == 2, !run.href.isEmpty { decoration = decoration.isEmpty ? "underline" : decoration + " underline" }
                run.decoration = decoration
                // A short line between blocks keeps the node's line height
                // from stretching it back to a full line.
                if p.scale < 1 && run.text == "\n" { run.lineHeight = nil }
                let atom = p.atom.flatMap { a in p.atom_len > 0 ? try? JSONSerialization.jsonObject(with: Data(bytes: a, count: p.atom_len)) as? [String: Any] : nil }
                // A link's piece takes `linkColor` and an atom's `atomColor`, and an atom its own ink over either
                let ink = atom != nil ? atomColor ?? color : run.href.isEmpty ? color : linkColor ?? color
                switch p.role {
                case 3, 4: run.color = ink.map { c in [c[0], c[1], c[2], (c.count > 3 ? c[3] : 255) * 0.62] }
                default: run.color = ink
                }
                if let detail = atom {
                    let kind = detail["kind"] as? String ?? ""
                    // One line, as a pill is (D4): its spaces and hyphens do not break.
                    let unbroken = run.text.replacingOccurrences(of: " ", with: "\u{00A0}").replacingOccurrences(of: "-", with: "\u{2011}")
                    run.text = "\u{202F}" + unbroken + "\u{202F}"
                    // Its press is the atom's, not a link's: never underlined as one.
                    run.decoration = p.strike != 0 ? "line-through" : "none"
                    run.background = paint[kind]?.background ?? AtomPaint.token(dark: atoms.dark, contrast: atoms.contrast)
                    if let ink = paint[kind]?.ink { run.color = ink }
                    boxes += 1
                    run.backgroundBox = 0xA700_0000 | boxes
                    run.backgroundRadii = Array(repeating: InlineRadius(px: Double(run.size) * 0.3), count: 4)
                    let press: [Any] = [kind, detail["source"] ?? "", detail["href"] ?? "", detail["start"] ?? 0, detail["end"] ?? 0, detail["role"] ?? "button", detail["title"] ?? ""]
                    if let json = try? JSONSerialization.data(withJSONObject: press), let value = String(data: json, encoding: .utf8) {
                        run.href = atomScheme + (value.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? "")
                    }
                }
                out.append(run)
            }
        }
        return out
    }
}

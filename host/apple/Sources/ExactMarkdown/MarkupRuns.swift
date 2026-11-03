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
    /// `color` is the node's ink; nil keeps the paragraph colour. `boldText`:
    /// Bold Text weights `base`, and a piece's own weight takes its step.
    /// An atom (LLP 1045.000 D6) is its label on a rounded inline box, a
    /// narrow no-break space each side its padding (none where its kind's
    /// padding is zero), so a line breaks around it and never inside its
    /// ends; its label is isolated in the direction its face pins, else in
    /// the lines' (`atoms.rtl`, D4). Its kind's own paint draws (colours,
    /// radius, underline); a border, a ring, a state's paint and an icon do
    /// not, in a reader.
    static func expand(_ source: String, base: Run, color: [Double]?, boldText: Bool = false, atoms: MarkdownAtoms = MarkdownAtoms()) -> [Run] {
        var out: [Run] = []
        var text = source
        let handle = AtomHandle.of(atoms), paint = AtomPaint.resolve(atoms), metrics = AtomPaint.metrics(atoms)
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
                switch p.role {
                case 3, 4: run.color = color.map { c in [c[0], c[1], c[2], (c.count > 3 ? c[3] : 255) * 0.62] }
                default: run.color = color
                }
                if let detail = atom {
                    let kind = detail["kind"] as? String ?? ""
                    // One line, as a pill is (D4): its spaces and hyphens do not break.
                    let unbroken = run.text.replacingOccurrences(of: " ", with: "\u{00A0}").replacingOccurrences(of: "-", with: "\u{2011}")
                    // One box in the line, as the web's is: isolated in the direction its
                    // face pins, else in its line's (D4).
                    let pinned = ["ltr": "\u{2066}", "rtl": "\u{2067}"][detail["direction"] as? String ?? ""]
                    let pad = metrics[kind]?.padding == 0 ? "" : "\u{202F}"
                    run.text = pad + (pinned ?? (atoms.rtl ? "\u{2067}" : "\u{2066}")) + unbroken + "\u{2069}" + pad
                    // Its press is the atom's, not a link's: never underlined as one, but as its kind says.
                    let rows = paint[kind]
                    run.decoration = [p.strike != 0 ? "line-through" : "", rows?.underline == true ? "underline" : ""].filter { !$0.isEmpty }.joined(separator: " ")
                    if run.decoration.isEmpty { run.decoration = "none" }
                    // The face's paint over its kind's (D2, D4).
                    run.background = AtomPaint.face(detail["background"], atoms) ?? rows?.background ?? AtomPaint.token(dark: atoms.dark, contrast: atoms.contrast)
                    if let ink = AtomPaint.face(detail["color"], atoms) ?? rows?.ink { run.color = ink }
                    boxes += 1
                    run.backgroundBox = 0xA700_0000 | boxes
                    run.backgroundRadii = Array(repeating: InlineRadius(px: Double(rows?.radius ?? run.size * 0.3)), count: 4)
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

    /// A text area's `source` as its editor lays it out (`MarkupEditor.restyle`,
    /// MarkupHidden.swift), over `base`, for measuring the field's own text:
    /// its syntax at full advance; headings, code and the spans' bold, italic
    /// and monospace in the editor's fonts; a link's destination and each
    /// atom's other code units as word joiners; each atom's first code unit its
    /// pill, a box `AtomAttachment` sizes in its run's `font`, no wider than
    /// `width`. A list item's wrapped lines' deeper head indent is left out.
    static func editor(_ source: String, base: Run, atoms: MarkdownAtoms, width: CGFloat?, font: (Run) -> PlatformFont) -> [Run] {
        let units = Array(source.utf16)
        guard !units.isEmpty else { return [base] }
        var text = source, json: UnsafePointer<UInt8>? = nil, count = 0
        let handle = text.withUTF8 { bytes in exact_markup_style(bytes.baseAddress, bytes.count, AtomHandle.of(atoms), 0, 0, &json, &count) }
        defer { exact_markup_free(handle) }
        guard handle != 0, let json,
              let object = try? JSONSerialization.jsonObject(with: Data(bytes: json, count: count)) as? [String: [[Any]]] else { return [base] }
        let n = units.count
        let span = { (a: Any, b: Any) -> Range<Int> in
            let s = min(max((a as? Int) ?? 0, 0), n), e = min(max((b as? Int) ?? 0, 0), n)
            return s..<max(s, e)
        }
        // Each code unit's run, and what it is drawn as: 0 text, 1 a word joiner, 2 a pill.
        var blank = base
        blank.text = ""
        var runs = [Run](repeating: blank, count: n), shown = [UInt8](repeating: 0, count: n)
        var pills: [Int: MarkupAtom] = [:]
        for p in object["p"] ?? [] where p.count >= 6 {
            let range = span(p[0], p[1]), kind = (p[2] as? Int) ?? 0, level = (p[3] as? Int) ?? 0
            let indent = CGFloat((p[4] as? Int) ?? 0) * 20 + CGFloat((p[5] as? Int) ?? 0) * 16
            for i in range {
                runs[i].indent = indent
                switch kind {
                case 1:
                    let scale: CGFloat = [1.6, 1.4, 1.2, 1.1, 1.0, 1.0][max(0, min(5, level - 1))]
                    runs[i].size = (base.size * scale).rounded(); runs[i].weight = 700; runs[i].lineHeight = nil
                case 6, 7, 9:
                    runs[i].size = (base.size * 0.92).rounded(); runs[i].family = monospaceFamily; runs[i].italic = false
                default: break
                }
            }
        }
        for s in object["s"] ?? [] where s.count >= 3 {
            let flags = (s[2] as? Int) ?? 0
            guard flags & 7 != 0 else { continue }
            for i in span(s[0], s[1]) {
                if flags & 4 != 0 { runs[i].size = (runs[i].size * 0.92).rounded(); runs[i].family = monospaceFamily }
                if flags & 1 != 0 { runs[i].weight = 700 }
                if flags & 2 != 0 { runs[i].italic = true }
            }
        }
        // A footnote mark's small run.
        for r in object["r"] ?? [] where r.count >= 3 && ((r[2] as? Int) ?? 0) > 3 {
            for i in span(r[0], r[1]) { runs[i].size = (base.size * 0.75).rounded(); runs[i].family = base.family; runs[i].weight = base.weight; runs[i].italic = false }
        }
        for l in object["l"] ?? [] where l.count >= 4 { for i in span(l[2], l[3]) { shown[i] = 1 } }
        let paint = AtomPaint.resolve(atoms), metrics = AtomPaint.metrics(atoms)
        for row in object["a"] ?? [] {
            guard let (range, atom) = MarkupAtom.of(row, paint: paint, metrics: metrics, selection: NSRange(location: 0, length: 0), atoms: atoms),
                  range.location >= 0, NSMaxRange(range) <= n else { continue }
            for i in range.location..<NSMaxRange(range) { shown[i] = 1 }
            shown[range.location] = 2
            pills[range.location] = atom
        }
        var out: [Run] = []
        var i = 0
        while i < n {
            if shown[i] == 2, let atom = pills[i] {
                let face = font(runs[i])
                let bounds = AtomAttachment.bounds(AtomAttachment(atom).width(face), face: face, line: width ?? .greatestFiniteMagnitude)
                var pill = runs[i]
                pill.text = "\u{FFFC}"; pill.hidden = true
                // A point above the descender, where TextKit's line ends: the
                // line keeps its height, as the field's does (§5.1).
                pill.box = InlineBox(width: bounds.width, height: bounds.height - 1, align: 0, shift: bounds.minY + 1)
                out.append(pill)
                i += 1; continue
            }
            var j = i + 1
            while j < n, shown[j] != 2, (shown[j] == 1) == (shown[i] == 1), runs[j] == runs[i] { j += 1 }
            var run = runs[i]
            run.text = shown[i] == 1 ? String(repeating: "\u{2060}", count: j - i) : String(decoding: units[i..<j], as: UTF16.self)
            out.append(run)
            i = j
        }
        return out
    }
}

// Source-backed TextKit 2 Markdown styling, shared by UIKit and AppKit.
// @ref LLP 1045 D1, D5, D11 — only attributes change; source and native selection
// stay intact. Hosts guard composition before entering this styler. A link's
// destination is hidden, as the web editor hides it (`MarkupHidden.swift`);
// other syntax is dimmed at full advance.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import CExact
import ExactKit

final class MarkupEditor: NSObject, MarkdownEditing {
    private(set) var styling = false
    var applying = false
    var bookmark: NSRange?
    var lastSelectionState: String?
    private(set) var plainAttributes: [NSAttributedString.Key: Any] = [:]
    /// The `paragraphDirs` the last restyle drew.
    private(set) var paragraphDirs = ""

    struct Look {
        let font: (CGFloat, Int, Int, Bool) -> PlatformFont
        let size: CGFloat
        let weight: Int
        let family: Int
        let italic: Bool
        let lineHeight: CGFloat?
        let ink: PlatformColor
        /// The node's inline atoms (LLP 1045.000), and the appearance their pills draw for.
        var atoms = MarkdownAtoms()
        /// The app's base direction for each line, `paragraphDirs` (LLP 1045 D11).
        var paragraphDirs = ""
    }

    /// The attributes plain typing takes, whatever the caret sits beside.
    func baseAttributes(_ look: Look) -> [NSAttributedString.Key: Any] {
        let paragraph = NSMutableParagraphStyle()
        if let h = look.lineHeight { paragraph.minimumLineHeight = h; paragraph.maximumLineHeight = h }
        return [.font: look.font(look.size, look.weight, look.family, look.italic), .foregroundColor: look.ink, .paragraphStyle: paragraph]
    }

    /// The base attributes in the direction of the line `caret` is on: an
    /// empty last line has no text of its own, and draws its caret by these.
    func typingAttributes(_ look: Look, caret: Int, in storage: NSAttributedString) -> [NSAttributedString.Key: Any] {
        var attributes = baseAttributes(look)
        let line = storage.string.utf16.prefix(max(0, caret)).reduce(0) { $1 == 10 ? $0 + 1 : $0 }
        if let direction = Self.direction(of: line, in: Self.words(look.paragraphDirs)),
           let paragraph = (attributes[.paragraphStyle] as? NSParagraphStyle)?.mutableCopy() as? NSMutableParagraphStyle {
            paragraph.baseWritingDirection = direction
            attributes[.paragraphStyle] = paragraph
        }
        return attributes
    }

    /// `paragraphDirs`' words, split at HTML's ASCII white space.
    static func words(_ said: String) -> [Substring] {
        said.split { $0 == " " || $0 == "\t" || $0 == "\n" || $0 == "\r" || $0 == "\r\n" || $0 == "\u{0C}" }
    }

    /// Line `line`'s base direction (LLP 1045 D11): its word, HTML's `dir`, a
    /// line past the last word taking the last. Nil for none, or another
    /// word, which leaves the natural one, the line's first strong letter.
    static func direction(of line: Int, in words: [Substring]) -> NSWritingDirection? {
        guard !words.isEmpty else { return nil }
        switch words[min(line, words.count - 1)] {
        case "ltr": return .leftToRight
        case "rtl": return .rightToLeft
        case "auto": return .natural
        default: return nil
        }
    }

    /// Remove all Markdown attributes when the authored row becomes `none`.
    func detach(_ storage: NSTextStorage) {
        styling = true; defer { styling = false }
        storage.setAttributes(plainAttributes, range: NSRange(location: 0, length: storage.length))
    }

    /// Restyle the source storage for its current selection.
    func restyle(_ storage: NSTextStorage, selection: NSRange, look: Look) {
        guard !styling else { return }
        styling = true; defer { styling = false }
        var text = storage.string
        var json: UnsafePointer<UInt8>? = nil
        var count = 0
        let handle = text.withUTF8 { bytes in
            exact_markup_style(bytes.baseAddress, bytes.count, AtomHandle.of(look.atoms), UInt32(clamping: selection.location), UInt32(clamping: selection.location + selection.length), &json, &count)
        }
        defer { exact_markup_free(handle) }
        guard handle != 0, let json,
              let object = try? JSONSerialization.jsonObject(with: Data(bytes: json, count: count)) as? [String: [[Any]]] else { return }
        let length = storage.length
        let clamp = { (a: Any, b: Any) -> NSRange in
            let s = min((a as? Int) ?? 0, length), e = min((b as? Int) ?? 0, length)
            return NSRange(location: s, length: max(0, e - s))
        }
        let base = baseAttributes(look)
        plainAttributes = base
        let baseFont = base[.font] as! PlatformFont
        let paragraph = base[.paragraphStyle] as! NSParagraphStyle
        storage.beginEditing()
        storage.setAttributes(base, range: NSRange(location: 0, length: length))
        // Paragraphs: headings scale and embolden; code is monospace on a
        // tint; quotes dim; list items and quotes indent.
        for p in object["p"] ?? [] where p.count >= 6 {
            let range = clamp(p[0], p[1])
            let kind = (p[2] as? Int) ?? 0, level = (p[3] as? Int) ?? 0, depth = (p[4] as? Int) ?? 0, quote = (p[5] as? Int) ?? 0
            var attributes: [NSAttributedString.Key: Any] = [:]
            let style = paragraph.mutableCopy() as! NSMutableParagraphStyle
            let indent = CGFloat(depth) * 20 + CGFloat(quote) * 16
            style.headIndent = indent; style.firstLineHeadIndent = indent
            switch kind {
            case 1:
                let scale: CGFloat = [1.6, 1.4, 1.2, 1.1, 1.0, 1.0][max(0, min(5, level - 1))]
                attributes[.font] = look.font((look.size * scale).rounded(), 700, look.family, look.italic)
                if look.lineHeight != nil { style.minimumLineHeight = 0; style.maximumLineHeight = 0 }
            case 2, 3, 4:
                style.headIndent = indent + 20
            case 6, 7, 9:
                attributes[.font] = look.font((look.size * 0.92).rounded(), look.weight, MarkupRuns.monospaceFamily, false)
                attributes[.backgroundColor] = look.ink.withAlphaComponent(0.06)
            default: break
            }
            if quote > 0 { attributes[.foregroundColor] = look.ink.withAlphaComponent(0.62) }
            attributes[.paragraphStyle] = style
            storage.addAttributes(attributes, range: range)
        }
        // Each line's base direction, the app's (LLP 1045 D11): TextKit takes
        // a paragraph's from its style, and a line feed ends a paragraph.
        let words = Self.words(look.paragraphDirs)
        if !words.isEmpty {
            let text = storage.string as NSString
            var line = 0, start = 0
            while start < length {
                let feed = text.range(of: "\n", options: .literal, range: NSRange(location: start, length: length - start))
                let end = feed.location == NSNotFound ? length : NSMaxRange(feed)
                if let direction = Self.direction(of: line, in: words) {
                    storage.enumerateAttribute(.paragraphStyle, in: NSRange(location: start, length: end - start)) { value, run, _ in
                        let style = (value as? NSParagraphStyle)?.mutableCopy() as? NSMutableParagraphStyle ?? NSMutableParagraphStyle()
                        style.baseWritingDirection = direction
                        storage.addAttribute(.paragraphStyle, value: style, range: run)
                    }
                }
                line += 1
                start = end
            }
        }
        paragraphDirs = look.paragraphDirs
        for s in object["s"] ?? [] where s.count >= 4 {
            let range = clamp(s[0], s[1])
            guard range.length > 0 else { continue }
            let flags = (s[2] as? Int) ?? 0
            let href = (s[3] as? String) ?? ""
            var attributes: [NSAttributedString.Key: Any] = [:]
            let current = storage.attribute(.font, at: range.location, effectiveRange: nil) as? PlatformFont ?? baseFont
            if flags & 64 != 0 { attributes[.foregroundColor] = look.ink.withAlphaComponent(0.4) }
            if flags & 7 != 0 {
                let mono = flags & 4 != 0
                #if canImport(UIKit)
                let bold = flags & 1 != 0 || current.fontDescriptor.symbolicTraits.contains(.traitBold)
                #else
                let bold = flags & 1 != 0 || current.fontDescriptor.symbolicTraits.contains(.bold)
                #endif
                attributes[.font] = look.font(mono ? (current.pointSize * 0.92).rounded() : current.pointSize, bold ? 700 : look.weight, mono ? MarkupRuns.monospaceFamily : look.family, flags & 2 != 0 || look.italic)
                if mono { attributes[.backgroundColor] = look.ink.withAlphaComponent(0.06) }
            }
            if flags & 8 != 0 { attributes[.strikethroughStyle] = NSUnderlineStyle.single.rawValue }
            if flags & 16 != 0 {
                attributes[.underlineStyle] = NSUnderlineStyle.single.rawValue
                attributes[.foregroundColor] = look.ink
                if let url = MarkupRuns.navigationURL(href) { attributes[.link] = url }
            }
            storage.addAttributes(attributes, range: range)
        }
        // Syntax outside the active paragraph is quieter, at full advance.
        for h in object["h"] ?? [] where h.count >= 2 {
            storage.addAttribute(.foregroundColor, value: look.ink.withAlphaComponent(0.3), range: clamp(h[0], h[1]))
        }
        for r in object["r"] ?? [] where r.count >= 4 {
            let range = clamp(r[0], r[1])
            guard range.length > 0 else { continue }
            let kind = (r[2] as? Int) ?? 0
            switch kind {
            case 0, 1, 2:
                // The list marker, dimmed, until a drawn bullet or box replaces it.
                storage.addAttribute(.foregroundColor, value: look.ink.withAlphaComponent(0.4), range: range)
            case 3: storage.addAttribute(.foregroundColor, value: look.ink.withAlphaComponent(0.3), range: range)
            default:
                // A footnote mark: the label in a small raised run.
                storage.addAttributes([.font: look.font((look.size * 0.75).rounded(), look.weight, look.family, false), .baselineOffset: look.size * 0.33, .foregroundColor: look.ink.withAlphaComponent(0.62)], range: range)
            }
        }
        // A link's `](target)`, whatever the selection, and its `[`
        // (MarkupHidden.swift). One value marks both of a link.
        for (n, l) in (object["l"] ?? []).enumerated() where l.count >= 4 {
            let open = clamp(l[0], l[1]), close = clamp(l[2], l[3])
            guard close.length > 0 else { continue }
            storage.addAttribute(.markupHidden, value: n, range: close)
            if open.length > 0 { storage.addAttribute(.markupOpener, value: n, range: open) }
        }
        // Each inline atom, drawn as its pill whatever the selection
        // (MarkupHidden.swift); one covered by the selection draws selected.
        let atoms = object["a"] ?? []
        let paint = atoms.isEmpty ? [:] : AtomPaint.resolve(look.atoms)
        for row in atoms {
            guard let (range, atom) = MarkupAtom.of(row, paint: paint, selection: selection, atoms: look.atoms), NSMaxRange(range) <= length else { continue }
            storage.addAttribute(.markupAtom, value: atom, range: range)
        }
        storage.endEditing()
    }
}

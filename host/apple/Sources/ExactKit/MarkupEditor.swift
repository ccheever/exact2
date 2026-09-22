// Source-backed TextKit 2 Markdown styling, shared by UIKit and AppKit.
// @ref LLP 1045 D1, D5 — only attributes change; source and native selection
// stay intact. Hosts guard composition before entering this styler. Hidden
// syntax is deliberately dimmed at full advance in the dependable baseline;
// zero-width glyphs need phone selection/caret proof before adoption.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import CExact

final class MarkupEditor: NSObject {
    private(set) var styling = false
    var applying = false
    var bookmark: NSRange?
    var lastSelectionState: String?
    private(set) var plainAttributes: [NSAttributedString.Key: Any] = [:]


    struct Look {
        let font: (CGFloat, Int, Int, Bool) -> PlatformFont
        let size: CGFloat
        let weight: Int
        let family: Int
        let italic: Bool
        let lineHeight: CGFloat?
        let ink: PlatformColor
    }

    /// The attributes plain typing takes, whatever the caret sits beside.
    func baseAttributes(_ look: Look) -> [NSAttributedString.Key: Any] {
        let paragraph = NSMutableParagraphStyle()
        if let h = look.lineHeight { paragraph.minimumLineHeight = h; paragraph.maximumLineHeight = h }
        return [.font: look.font(look.size, look.weight, look.family, look.italic), .foregroundColor: look.ink, .paragraphStyle: paragraph]
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
            exact_markup_style(bytes.baseAddress, bytes.count, UInt32(clamping: selection.location), UInt32(clamping: selection.location + selection.length), &json, &count)
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
                if !href.isEmpty, let url = URL(string: href) { attributes[.link] = url }
            }
            storage.addAttributes(attributes, range: range)
        }
        // Until hidden-range caret and selection gestures are proven on a
        // phone, keep every source character at its normal advance. Syntax
        // outside the active paragraph is quieter, never zero-width.
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
        storage.endEditing()
    }
}

/// Native history, scoped to one text view. TextKit can restore its storage
/// without a text-change delegate callback; completion publishes the source
/// once, after the complete native undo/redo transaction and its selection.
final class NativeTextUndo {
    let manager = UndoManager()
    private var observers: [NSObjectProtocol] = []

    init(before: @escaping () -> Void, after: @escaping () -> Void) {
        let center = NotificationCenter.default
        for name in [Notification.Name.NSUndoManagerWillUndoChange, .NSUndoManagerWillRedoChange] {
            observers.append(center.addObserver(forName: name, object: manager, queue: nil) { _ in before() })
        }
        for name in [Notification.Name.NSUndoManagerDidUndoChange, .NSUndoManagerDidRedoChange] {
            observers.append(center.addObserver(forName: name, object: manager, queue: nil) { _ in after() })
        }
    }
    deinit { for observer in observers { NotificationCenter.default.removeObserver(observer) } }
}

// Inline atoms on Apple (LLP 1045.000 §4, §10 Q9): the editor draws each
// atom's first code unit as U+FFFC carrying a new `NSTextAttachment` whose
// picture is the pill (`image(for:…)`, sized by `attachmentBounds`, no
// attachment view: AppKit keeps every view provider while the text view
// lives, §5.1), and the rest as word joiners; the reader draws an atom's
// piece as its label on an inline box's background. The pill looks like the
// platform's token: NSTokenField's on macOS, UISearchTextField's on iOS (D4),
// unless its face or its kind gives a colour, the face's first (D4). It lays
// out its icon and then its label in its line's direction, or in the one its
// face pins (D4).
#if canImport(UIKit)
import UIKit
typealias AtomImage = UIImage
#else
import AppKit
typealias AtomImage = NSImage
#endif
import CExact
import ExactKit

/// A node's atoms parsed in the archive, by their JSON. The archive's
/// handles are thread-confined, so each thread that styles, measures or
/// paints keeps its own few.
enum AtomHandle {
    private final class Held {
        let value: UInt64
        init(_ value: UInt64) { self.value = value }
        deinit { exact_markup_free(value) }
    }
    private static let key = "ExactMarkupAtomHandles"

    /// The handle for `atoms`; zero when it declares none.
    static func of(_ atoms: MarkdownAtoms) -> UInt64 {
        guard !atoms.isEmpty else { return 0 }
        let name = atoms.kinds + "\u{0}" + atoms.faces
        let store = Thread.current.threadDictionary
        var held = store[key] as? [(String, Held)] ?? []
        if let found = held.first(where: { $0.0 == name }) { return found.1.value }
        var kinds = atoms.kinds, faces = atoms.faces
        let value = kinds.withUTF8 { k in faces.withUTF8 { f in exact_markup_atoms(k.baseAddress, k.count, f.baseAddress, f.count) } }
        held.append((name, Held(value)))
        if held.count > 8 { held.removeFirst() }
        store[key] = held
        return value
    }
}

/// The paint each kind declares (`atomStyles`), and a face's own over it,
/// as RGBA channels for one appearance.
enum AtomPaint {
    /// A face's `color` or `backgroundColor` in the colour wire form the
    /// archive wrote (`face_paint`), as a bound text colour's channels are
    /// (`textChannels`): a colour in its own space keeps it, so
    /// `TextEngine.color` draws it unclipped (LLP 1100 D2); nil where the
    /// kind's paint stands.
    static func face(_ value: Any?, _ atoms: MarkdownAtoms) -> [Double]? {
        batch(value)?.textChannels(dark: atoms.dark, contrast: atoms.contrast)
    }

    /// `(ink, background)` by kind name.
    static func resolve(_ atoms: MarkdownAtoms) -> [String: (ink: [Double]?, background: [Double]?)] {
        guard !atoms.styles.isEmpty, atoms.styles != "{}" else { return [:] }
        // The decorations resolver reads the same rows (`[style id, literal]`).
        let request = ["record": "{\"value\":\"\",\"spans\":[]}", "current": "", "styles": atoms.styles]
        guard let data = try? JSONSerialization.data(withJSONObject: request) else { return [:] }
        var count = 0
        let reply = data.withUnsafeBytes { raw in exact_field_decorations(raw.bindMemory(to: UInt8.self).baseAddress, raw.count, &count) }
        guard let reply, let object = try? JSONSerialization.jsonObject(with: Data(bytes: reply, count: count)) as? [String: Any],
              let styles = object["styles"] as? [String: [String: Any]] else { return [:] }
        return styles.mapValues { rows in
            (batch(rows["text_color"])?.channels(dark: atoms.dark, contrast: atoms.contrast),
             batch(rows["background_color"])?.channels(dark: atoms.dark, contrast: atoms.contrast))
        }
    }

    private static func batch(_ value: Any?) -> BatchValue? {
        switch value {
        case let value as String: return .string(value)
        case let value as NSNumber: return .number(value.doubleValue)
        case let value as [Any]: return .array(value.compactMap(batch))
        case let value as [String: Any]: return .object(value.compactMapValues(batch))
        default: return nil
        }
    }

    /// The platform token's fill: NSTokenField's accent tint on macOS,
    /// UISearchTextField's tertiary fill on iOS; a stronger one with
    /// Increase Contrast. RGBA channels, 0 to 255.
    static func token(dark: Bool, contrast: Bool) -> [Double] {
        #if os(macOS)
        let accent = NSColor.controlAccentColor.usingColorSpace(.sRGB) ?? .systemBlue
        let a = (dark ? 0.32 : 0.2) * (contrast ? 1.6 : 1)
        return [accent.redComponent * 255, accent.greenComponent * 255, accent.blueComponent * 255, a * 255]
        #else
        return [118, 118, 128, (dark ? 0.24 : 0.12) * (contrast ? 1.8 : 1) * 255]
        #endif
    }
}

/// The pill an editor atom draws: its picture, the attachment's only
/// drawing, so nothing outlives a restyle (§5.1).
final class AtomAttachment: NSTextAttachment {
    let atom: MarkupAtom
    init(_ atom: MarkupAtom) {
        self.atom = atom
        super.init(data: nil, ofType: nil)
        allowsTextAttachmentView = false
    }
    required init?(coder: NSCoder) { fatalError("not coded") }
    /// No view: TextKit draws the picture (§5.1).
    #if os(macOS)
    override func viewProvider(for parentView: NSView?, location: NSTextLocation, textContainer: NSTextContainer?) -> NSTextAttachmentViewProvider? { nil }
    #else
    override func viewProvider(for parentView: UIView?, location: NSTextLocation, textContainer: NSTextContainer?) -> NSTextAttachmentViewProvider? { nil }
    #endif

    /// Horizontal padding, of the label's font size, as the web's pill has.
    static let padding: CGFloat = 0.3

    private static func font(_ attributes: [NSAttributedString.Key: Any]) -> PlatformFont {
        attributes[.font] as? PlatformFont ?? .systemFont(ofSize: 16)
    }
    private func icon(_ size: CGFloat) -> AtomImage? {
        guard atom.icon.hasPrefix("symbol:sf/") else { return nil }
        let name = String(atom.icon.dropFirst("symbol:sf/".count))
        #if os(macOS)
        return NSImage(systemSymbolName: name, accessibilityDescription: nil)?.withSymbolConfiguration(.init(pointSize: size, weight: .regular))
        #else
        return UIImage(systemName: name, withConfiguration: UIImage.SymbolConfiguration(pointSize: size))
        #endif
    }

    /// The label's width and padding, an icon's, no wider than the line
    /// (D4), and a point inside the font's ascender and descender, so the
    /// line keeps the height it has without the atom (§5.1).
    override func attachmentBounds(for attributes: [NSAttributedString.Key: Any], location: NSTextLocation, textContainer: NSTextContainer?,
                                   proposedLineFragment: CGRect, position: CGPoint) -> CGRect {
        let face = Self.font(attributes)
        let label = ceil((atom.label as NSString).size(withAttributes: [.font: face]).width)
        let cap = face.capHeight, room = icon(cap) == nil ? 0 : cap + face.pointSize * 0.25
        let wide = label + room + 2 * Self.padding * face.pointSize
        return CGRect(x: 0, y: face.descender + 1, width: max(1, min(wide, proposedLineFragment.width)), height: face.ascender - face.descender - 2)
    }

    override func image(for bounds: CGRect, attributes: [NSAttributedString.Key: Any], location: NSTextLocation, textContainer: NSTextContainer?) -> AtomImage? {
        let face = Self.font(attributes), atom = atom
        let fill = TextEngine.color(atom.background ?? AtomPaint.token(dark: atom.dark, contrast: atom.contrast))
        #if os(macOS)
        // As a token field's selected token: emphasized only in the key window.
        let selected = atom.emphasized ? NSColor.selectedContentBackgroundColor : .unemphasizedSelectedContentBackgroundColor
        let selectedInk = NSColor.alternateSelectedControlTextColor
        let plainInk = atom.ink.map(TextEngine.color) ?? attributes[.foregroundColor] as? NSColor ?? .labelColor
        #else
        let selected = UIColor.tintColor, selectedInk = UIColor.white
        let plainInk = atom.ink.map(TextEngine.color) ?? attributes[.foregroundColor] as? UIColor ?? .label
        #endif
        let ink = atom.selected && atom.emphasized ? selectedInk : plainInk
        let symbol = icon(face.capHeight), rtl = atom.rightToLeft(in: attributes)
        let draw = { (rect: CGRect) in
            let radius = min(rect.height / 2, face.pointSize * 0.3)
            (atom.selected ? selected : fill).setFill()
            #if os(macOS)
            NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius).fill()
            #else
            UIBezierPath(roundedRect: rect, cornerRadius: radius).fill()
            #endif
            // The icon first, then the label, from the pill's leading edge.
            var lead = rect.minX + Self.padding * face.pointSize, trail = rect.maxX - Self.padding * face.pointSize
            let line = face.ascender - face.descender, top = rect.minY + (rect.height - line) / 2
            if let symbol {
                let side = face.capHeight, baseline = top + face.ascender
                let frame = CGRect(x: rtl ? trail - side : lead, y: baseline - side, width: side, height: side)
                #if os(macOS)
                symbol.isTemplate = true
                let tinted = NSImage(size: frame.size, flipped: false) { r in symbol.draw(in: r); ink.set(); r.fill(using: .sourceAtop); return true }
                tinted.draw(in: frame)
                #else
                symbol.withTintColor(ink, renderingMode: .alwaysOriginal).draw(in: frame)
                #endif
                if rtl { trail -= side + face.pointSize * 0.25 } else { lead += side + face.pointSize * 0.25 }
            }
            let style = NSMutableParagraphStyle()
            style.lineBreakMode = .byTruncatingTail
            style.baseWritingDirection = rtl ? .rightToLeft : .leftToRight
            let label = CGRect(x: lead, y: top, width: max(0, trail - lead), height: line)
            (atom.label as NSString).draw(with: label, options: [.usesLineFragmentOrigin, .truncatesLastVisibleLine],
                                          attributes: [.font: face, .foregroundColor: ink, .paragraphStyle: style], context: nil)
        }
        #if os(macOS)
        return NSImage(size: bounds.size, flipped: true) { rect in draw(rect); return true }
        #else
        // TextKit asks for the picture with the text view's traits current.
        let format = UIGraphicsImageRendererFormat(for: UITraitCollection.current)
        format.opaque = false
        return UIGraphicsImageRenderer(size: bounds.size, format: format).image { _ in draw(CGRect(origin: .zero, size: bounds.size)) }
        #endif
    }
}

extension MarkupAtom {
    /// Whether its pill lays out right to left (D4): its face's direction,
    /// else its line's, the base direction of the paragraph it is drawn in.
    func rightToLeft(in attributes: [NSAttributedString.Key: Any]) -> Bool {
        switch direction {
        case "ltr": return false
        case "rtl": return true
        default: return (attributes[.paragraphStyle] as? NSParagraphStyle)?.baseWritingDirection == .rightToLeft
        }
    }

    /// The atom of a styling row `[start, end, kind, label, icon, title, role, href, ink, background,
    /// direction, ariaLabel]`, its face's paint over its kind's.
    static func of(_ row: [Any], paint: [String: (ink: [Double]?, background: [Double]?)], selection: NSRange, atoms: MarkdownAtoms) -> (NSRange, MarkupAtom)? {
        guard row.count >= 8, let start = row[0] as? Int, let end = row[1] as? Int, end > start else { return nil }
        let text = { (i: Int) in row[i] as? String ?? "" }
        let face = { (i: Int) in i < row.count ? AtomPaint.face(row[i], atoms) : nil }
        let range = NSRange(location: start, length: end - start), kind = text(2)
        let selected = selection.length > 0 && selection.location <= start && end <= NSMaxRange(selection)
        return (range, MarkupAtom(kind: kind, label: text(3), icon: text(4), title: text(5), role: text(6), href: text(7),
                                  ink: face(8) ?? paint[kind]?.ink, background: face(9) ?? paint[kind]?.background, selected: selected,
                                  emphasized: atoms.emphasized, dark: atoms.dark, contrast: atoms.contrast,
                                  direction: row.count > 10 ? text(10) : "", ariaLabel: row.count > 11 ? text(11) : ""))
    }
}

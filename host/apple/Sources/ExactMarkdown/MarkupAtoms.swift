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

    /// Each kind's paint rows (`atomStyles`), its own under its name and
    /// each state's under `<kind>:hover` and `<kind>:checked`.
    static func resolve(_ atoms: MarkdownAtoms) -> [String: AtomRows] {
        guard !atoms.styles.isEmpty, atoms.styles != "{}" else { return [:] }
        // The decorations resolver reads the same rows (`[style id, literal]`).
        let request = ["record": "{\"value\":\"\",\"spans\":[]}", "current": "", "styles": atoms.styles]
        guard let data = try? JSONSerialization.data(withJSONObject: request) else { return [:] }
        var count = 0
        let reply = data.withUnsafeBytes { raw in exact_field_decorations(raw.bindMemory(to: UInt8.self).baseAddress, raw.count, &count) }
        guard let reply, let object = try? JSONSerialization.jsonObject(with: Data(bytes: reply, count: count)) as? [String: Any],
              let styles = object["styles"] as? [String: [String: Any]] else { return [:] }
        let colour = { (value: Any?) in batch(value)?.channels(dark: atoms.dark, contrast: atoms.contrast) }
        let length = { (value: Any?) in (value as? NSNumber).map { CGFloat($0.doubleValue) } }
        return styles.mapValues { rows in
            AtomRows(ink: colour(rows["text_color"]), background: colour(rows["background_color"]), border: colour(rows["border_color_top"]),
                     radius: length(rows["border_radius_top_left"]), ring: length(rows["outline_width"]), ringInk: colour(rows["outline_color"]),
                     ringOffset: length(rows["outline_offset"]), underline: (rows["text_decoration_line"] as? String).map { $0.contains("underline") })
        }
    }

    /// Each kind's measured style from its table row (`atoms`): its inline
    /// padding, its icon's side and the gap after it, each `"<n>px"` or
    /// `"<n>em"`, and its border's width in px.
    static func metrics(_ atoms: MarkdownAtoms) -> [String: AtomMetrics] {
        guard ["\"padding\"", "\"border\"", "\"iconSize\"", "\"gap\""].contains(where: atoms.kinds.contains),
              let rows = try? JSONSerialization.jsonObject(with: Data(atoms.kinds.utf8)) as? [[String: Any]] else { return [:] }
        var out: [String: AtomMetrics] = [:]
        for row in rows {
            guard let name = row["name"] as? String else { continue }
            let length = { (key: String) -> (CGFloat?, Bool) in
                let text = row[key] as? String ?? ""
                return (Double(text.dropLast(2)).map { CGFloat($0) }, text.hasSuffix("em"))
            }
            let (padding, em) = length("padding"), (icon, iconEm) = length("iconSize"), (gap, gapEm) = length("gap")
            out[name] = (padding, em, CGFloat((row["border"] as? NSNumber)?.doubleValue ?? 0), icon, iconEm, gap, gapEm)
        }
        return out
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

/// What a kind's table row measures: its padding, border, icon size and gap
/// (`AtomPaint.metrics`); nil where it says nothing.
typealias AtomMetrics = (padding: CGFloat?, em: Bool, border: CGFloat, icon: CGFloat?, iconEm: Bool, gap: CGFloat?, gapEm: Bool)

/// What a kind's paint rows say for one state; nil where they say nothing.
struct AtomRows {
    var ink: [Double]?, background: [Double]?, border: [Double]?, radius: CGFloat?
    var ring: CGFloat?, ringInk: [Double]?, ringOffset: CGFloat?, underline: Bool?
    /// `state`'s rows over these, row by row, as a state's block wins.
    func with(_ state: AtomRows?) -> AtomRows {
        guard let state else { return self }
        return AtomRows(ink: state.ink ?? ink, background: state.background ?? background, border: state.border ?? border,
                        radius: state.radius ?? radius, ring: state.ring ?? ring, ringInk: state.ringInk ?? ringInk,
                        ringOffset: state.ringOffset ?? ringOffset, underline: state.underline ?? underline)
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

    /// The platform's horizontal padding, of the label's font size, as the web's pill has.
    static let padding: CGFloat = 0.3

    static func font(_ attributes: [NSAttributedString.Key: Any]) -> PlatformFont {
        attributes[.font] as? PlatformFont ?? .systemFont(ofSize: 16)
    }
    /// The padding on each inline side: its kind's, else the platform's.
    private func padding(_ face: PlatformFont) -> CGFloat {
        guard let padding = atom.padding else { return Self.padding * face.pointSize }
        return atom.paddingEm ? padding * face.pointSize : padding
    }

    /// Its icon at `size`: an SF Symbol, tinted with the ink, or a picture
    /// from any other image source, in its own colours (`AtomIcons`), which
    /// draws the pill again when it arrives.
    private func icon(_ size: CGFloat) -> (image: AtomImage, tinted: Bool)? {
        guard !atom.icon.isEmpty else { return nil }
        if atom.icon.hasPrefix("symbol:sf/") {
            let name = String(atom.icon.dropFirst("symbol:sf/".count))
            #if os(macOS)
            return NSImage(systemSymbolName: name, accessibilityDescription: nil)?.withSymbolConfiguration(.init(pointSize: size, weight: .regular)).map { ($0, true) }
            #else
            return UIImage(systemName: name, withConfiguration: UIImage.SymbolConfiguration(pointSize: size)).map { ($0, true) }
            #endif
        }
        // Symbol roles name their glyph through the image node, which a face has none of.
        guard !atom.icon.hasPrefix("symbol:") else { return nil }
        guard let picture = AtomIcons.image(atom.icon, pixels: Self.pixels(size), node: atom.node) else { return nil }
        #if os(macOS)
        return (NSImage(cgImage: picture, size: CGSize(width: picture.width, height: picture.height)), false)
        #else
        return (UIImage(cgImage: picture), false)
        #endif
    }

    /// Its icon's side: its kind's `-exact-icon-size`, else the text's cap
    /// height, as the platform's token has it.
    private func iconSide(_ face: PlatformFont) -> CGFloat {
        atom.iconSize.map { atom.iconSizeEm ? $0 * face.pointSize : $0 } ?? face.capHeight
    }
    /// The space between its icon and its label: its kind's `gap`, else a
    /// quarter of the font's size.
    private func iconGap(_ face: PlatformFont) -> CGFloat {
        atom.gap.map { atom.gapEm ? $0 * face.pointSize : $0 } ?? face.pointSize * 0.25
    }

    /// A picture's pixels for an icon of `size` points: three device pixels
    /// a point, sharp at every display scale.
    private static func pixels(_ size: CGFloat) -> Int { Int((size * 3).rounded(.up)) }

    /// Whether the pill keeps room for an icon: an SF Symbol there is, or a
    /// picture, before it arrives too, so its arrival moves no text; not one
    /// that could not be read.
    private func hasIcon(_ size: CGFloat) -> Bool {
        if atom.icon.hasPrefix("symbol:sf/") { return icon(size) != nil }
        return !atom.icon.isEmpty && !atom.icon.hasPrefix("symbol:") && !AtomIcons.failed(atom.icon, pixels: Self.pixels(size))
    }

    /// The label's width and padding, an icon's, a border's, no wider than
    /// the line (D4), and the font's height less a point at its ascender, so
    /// the line keeps the height it has without the atom (§5.1); a border's
    /// top and bottom are drawn inside that.
    override func attachmentBounds(for attributes: [NSAttributedString.Key: Any], location: NSTextLocation, textContainer: NSTextContainer?,
                                   proposedLineFragment: CGRect, position: CGPoint) -> CGRect {
        let face = Self.font(attributes)
        return Self.bounds(width(face), face: face, line: proposedLineFragment.width)
    }

    /// The pill's width in `face`, before the line's bound.
    func width(_ face: PlatformFont) -> CGFloat {
        let label = ceil((atom.label as NSString).size(withAttributes: [.font: face]).width)
        let side = iconSide(face), room = hasIcon(side) ? side + iconGap(face) : 0
        return label + room + 2 * (padding(face) + atom.borderWidth)
    }

    /// A pill `width` wide in `face`, no wider than a `line` wide, from the
    /// font's descender, where the label's descenders end, to a point inside
    /// its ascender.
    static func bounds(_ width: CGFloat, face: PlatformFont, line: CGFloat) -> CGRect {
        CGRect(x: 0, y: face.descender, width: max(1, min(width, line)), height: face.ascender - face.descender - 1)
    }

    override func image(for bounds: CGRect, attributes: [NSAttributedString.Key: Any], location: NSTextLocation, textContainer: NSTextContainer?) -> AtomImage? {
        let face = Self.font(attributes), atom = atom
        let fill = atom.background.map(TextEngine.color) ?? TextEngine.color(AtomPaint.token(dark: atom.dark, contrast: atom.contrast))
        #if os(macOS)
        // As a token field's selected token: emphasized only in the key window.
        let selected = atom.emphasized ? NSColor.selectedContentBackgroundColor : .unemphasizedSelectedContentBackgroundColor
        let selectedInk = NSColor.alternateSelectedControlTextColor
        let plainInk = atom.ink.map(TextEngine.color) ?? attributes[.foregroundColor] as? NSColor ?? .labelColor
        #else
        let selected = UIColor.tintColor, selectedInk = UIColor.white
        let plainInk = atom.ink.map(TextEngine.color) ?? attributes[.foregroundColor] as? UIColor ?? .label
        #endif
        // Exactly selected, its kind's `:checked` fill and ink win over the platform's selected look.
        let selectedLook = atom.selected && !(atom.exact && atom.ownsSelectedFill)
        let ink = atom.selected && atom.emphasized && !(atom.exact && atom.ownsSelectedInk) ? selectedInk : plainInk
        let side = iconSide(face), gap = iconGap(face)
        let symbol = icon(side), rtl = atom.rightToLeft(in: attributes)
        // A picture on its way keeps its room: the label is drawn where it stays.
        let pending = symbol == nil && hasIcon(side)
        let pad = padding(face), edge = atom.borderWidth
        let draw = { (rect: CGRect) in
            let radius = max(0, min(rect.height / 2, atom.radius ?? face.pointSize * 0.3))
            (selectedLook ? selected : fill).setFill()
            Self.rounded(rect, radius).fill()
            // A border inside the pill's box, its colour else the ink.
            if edge > 0 {
                let path = Self.rounded(rect.insetBy(dx: edge / 2, dy: edge / 2), max(0, radius - edge / 2))
                path.lineWidth = edge
                (atom.border.map(TextEngine.color) ?? ink).setStroke()
                path.stroke()
            }
            // The icon first, then the label, from the pill's leading edge.
            var lead = rect.minX + pad + edge, trail = rect.maxX - pad - edge
            // The label's line ends at the pill's bottom, the font's descender.
            let line = face.ascender - face.descender, top = rect.maxY - line
            if let (image, tinted) = symbol {
                // Centred on half the cap height above the baseline, so one
                // as tall as the capitals stands on the baseline.
                let baseline = top + face.ascender
                let square = CGRect(x: rtl ? trail - side : lead, y: baseline - (face.capHeight + side) / 2, width: side, height: side)
                // A picture or a symbol keeps its proportions in the square.
                let size = image.size, scale = min(side / max(size.width, 1), side / max(size.height, 1))
                let frame = CGRect(x: square.midX - size.width * scale / 2, y: square.midY - size.height * scale / 2,
                                   width: size.width * scale, height: size.height * scale)
                #if os(macOS)
                if tinted {
                    image.isTemplate = true
                    let tinted = NSImage(size: frame.size, flipped: false) { r in image.draw(in: r); ink.set(); r.fill(using: .sourceAtop); return true }
                    tinted.draw(in: frame)
                } else {
                    image.draw(in: frame, from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: nil)
                }
                #else
                (tinted ? image.withTintColor(ink, renderingMode: .alwaysOriginal) : image).draw(in: frame)
                #endif
                if rtl { trail -= side + gap } else { lead += side + gap }
            } else if pending {
                if rtl { trail -= side + gap } else { lead += side + gap }
            }
            let style = NSMutableParagraphStyle()
            style.lineBreakMode = .byTruncatingTail
            style.baseWritingDirection = rtl ? .rightToLeft : .leftToRight
            let label = CGRect(x: lead, y: top, width: max(0, trail - lead), height: line)
            var text: [NSAttributedString.Key: Any] = [.font: face, .foregroundColor: ink, .paragraphStyle: style]
            if atom.underline { text[.underlineStyle] = NSUnderlineStyle.single.rawValue }
            (atom.label as NSString).draw(with: label, options: [.usesLineFragmentOrigin, .truncatesLastVisibleLine], attributes: text, context: nil)
            // The ring, which follows the pill's corners as CSS's outline
            // does; the picture holds only the pill, so one that reaches past
            // its edge is cut there.
            if atom.ring > 0 {
                let out = atom.ringOffset + atom.ring / 2
                let path = Self.rounded(rect.insetBy(dx: -out, dy: -out), max(0, radius + out))
                path.lineWidth = atom.ring
                (atom.ringInk.map(TextEngine.color) ?? ink).setStroke()
                path.stroke()
            }
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

    #if os(macOS)
    private static func rounded(_ rect: CGRect, _ radius: CGFloat) -> NSBezierPath { NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius) }
    #else
    private static func rounded(_ rect: CGRect, _ radius: CGFloat) -> UIBezierPath { UIBezierPath(roundedRect: rect, cornerRadius: radius) }
    #endif
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
    /// direction, ariaLabel]`: its kind's paint, then its `:hover` paint when the pointer is over it
    /// and its `:checked` paint when the selection is exactly it, and its face's colours over all.
    static func of(_ row: [Any], paint: [String: AtomRows], metrics: [String: AtomMetrics] = [:],
                   selection: NSRange, atoms: MarkdownAtoms, node: NodeView? = nil) -> (NSRange, MarkupAtom)? {
        guard row.count >= 8, let start = row[0] as? Int, let end = row[1] as? Int, end > start else { return nil }
        let text = { (i: Int) in row[i] as? String ?? "" }
        let face = { (i: Int) in i < row.count ? AtomPaint.face(row[i], atoms) : nil }
        let range = NSRange(location: start, length: end - start), kind = text(2)
        let selected = selection.length > 0 && selection.location <= start && end <= NSMaxRange(selection)
        let exact = selection == range, hovered = atoms.hovered == start
        let checked = exact ? paint[kind + ":checked"] : nil
        let rows = (paint[kind] ?? AtomRows()).with(hovered ? paint[kind + ":hover"] : nil).with(checked)
        let atom = MarkupAtom(kind: kind, label: text(3), icon: text(4), title: text(5), role: text(6), href: text(7),
                              ink: face(8) ?? rows.ink, background: face(9) ?? rows.background, selected: selected,
                              emphasized: atoms.emphasized, dark: atoms.dark, contrast: atoms.contrast,
                              direction: row.count > 10 ? text(10) : "", ariaLabel: row.count > 11 ? text(11) : "")
        (atom.exact, atom.hovered) = (exact, hovered)
        if let measured = metrics[kind] {
            (atom.padding, atom.paddingEm, atom.borderWidth) = (measured.padding, measured.em, measured.border)
            (atom.iconSize, atom.iconSizeEm, atom.gap, atom.gapEm) = (measured.icon, measured.iconEm, measured.gap, measured.gapEm)
        }
        (atom.border, atom.radius, atom.underline) = (rows.border, rows.radius, rows.underline ?? false)
        (atom.ring, atom.ringInk, atom.ringOffset) = (rows.ring ?? 0, rows.ringInk, rows.ringOffset ?? 0)
        (atom.ownsSelectedFill, atom.ownsSelectedInk) = (checked?.background != nil, checked?.ink != nil)
        atom.node = node
        return (range, atom)
    }
}

// @ref LLP 1104 D3–D6: AppKit chrome around the kernel's editor content rect.
#if os(macOS)
import AppKit
import CoreText

extension NodeView {
    var isNativeTextControl: Bool {
        (field != nil || textArea != nil) && props["markup"] != "markdown" && style["appearance"]?.string != "none"
    }
    func nativeEditorRect(in proposed: NSRect) -> NSRect? {
        guard isNativeTextControl, let rect = nativeFieldContent else { return nil }
        var content = proposed.insetBy(left: rect.minX, top: rect.minY,
            right: max(0, bounds.width - rect.maxX), bottom: max(0, bounds.height - rect.maxY))
        // NSTextFieldCell top-aligns a tall title rect. Centre its one line
        // inside the published box, with the same line metrics as the kernel.
        if let font = field?.font {
            let (above, below) = CSSLineBox.extents(font as CTFont, height: usedLineHeight)
            let height = min(content.height, above + below)
            content.origin.y += (content.height - height) / 2
            content.size.height = height
        }
        return content
    }
    func layoutField() {
        guard let field else { return }
        field.frame = isNativeTextControl ? bounds : contentBox()
        field.needsDisplay = true
    }
    /// The AppKit class an input's field is: a password's secure field; a
    /// native search input's `NSSearchField` (LLP 1115 D8: its magnifier, its
    /// clear button, Escape clearing it); a plain field otherwise, including
    /// a search input the author draws (`appearance: none`).
    var wantedFieldClass: FieldClass {
        if props["type"] == "password" { return .secure }
        if props["type"] == "search", props["markup"] != "markdown", style["appearance"]?.string != "none" { return .search }
        return .plain
    }
    /// A different class on AppKit, so a changed `type` or appearance remakes
    /// the field in place.
    func remakeFieldIfNeeded() {
        guard let f = field else { return }
        let wanted = wantedFieldClass
        guard FieldClass(f) != wanted else { return }
        let n = makeField(wanted)
        n.frame = f.frame
        n.stringValue = f.stringValue
        n.font = f.font
        n.textColor = f.textColor
        let focused = window != nil && f.currentEditor() != nil
        f.removeFromSuperview()
        addSubview(n)
        field = n
        if focused { window?.makeFirstResponder(n) }
    }
    func styleNativeField() {
        remakeFieldIfNeeded()
        guard let field else { return }
        let native = isNativeTextControl
        let standard = FieldChromeCache.platformField(kind: FieldClass(field).chromeKind)
        field.isBezeled = native
        field.bezelStyle = standard.bezelStyle
        field.drawsBackground = native && standard.drawsBackground
        field.backgroundColor = native ? standard.backgroundColor : .clear
        field.focusRingType = native ? .default : .exterior
        field.alignment = fieldAlignment
        if native, field.currentEditor() == nil {
            field.attributedStringValue = NSAttributedString(string: field.stringValue, attributes: fieldTextAttributes)
        }
        if let editor = field.currentEditor() as? NSTextView { styleFieldEditor(editor) }
        applyPlaceholder(field)
        layoutField()
    }
    var fieldAlignment: NSTextAlignment {
        switch style["text_align"]?.string {
        case "center": return .center
        case "right": return .right
        case "end": return style["direction"]?.string == "rtl" ? .left : .right
        case "start": return style["direction"]?.string == "rtl" ? .right : .left
        case "justify": return .justified
        default: return .left
        }
    }
    var fieldTextAttributes: [NSAttributedString.Key: Any] {
        let paragraph = NSMutableParagraphStyle()
        paragraph.alignment = fieldAlignment
        return [.font: field?.font ?? NSFont.systemFont(ofSize: NSFont.systemFontSize(for: .regular)),
                .foregroundColor: field?.textColor ?? NSColor.controlTextColor,
                .kern: number("letter_spacing"), .paragraphStyle: paragraph]
    }
    func styleFieldEditor(_ editor: NSTextView) {
        editor.insertionPointColor = caretColor
        if isNativeTextControl, !editor.hasMarkedText() {
            let attributes = fieldTextAttributes
            editor.textStorage?.addAttributes(attributes, range: NSRange(location: 0, length: editor.string.utf16.count))
            editor.typingAttributes.merge(attributes) { _, next in next }
            editor.alignment = fieldAlignment
        }
        applyTextChecking(editor)
    }
    func applyFieldContent(_ payload: [String: Any]) {
        if let r = payload["rect"] as? [Double], r.count == 4 {
            nativeFieldContent = NSRect(x: r[0], y: r[1], width: r[2], height: r[3])
        } else { nativeFieldContent = nil }
        layoutField(); layoutTextArea()
    }
}

/// AppKit draws the scroll view's ring while its document editor owns focus.
final class TextAreaScroll: NSScrollView {
    weak var owner: NodeView?
    override var focusRingMaskBounds: NSRect {
        owner?.isNativeTextControl == true && window?.firstResponder === documentView ? bounds : .zero
    }
    override func drawFocusRingMask() {
        if !focusRingMaskBounds.isEmpty { NSBezierPath(rect: bounds).fill() }
    }
}
enum FieldClass {
    case plain, secure, search
    init(_ field: NSTextField) { self = field is NSSecureTextField ? .secure : field is NSSearchField ? .search : .plain }
    /// The kernel's `FieldKind` code, which keys `FieldChromeCache`.
    var chromeKind: UInt8 { switch self { case .plain: 0; case .secure: 1; case .search: 2 } }
}

/// A search field's cell, as `FieldCell` (Accessibility.swift): its text in
/// the kernel's content rect (measured from this same class,
/// `FieldChromeCache`), its buttons where AppKit puts them.
final class SearchFieldCell: NSSearchFieldCell {
    private lazy var clipboardEditor: FieldEditor = {
        let editor = FieldEditor(frame: .zero)
        editor.isFieldEditor = true
        return editor
    }()
    override func fieldEditor(for controlView: NSView) -> NSTextView? {
        (controlView.superview as? NodeView)?.hearsFieldClipboard() == true ? clipboardEditor : super.fieldEditor(for: controlView)
    }
    override func drawingRect(forBounds rect: NSRect) -> NSRect {
        (controlView?.superview as? NodeView)?.nativeEditorRect(in: rect) ?? super.drawingRect(forBounds: rect)
    }
    override func titleRect(forBounds rect: NSRect) -> NSRect {
        (controlView?.superview as? NodeView)?.nativeEditorRect(in: rect) ?? super.titleRect(forBounds: rect)
    }
    override func searchTextRect(forBounds rect: NSRect) -> NSRect {
        (controlView?.superview as? NodeView)?.nativeEditorRect(in: rect) ?? super.searchTextRect(forBounds: rect)
    }
    override func accessibilityAttributeNames() -> [NSAccessibility.Attribute] {
        super.accessibilityAttributeNames() + NodeView.ariaAttributes.filter { (controlView?.superview as? NodeView)?.ariaAttribute($0) != nil }.map { .init(rawValue: $0) }
    }
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        (controlView?.superview as? NodeView)?.ariaAttribute(attribute.rawValue) ?? super.accessibilityAttributeValue(attribute)
    }
}
#endif

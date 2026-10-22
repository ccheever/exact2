// @ref LLP 1104 D3–D6: AppKit chrome around the kernel's editor content rect.
#if os(macOS)
import AppKit
import CoreText

extension NodeView {
    /// A field's ground where it writes none: none on the Mac, where the
    /// bezeled NSTextField draws its own (the textarea's is its scroll
    /// view's, `TextAreaMac.swift`).
    var fieldGround: CGColor? { nil }
    func fieldOutline() -> CGPath? { nil }
    var isNativeTextControl: Bool {
        (field != nil || textArea != nil) && props["markup"] != "markdown" && style["appearance"]?.string != "none"
    }
    func nativeEditorRect(in proposed: NSRect) -> NSRect? {
        guard isNativeTextControl, let rect = kernelContentBox else { return nil }
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
        // A field the window's toolbar hosts is AppKit's to place and to style (LLP 1115.002 D2).
        guard let field, pageField == nil else { return }
        field.frame = isNativeTextControl ? bounds : contentBox()
        layoutStepper() // a number input's stepper takes the field's trailing end (LLP 1116.007 D4)
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
        guard let f = field, pageField == nil else { return }
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
        guard let field, pageField == nil else { return }
        let native = isNativeTextControl
        // @ref LLP 1115.002 D6 — a grouped form's field has no bezel, no
        // ground and no ring: the row is the field, as SwiftUI's `Form` has it.
        let bezel = native && !fieldInCell
        let standard = FieldChromeCache.platformField(kind: FieldClass(field).chromeKind)
        field.isBezeled = bezel
        field.bezelStyle = standard.bezelStyle
        field.drawsBackground = bezel && standard.drawsBackground
        field.backgroundColor = bezel ? standard.backgroundColor : .clear
        field.focusRingType = outlineNone || (native && fieldInCell) ? .none : native ? .default : .exterior
        restyleFieldFocus()
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
        let r = payload["rect"] as? [Double]
        let box = r.flatMap { $0.count == 4 ? NSRect(x: $0[0], y: $0[1], width: $0[2], height: $0[3]) : nil }
        applyKernelContentBox(box, padding: NodeView.sentPadding(payload))
        let cell = box != nil && payload["cell"] as? Bool == true
        if cell != fieldInCell { fieldInCell = cell; styleNativeField() }
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
    /// The input's node is its field's delegate, in the page and in the
    /// window's toolbar alike (LLP 1115.002 D2): there too it edits in the
    /// session's editor, which tells the node of a composition and of a
    /// copy, cut or paste. The rectangles below are the page's: a field
    /// the toolbar hosts has no node above it, and its geometry is AppKit's.
    override func fieldEditor(for controlView: NSView) -> NSTextView? {
        ((controlView as? NSTextField)?.delegate as? NodeView)?.presenter?.fieldEditor ?? super.fieldEditor(for: controlView)
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
    private var node: NodeView? { (controlView as? NSTextField)?.delegate as? NodeView }
    override func accessibilityAttributeNames() -> [NSAccessibility.Attribute] {
        super.accessibilityAttributeNames() + NodeView.ariaAttributes.filter { node?.ariaAttribute($0) != nil }.map { .init(rawValue: $0) }
            + ((controlView as? NSTextField)?.comboOwner != nil ? [.expanded] : [])
    }
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        if let value = (controlView as? NSTextField)?.comboboxAttribute(attribute) { return value }
        return node?.ariaAttribute(attribute.rawValue) ?? super.accessibilityAttributeValue(attribute)
    }
}

// @ref LLP 1115.002 D2 — a header's search input in the window's toolbar.
// The toolbar's search item holds a field (`NSSearchToolbarItem.searchField`,
// which SwiftUI gives a subclass of its own too), and that field is this
// node's: one of the node's own class with the node its delegate, so its
// value, its editing, its focus and every event are the input's as in the
// page. It is made here and never styled by the page, so the item draws
// and places it as its own. The page's field waits in the node meanwhile.
extension NodeView {
    /// Whether this input has a field the window's toolbar can host: a
    /// native search field. One the author draws (`appearance: none`) is
    /// theirs to draw. What decides that a header holding it is the bar's
    /// is exactly what `lendFieldToBar` then answers.
    var lendsFieldToBar: Bool { wantedFieldClass == .search && (pageField != nil ? field is NSSearchField : field is SearchField) }
    /// This search input's field for the window's toolbar, the node's own
    /// while it is there.
    func lendFieldToBar() -> NSSearchField? {
        guard lendsFieldToBar else { return nil }
        if pageField != nil { return field as? NSSearchField }
        guard let page = field as? SearchField else { return nil }
        let bar = SearchField(frame: .zero)
        (bar as NSTextField).delegate = self
        bar.formatter = TextInputFormatter(self)
        bar.stringValue = page.stringValue
        bar.isEnabled = page.isEnabled
        bar.isEditable = page.isEditable
        pageField = page
        field = bar
        return bar
    }
    /// A prompt's field for its alert's accessory view (AlertsMac.swift), lent
    /// as the toolbar's search field is: of the input's own kind, the node its
    /// delegate and its editor the session's, AppKit's to draw and place.
    func lendFieldToAlert() -> NSTextField? {
        guard pageField == nil, let page = field else { return nil }
        let lent: NSTextField = page is NSSecureTextField ? SecureField(frame: .zero) : Field(frame: .zero)
        lent.delegate = self
        lent.formatter = TextInputFormatter(self)
        lent.stringValue = page.stringValue
        lent.isEnabled = page.isEnabled
        lent.isEditable = page.isEditable
        lent.contentType = page.contentType // `autocomplete` (LLP 1102 §3.6)
        lent.cell?.isScrollable = true
        lent.cell?.wraps = false
        lent.cell?.usesSingleLineMode = true
        pageField = page
        field = lent
        syncBarField()
        return lent
    }
    /// The page has its field again, holding what the toolbar's or the
    /// alert's held, in the page's look, which was not written meanwhile.
    func reclaimLentField() {
        guard let page = pageField else { return }
        if let bar = field, page.stringValue != bar.stringValue { page.stringValue = bar.stringValue }
        (field, pageField) = (page, nil)
        applyStyle(style)
        applyProps(set: [:], clear: [])
    }
    /// What the page says of the field the toolbar hosts: its placeholder
    /// and its names. Everything else of it is AppKit's.
    func syncBarField() {
        guard pageField != nil, let bar = field else { return }
        if let words = props["placeholder"], !words.isEmpty, bar.placeholderString != words { bar.placeholderString = words }
        if bar.accessibilityIdentifier() != props["testId"] ?? "" { bar.setAccessibilityIdentifier(props["testId"]) }
        if bar.accessibilityLabel() != authoredLabel { bar.setAccessibilityLabel(authoredLabel) }
    }
}
#endif

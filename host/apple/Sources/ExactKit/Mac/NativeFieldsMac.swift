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
    func styleNativeField() {
        guard let field else { return }
        let native = isNativeTextControl
        let standard = FieldChromeCache.platformField(kind: field is NSSecureTextField ? 1 : 0)
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
#endif

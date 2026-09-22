// Contract textarea: native multiline editing, with the same change seam as input.
#if os(macOS)
import AppKit

final class TextArea: NSTextView {
    var placeholder = "" { didSet { needsDisplay = true } }
    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        if string.isEmpty, !placeholder.isEmpty {
            (placeholder as NSString).draw(in: bounds, withAttributes: [
                .font: font ?? NSFont.systemFont(ofSize: 16),
                .foregroundColor: (textColor ?? .black).withAlphaComponent(0.3),
            ])
        }
    }
}

extension NodeView {
    var caretColor: NSColor { color("caret_color", color("text_color", .textColor)) }

    var allowsInputCorrection: Bool {
        if textArea == nil, ["email", "url", "password"].contains((props["type"] ?? "").lowercased()) { return false }
        return (props["autocorrect"] ?? "").lowercased() != "off"
    }

    var allowsInputSpellChecking: Bool { props["spellcheck"] != "false" }

    func makeTextArea() {
        let f = TextArea(frame: .zero)
        f.isRichText = false
        f.importsGraphics = false
        f.drawsBackground = false
        f.textContainerInset = .zero
        f.textContainer?.lineFragmentPadding = 0
        f.isVerticallyResizable = true
        f.isHorizontallyResizable = false
        f.autoresizingMask = [.width]
        f.textContainer?.widthTracksTextView = true
        f.delegate = self
        let scroller = NSScrollView(frame: .zero)
        scroller.drawsBackground = false
        scroller.hasVerticalScroller = true
        scroller.documentView = f
        addSubview(scroller)
        textArea = f
        textAreaScroll = scroller
    }
    /// The app's value into the editor: nothing while text is being composed
    /// (held until the composition ends), else the changed middle only, with
    /// the selection carried through (LLP 1045 D5).
    func writeValue(_ value: String, into f: NSTextView) {
        if f.hasMarkedText() { pendingValue = value; return }
        pendingValue = nil
        guard let edit = minimalTextEdit(from: f.string, to: value) else { return }
        let selection = f.selectedRange()
        if let storage = f.textStorage, storage.length > 0, edit.range.length < storage.length {
            storage.replaceCharacters(in: edit.range, with: NSAttributedString(string: edit.text, attributes: f.typingAttributes))
        } else {
            f.string = value
        }
        f.setSelectedRange(carrySelection(selection, through: edit))
    }
    func applyTextArea() {
        guard let f = textArea else { return }
        writeValue(props["value"] ?? "", into: f)
        f.isEditable = !disabled && props["editable"] != "false"
        f.isSelectable = !disabled
        f.isAutomaticSpellingCorrectionEnabled = allowsInputCorrection
        f.isContinuousSpellCheckingEnabled = allowsInputSpellChecking
        f.setAccessibilityLabel(props["accessibilityLabel"])
        f.setAccessibilityIdentifier(props["testId"])
        (f as? TextArea)?.placeholder = props["placeholder"] ?? ""
    }
    func styleTextArea() {
        guard let f = textArea, let t = text else { return }
        f.font = t.font(size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"] as? String) == "italic")
        f.textColor = color("text_color", .black)
        f.insertionPointColor = caretColor
        let paragraph = NSMutableParagraphStyle()
        if let height = usedLineHeight {
            paragraph.minimumLineHeight = height
            paragraph.maximumLineHeight = height
            if height == 0 { paragraph.lineHeightMultiple = .leastNormalMagnitude }
        }
        f.defaultParagraphStyle = paragraph
        let attributes: [NSAttributedString.Key: Any] = [.paragraphStyle: paragraph]
        f.textStorage?.addAttributes(attributes, range: NSRange(location: 0, length: (f.string as NSString).length))
        f.typingAttributes.merge(attributes) { _, authored in authored }
        layoutTextArea()
    }
    func layoutTextArea() {
        guard let f = textArea, let scroller = textAreaScroll else { return }
        scroller.frame = contentBox()
        f.minSize = NSSize(width: 0, height: scroller.contentSize.height)
        f.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        f.setFrameSize(NSSize(width: scroller.contentSize.width, height: max(f.frame.height, scroller.contentSize.height)))
    }
    func textDidChange(_ notification: Notification) {
        textArea?.needsDisplay = true
        if !disabled, handlers.contains("change") { presenter?.change(id, textArea?.string ?? "") }
        if let f = textArea, !f.hasMarkedText(), let held = pendingValue { writeValue(held, into: f) }
    }
    func textDidBeginEditing(_ notification: Notification) { presenter?.collections.pinsChanged(); if handlers.contains("focus") { presenter?.focus(id) } }
    func textDidEndEditing(_ notification: Notification) { presenter?.collections.pinsChanged(); if handlers.contains("blur") { presenter?.blur(id) } }
}
#endif

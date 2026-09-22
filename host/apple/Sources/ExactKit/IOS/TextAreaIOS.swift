// Contract textarea: UITextView keeps Enter as a newline, never a submission.
#if os(iOS)
import UIKit

// The software keyboard calls UIKeyInput directly, including deletion in an
// empty field. Its editing command still belongs to the authored key handler.
final class TextField: UITextField {
    weak var owner: NodeView?
    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let remaining=owner?.pressedControls(presses,down:true) ?? presses
        if !remaining.isEmpty {super.pressesBegan(remaining,with:event)}
    }
    override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let remaining=owner?.pressedControls(presses,down:false) ?? presses
        if !remaining.isEmpty {super.pressesEnded(remaining,with:event)}
    }
    override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let remaining=owner?.pressedControls(presses,down:false) ?? presses
        if !remaining.isEmpty {super.pressesCancelled(remaining,with:event)}
    }
    override var textInputMode: UITextInputMode? {
        if owner?.props["emojiPicker"] == "true",
           let emoji = UITextInputMode.activeInputModes.first(where: { $0.primaryLanguage == "emoji" }) {
            return emoji
        }
        return super.textInputMode
    }
    override func deleteBackward() {
        if let owner, !owner.disabled {
            var node: UIView? = owner
            while let current = node, !((current as? NodeView)?.handlers.contains("key") ?? false) { node = current.superview }
            if let target = node as? NodeView { target.presenter?.key(target.id, "Backspace") }
        }
        super.deleteBackward()
    }
}

final class TextArea: UITextView {
    weak var owner: NodeView?
    /// The Markdown styler when `markup="markdown"` (LLP 1045 D5).
    var markup: MarkupEditor?
    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let remaining=owner?.pressedControls(presses,down:true) ?? presses
        if !remaining.isEmpty {super.pressesBegan(remaining,with:event)}
    }
    override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let remaining=owner?.pressedControls(presses,down:false) ?? presses
        if !remaining.isEmpty {super.pressesEnded(remaining,with:event)}
    }
    override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let remaining=owner?.pressedControls(presses,down:false) ?? presses
        if !remaining.isEmpty {super.pressesCancelled(remaining,with:event)}
    }
    override func resignFirstResponder() -> Bool {
        let wasFirst = isFirstResponder
        let resigned = super.resignFirstResponder()
        // UITextView's editing delegate omits read-only selection sessions.
        // They still blur in HTML, and the app must be able to remove its
        // transient selection surface after focus moves elsewhere.
        if resigned, wasFirst, !isEditable, let owner, owner.handlers.contains("blur") {
            owner.presenter?.blur(owner.id)
        }
        return resigned
    }
    // Keep TextKit's line pitch equal to the authored CSS line box. Updating
    // storage attributes preserves the value and selected range; replacing
    // attributedText would reset a selection (including a read-only one).
    func applyLineHeight(_ height: CGFloat?) {
        let paragraph = NSMutableParagraphStyle()
        if let height {
            paragraph.minimumLineHeight = height
            paragraph.maximumLineHeight = height
            // TextKit's zero min/max mean unconstrained; its smallest positive
            // multiple produces the explicit zero box at driver precision.
            if height == 0 { paragraph.lineHeightMultiple = .leastNormalMagnitude }
        }
        let attributes: [NSAttributedString.Key: Any] = [.paragraphStyle: paragraph]
        textStorage.addAttributes(attributes, range: NSRange(location: 0, length: textStorage.length))
        typingAttributes.merge(attributes) { _, authored in authored }
    }
    var placeholder = "" { didSet { setNeedsDisplay() } }
    override func draw(_ rect: CGRect) {
        super.draw(rect)
        if text.isEmpty, !placeholder.isEmpty {
            (placeholder as NSString).draw(in: bounds, withAttributes: [
                .font: font ?? UIFont.systemFont(ofSize: 16),
                .foregroundColor: UIColor.placeholderText,
            ])
        }
    }
}

extension NodeView {
    // CSS auto leaves UIKit's editor tint alone. An explicit caret colour
    // also colours UIKit's selection handles and highlight.
    var caretColor: UIColor? { channels("caret_color").map { TextEngine.color($0) } }

    // HTML's hints apply to both editors. Email/URL/password input states
    // override author hints; textarea has no input type state or form owner.
    var inputCapitalization: UITextAutocapitalizationType {
        if textArea == nil, ["email", "url", "password"].contains((props["type"] ?? "").lowercased()) { return .none }
        switch (props["autocapitalize"] ?? "").lowercased() {
        case "off", "none": return .none
        case "words": return .words
        case "characters": return .allCharacters
        default: return .sentences
        }
    }
    var inputCorrection: UITextAutocorrectionType {
        if textArea == nil, ["email", "url", "password"].contains((props["type"] ?? "").lowercased()) { return .no }
        // On permits correction; UIKit's default also respects user preferences.
        return (props["autocorrect"] ?? "").lowercased() == "off" ? .no : .default
    }

    var inputSpellChecking: UITextSpellCheckingType {
        switch props["spellcheck"] {
        case "false": return .no
        case "true": return .yes
        default: return .default
        }
    }

    func makeTextArea() {
        // A Markdown editor stays on TextKit 2 (LLP 1045 D5): markers hide
        // by attributes and marker glyphs draw from a layout fragment, so
        // `layoutManager` is never touched.
        let f = TextArea(frame: .zero)
        f.owner = self
        f.backgroundColor = .clear
        f.textContainerInset = .zero
        f.textContainer.lineFragmentPadding = 0
        f.delegate = self
        addSubview(f)
        textArea = f
    }
    func applyTextArea() {
        guard let f = textArea else { return }
        writeValue(props["value"] ?? "", into: f)
        f.isEditable = !disabled && props["editable"] != "false"
        f.isSelectable = !disabled
        let traitsChanged = f.autocapitalizationType != inputCapitalization || f.autocorrectionType != inputCorrection || f.spellCheckingType != inputSpellChecking
        f.autocapitalizationType = inputCapitalization
        f.autocorrectionType = inputCorrection
        f.spellCheckingType = inputSpellChecking
        if traitsChanged, f.isFirstResponder { f.reloadInputViews() }
        f.accessibilityLabel = props["accessibilityLabel"]
        f.accessibilityIdentifier = props["testId"]
        (f as? TextArea)?.placeholder = props["placeholder"] ?? ""
    }
    func styleTextArea() {
        guard let f = textArea, let t = text else { return }
        f.font = t.font(size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"]?.string) == "italic")
        f.textColor = color("text_color", .black)
        f.tintColor = caretColor
        (f as? TextArea)?.applyLineHeight(usedLineHeight)
        restyleMarkup()
        f.setNeedsDisplay()
        layoutTextArea()
    }
    func layoutTextArea() { textArea?.frame = contentBox() }
    /// Restyle a Markdown editor's storage for its text and selection.
    func restyleMarkup() {
        guard let f = textArea as? TextArea, let t = text else { return }
        // The props arrive after the view; the styler attaches on the first
        // apply that names Markdown and stays for the view's life.
        if f.markup == nil, props["markup"] == "markdown" { f.markup = MarkupEditor() }
        guard let editor = f.markup else { return }
        let look = MarkupEditor.Look(
            font: { size, weight, family, italic in t.font(size: size, weight: weight, family: family, italic: italic) },
            size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")),
            italic: (style["font_style"]?.string) == "italic", lineHeight: usedLineHeight, ink: color("text_color", .black))
        editor.restyle(f, look: look)
    }
    /// The app's value into the editor: nothing while text is being composed
    /// (held until the composition ends), else the changed middle only, with
    /// the selection carried through (LLP 1045 D5).
    func writeValue(_ value: String, into f: UITextView) {
        if f.markedTextRange != nil { pendingValue = value; return }
        pendingValue = nil
        guard let edit = minimalTextEdit(from: f.text ?? "", to: value) else { return }
        let selection = f.selectedRange
        if f.textStorage.length == 0 || edit.range.length == f.textStorage.length {
            f.text = value
        } else {
            f.textStorage.replaceCharacters(in: edit.range, with: NSAttributedString(string: edit.text, attributes: f.typingAttributes))
        }
        f.selectedRange = carrySelection(selection, through: edit)
        restyleMarkup()
    }
    func textViewDidChange(_ textView: UITextView) {
        textView.setNeedsDisplay()
        if !disabled, handlers.contains("change") { presenter?.change(id, textView.text ?? "") }
        if textView.markedTextRange == nil, let held = pendingValue { writeValue(held, into: textView) }
        restyleMarkup()
    }
    func textViewDidChangeSelection(_ textView: UITextView) {
        // The caret reveals the markers of what it touches (LLP 1045 D1).
        if (textView as? TextArea)?.markup != nil { restyleMarkup() }
    }
    func textViewDidBeginEditing(_ textView: UITextView) {
        presenter?.collections.pinsChanged()
        presenter?.editing = self
        if handlers.contains("focus") { presenter?.focus(id) }
        presenter?.reveal(self)
    }
    func textViewDidEndEditing(_ textView: UITextView) { presenter?.collections.pinsChanged();
        if presenter?.editing === self { presenter?.editing = nil }
        if handlers.contains("blur") { presenter?.blur(id) }
    }
}
#endif

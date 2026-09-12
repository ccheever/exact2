// Contract textarea: UITextView keeps Enter as a newline, never a submission.
#if os(iOS)
import UIKit

// The software keyboard calls UIKeyInput directly, including deletion in an
// empty field. Its editing command still belongs to the authored key handler.
final class TextField: UITextField {
    weak var owner: NodeView?
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
        let value = props["value"] ?? ""
        if f.text != value { f.text = value }
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
        f.font = t.font(size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"] as? String) == "italic")
        f.textColor = color("text_color", .black)
        f.tintColor = caretColor
        (f as? TextArea)?.applyLineHeight(usedLineHeight)
        f.setNeedsDisplay()
        layoutTextArea()
    }
    func layoutTextArea() { textArea?.frame = contentBox() }
    func textViewDidChange(_ textView: UITextView) {
        textView.setNeedsDisplay()
        if !disabled, handlers.contains("change") { presenter?.change(id, textView.text ?? "") }
    }
    func textViewDidBeginEditing(_ textView: UITextView) {
        presenter?.editing = self
        if handlers.contains("focus") { presenter?.focus(id) }
        presenter?.reveal(self)
    }
    func textViewDidEndEditing(_ textView: UITextView) {
        if presenter?.editing === self { presenter?.editing = nil }
        if handlers.contains("blur") { presenter?.blur(id) }
    }
}
#endif

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
    // UIKit already owns a specialized manager per editor. Its native text
    // undo actions require that manager; observe it without replacing it.
    private var textUndo: NativeTextUndo?
    override var undoManager: UndoManager? {
        guard let manager = super.undoManager else { return nil }
        if textUndo?.manager !== manager {
            textUndo = NativeTextUndo(manager: manager, before: { [weak self] in
                self?.markup?.applying = true
            }, after: { [weak self] in
                guard let self, let editor = markup else { return }
                editor.applying = false
                editor.bookmark = selectedRange
                owner?.textViewDidChange(self)
            })
        }
        return manager
    }

    weak var owner: NodeView?
    /// The Markdown styler when `markup="markdown"` (LLP 1045 D5).
    var markup: MarkupEditor?

    override func insertText(_ text: String) {
        // UIKeyInput (including the software keyboard) need not ask the
        // text-view delegate before insertion. Handle a plain Return here;
        // the command's own native insertion bypasses through `applying`.
        if text == "\n", let editor = markup, !editor.applying,
           markedTextRange == nil, owner?.formatMarkup("newline", selection: selectedRange) == true { return }
        super.insertText(text)
    }

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
        if wasFirst, markedTextRange == nil { markup?.bookmark = selectedRange }
        let resigned = super.resignFirstResponder()
        // UITextView's editing delegate omits read-only selection sessions.
        // They still blur in HTML, and the app must be able to remove its
        // transient selection surface after focus moves elsewhere.
        if resigned, wasFirst, !isEditable, let owner, owner.handlers.contains("blur") {
            owner.presenter?.blur(owner.id)
        }
        return resigned
    }
    override var keyCommands: [UIKeyCommand]? {
        guard markup != nil, isEditable else { return super.keyCommands }
        return (super.keyCommands ?? []) + [
            UIKeyCommand(title: "Bold", action: #selector(markupBold), input: "b", modifierFlags: .command),
            UIKeyCommand(title: "Italic", action: #selector(markupItalic), input: "i", modifierFlags: .command),
            UIKeyCommand(title: "Link", action: #selector(markupLink), input: "k", modifierFlags: .command),
            UIKeyCommand(title: "Copy Plain Text", action: #selector(copyPlainText), input: "c", modifierFlags: [.command, .shift]),
        ]
    }
    @objc private func markupBold() { owner?.formatMarkup("bold") }
    @objc private func markupItalic() { owner?.formatMarkup("italic") }
    @objc private func markupLink() { owner?.editMarkupLink() }
    @objc func copyPlainText() {
        guard markup != nil, selectedRange.length > 0, let plain = MarkupCommands.plain((text as NSString).substring(with: selectedRange)) else { return }
        UIPasteboard.general.string = plain
    }
    override func copy(_ sender: Any?) {
        guard markup != nil else { super.copy(sender); return }
        if selectedRange.length > 0 { UIPasteboard.general.string = (text as NSString).substring(with: selectedRange) }
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
    /// A single-line field is the element VoiceOver reaches: it carries the
    /// node's name and identifier (found by `tree --ax`, LLP 1080.002).
    func applyFieldName(_ f: UITextField) {
        f.accessibilityLabel = props["accessibilityLabel"]
        f.accessibilityIdentifier = props["testId"]
    }
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
        // UIKit defaults to TextKit 2 on our iOS 17 floor. Never access
        // `layoutManager`: that would irreversibly switch back to TextKit 1.
        let f = TextArea(frame: .zero)
        f.owner = self
        f.backgroundColor = .clear
        f.textContainerInset = .zero
        f.textContainer.lineFragmentPadding = 0
        f.delegate = self
        addSubview(f)
        textArea = f
    }
    func configureMarkup() {
        guard let f = textArea as? TextArea else { return }
        if props["markup"] == "markdown" {
            if f.markup == nil { f.markup = MarkupEditor() }
        } else if let editor = f.markup, f.markedTextRange == nil {
            let storage = f.textStorage
            editor.detach(storage)
            f.typingAttributes = editor.plainAttributes
            f.markup = nil
        }
    }
    func applyTextArea() {
        guard let f = textArea else { return }
        configureMarkup()
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
        guard f.markedTextRange == nil else { layoutTextArea(); return }
        f.font = t.font(size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"]?.string) == "italic", numeric: Int(number("font_variant_numeric")))
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
        guard props["markup"] == "markdown", let f = textArea as? TextArea, let t = text, let editor = f.markup, f.markedTextRange == nil else { return }
        let look = MarkupEditor.Look(
            font: { size, weight, family, italic in t.font(size: size, weight: weight, family: family, italic: italic) },
            size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")),
            italic: (style["font_style"]?.string) == "italic", lineHeight: usedLineHeight, ink: color("text_color", .black))
        editor.restyle(f.textStorage, selection: f.selectedRange, look: look)
        f.typingAttributes = editor.baseAttributes(look)
    }
    /// The app's value into the editor: nothing while text is being composed
    /// (held until the composition ends), else the changed middle only, with
    /// the selection carried through (LLP 1045 D5).
    func writeValue(_ value: String, into f: UITextView) {
        if (f.text ?? "").utf16.elementsEqual(value.utf16) { pendingValue = nil; return }
        if f.markedTextRange != nil { pendingValue = value; return }
        pendingValue = nil
        guard let edit = minimalTextEdit(from: f.text ?? "", to: value) else { return }
        // Native undo ranges refer to the old buffer. An external edit has
        // no position map for those ranges; discard only this view's history.
        f.undoManager?.removeAllActions()
        let selection = f.selectedRange
        if f.textStorage.length == 0 || edit.range.length == f.textStorage.length {
            f.text = value
        } else {
            f.textStorage.replaceCharacters(in: edit.range, with: NSAttributedString(string: edit.text, attributes: f.typingAttributes))
        }
        f.selectedRange = carrySelection(selection, through: edit)
        (f as? TextArea)?.markup?.bookmark = f.selectedRange
        restyleMarkup()
    }
    func textViewDidChange(_ textView: UITextView) {
        if let editor = (textView as? TextArea)?.markup, editor.applying || editor.styling { return }
        textView.setNeedsDisplay()
        if !disabled { presenter?.typed(id, textView.text ?? "", input: handlers.contains("input")) }
        if textView.markedTextRange == nil, let held = pendingValue { writeValue(held, into: textView) }
        configureMarkup()
        restyleMarkup()
        publishMarkupSelection()
    }
    func textViewDidChangeSelection(_ textView: UITextView) {
        guard let f = textView as? TextArea, let editor = f.markup, !editor.applying, !editor.styling, f.markedTextRange == nil else { return }
        if f.isFirstResponder { editor.bookmark = f.selectedRange }
        restyleMarkup()
        publishMarkupSelection()
    }
    func textViewDidBeginEditing(_ textView: UITextView) {
        presenter?.collections.pinsChanged()
        presenter?.editing = self
        if handlers.contains("focus") { presenter?.focus(id) }
        presenter?.reveal(self)
        publishMarkupSelection(force: true)
    }
    func textViewDidEndEditing(_ textView: UITextView) { presenter?.collections.pinsChanged();
        if presenter?.editing === self { presenter?.editing = nil }
        presenter?.commitEdit(id, textView.text ?? "", change: handlers.contains("change"))
        if handlers.contains("blur") { presenter?.blur(id) }
    }
}
#endif

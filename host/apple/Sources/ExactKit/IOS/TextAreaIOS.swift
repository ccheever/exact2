// Contract textarea: UITextView keeps Enter as a newline, never a submission.
#if os(iOS) || os(tvOS)
import UIKit

// The software keyboard calls UIKeyInput directly, including deletion in an
// empty field: its Backspace and Return are keys the `key` handlers hear
// too. A hardware keyboard's keys reach them in `pressesBegan`, before UIKit
// edits with them (KeyEvents.swift).
final class TextField: UITextField {
    weak var owner: NodeView?
    /// The hardware key whose handlers ran in `pressesBegan`: UIKit's own
    /// Backspace or Return for it does not run them again.
    var heard: String?
    override func textRect(forBounds bounds: CGRect) -> CGRect { owner?.nativeEditorRect(in: bounds) ?? super.textRect(forBounds: bounds) }
    override func editingRect(forBounds bounds: CGRect) -> CGRect { owner?.nativeEditorRect(in: bounds) ?? super.editingRect(forBounds: bounds) }
    override func placeholderRect(forBounds bounds: CGRect) -> CGRect { owner?.nativeEditorRect(in: bounds) ?? super.placeholderRect(forBounds: bounds) }
    #if !os(tvOS)
    /// A search field's clear button sits where `UISearchTextField`'s does.
    override func clearButtonRect(forBounds bounds: CGRect) -> CGRect { owner?.searchChrome?.clearButtonRect(forBounds: bounds) ?? super.clearButtonRect(forBounds: bounds) }
    #endif
    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let remaining=owner?.pressedControls(presses,down:true) ?? presses
        if remaining.isEmpty { return }
        if owner?.editorKeyDown(remaining) == true { return }
        heard = remaining.first?.key.map(NodeView.keyName)
        super.pressesBegan(remaining,with:event)
    }
    override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        heard = nil
        let remaining=owner?.pressedControls(presses,down:false) ?? presses
        if !remaining.isEmpty {owner?.editorKeyUp(remaining); super.pressesEnded(remaining,with:event)}
    }
    override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        heard = nil
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
    /// UIKit puts the caret at the end as a field takes the focus, which is
    /// no selection of the person's; one a script set while it had no focus
    /// is shown instead (x2apps codeedit #2).
    override func becomeFirstResponder() -> Bool {
        let selections = owner?.presenter?.fieldSelections
        let ok = selections?.quietly { super.becomeFirstResponder() } ?? super.becomeFirstResponder()
        if ok, let owner { selections?.focused(owner) }
        return ok
    }
    override func deleteBackward() {
        if heard != "Backspace", let owner, !owner.disabled, owner.presenter?.keyDown(at: owner, "Backspace") == true { return }
        super.deleteBackward()
    }
    // DOM's clipboard events, before the field's own (#125, Clipboard.swift).
    override func copy(_ sender: Any?) { NodeView.fieldEdit(owner, #selector(NodeView.copy(_:))) { super.copy(sender) } }
    override func cut(_ sender: Any?) { NodeView.fieldEdit(owner, #selector(NodeView.cut(_:))) { super.cut(sender) } }
    override func paste(_ sender: Any?) { NodeView.fieldEdit(owner, #selector(NodeView.paste(_:))) { super.paste(sender) } }
}

package final class TextArea: UITextView {
    // UIKit already owns a specialized manager per editor. Its native text
    // undo actions require that manager; observe it without replacing it.
    private var textUndo: NativeTextUndo?
    package override var undoManager: UndoManager? {
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
    package var markup: MarkdownEditing?

    package override func insertText(_ text: String) {
        // UIKeyInput (including the software keyboard) need not ask the
        // text-view delegate before insertion. Handle a plain Return here;
        // the command's own native insertion bypasses through `applying`.
        if text == "\n", let editor = markup, !editor.applying,
           markedTextRange == nil, owner?.formatMarkup("newline", selection: selectedRange) == true { return }
        super.insertText(text)
    }

    package override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let remaining=owner?.pressedControls(presses,down:true) ?? presses
        if remaining.isEmpty || owner?.editorKeyDown(remaining) == true { return }
        super.pressesBegan(remaining,with:event)
    }
    package override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let remaining=owner?.pressedControls(presses,down:false) ?? presses
        if !remaining.isEmpty {owner?.editorKeyUp(remaining); super.pressesEnded(remaining,with:event)}
    }
    package override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let remaining=owner?.pressedControls(presses,down:false) ?? presses
        if !remaining.isEmpty {super.pressesCancelled(remaining,with:event)}
    }
    package override func resignFirstResponder() -> Bool {
        let wasFirst = isFirstResponder
        if wasFirst, markedTextRange == nil { markup?.bookmark = selectedRange }
        let resigned = super.resignFirstResponder()
        // UITextView's editing delegate omits read-only selection sessions.
        // They still blur in HTML, and the app must be able to remove its
        // transient selection surface after focus moves elsewhere.
        #if os(tvOS)
        // tvOS text views do not edit, so every session is read-only.
        if resigned, wasFirst, let owner, owner.handlers.contains("blur") {
            owner.presenter?.blur(owner.id)
        }
        #else
        if resigned, wasFirst, !isEditable, let owner, owner.handlers.contains("blur") {
            owner.presenter?.blur(owner.id)
        }
        #endif
        return resigned
    }
    package override var keyCommands: [UIKeyCommand]? {
        #if os(tvOS)
        // tvOS text views do not edit.
        return super.keyCommands
        #else
        guard markup != nil, isEditable else { return super.keyCommands }
        return (super.keyCommands ?? []) + [
            UIKeyCommand(title: "Bold", action: #selector(markupBold), input: "b", modifierFlags: .command),
            UIKeyCommand(title: "Italic", action: #selector(markupItalic), input: "i", modifierFlags: .command),
            UIKeyCommand(title: "Link", action: #selector(markupLink), input: "k", modifierFlags: .command),
            UIKeyCommand(title: "Copy Plain Text", action: #selector(copyPlainText), input: "c", modifierFlags: [.command, .shift]),
        ]
        #endif
    }
    @objc private func markupBold() { owner?.formatMarkup("bold") }
    @objc private func markupItalic() { owner?.formatMarkup("italic") }
    @objc private func markupLink() { owner?.editMarkupLink() }
    @objc package func copyPlainText() {
        // tvOS has no pasteboard.
        #if !os(tvOS)
        guard markup != nil, selectedRange.length > 0, let plain = MarkdownLink.installed?.plain((text as NSString).substring(with: selectedRange)) else { return }
        UIPasteboard.general.string = plain
        #endif
    }
    // DOM's clipboard events, before the editor's own (#125, Clipboard.swift).
    package override func copy(_ sender: Any?) {
        NodeView.fieldEdit(owner, #selector(NodeView.copy(_:))) {
            guard markup != nil else { super.copy(sender); return }
            #if !os(tvOS)
            if selectedRange.length > 0 { UIPasteboard.general.string = (text as NSString).substring(with: selectedRange) }
            #endif
        }
    }
    package override func cut(_ sender: Any?) { NodeView.fieldEdit(owner, #selector(NodeView.cut(_:))) { super.cut(sender) } }
    package override func paste(_ sender: Any?) { NodeView.fieldEdit(owner, #selector(NodeView.paste(_:))) { super.paste(sender) } }
    // Keep TextKit's line pitch equal to the authored CSS line box. Updating
    // storage attributes preserves the value and selected range; replacing
    // attributedText would reset a selection (including a read-only one).
    func applyLineHeight(_ height: CGFloat?) {
        let paragraph = NSMutableParagraphStyle()
        paragraph.alignment = textAlignment
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
    package override func draw(_ rect: CGRect) {
        super.draw(rect)
        if text.isEmpty, !placeholder.isEmpty {
            (placeholder as NSString).draw(in: bounds.inset(by: textContainerInset), withAttributes: [
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
    /// No correction (`autocorrect="off"`, or an email, URL or password
    /// input) keeps the text as typed, as HTML asks: no smart quotes or
    /// dashes either (#111). Otherwise the keyboard's.
    var inputSmartQuotes: UITextSmartQuotesType { inputCorrection == .no ? .no : .default }
    var inputSmartDashes: UITextSmartDashesType { inputCorrection == .no ? .no : .default }

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
        #if !os(tvOS)
        // A CGColor does not retain UIColor's semantic appearance. The
        // native textarea's drawn separator follows UIKit's trait change.
        f.registerForTraitChanges([UITraitUserInterfaceStyle.self, UITraitAccessibilityContrast.self]) { (view: TextArea, _: UITraitCollection) in
            if view.owner?.isNativeTextControl == true {
                view.layer.borderColor = UIColor.separator.resolvedColor(with: view.traitCollection).cgColor
            }
        }
        #endif
        f.backgroundColor = .clear
        f.textContainerInset = .zero
        f.textContainer.lineFragmentPadding = 0
        f.delegate = self
        #if os(iOS)
        // Not the screen's scroller: the status bar scrolls that (LLP 1115).
        f.scrollsToTop = false
        #endif
        addSubview(f)
        textArea = f
    }
    func configureMarkup() {
        guard let f = textArea as? TextArea else { return }
        if props["markup"] == "markdown" {
            if f.markup == nil { f.markup = MarkdownLink.installed?.editor() }
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
        #if !os(tvOS)
        f.isEditable = !disabled && props["editable"] != "false"
        #endif
        f.isSelectable = !disabled
        let content = Autofill.contentType(props["autocomplete"], fallback: nil)
        let traitsChanged = f.autocapitalizationType != inputCapitalization || f.autocorrectionType != inputCorrection || f.spellCheckingType != inputSpellChecking || f.smartQuotesType != inputSmartQuotes || f.smartDashesType != inputSmartDashes || f.textContentType != content
        f.autocapitalizationType = inputCapitalization
        f.autocorrectionType = inputCorrection
        f.spellCheckingType = inputSpellChecking
        f.smartQuotesType = inputSmartQuotes
        f.smartDashesType = inputSmartDashes
        f.textContentType = content
        if traitsChanged, f.isFirstResponder { f.reloadInputViews() }
        f.accessibilityLabel = props["accessibilityLabel"]
        f.accessibilityIdentifier = props["testId"]
        (f as? TextArea)?.placeholder = props["placeholder"] ?? ""
    }
    func styleTextArea() {
        textArea?.textAlignment = NSTextAlignment(rawValue: textAlignmentCode) ?? .left
        guard let f = textArea, let t = text else { return }
        guard f.markedTextRange == nil else { layoutTextArea(); return }
        f.font = t.font(size: number("font_size", PageFacts.defaultRootFontSize), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"]?.string) == "italic", numeric: Int(number("font_variant_numeric")))
        f.textColor = color("text_color", SystemColor.canvasText)
        f.tintColor = isNativeTextControl ? caretColor ?? channels("accent_color").map { TextEngine.color($0) } : caretColor
        if isNativeTextControl {
            f.typingAttributes[.kern] = number("letter_spacing")
            f.textStorage.addAttribute(.kern, value: number("letter_spacing"), range: NSRange(location: 0, length: f.textStorage.length))
        }
        (f as? TextArea)?.configureNativeChrome(isNativeTextControl)
        (f as? TextArea)?.applyLineHeight(usedLineHeight)
        restyleMarkup()
        f.setNeedsDisplay()
        layoutTextArea()
    }
    func layoutTextArea() {
        guard let area = textArea else { return }
        #if os(tvOS)
        area.frame = contentBox() // tvOS keeps its read-only presentation.
        #else
        if isNativeTextControl {
            area.frame = bounds
            if let r = nativeFieldContent {
                area.textContainerInset = UIEdgeInsets(top: r.minY, left: r.minX, bottom: max(0, bounds.height - r.maxY), right: max(0, bounds.width - r.maxX))
            }
        } else { area.frame = contentBox() }
        #endif
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
    package func textView(_ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool {
        TextInputLimit.allows(textView.text ?? "", range: range, replacement: text, props: props)
    }
    package func textViewDidChange(_ textView: UITextView) {
        if let editor = (textView as? TextArea)?.markup, editor.applying || editor.styling { return }
        textView.setNeedsDisplay()
        if !disabled { presenter?.typed(id, textView.text ?? "", input: handlers.contains("input")) }
        if textView.markedTextRange == nil, let held = pendingValue { writeValue(held, into: textView) }
        configureMarkup()
        restyleMarkup()
        publishMarkupSelection()
    }
    package func textViewDidChangeSelection(_ textView: UITextView) {
        presenter?.fieldSelections.changed(self) // a plain textarea's `select` (x2apps codeedit #2)
        guard let f = textView as? TextArea, let editor = f.markup, !editor.applying, !editor.styling, f.markedTextRange == nil else { return }
        if f.isFirstResponder { editor.bookmark = f.selectedRange }
        restyleMarkup()
        publishMarkupSelection()
    }
    package func textViewDidBeginEditing(_ textView: UITextView) {
        presenter?.collections.pinsChanged()
        presenter?.editing = self
        presenter?.fieldSelections.focused(self)
        if handlers.contains("focus") { presenter?.focus(id) }
        presenter?.reveal(self)
        publishMarkupSelection(force: true)
    }
    package func textViewDidEndEditing(_ textView: UITextView) { presenter?.collections.pinsChanged();
        if presenter?.editing === self { presenter?.editing = nil }
        presenter?.commitEdit(id, textView.text ?? "", change: handlers.contains("change"))
        if handlers.contains("blur") { presenter?.blur(id) }
    }
}
#endif

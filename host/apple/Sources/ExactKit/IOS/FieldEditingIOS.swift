// A text field's editing as NodeView hears it from UIKit (UITextFieldDelegate):
// a value written through composition with the caret carried, each change's
// `input`, and the focus, reveal, commit and blur as editing begins and ends.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    /// As `writeValue(_:into:)` for a text view: composition defers the write,
    /// and the caret is carried through the changed middle.
    func writeValue(_ value: String, into f: UITextField) {
        if f.markedTextRange != nil { pendingValue = value; return }
        pendingValue = nil
        guard let edit = minimalTextEdit(from: f.text ?? "", to: value) else { return }
        var selection = NSRange(location: (f.text ?? "").utf16.count, length: 0)
        if let r = f.selectedTextRange {
            selection = NSRange(location: f.offset(from: f.beginningOfDocument, to: r.start), length: f.offset(from: r.start, to: r.end))
        }
        f.text = value
        let carried = carrySelection(selection, through: edit)
        if let start = f.position(from: f.beginningOfDocument, offset: carried.location), let end = f.position(from: start, offset: carried.length) {
            f.selectedTextRange = f.textRange(from: start, to: end)
        }
    }
    @objc func fieldChanged() {
        if props["emojiPicker"] == "true", let field {
            if field.markedTextRange == nil, let held = pendingValue { writeValue(held, into: field) }
            let value = field.text ?? ""
            field.text = ""
            if !disabled, EmojiSelection.accepts(value) { presenter?.typed(id, value, input: handlers.contains("input")) }
            return
        }
        // The textarea's order (`textViewDidChange`): the text the field now
        // holds is reported first, and only then does a value held while
        // composing apply. Writing it first reported the composing text —
        // 你好 committed as "nihao".
        if !disabled { presenter?.typed(id, field?.text ?? "", input: handlers.contains("input")) }
        if let f = field, f.markedTextRange == nil, let held = pendingValue { writeValue(held, into: f) }
    }
    @objc func textFieldDidBeginEditing(_ textField: UITextField) {
        presenter?.collections.pinsChanged()
        presenter?.editing = self
        // The keyboard is already up (another field had it): it will not
        // move, so this field is revealed here, as a browser scrolls a
        // newly focused field into view.
        if let p = presenter, p.keyboardInset > 0 {
            if ExactEnv.agentFreezes {
                p.reveal(self)
            } else {
                UIView.animate(withDuration: 0.25) { p.reveal(self) }
            }
        }
        if handlers.contains("focus") { presenter?.focus(id) }
    }
    @objc func textFieldDidEndEditing(_ textField: UITextField) {
        presenter?.collections.pinsChanged()
        if presenter?.editing === self { presenter?.editing = nil }
        presenter?.commitEdit(id, textField.text ?? "", change: handlers.contains("change"))
        if handlers.contains("blur") { presenter?.blur(id) }
    }
}
#endif

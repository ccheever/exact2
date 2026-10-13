// A text field's editing as NodeView hears it from UIKit (UITextFieldDelegate):
// a value written through composition with the caret carried, each change's
// `input`, and the focus, reveal, commit and blur as editing begins and ends.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    package func textField(_ textField: UITextField, shouldChangeCharactersIn range: NSRange, replacementString string: String) -> Bool {
        guard !disabled, !inert, props["editable"] != "false" else { return false }
        if props["emojiPicker"] == "true" {
            if EmojiSelection.accepts(string) { presenter?.typed(id, string, input: handlers.contains("input")) }
            return false
        }
        if props["type"]?.lowercased() == "number", textField.markedTextRange == nil {
            let filtered = NumberStepping.insertion(string, in: textField.text ?? "", range: range)
            if filtered != string {
                guard !filtered.isEmpty,
                      TextInputLimit.allows(textField.text ?? "", range: range, replacement: filtered, props: props),
                      let start = textField.position(from: textField.beginningOfDocument, offset: range.location),
                      let end = textField.position(from: start, offset: range.length),
                      let edit = textField.textRange(from: start, to: end) else { return false }
                textField.replace(edit, withText: filtered)
                fieldChanged()
                return false
            }
        }
        return TextInputLimit.allows(textField.text ?? "", range: range, replacement: string, props: props)
    }

    /// As `writeValue(_:into:)` for a text view: the value the field already
    /// shows changes nothing, so a composition and its selection stand; a
    /// different one is held while text is marked and applied when the
    /// composition ends; with nothing marked it replaces the text, the
    /// caret carried through the changed middle.
    func writeValue(_ value: String, into f: UITextField) {
        if (f.text ?? "").utf16.elementsEqual(value.utf16) { pendingValue = nil; return }
        if f.markedTextRange != nil { pendingValue = value; return }
        pendingValue = nil
        toldText = nil
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
            let value = field.text ?? ""
            field.text = ""
            if !disabled, EmojiSelection.accepts(value) { presenter?.typed(id, value, input: handlers.contains("input")) }
            return
        }
        // UIKit says so at each change, the text an input method has marked
        // included: the app hears the field's text as it stands, and only
        // then does a value held while it composed apply
        // (`FieldComposition.swift`). Writing it first told the app its own
        // held value in place of the committed text.
        if !disabled { typedText(field?.text ?? "") }
        if stepper != nil { updateNumberStepper() } // the stepper shows what the field holds (LLP 1116.007 D4)
        if let f = field, f.markedTextRange == nil, let held = pendingValue { writeValue(held, into: f) }
    }
    @objc package func textFieldDidBeginEditing(_ textField: UITextField) {
        presenter?.collections.pinsChanged()
        presenter?.editing = self
        // A focus UIKit gives back, UIKit scrolls into view (`FocusReturn`).
        let returned = (textField as? TextField)?.focusReturn.returning == true
        presenter?.focusReturned = returned ? self : nil
        // The keyboard is already up (another field had it): it will not
        // move, so this field is revealed here, as a browser scrolls a
        // newly focused field into view.
        if let p = presenter, p.keyboardInset > 0, !returned {
            if ExactEnv.agentFreezes {
                p.reveal(self)
            } else {
                UIView.animate(withDuration: 0.25) { p.reveal(self) }
            }
        }
        presenter?.focus(id)
    }
    /// The selection moved: a person's non-collapsed one is a `select`
    /// (x2apps codeedit #2, `FieldSelections`).
    @objc package func textFieldDidChangeSelection(_ textField: UITextField) { presenter?.fieldSelections.changed(self) }
    @objc package func textFieldDidEndEditing(_ textField: UITextField) {
        presenter?.collections.pinsChanged()
        if presenter?.editing === self { presenter?.editing = nil }
        if presenter?.focusReturned === self { presenter?.focusReturned = nil }
        presenter?.commitEdit(id, textField.text ?? "", change: handlers.contains("change"))
        presenter?.blur(id)
    }
}
#endif

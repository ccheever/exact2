// A text field's editing as NodeView hears it from AppKit (NSTextFieldDelegate):
// Enter's change and implicit submission, the editor's caret and spelling as
// it begins, the commit and blur as it ends, and each change's `input`.
#if os(macOS)
import AppKit

extension NodeView {
    /// Enter in a text field's editor: its `change` commits and, with a
    /// `submit` handler, the web's implicit submission. Taken here, so it
    /// does not end the editing as AppKit would. Its `key` handlers heard
    /// every key before the editor did (`Presenter.keyDown`).
    @objc func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        guard selector == #selector(NSResponder.insertNewline(_:)) else { return false }
        presenter?.commitEdit(id, textView.string, change: handlers.contains("change"))
        if handlers.contains("submit") { presenter?.submit(id) }
        return true
    }
    @objc func controlTextDidBeginEditing(_ obj: Notification) {
        presenter?.collections.pinsChanged()
        guard let editor = field?.currentEditor() as? NSTextView else { return }
        editor.insertionPointColor = caretColor
        applyTextChecking(editor)
    }
    @objc func controlTextDidEndEditing(_ obj: Notification) {
        presenter?.collections.pinsChanged()
        showFieldFocus(false)
        presenter?.commitEdit(id, field?.stringValue ?? "", change: handlers.contains("change"))
        if handlers.contains("blur") { presenter?.blur(id) }
    }
    @objc func controlTextDidChange(_ obj: Notification) {
        if let editor = field?.currentEditor() as? NSTextView, !editor.hasMarkedText(), let held = pendingValue { writeValue(held, into: editor) }
        if props["emojiPicker"] == "true", let field {
            let value = field.stringValue
            field.stringValue = ""
            if !disabled, EmojiSelection.accepts(value) { presenter?.typed(id, value, input: handlers.contains("input")) }
            return
        }
        if !disabled { presenter?.typed(id, field?.stringValue ?? "", input: handlers.contains("input")) }
    }
    /// A field in its default look (`fieldStyle`, LLP 1104 D4) marks its
    /// focus as the web's `:focus-visible` ring does: its border, drawn by
    /// the box painter (`boxPlan`), two points wide in the focus colour, so
    /// it follows the box's shape, size, clip and appearance. A bare or
    /// disabled field draws its own. Set as the field takes the focus,
    /// cleared as its editor leaves.
    func showFieldFocus(_ on: Bool) {
        fieldFocused = on && props["fieldStyle"] != nil && !disabled
    }
}
#endif

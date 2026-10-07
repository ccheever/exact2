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
    @objc package func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        guard selector == #selector(NSResponder.insertNewline(_:)) else { return false }
        presenter?.commitEdit(id, textView.string, change: handlers.contains("change"))
        if handlers.contains("submit") { presenter?.submit(id) }
        return true
    }
    @objc package func controlTextDidBeginEditing(_ obj: Notification) {
        presenter?.collections.pinsChanged()
        guard let editor = field?.currentEditor() as? NSTextView else { return }
        styleFieldEditor(editor)
    }
    @objc package func controlTextDidEndEditing(_ obj: Notification) {
        presenter?.collections.pinsChanged()
        presenter?.commitEdit(id, field?.stringValue ?? "", change: handlers.contains("change"))
        if handlers.contains("blur") { presenter?.blur(id) }
    }
    @objc package func controlTextDidChange(_ obj: Notification) {
        if let editor = field?.currentEditor() as? NSTextView, !editor.hasMarkedText(), let held = pendingValue { writeValue(held, into: editor) }
        if props["emojiPicker"] == "true", let field {
            let value = field.stringValue
            field.stringValue = ""
            if !disabled, EmojiSelection.accepts(value) { presenter?.typed(id, value, input: handlers.contains("input")) }
            return
        }
        if !disabled { presenter?.typed(id, field?.stringValue ?? "", input: handlers.contains("input")) }
    }
}
#endif

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
        (field?.currentEditor() as? NSTextView)?.insertionPointColor = caretColor
        (field?.currentEditor() as? NSTextView)?.isAutomaticSpellingCorrectionEnabled = allowsInputCorrection
        (field?.currentEditor() as? NSTextView)?.isContinuousSpellCheckingEnabled = allowsInputSpellChecking
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
    /// focus with a two-point ring in the accent colour over its border,
    /// where the web draws its `:focus-visible` ring; a bare field draws
    /// its own. Shown as the field takes the focus, gone as its editor
    /// leaves.
    func showFieldFocus(_ on: Bool) {
        let ring = layer?.sublayers?.first { $0.name == "exact.fieldFocus" }
        guard on, props["fieldStyle"] != nil, let layer else { ring?.removeFromSuperlayer(); return }
        let r = ring ?? CALayer()
        r.name = "exact.fieldFocus"
        r.frame = layer.bounds
        r.autoresizingMask = [.layerWidthSizable, .layerHeightSizable]
        r.cornerRadius = max(layer.cornerRadius, boxBorder?.cornerRadius ?? 0, boxFill?.cornerRadius ?? 0)
        r.borderWidth = 2
        r.borderColor = NSColor.keyboardFocusIndicatorColor.withAlphaComponent(1).cgColor
        r.zPosition = 1
        if ring == nil { layer.addSublayer(r) }
    }
}
#endif

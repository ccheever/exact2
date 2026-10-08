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
        showFieldFocus(false)
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
    /// The ring Exact draws for a focused field (LLP 1104 D6): its border,
    /// painted by the box painter (`boxPlan`) two points wide in the focus
    /// colour, so it follows the box's shape, size, clip and appearance. A
    /// bare field or textarea keeps Exact's ring, as a bare button does; a
    /// native textarea takes it as a stand-in, because AppKit asks only the
    /// first responder for a ring and the text view's sits inside its clip
    /// view. A native single-line field is AppKit's own ring. Set as the
    /// editor takes the focus, cleared as it leaves.
    func showFieldFocus(_ on: Bool) {
        let show = on && !disabled && (!isNativeTextControl || textArea != nil)
        // A textarea's scroll view covers the node's own border, so its ring
        // is a layer above the node's subviews instead.
        guard textArea != nil else { fieldFocused = show; return }
        fieldFocused = false
        let ring = layer?.sublayers?.first { $0.name == "exact.fieldFocus" }
        guard show, let layer else { ring?.removeFromSuperlayer(); return }
        let r = ring ?? CALayer()
        r.name = "exact.fieldFocus"
        r.frame = layer.bounds
        r.autoresizingMask = [.layerWidthSizable, .layerHeightSizable]
        r.cornerRadius = max(layer.cornerRadius, boxBorder?.cornerRadius ?? 0, boxFill?.cornerRadius ?? 0, isNativeTextControl ? 5 : 0)
        r.borderWidth = 2
        effectiveAppearance.performAsCurrentDrawingAppearance {
            r.borderColor = NSColor.keyboardFocusIndicatorColor.withAlphaComponent(1).cgColor
        }
        r.zPosition = 1
        if ring == nil { layer.addSublayer(r) }
    }
}
#endif

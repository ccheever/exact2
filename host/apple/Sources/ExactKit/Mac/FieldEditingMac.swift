// A text field's editing as NodeView hears it from AppKit (NSTextFieldDelegate):
// Return's change, `submit` and end of editing, the editor's caret and spelling
// as it begins, the commit and blur as it ends, and each change's `input`.
#if os(macOS)
import AppKit

extension NodeView {
    /// Return in a single-line field's editor (LLP 1115.000 D7.4): its
    /// `change` commits, its `submit` is sent, and the editing ends as
    /// `NSTextField` ends it, with the field's text selected. A focus the
    /// action set is kept: the text is selected only while this field still
    /// has the editor. In a dialog a field with no `submit` handler passed
    /// Return to the default button before it came here (`DialogHost.key`),
    /// and a key the app prevented never came at all: its `key` handlers
    /// heard every key before the editor did (`Presenter.keyDown`).
    @objc package func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        if props["type"]?.lowercased() == "number", !textView.hasMarkedText(),
           selector == #selector(NSResponder.moveUp(_:)) || selector == #selector(NSResponder.moveDown(_:)) {
            step(up: selector == #selector(NSResponder.moveUp(_:)))
            return true
        }
        guard selector == #selector(NSResponder.insertNewline(_:)) else { return false }
        // A prompt's field (AlertsMac.swift): its alert's Return rule.
        if presenter?.alerts.fieldReturned(self) == true { return true }
        presenter?.commitEdit(id, textView.string, change: handlers.contains("change"))
        if handlers.contains("submit") { presenter?.submit(id) }
        if presenter?.views[id] === self, let field, field.currentEditor() === textView, window?.firstResponder === textView {
            NodeView.selectAll(in: field)
        }
        return true
    }
    @objc package func controlTextDidBeginEditing(_ obj: Notification) {
        presenter?.collections.pinsChanged()
        guard let editor = field?.currentEditor() as? NSTextView else { return }
        styleFieldEditor(editor)
    }
    @objc package func controlTextDidEndEditing(_ obj: Notification) {
        if Self.restartingEditing === self { return }
        editingEnded()
    }
    /// The end of a field's editing: its `change` commits and it blurs.
    private func editingEnded() {
        presenter?.collections.pinsChanged()
        // Its editor still holds it here; a turn later it is the field's alone.
        RunLoop.main.perform(inModes: [.common]) { [weak self] in self?.syncFieldSide() }
        leavingFocus {
            showFieldFocus(false)
            presenter?.commitEdit(id, field?.stringValue ?? "", change: handlers.contains("change"))
            presenter?.blur(id)
        }
    }
    /// Runs `body`, the end of a node's editing, with the node counted
    /// as unfocused. AppKit posts the end before it detaches a field's
    /// editor (`currentEditor()` still returns it) and before a textarea
    /// resigns first responder, and the app's `change` and `blur` handlers
    /// run in `body`. A restyle there draws no ring (`restyleFieldFocus`),
    /// and a value written there goes to the field, because AppKit does
    /// not read the leaving editor's text back (`editingEditor`).
    func leavingFocus(_ body: () -> Void) {
        let me = ObjectIdentifier(self)
        let outermost = Self.leaving.insert(me).inserted
        defer { if outermost { Self.leaving.remove(me) } }
        body()
    }
    nonisolated(unsafe) private static var leaving: Set<ObjectIdentifier> = []
    private var isLeavingFocus: Bool { Self.leaving.contains(ObjectIdentifier(self)) }
    /// The editor editing this node's field: none once its editing is ending (`leavingFocus`).
    var editingEditor: NSTextView? { isLeavingFocus ? nil : field?.currentEditor() as? NSTextView }
    /// A write to a field that AppKit applies by ending the field editor's
    /// editing and beginning it again
    /// (`-[NSTextField setPlaceholderAttributedString:]` does, so the editor
    /// shows the new placeholder). The field keeps the focus throughout, so
    /// the app hears no blur and no `change` from it, as the web changes a
    /// focused input's placeholder with neither. Editing that does not come
    /// back ends as any does.
    func keepingEditing(_ f: NSTextField, _ change: () -> Void) {
        guard f.currentEditor() != nil, Self.restartingEditing == nil else { return change() }
        Self.restartingEditing = self
        change()
        Self.restartingEditing = nil
        if f.currentEditor() == nil { editingEnded() }
    }
    nonisolated(unsafe) private static weak var restartingEditing: NodeView?
    @objc package func controlTextDidChange(_ obj: Notification) {
        // What AppKit says of the app's own value going in is not typing.
        if writingValue { return }
        if props["emojiPicker"] == "true", let field {
            let value = field.stringValue
            field.stringValue = ""
            if !disabled, EmojiSelection.accepts(value) { presenter?.typed(id, value, input: handlers.contains("input")) }
            return
        }
        if !disabled { typedText(field?.stringValue ?? "") }
        if let editor = field?.currentEditor() as? NSTextView {
            applyHeldValue(to: editor)
            applyBaseDirection(to: editor) // an undo brings back the attributes of the text it restores
        }
        if stepper != nil { updateNumberStepper() } // the stepper shows what the field holds (LLP 1116.007 D4)
    }
    /// The composition in `editor` is over, the app told of its text: what
    /// the app wrote while it lasted and is still held lands now
    /// (`pendingValue`). Nothing while text is still marked.
    func applyHeldValue(to editor: NSTextView) {
        if !editor.hasMarkedText(), let held = pendingValue { writeValue(held, into: editor) }
    }
    /// What an input method has marked in `editor` becomes text, now: a ⌘
    /// chord's keydown ends the composition.
    /// The input method is asked to let go of it first, which it answers
    /// by inserting the text, and what it leaves marked is committed
    /// (Firefox's `CommitIMEComposition`; committing first could insert the
    /// syllable twice).
    func commitComposition(in editor: NSTextView) {
        settlingComposition(editor) {
            editor.inputContext?.discardMarkedText()
            if editor.hasMarkedText() { editor.unmarkText() }
        }
    }
    /// Marked text is committed in steps: `unmarkText` takes it out of the
    /// text and inserts it again, and an input context told to let go does
    /// the same. The steps are not changes for the app to hear, so `body`
    /// runs with them unheard, and its result is heard once, if it is not
    /// the text the app knows (`typedText`).
    func settlingComposition(_ editor: NSTextView, _ body: () -> Void) {
        let writing = writingValue
        writingValue = true
        body()
        writingValue = writing
        if !writing, !disabled { typedText(editor.string) }
        if !writing { applyHeldValue(to: editor) }
    }
    /// The text an input method is composing in this node's editor changed
    /// (`setMarkedText`, which `FieldEditor` and `TextArea` say here). AppKit
    /// posts no change for it, only for the commit; the app hears the
    /// editor's text with the marked text in it at each change, as on every
    /// host (`FieldComposition.swift`). An emoji picker's field composes nothing.
    func markedTextChanged(_ editor: NSTextView) {
        guard !writingValue, !disabled, props["emojiPicker"] != "true" else { return }
        if editor === textArea { editor.needsDisplay = true }
        typedText(editor.string)
        // Cancelled: AppKit posts no change for it, so the held value lands
        // here, and faces that came while it was marked are drawn.
        applyHeldValue(to: editor)
        if editor === textArea, !editor.hasMarkedText() { restyleMarkupProps() }
        // And a `direction` that came while it was marked (LLP 1001 §1).
        // The app heard its text first, and may have moved the focus: the editor is styled for its field now.
        if !editor.hasMarkedText() {
            if editor === textArea { applyBaseDirection(to: editor) }
            else if let now = (editor as? FieldEditor)?.owner ?? (field?.currentEditor() === editor ? self : nil) { now.styleFieldEditor(editor) }
        }
    }
    /// The ring Exact draws for a focused field (LLP 1104 D6): its border,
    /// painted by the box painter (`boxPlan`) two points wide in the focus
    /// colour, so it follows the box's shape, size, clip and appearance. A
    /// bare field or textarea keeps Exact's ring, as a bare button does; a
    /// native textarea takes it as a stand-in, because AppKit asks only the
    /// first responder for a ring and the text view's sits inside its clip
    /// view. A native single-line field is AppKit's own ring. Set as the
    /// editor takes the focus, cleared as it leaves. `outline: none` draws
    /// neither (`outlineNone`).
    ///
    /// A border the author wrote is left alone (LLP 1115.000 D7.9), as a
    /// native control with a border of its own keeps it under AppKit's
    /// ring. A bare field with one takes no ring of Exact's: its ring is
    /// AppKit's exterior one around the node's box, which its field asks
    /// for (`bareFieldMask`). A textarea with one keeps Exact's ring, since
    /// AppKit's is cut off by its clip view, but outside the box where
    /// AppKit's would be, in AppKit's colour at its own strength; where the
    /// node clips its box, inside the border.
    func showFieldFocus(_ on: Bool) {
        let show = on && !disabled && !outlineNone && (!isNativeTextControl || textArea != nil || fieldGround != nil)
        let bordered = bordersItself
        // A textarea's scroll view covers the node's own border, so its ring
        // is a layer above the node's subviews instead.
        guard textArea != nil else { fieldFocused = show && !bordered; return }
        fieldFocused = false
        let ring = layer?.sublayers?.first { $0.name == "exact.fieldFocus" }
        guard show, let layer else { ring?.removeFromSuperlayer(); return }
        let r = ring ?? CALayer()
        r.name = "exact.fieldFocus"
        let radius = max(layer.cornerRadius, boxBorder?.cornerRadius ?? 0, boxFill?.cornerRadius ?? 0, isNativeTextControl ? 5 : 0)
        let width: CGFloat = bordered ? Self.exteriorRing : 2
        // Outside the box by the ring's width, or inside the author's border by its own.
        let outset = !bordered ? 0 : layer.masksToBounds ? -authoredBorderWidth : width
        r.frame = layer.bounds.insetBy(dx: -outset, dy: -outset)
        r.autoresizingMask = [.layerWidthSizable, .layerHeightSizable]
        r.cornerRadius = max(0, radius + outset)
        r.borderWidth = width
        effectiveAppearance.performAsCurrentDrawingAppearance {
            let color = NSColor.keyboardFocusIndicatorColor
            r.borderColor = (bordered ? color : color.withAlphaComponent(1)).cgColor
        }
        r.zPosition = 1
        if ring == nil { layer.addSublayer(r) }
    }
    /// The width AppKit's exterior focus ring is drawn at.
    static let exteriorRing: CGFloat = 3
    /// The widest side of the border the author wrote, or 0.
    private var authoredBorderWidth: CGFloat {
        let uniform = number("border_width")
        return bordersItself ? ["top", "right", "bottom", "left"].map { number("border_width_" + $0, uniform) }.max() ?? 0 : 0
    }
    /// Whether the author wrote a border on this node that draws: a side
    /// with a width, and a border row in the rows they wrote (LLP 1115.000
    /// D2's bit, `written`). A border the compiler implied is the
    /// platform's stand-in, and the platform's ring may stand in for it.
    var bordersItself: Bool {
        guard case .array(let rows)? = style["written"] else { return false }
        let wrote = rows.contains { row in ["border_width", "border_style", "border_color"].contains { row.string?.hasPrefix($0) == true } }
        let uniform = number("border_width")
        return wrote && (uniform > 0 || ["top", "right", "bottom", "left"].contains { number("border_width_" + $0, uniform) > 0 })
    }
    /// CSS `outline-style: none` on a text field (LLP 1104 D6): no ring,
    /// Exact's or AppKit's, since the author draws the focus another way.
    /// Tab still reaches the field, and its accessibility is unchanged.
    var outlineNone: Bool { style["outline_style"]?.string == "none" }
    /// Exact's ring again as the style changes, for a focused editor
    /// whose `outline` may have.
    func restyleFieldFocus() {
        guard !isLeavingFocus else { return }
        if field?.currentEditor() != nil || (textArea != nil && window?.firstResponder === textArea) { showFieldFocus(true) }
    }
}
#endif

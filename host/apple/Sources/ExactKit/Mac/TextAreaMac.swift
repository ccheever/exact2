// Contract textarea: native multiline editing, with the same change seam as input.
#if os(macOS)
import AppKit

final class TextArea: NSTextView {
    // Each editor owns its native undo history. An authoritative external
    // source update can reset this editor without clearing another field's
    // actions from a window-wide manager.
    private lazy var textUndo = NativeTextUndo(before: { [weak self] in
        self?.markup?.applying = true
    }, after: { [weak self] in
        guard let self, let editor = markup else { return }
        editor.applying = false
        editor.bookmark = selectedRange()
        owner?.textDidChange(Notification(name: NSText.didChangeNotification, object: self))
    })
    override var undoManager: UndoManager? { textUndo.manager }

    weak var owner: NodeView?
    var markup: MarkupEditor?
    var nativeOrigin: NSPoint?
    // AppKit asks the first responder for the mask. Forward the textarea's
    // mask to its scroll view, so the ring encloses the bezel, not the text.
    override var focusRingMaskBounds: NSRect {
        guard owner?.isNativeTextControl == true, let scroll = owner?.textAreaScroll else { return super.focusRingMaskBounds }
        return convert(scroll.bounds, from: scroll)
    }
    override func drawFocusRingMask() {
        guard owner?.isNativeTextControl == true, let scroll = owner?.textAreaScroll else { super.drawFocusRingMask(); return }
        NSGraphicsContext.saveGraphicsState()
        let origin = convert(NSPoint.zero, from: scroll)
        let transform = AffineTransform(translationByX: origin.x, byY: origin.y)
        (transform as NSAffineTransform).concat()
        scroll.drawFocusRingMask()
        NSGraphicsContext.restoreGraphicsState()
    }
    override var textContainerOrigin: NSPoint { nativeOrigin ?? super.textContainerOrigin }

    // The ARIA states AppKit has no property for (`NodeView.ariaAttribute`).
    override func accessibilityAttributeNames() -> [NSAccessibility.Attribute] {
        super.accessibilityAttributeNames() + NodeView.ariaAttributes.filter { owner?.ariaAttribute($0) != nil }.map { .init(rawValue: $0) }
    }
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        owner?.ariaAttribute(attribute.rawValue) ?? super.accessibilityAttributeValue(attribute)
    }

    override func resignFirstResponder() -> Bool {
        if let markup, !hasMarkedText() { markup.bookmark = selectedRange() }
        let ok = super.resignFirstResponder()
        if ok { noteFocusRingMaskChanged(); owner?.textAreaScroll?.noteFocusRingMaskChanged() }
        return ok
    }
    /// `focus` as the web fires it: when the editor takes the focus, not at
    /// its first edit (`textDidBeginEditing`; jukebox F23). `blur` is
    /// `textDidEndEditing`, which AppKit posts whenever the focus leaves.
    override func becomeFirstResponder() -> Bool {
        let ok = super.becomeFirstResponder()
        // A selection a script set while it had no focus (x2apps codeedit #2).
        if ok, let owner { owner.presenter?.fieldSelections.focused(owner) }
        if ok { noteFocusRingMaskChanged(); owner?.textAreaScroll?.noteFocusRingMaskChanged() }
        if ok, let owner, owner.handlers.contains("focus") { owner.presenter?.focus(owner.id) }
        return ok
    }

    override func insertTab(_ sender: Any?) {
        if markup != nil { super.insertTab(sender) } else { window?.selectNextKeyView(self) }
    }
    override func insertBacktab(_ sender: Any?) {
        if markup != nil { super.insertBacktab(sender) } else { window?.selectPreviousKeyView(self) }
    }

    override func insertNewline(_ sender: Any?) {
        if markup != nil, !hasMarkedText(), owner?.formatMarkup("newline") == true { return }
        super.insertNewline(sender)
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        // An embedded/agent window need not own NSApp.keyWindow, so the
        // menu's responder lookup may miss its editor. Route the ordinary
        // undo chord straight to this text view's native manager.
        let modifiers = event.modifierFlags.intersection([.command, .shift, .control, .option])
        if markup != nil, isEditable, !hasMarkedText(), event.charactersIgnoringModifiers?.lowercased() == "z" {
            if modifiers == .command { if undoManager?.canUndo == true { undoManager?.undo() }; return true }
            if modifiers == [.command, .shift] { if undoManager?.canRedo == true { undoManager?.redo() }; return true }
        }
        if markup != nil, event.modifierFlags.intersection([.command, .shift, .control, .option]) == [.command, .shift],
           event.charactersIgnoringModifiers?.lowercased() == "c" {
            copyPlainText(nil); return true
        }
        if markup != nil, isEditable, !hasMarkedText(),
           event.modifierFlags.intersection([.command, .shift, .control, .option]) == .command {
            switch event.charactersIgnoringModifiers?.lowercased() {
            case "b": return owner?.formatMarkup("bold") == true
            case "i": return owner?.formatMarkup("italic") == true
            case "k": owner?.editMarkupLink(); return true
            default: break
            }
        }
        return super.performKeyEquivalent(with: event)
    }

    @objc func copyPlainText(_ sender: Any?) {
        guard markup != nil, selectedRange().length > 0,
              let plain = MarkupCommands.plain((string as NSString).substring(with: selectedRange())) else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(plain, forType: .string)
    }
    // DOM's clipboard events, before the editor's own (#125, Clipboard.swift).
    override func copy(_ sender: Any?) {
        NodeView.fieldEdit(owner, #selector(NodeView.copy(_:))) {
            guard markup != nil else { super.copy(sender); return }
            let selection = selectedRange()
            guard selection.length > 0 else { return }
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString((string as NSString).substring(with: selection), forType: .string)
        }
    }

    override func cut(_ sender: Any?) { NodeView.fieldEdit(owner, #selector(NodeView.cut(_:))) { super.cut(sender) } }
    override func paste(_ sender: Any?) { NodeView.fieldEdit(owner, #selector(NodeView.paste(_:))) { super.paste(sender) } }

    var placeholder = "" { didSet { needsDisplay = true } }
    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        if string.isEmpty, !placeholder.isEmpty {
            let origin = textContainerOrigin
            let rect = NSRect(x: origin.x, y: origin.y, width: textContainer?.containerSize.width ?? bounds.width, height: bounds.height - origin.y)
            (placeholder as NSString).draw(in: rect, withAttributes: [
                .font: font ?? NSFont.systemFont(ofSize: 16),
                .foregroundColor: owner?.isNativeTextControl == true ? NSColor.placeholderTextColor : (textColor ?? SystemColor.canvasText).withAlphaComponent(0.3),
            ])
        }
    }
}

extension NodeView {
    var caretColor: NSColor {
        color("caret_color", isNativeTextControl ? color("accent_color", .controlAccentColor) : color("text_color", .textColor))
    }

    var allowsInputCorrection: Bool {
        if textArea == nil, ["email", "url", "password"].contains((props["type"] ?? "").lowercased()) { return false }
        return (props["autocorrect"] ?? "").lowercased() != "off"
    }

    var allowsInputSpellChecking: Bool { props["spellcheck"] != "false" }

    /// What `autocorrect` and `spellcheck` ask of an editor: a textarea's
    /// text view or an input's field editor.
    func applyTextChecking(_ v: NSTextView) {
        v.isAutomaticSpellingCorrectionEnabled = allowsInputCorrection
        v.isContinuousSpellCheckingEnabled = allowsInputSpellChecking
        v.keepsTypedText(!allowsInputCorrection)
    }

    func makeTextArea() {
        let f = TextArea(usingTextLayoutManager: true)
        f.owner = self
        f.allowsUndo = true
        f.isRichText = false
        f.importsGraphics = false
        f.drawsBackground = false
        f.textContainerInset = .zero
        f.textContainer?.lineFragmentPadding = 0
        f.isVerticallyResizable = true
        f.isHorizontallyResizable = false
        f.autoresizingMask = [.width]
        f.textContainer?.widthTracksTextView = true
        f.delegate = self
        let scroller = TextAreaScroll(frame: .zero)
        scroller.owner = self
        scroller.drawsBackground = false
        scroller.hasVerticalScroller = true
        scroller.autohidesScrollers = true
        scroller.documentView = f
        addSubview(scroller)
        textArea = f
        textAreaScroll = scroller
    }
    /// The app's value into the editor: nothing while text is being composed
    /// (held until the composition ends), else the changed middle only, with
    /// the selection carried through (LLP 1045 D5).
    func writeValue(_ value: String, into f: NSTextView) {
        if f.string.utf16.elementsEqual(value.utf16) { pendingValue = nil; return }
        if f.hasMarkedText() { pendingValue = value; return }
        pendingValue = nil
        guard let edit = minimalTextEdit(from: f.string, to: value) else { return }
        // Native undo ranges refer to the old buffer. An external edit has
        // no position map for those ranges; discard only this view's history.
        f.breakUndoCoalescing()
        f.undoManager?.removeAllActions()
        let selection = f.selectedRange()
        if let storage = f.textStorage, storage.length > 0, edit.range.length < storage.length {
            storage.replaceCharacters(in: edit.range, with: NSAttributedString(string: edit.text, attributes: f.typingAttributes))
        } else {
            f.string = value
        }
        f.setSelectedRange(carrySelection(selection, through: edit))
        (f as? TextArea)?.markup?.bookmark = f.selectedRange()
        restyleMarkup()
    }
    func configureMarkup() {
        guard let f = textArea as? TextArea else { return }
        if props["markup"] == "markdown" {
            if f.markup == nil { f.markup = MarkupEditor() }
        } else if let editor = f.markup, !f.hasMarkedText() {
            guard let storage = f.textStorage else { return }
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
        applyTextChecking(f)
        f.contentType = Autofill.contentType(props["autocomplete"], fallback: nil)
        f.setAccessibilityLabel(props["accessibilityLabel"])
        f.setAccessibilityIdentifier(props["testId"])
        (f as? TextArea)?.placeholder = props["placeholder"] ?? ""
    }
    func styleTextArea() {
        guard let f = textArea, let t = text else { return }
        guard !f.hasMarkedText() else { layoutTextArea(); return }
        f.font = t.font(size: number("font_size", 16), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"]?.string) == "italic", numeric: Int(number("font_variant_numeric")))
        f.textColor = color("text_color", SystemColor.canvasText)
        f.insertionPointColor = caretColor
        let native = isNativeTextControl
        textAreaScroll?.borderType = native ? .bezelBorder : .noBorder
        textAreaScroll?.focusRingType = native ? .default : .none
        textAreaScroll?.drawsBackground = native
        f.drawsBackground = native
        f.focusRingType = .default
        f.textContainerInset = native ? FieldChromeCache.textareaInset : .zero
        let paragraph = NSMutableParagraphStyle()
        if let height = usedLineHeight {
            paragraph.minimumLineHeight = height
            paragraph.maximumLineHeight = height
            if height == 0 { paragraph.lineHeightMultiple = .leastNormalMagnitude }
        }
        f.defaultParagraphStyle = paragraph
        if isNativeTextControl { paragraph.alignment = fieldAlignment }
        var attributes: [NSAttributedString.Key: Any] = [.paragraphStyle: paragraph]
        if isNativeTextControl { attributes[.kern] = number("letter_spacing") }
        f.textStorage?.addAttributes(attributes, range: NSRange(location: 0, length: (f.string as NSString).length))
        f.typingAttributes.merge(attributes) { _, authored in authored }
        restyleMarkup()
        layoutTextArea()
    }
    func layoutTextArea() {
        guard let f = textArea, let scroller = textAreaScroll else { return }
        scroller.frame = isNativeTextControl ? bounds : contentBox()
        if let f = f as? TextArea {
            if isNativeTextControl, let rect = nativeFieldContent {
                let clip = scroller.contentView.frame
                f.nativeOrigin = NSPoint(x: max(0, rect.minX - clip.minX), y: max(0, rect.minY - clip.minY))
                f.textContainer?.widthTracksTextView = false
                f.textContainer?.containerSize.width = rect.width
            } else {
                f.nativeOrigin = nil
                f.textContainer?.widthTracksTextView = true
            }
        }
        f.minSize = NSSize(width: 0, height: scroller.contentSize.height)
        f.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        f.setFrameSize(NSSize(width: scroller.contentSize.width, height: max(f.frame.height, scroller.contentSize.height)))
    }
    func textView(_ textView: NSTextView, shouldChangeTextIn affectedCharRange: NSRange, replacementString: String?) -> Bool {
        TextInputLimit.allows(textView.string, range: affectedCharRange, replacement: replacementString ?? "", props: props)
    }
    func textDidChange(_ notification: Notification) {
        guard let f = textArea else { return }
        if let editor = (f as? TextArea)?.markup, editor.applying || editor.styling { return }
        f.needsDisplay = true
        if !disabled { presenter?.typed(id, f.string, input: handlers.contains("input")) }
        if !f.hasMarkedText(), let held = pendingValue { writeValue(held, into: f) }
        configureMarkup()
        restyleMarkup()
        publishMarkupSelection()
    }
    func textDidBeginEditing(_ notification: Notification) {
        presenter?.collections.pinsChanged()
        publishMarkupSelection(force: true)
    }
    func textViewDidChangeSelection(_ notification: Notification) {
        guard let f = textArea as? TextArea, let editor = f.markup, !editor.applying, !editor.styling, !f.hasMarkedText() else { return }
        if f.window?.firstResponder === f { editor.bookmark = f.selectedRange() }
        restyleMarkup()
        publishMarkupSelection()
    }
    func textDidEndEditing(_ notification: Notification) {
        presenter?.collections.pinsChanged()
        textAreaScroll?.noteFocusRingMaskChanged()
        presenter?.commitEdit(id, textArea?.string ?? "", change: handlers.contains("change"))
        if handlers.contains("blur") { presenter?.blur(id) }
    }
}

private var heldSubstitutions: UInt8 = 0

extension NSTextView {
    /// No correction keeps the text as typed, as HTML's `autocorrect="off"`
    /// asks (#111): no smart quotes or dashes and no text replacement, which
    /// AppKit would otherwise apply after a pause in typing. What the view
    /// had (AppKit's, or the person's from the Substitutions menu) is held,
    /// and given back once correction is allowed again: for the window's one
    /// field editor, when a field that allows it takes the focus.
    func keepsTypedText(_ keep: Bool) {
        let held = objc_getAssociatedObject(self, &heldSubstitutions) as? [Bool]
        if keep, held == nil {
            objc_setAssociatedObject(self, &heldSubstitutions, [isAutomaticQuoteSubstitutionEnabled, isAutomaticDashSubstitutionEnabled, isAutomaticTextReplacementEnabled], .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
            isAutomaticQuoteSubstitutionEnabled = false
            isAutomaticDashSubstitutionEnabled = false
            isAutomaticTextReplacementEnabled = false
        } else if !keep, let held {
            objc_setAssociatedObject(self, &heldSubstitutions, nil, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
            isAutomaticQuoteSubstitutionEnabled = held[0]
            isAutomaticDashSubstitutionEnabled = held[1]
            isAutomaticTextReplacementEnabled = held[2]
        }
    }
}

/// An input's field: `focus` when it takes the focus (its field editor then
/// edits it), as the web fires it, not at its first edit
/// (`controlTextDidBeginEditing`; jukebox F23). `blur` is
/// `controlTextDidEndEditing`, which AppKit sends whenever the editor leaves.
/// Its cell serves the node's ARIA attributes (`FieldCell`, Accessibility.swift).
final class Field: NSTextField {
    override class var cellClass: AnyClass? { get { FieldCell.self } set {} }
    override func becomeFirstResponder() -> Bool { focused(delegate) { super.becomeFirstResponder() } }
    override var focusRingMaskBounds: NSRect { bareFieldMask(self) ?? super.focusRingMaskBounds }
    override func drawFocusRingMask() { if !drawBareFieldMask(self) { super.drawFocusRingMask() } }
}
/// The editor an input's field takes while the node or an ancestor hears
/// `copy`, `cut` or `paste` (`FieldCell.fieldEditor(for:)`): the window's
/// shared one, which fires none of them, otherwise.
final class FieldEditor: NSTextView {
    var owner: NodeView? { (delegate as? NSTextField)?.delegate as? NodeView }
    override func copy(_ sender: Any?) { NodeView.fieldEdit(owner, #selector(NodeView.copy(_:))) { super.copy(sender) } }
    override func cut(_ sender: Any?) { NodeView.fieldEdit(owner, #selector(NodeView.cut(_:))) { super.cut(sender) } }
    override func paste(_ sender: Any?) { NodeView.fieldEdit(owner, #selector(NodeView.paste(_:))) { super.paste(sender) } }
}
final class SecureField: NSSecureTextField {
    override class var cellClass: AnyClass? { get { SecureFieldCell.self } set {} }
    override func becomeFirstResponder() -> Bool { focused(delegate) { super.becomeFirstResponder() } }
    override var focusRingMaskBounds: NSRect { bareFieldMask(self) ?? super.focusRingMaskBounds }
    override func drawFocusRingMask() { if !drawBareFieldMask(self) { super.drawFocusRingMask() } }
}
private func bareFieldMask(_ field: NSTextField) -> NSRect? {
    guard let owner = field.delegate as? NodeView, !owner.isNativeTextControl else { return nil }
    return field.convert(owner.bounds, from: owner)
}
private func drawBareFieldMask(_ field: NSTextField) -> Bool {
    guard let owner = field.delegate as? NodeView, !owner.isNativeTextControl else { return false }
    let origin = field.convert(NSPoint.zero, from: owner)
    NSGraphicsContext.saveGraphicsState()
    (AffineTransform(translationByX: origin.x, byY: origin.y) as NSAffineTransform).concat()
    owner.roundedPath(in: owner.bounds).fill()
    NSGraphicsContext.restoreGraphicsState()
    return true
}
/// The field editor selects the whole value as it takes a field, which is
/// no `select` of the person's; a selection a script set while the field
/// had no focus is shown instead (x2apps codeedit #2).
private func focused(_ delegate: NSTextFieldDelegate?, _ become: () -> Bool) -> Bool {
    let owner = delegate as? NodeView, selections = owner?.presenter?.fieldSelections
    let ok = selections?.quietly(become) ?? become()
    if ok, let owner { selections?.focused(owner) }
    // The window's one field editor still has the last field's checking.
    if ok, let owner, let editor = owner.field?.currentEditor() as? NSTextView { owner.styleFieldEditor(editor) }
    if ok, let owner, owner.handlers.contains("focus") { owner.presenter?.focus(owner.id) }
    return ok
}
#endif

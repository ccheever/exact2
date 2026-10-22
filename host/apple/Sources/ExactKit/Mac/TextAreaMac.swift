// Contract textarea: native multiline editing, with the same change seam as input.
#if os(macOS)
import AppKit

package final class TextArea: NSTextView {
    // Each editor owns its native undo history. An authoritative external
    // source update can reset this editor without clearing another field's
    // actions from a window-wide manager.
    private lazy var textUndo = NativeTextUndo(before: { [weak self] in
        self?.markup?.applying = true
    }, after: { [weak self] in
        guard let self else { return }
        if let editor = markup {
            editor.applying = false
            editor.bookmark = selectedRange()
        }
        owner?.textDidChange(Notification(name: NSText.didChangeNotification, object: self))
    })
    package override var undoManager: UndoManager? { textUndo.manager }

    weak var owner: NodeView?
    package var markup: MarkdownEditing?
    /// A plain textarea's painted decorations (LLP 1114.007).
    var decorations: FieldDecorations?
    var nativeOrigin: NSPoint?
    /// An edit is under way: the selection it moves is the edit's, not the
    /// person's, and stays where the edit puts it (`MarkupHidden.swift`).
    var changingText = false
    /// The host is moving the caret across a hidden link destination.
    var placingCaret = false
    /// An input method is marking text: its edit stays the platform's.
    var markingText = false
    /// The source's length at the last selection change: a change that
    /// comes with a new length is an edit's.
    var markupLength = 0
    // Typed text never takes a link's marks from the character before the
    // caret (its closing parenthesis, or its `[`).
    package override var typingAttributes: [NSAttributedString.Key: Any] {
        get { super.typingAttributes }
        set { var shown = newValue; shown[.markupHidden] = nil; shown[.markupOpener] = nil; super.typingAttributes = shown }
    }
    // AppKit asks the first responder for the mask: the native bezel's
    // scroll view, or the bare node's whole authored box.
    package override var focusRingMaskBounds: NSRect {
        guard let owner else { return super.focusRingMaskBounds }
        if !owner.isNativeTextControl { return convert(owner.focusRingMaskBounds, from: owner) }
        guard let scroll = owner.textAreaScroll else { return super.focusRingMaskBounds }
        return convert(scroll.bounds, from: scroll)
    }
    package override func drawFocusRingMask() {
        guard let owner else { super.drawFocusRingMask(); return }
        let mask: NSView
        if owner.isNativeTextControl {
            guard let scroll = owner.textAreaScroll else { return }
            mask = scroll
        } else { mask = owner }
        NSGraphicsContext.saveGraphicsState()
        let origin = convert(NSPoint.zero, from: mask)
        let transform = AffineTransform(translationByX: origin.x, byY: origin.y)
        (transform as NSAffineTransform).concat()
        mask.drawFocusRingMask()
        NSGraphicsContext.restoreGraphicsState()
    }
    package override var textContainerOrigin: NSPoint { nativeOrigin ?? super.textContainerOrigin }

    // The ARIA states AppKit has no property for (`NodeView.ariaAttribute`).
    package override func accessibilityAttributeNames() -> [NSAccessibility.Attribute] {
        super.accessibilityAttributeNames() + NodeView.ariaAttributes.filter { owner?.ariaAttribute($0) != nil }.map { .init(rawValue: $0) }
    }
    package override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        if attribute == .value, markup != nil { return accessibilityValue() }
        return owner?.ariaAttribute(attribute.rawValue) ?? super.accessibilityAttributeValue(attribute)
    }
    // VoiceOver reads the text without hidden link destinations (LLP 1045
    // D1); its ranges stay source ranges, as the selection's are.
    package override func accessibilityValue() -> String? {
        guard markup != nil, let storage = textStorage else { return super.accessibilityValue() }
        return storage.shownText(in: NSRange(location: 0, length: storage.length))
    }
    package override func accessibilityString(for range: NSRange) -> String? {
        guard markup != nil, let storage = textStorage, NSMaxRange(range) <= storage.length else { return super.accessibilityString(for: range) }
        return storage.shownText(in: range)
    }
    package override func accessibilitySelectedText() -> String? {
        guard markup != nil, let storage = textStorage, NSMaxRange(selectedRange()) <= storage.length else { return super.accessibilitySelectedText() }
        return storage.shownText(in: selectedRange())
    }
    package override func accessibilityAttributedString(for range: NSRange) -> NSAttributedString? {
        let full = super.accessibilityAttributedString(for: range)
        guard markup != nil, let storage = textStorage, let full, full.length == range.length, NSMaxRange(range) <= storage.length,
              storage.hidesSource(in: range) else { return full }
        let shown = NSMutableAttributedString(attributedString: full)
        storage.enumerateAttribute(.markupHidden, in: range, options: .reverse) { value, run, _ in
            if value != nil { shown.deleteCharacters(in: NSRange(location: run.location - range.location, length: run.length)) }
        }
        return shown
    }

    package override func resignFirstResponder() -> Bool {
        if let markup, !hasMarkedText() { markup.bookmark = selectedRange() }
        let ok = super.resignFirstResponder()
        if ok { noteFocusRingMaskChanged(); owner?.textAreaScroll?.noteFocusRingMaskChanged() }
        return ok
    }
    /// `focus` as the web fires it: when the editor takes the focus, not at
    /// its first edit (`textDidBeginEditing`; jukebox F23). `blur` is
    /// `textDidEndEditing`, which AppKit posts whenever the focus leaves.
    package override func becomeFirstResponder() -> Bool {
        let ok = super.becomeFirstResponder()
        // A selection a script set while it had no focus (x2apps codeedit #2).
        if ok, let owner { owner.presenter?.fieldSelections.focused(owner) }
        if ok { owner?.showFieldFocus(true) }
        if ok { noteFocusRingMaskChanged(); owner?.textAreaScroll?.noteFocusRingMaskChanged() }
        if ok, let owner { owner.presenter?.focus(owner.id) }
        return ok
    }

    package override func doCommand(by selector: Selector) {
        let types = ["deleteBackward:": "deleteContentBackward", "deleteForward:": "deleteContentForward", "deleteWordBackward:": "deleteWordBackward", "deleteWordForward:": "deleteWordForward", "deleteToBeginningOfLine:": "deleteSoftLineBackward", "deleteToEndOfLine:": "deleteSoftLineForward", "deleteToBeginningOfParagraph:": "deleteHardLineBackward", "deleteToEndOfParagraph:": "deleteHardLineForward"]
        let name = NSStringFromSelector(selector)
        crossHidden(backward: Self.backwardSteps.contains(name), forward: Self.forwardSteps.contains(name))
        if let type = types[name], let fields = owner?.presenter?.fieldSelections {
            fields.withIntent(owner, type) { super.doCommand(by: selector) }
        } else { super.doCommand(by: selector) }
    }
    private static let backwardSteps: Set = ["moveLeft:", "moveBackward:", "moveWordLeft:", "moveWordBackward:", "deleteBackward:", "deleteWordBackward:",
                                             "deleteBackwardByDecomposingPreviousCharacter:", "deleteToBeginningOfLine:", "deleteToBeginningOfParagraph:"]
    private static let forwardSteps: Set = ["moveRight:", "moveForward:", "moveWordRight:", "moveWordForward:", "deleteForward:", "deleteWordForward:",
                                            "deleteToEndOfLine:", "deleteToEndOfParagraph:"]
    /// A caret at a hidden link destination's edge steps, or deletes, from
    /// its other edge, so the step is over visible text (LLP 1045 D5).
    private func crossHidden(backward: Bool, forward: Bool) {
        guard backward || forward, let markup, !hasMarkedText(), let storage = textStorage, selectedRange().length == 0,
              let from = markup.crossing(selectedRange().location, backward: backward, in: storage) else { return }
        placingCaret = true
        defer { placingCaret = false }
        let place = { self.setSelectedRange(NSRange(location: from, length: 0)) }
        if let fields = owner?.presenter?.fieldSelections { fields.quietly(place) } else { place() }
    }
    package override func shouldChangeText(in affectedCharRange: NSRange, replacementString: String?) -> Bool {
        // Asked before `super`, which registers the native edit's undo: the
        // editor's own edit replaces it and registers its own.
        if let markup, !markup.applying, !hasMarkedText(), !markingText, let text = replacementString, let storage = textStorage,
           let rewrite = markup.rewrite(affectedCharRange, with: text, caret: selectedRange().length == 0 ? selectedRange().location : nil, in: storage) {
            if let owner, owner.presenter?.fieldSelections.shouldEdit(owner, range: affectedCharRange, replacement: text, marked: false) == false { return false }
            apply(rewrite, markup)
            return false
        }
        guard super.shouldChangeText(in: affectedCharRange, replacementString: replacementString) else { return false }
        guard let owner else { return true }
        guard owner.presenter?.fieldSelections.shouldEdit(owner, range: affectedCharRange, replacement: replacementString ?? "", marked: hasMarkedText()) ?? true else { return false }
        changingText = true
        DispatchQueue.main.async { [weak self] in self?.changingText = false }
        return true
    }
    /// The Markdown editor's edit in place of the native one, as one undo
    /// step through native insertion.
    private func apply(_ rewrite: MarkupRewrite, _ markup: MarkdownEditing) {
        markup.applying = true
        breakUndoCoalescing()
        undoManager?.beginUndoGrouping()
        // Each part its own native edit: typing coalescing would merge two
        // apart into one range that undo restores wrongly.
        for edit in rewrite.edits.reversed() {
            insertText(edit.text, replacementRange: edit.range)
            breakUndoCoalescing()
        }
        if let caret = rewrite.caret { setSelectedRange(NSRange(location: caret, length: 0)) }
        undoManager?.endUndoGrouping()
        breakUndoCoalescing()
        markup.applying = false
        if !rewrite.edits.isEmpty { owner?.textDidChange(Notification(name: NSText.didChangeNotification, object: self)) }
    }
    /// AppKit posts no change while text is marked: the owner hears each one
    /// here, and a commit's steps as one (`settlingComposition`).
    package override func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        // A composition that starts over a selection holding a hidden link
        // destination first deletes what the selection shows, as the editor
        // deletes it, and then composes at the caret.
        if let markup, !markup.applying, !hasMarkedText(), replacementRange.location == NSNotFound, let storage = textStorage,
           self.selectedRange().length > 0, let rewrite = markup.rewrite(self.selectedRange(), with: "", caret: nil, in: storage) {
            // The app hears the deletion as its own input first.
            let fields = owner?.presenter?.fieldSelections, range = self.selectedRange()
            let heard = fields.map { fields in fields.withIntent(owner, "deleteByComposition") {
                fields.shouldEdit(owner!, range: range, replacement: "", marked: false)
            } } ?? true
            if heard { apply(rewrite, markup) }
        }
        markingText = true
        defer { markingText = false }
        if let selections = owner?.presenter?.fieldSelections {
            selections.withComposition(owner) { super.setMarkedText(string, selectedRange: selectedRange, replacementRange: replacementRange) }
        } else { super.setMarkedText(string, selectedRange: selectedRange, replacementRange: replacementRange) }
        owner?.markedTextChanged(self)
    }
    package override func unmarkText() {
        guard let owner else { super.unmarkText(); return }
        owner.settlingComposition(self) { super.unmarkText() }
        // A commit that changes no text posts no change: paint held back
        // while it was marked catches up here.
        owner.restyleDecorations()
    }

    /// Tab and Shift-Tab move the focus, the Markdown editor's too, as the
    /// web's textarea does; an app that wants a tab or an indent prevents
    /// the `key` and calls `setRangeText` (LLP 1045 D6).
    package override func insertTab(_ sender: Any?) { window?.selectNextKeyView(self) }
    package override func insertBacktab(_ sender: Any?) { window?.selectPreviousKeyView(self) }

    /// Return: the app's `beforeinput` first, then the Markdown editor's
    /// list continuation, else a line break (LLP 1045 D6).
    package override func insertNewline(_ sender: Any?) {
        guard markup != nil, !hasMarkedText(), let owner else { super.insertNewline(sender); return }
        let line = { if !owner.formatMarkup("newline") { super.insertNewline(sender) } }
        if let fields = owner.presenter?.fieldSelections { fields.nativeEdit(owner, replacement: "\n", marked: false, line) } else { line() }
    }

    package override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard window?.firstResponder === self else { return super.performKeyEquivalent(with: event) }
        // An embedded/agent window need not own NSApp.keyWindow, so the
        // menu's responder lookup may miss its editor. Route the ordinary
        // undo chord straight to this text view's native manager.
        let modifiers = event.modifierFlags.intersection([.command, .shift, .control, .option])
        if isEditable, !hasMarkedText(), event.charactersIgnoringModifiers?.lowercased() == "z" {
            if modifiers == .command { undo(nil); return true }
            if modifiers == [.command, .shift] { redo(nil); return true }
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

    // NSTextView does not implement these responder actions. The Edit menu's
    // nil target must reach this editor's history, rather than NSWindow's.
    @objc package func undo(_ sender: Any?) {
        if isEditable, !hasMarkedText(), undoManager?.canUndo == true {
            if let fields = owner?.presenter?.fieldSelections { fields.history(owner, redo: false) { undoManager?.undo() } }
            else { undoManager?.undo() }
        }
    }
    @objc package func redo(_ sender: Any?) {
        if isEditable, !hasMarkedText(), undoManager?.canRedo == true {
            if let fields = owner?.presenter?.fieldSelections { fields.history(owner, redo: true) { undoManager?.redo() } }
            else { undoManager?.redo() }
        }
    }
    package override func validateMenuItem(_ item: NSMenuItem) -> Bool {
        if item.action == #selector(undo(_:)) { return isEditable && !hasMarkedText() && undoManager?.canUndo == true }
        if item.action == #selector(redo(_:)) { return isEditable && !hasMarkedText() && undoManager?.canRedo == true }
        if owner?.pastesFiles(item) == true { return isEditable }
        return super.validateMenuItem(item)
    }

    @objc package func copyPlainText(_ sender: Any?) {
        guard markup != nil, selectedRange().length > 0,
              let plain = MarkdownLink.installed?.plain((string as NSString).substring(with: selectedRange())) else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(plain, forType: .string)
    }
    // DOM's clipboard events, before the editor's own (#125, Clipboard.swift).
    package override func copy(_ sender: Any?) {
        NodeView.fieldEdit(owner, #selector(NodeView.copy(_:))) {
            guard let markup, let storage = textStorage else { super.copy(sender); return }
            let selection = markup.copyRange(selectedRange(), in: storage)
            guard selection.length > 0 else { return }
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString((string as NSString).substring(with: selection), forType: .string)
        }
    }

    package override func cut(_ sender: Any?) { NodeView.fieldEdit(owner, #selector(NodeView.cut(_:))) { super.cut(sender) } }
    package override func paste(_ sender: Any?) { NodeView.fieldPaste(owner, self) { super.paste(sender) } }
    package override func pasteAsPlainText(_ sender: Any?) { NodeView.fieldPaste(owner, self) { super.pasteAsPlainText(sender) } }

    var placeholder = "" { didSet { needsDisplay = true } }
    /// The authored `::placeholder` colour (LLP 1104 D3); nil, the platform's.
    var placeholderColor: NSColor? { didSet { if placeholderColor != oldValue { needsDisplay = true } } }
    package override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        decorations?.drawLines(in: self)
        if string.isEmpty, !placeholder.isEmpty {
            let origin = textContainerOrigin
            let rect = NSRect(x: origin.x, y: origin.y, width: textContainer?.containerSize.width ?? bounds.width, height: bounds.height - origin.y)
            (placeholder as NSString).draw(in: rect, withAttributes: [
                .font: font ?? NSFont.systemFont(ofSize: NSFont.systemFontSize),
                .foregroundColor: placeholderColor ?? (owner?.isNativeTextControl == true ? ThemeRoles.color("placeholder", in: self, or: NSColor.placeholderTextColor) : (textColor ?? SystemColor.canvasText).withAlphaComponent(0.3)),
            ])
        }
    }
}

extension NodeView {
    /// The caret: the author's `caret-color`, else AppKit's own (LLP 1115.000
    /// D7.9). CSS's `auto` is the text's colour, and a bare field's caret was
    /// that; a Mac's is the accent. Measured on macOS 27.0, light and dark:
    /// a plain `NSTextView`'s and an `NSTextField`'s editor's
    /// `insertionPointColor`, and `NSTextInsertionIndicator`'s colour, are
    /// `textInsertionPointColor`, which is the accent the person chose.
    var caretColor: NSColor {
        color("caret_color", color("accent_color", .textInsertionPointColor))
    }

    /// `autocorrect` as the author wrote it; nil when they said nothing (the
    /// prop is then absent: it is a prop, not a style row, so there is no
    /// written bit to read). An email, URL or password input says by its
    /// type that its text is not prose.
    var inputCorrection: Bool? {
        if textArea == nil, ["email", "url", "password"].contains((props["type"] ?? "").lowercased()) { return false }
        return props["autocorrect"].map { $0.lowercased() != "off" }
    }

    /// `spellcheck` as the author wrote it; nil when they said nothing.
    var inputSpellChecking: Bool? { props["spellcheck"].map { $0.lowercased() != "false" } }

    /// What `autocorrect` and `spellcheck` ask of an editor: a textarea's
    /// text view or an input's field editor. Only what the author wrote is
    /// written (LLP 1115.000 D7.7). Unsaid, both are the person's: what
    /// AppKit gave the editor from System Settings (Correct spelling
    /// automatically) and what they chose since in Edit ▸ Spelling and
    /// Grammar, which a write at every update used to undo.
    func applyTextChecking(_ v: NSTextView) {
        v.writes(\.isAutomaticSpellingCorrectionEnabled, inputCorrection, held: &heldCorrection)
        v.writes(\.isContinuousSpellCheckingEnabled, inputSpellChecking, held: &heldSpelling)
        v.keepsTypedText(inputCorrection == false)
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
    /// The app's value into the editor: the changed middle only, with the
    /// selection carried through (LLP 1045 D5). The value the editor already
    /// shows changes nothing, so a composition and its selection stand. A
    /// different one is held while text is marked, the last one written,
    /// and applied when the composition ends (`applyHeldValue`,
    /// `FieldComposition.swift`). The write is not typing, so the app does
    /// not hear it as `input`.
    func writeValue(_ value: String, into f: NSTextView) {
        if f.string.utf16.elementsEqual(value.utf16) { pendingValue = nil; return }
        if f.hasMarkedText() { pendingValue = value; return }
        pendingValue = nil
        writingValue = true
        defer { writingValue = false; toldText = nil }
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
            if f.markup == nil { f.markup = MarkdownLink.installed?.editor() }
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
        restyleDecorations()
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
        f.font = t.font(size: number("font_size", PageFacts.defaultRootFontSize), weight: Int(number("font_weight", 400)), family: Int(number("font_family")), italic: (style["font_style"]?.string) == "italic", numeric: Int(number("font_variant_numeric")),
            stretch: CGFloat(number("font_stretch", 100)), variations: FontAxis.of(style["font_variation_settings"]))
        f.textColor = fieldInk
        f.insertionPointColor = caretColor
        (f as? TextArea)?.placeholderColor = placeholderColor
        let native = isNativeTextControl
        textAreaScroll?.borderType = native ? .bezelBorder : .noBorder
        textAreaScroll?.focusRingType = native && !outlineNone ? .default : .none
        textAreaScroll?.drawsBackground = native
        textAreaScroll?.backgroundColor = ThemeRoles.color("Field", in: self, or: .controlBackgroundColor)
        f.drawsBackground = native
        f.backgroundColor = ThemeRoles.color("Field", in: self, or: .textBackgroundColor)
        f.focusRingType = outlineNone ? .none : native ? .default : .exterior
        restyleFieldFocus()
        f.textContainerInset = native ? FieldChromeCache.textareaInset(for: effectiveAppearance, scale: window?.backingScaleFactor ?? 1) : .zero
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
            if isNativeTextControl, let rect = kernelContentBox {
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
    package func textView(_ textView: NSTextView, shouldChangeTextIn affectedCharRange: NSRange, replacementString: String?) -> Bool {
        TextInputLimit.allows(textView.string, range: affectedCharRange, replacement: replacementString ?? "", props: props)
    }
    package func textDidChange(_ notification: Notification) {
        guard let f = textArea else { return }
        if let editor = (f as? TextArea)?.markup, editor.applying || editor.styling { return }
        f.needsDisplay = true
        // What AppKit says of the app's own value going in, or of a
        // composition's commit in its steps, is not typing.
        if !disabled, !writingValue { typedText(f.string) }
        if !writingValue { applyHeldValue(to: f) }
        configureMarkup()
        restyleMarkup()
        publishMarkupSelection()
    }
    package func textDidBeginEditing(_ notification: Notification) {
        presenter?.collections.pinsChanged()
        publishMarkupSelection(force: true)
    }
    /// A person's selection beside a hidden link destination (LLP 1045 D5).
    package func textView(_ textView: NSTextView, willChangeSelectionFromCharacterRange old: NSRange, toCharacterRange new: NSRange) -> NSRange {
        guard let f = textView as? TextArea, let editor = f.markup, let storage = f.textStorage else { return new }
        let edited = f.markupLength != storage.length
        f.markupLength = storage.length
        guard !edited, !editor.applying, !editor.styling, !f.changingText, !f.placingCaret, !f.hasMarkedText(),
              NSMaxRange(new) <= storage.length else { return new }
        return editor.place(new, from: old, in: storage)
    }
    package func textViewDidChangeSelection(_ notification: Notification) {
        guard let f = textArea as? TextArea, let editor = f.markup, !editor.applying, !editor.styling, !f.hasMarkedText() else { return }
        if f.window?.firstResponder === f { editor.bookmark = f.selectedRange() }
        restyleMarkup()
        publishMarkupSelection()
    }
    package func textDidEndEditing(_ notification: Notification) {
        presenter?.collections.pinsChanged()
        showFieldFocus(false)
        textAreaScroll?.noteFocusRingMaskChanged()
        presenter?.commitEdit(id, textArea?.string ?? "", change: handlers.contains("change"))
        presenter?.blur(id)
    }
}

private var heldSubstitutions: UInt8 = 0
private var heldCorrection: UInt8 = 0
private var heldSpelling: UInt8 = 0

extension NSTextView {
    /// A switch the author may have written. While they say what it is, it
    /// is that, and what the view had (AppKit's, or the person's) is held;
    /// when they say nothing it is given back and then left alone. The
    /// window's one field editor serves every field, so a field that says
    /// nothing must not keep the last field's answer.
    func writes(_ path: ReferenceWritableKeyPath<NSTextView, Bool>, _ value: Bool?, held key: UnsafeRawPointer) {
        let held = objc_getAssociatedObject(self, key) as? Bool
        if let value {
            if held == nil { objc_setAssociatedObject(self, key, self[keyPath: path], .OBJC_ASSOCIATION_RETAIN_NONATOMIC) }
            if self[keyPath: path] != value { self[keyPath: path] = value }
        } else if let held {
            objc_setAssociatedObject(self, key, nil, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
            if self[keyPath: path] != held { self[keyPath: path] = held }
        }
    }

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
    /// A `type="number"` input's node: the field is AppKit's stepper (incrementor) to accessibility.
    var numberOwner: NodeView? {
        guard let node = delegate as? NodeView, node.props["type"]?.lowercased() == "number" else { return nil }
        return node
    }
    /// AppKit serves the field's protocol role and its cell's legacy attributes.
    override func accessibilityRole() -> NSAccessibility.Role? {
        if comboOwner != nil { return .comboBox }
        return numberOwner != nil ? .incrementor : cell?.accessibilityRole() ?? super.accessibilityRole()
    }
    override func isAccessibilityExpanded() -> Bool {
        guard let node = comboOwner else { return super.isAccessibilityExpanded() }
        return node.props["accessibilityExpanded"] == "true"
    }
    override func accessibilityMinValue() -> Any? {
        guard let owner = numberOwner else { return super.accessibilityMinValue() }
        return NumberStepping(owner.props).min.map { NSDecimalNumber(decimal: $0) }
    }
    override func accessibilityMaxValue() -> Any? {
        guard let owner = numberOwner else { return super.accessibilityMaxValue() }
        return NumberStepping(owner.props).max.map { NSDecimalNumber(decimal: $0) }
    }
    override func accessibilityPerformIncrement() -> Bool { numberOwner?.step(up: true) ?? super.accessibilityPerformIncrement() }
    override func accessibilityPerformDecrement() -> Bool { numberOwner?.step(up: false) ?? super.accessibilityPerformDecrement() }
    override class var cellClass: AnyClass? { get { FieldCell.self } set {} }
    override func becomeFirstResponder() -> Bool { focused(delegate) { super.becomeFirstResponder() } }
    override var focusRingMaskBounds: NSRect { bareFieldMask(self) ?? super.focusRingMaskBounds }
    override func drawFocusRingMask() { if !drawBareFieldMask(self) { super.drawFocusRingMask() } }
}
/// The editor an `input`'s field edits in (`FieldCell.fieldEditor(for:)`):
/// one for the session's fields, as a window has one for its own, so what
/// a person turns on in one field (Edit ▸ Spelling and Grammar) is on in the
/// next, as in any AppKit window. It is not the window's own because it
/// tells the field's node what that one tells nobody: each change of the
/// text an input method is composing (`markedTextChanged`), and a `copy`,
/// `cut` or `paste` before it happens, for the node or an ancestor that
/// hears one (#125).
final class FieldEditor: NSTextView {
    override func doCommand(by selector: Selector) {
        let types = ["deleteBackward:": "deleteContentBackward", "deleteForward:": "deleteContentForward", "deleteWordBackward:": "deleteWordBackward", "deleteWordForward:": "deleteWordForward", "deleteToBeginningOfLine:": "deleteSoftLineBackward", "deleteToEndOfLine:": "deleteSoftLineForward", "deleteToBeginningOfParagraph:": "deleteHardLineBackward", "deleteToEndOfParagraph:": "deleteHardLineForward"]
        if let type = types[NSStringFromSelector(selector)], let fields = owner?.presenter?.fieldSelections {
            fields.withIntent(owner, type) { super.doCommand(by: selector) }
        } else { super.doCommand(by: selector) }
    }
    @objc func undo(_ sender: Any?) {
        guard isEditable, !hasMarkedText(), undoManager?.canUndo == true else { return }
        if let fields = owner?.presenter?.fieldSelections { fields.history(owner, redo: false) { undoManager?.undo() } }
        else { undoManager?.undo() }
    }
    @objc func redo(_ sender: Any?) {
        guard isEditable, !hasMarkedText(), undoManager?.canRedo == true else { return }
        if let fields = owner?.presenter?.fieldSelections { fields.history(owner, redo: true) { undoManager?.redo() } }
        else { undoManager?.redo() }
    }
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        let modifiers = event.modifierFlags.intersection([.command, .shift, .control, .option])
        if window?.firstResponder === self, event.charactersIgnoringModifiers?.lowercased() == "z" {
            if modifiers == .command { undo(nil); return true }
            if modifiers == [.command, .shift] { redo(nil); return true }
        }
        return super.performKeyEquivalent(with: event)
    }

    static func make() -> FieldEditor {
        let editor = FieldEditor(frame: .zero)
        editor.isFieldEditor = true
        return editor
    }
    var owner: NodeView? { (delegate as? NSTextField)?.delegate as? NodeView }
    override func readSelection(from pasteboard: NSPasteboard, type: NSPasteboard.PasteboardType) -> Bool {
        guard owner?.props["type"]?.lowercased() == "number" else {
            return super.readSelection(from: pasteboard, type: type)
        }
        let rich = pasteboard.readObjects(forClasses: [NSAttributedString.self], options: nil)?.first as? NSAttributedString
        guard let text = pasteboard.string(forType: .string) ?? rich?.string else { return false }
        insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
        return true
    }
    override func insertText(_ string: Any, replacementRange: NSRange) {
        guard owner?.props["type"]?.lowercased() == "number",
              let text = (string as? NSAttributedString)?.string ?? string as? String else {
            super.insertText(string, replacementRange: replacementRange); return
        }
        let range = replacementRange.location == NSNotFound ? (hasMarkedText() ? markedRange() : selectedRange()) : replacementRange
        let filtered = NumberStepping.insertion(text, in: self.string, range: range)
        if !text.isEmpty, filtered.isEmpty, !hasMarkedText() { return }
        super.insertText(filtered, replacementRange: replacementRange)
    }
    override func shouldChangeText(in affectedCharRange: NSRange, replacementString: String?) -> Bool {
        guard super.shouldChangeText(in: affectedCharRange, replacementString: replacementString) else { return false }
        guard let owner else { return true }
        return owner.presenter?.fieldSelections.shouldEdit(owner, range: affectedCharRange, replacement: replacementString ?? "", marked: hasMarkedText()) ?? true
    }
    override func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        if let selections = owner?.presenter?.fieldSelections {
            selections.withComposition(owner) { super.setMarkedText(string, selectedRange: selectedRange, replacementRange: replacementRange) }
        } else { super.setMarkedText(string, selectedRange: selectedRange, replacementRange: replacementRange) }
        owner?.markedTextChanged(self)
    }
    override func unmarkText() {
        guard let owner else { super.unmarkText(); return }
        owner.settlingComposition(self) { super.unmarkText() }
    }
    override func copy(_ sender: Any?) { NodeView.fieldEdit(owner, #selector(NodeView.copy(_:))) { super.copy(sender) } }
    override func cut(_ sender: Any?) { NodeView.fieldEdit(owner, #selector(NodeView.cut(_:))) { super.cut(sender) } }
    override func paste(_ sender: Any?) { NodeView.fieldPaste(owner, self) { super.paste(sender) } }
    override func pasteAsPlainText(_ sender: Any?) { NodeView.fieldPaste(owner, self) { super.pasteAsPlainText(sender) } }
    override func validateMenuItem(_ item: NSMenuItem) -> Bool { owner?.pastesFiles(item) == true ? isEditable : super.validateMenuItem(item) }
}
final class SecureField: NSSecureTextField {
    override class var cellClass: AnyClass? { get { SecureFieldCell.self } set {} }
    override func becomeFirstResponder() -> Bool { focused(delegate) { super.becomeFirstResponder() } }
    override var focusRingMaskBounds: NSRect { bareFieldMask(self) ?? super.focusRingMaskBounds }
    override func drawFocusRingMask() { if !drawBareFieldMask(self) { super.drawFocusRingMask() } }
}
extension NSTextField {
    var comboOwner: NodeView? {
        guard let node = delegate as? NodeView, node.props["accessibilityRole"] == "combobox" else { return nil }
        return node
    }
    func comboboxAttribute(_ attribute: NSAccessibility.Attribute) -> Any? {
        guard comboOwner != nil else { return nil }
        switch attribute {
        case .role: return NSAccessibility.Role.comboBox.rawValue
        case .roleDescription: return NSAccessibility.Role.comboBox.description(with: nil)
        case .expanded: return isAccessibilityExpanded()
        default: return nil
        }
    }
}
/// A native search input's field (`SearchFieldCell`, NativeFieldsMac.swift).
final class SearchField: NSSearchField {
    override func accessibilityRole() -> NSAccessibility.Role? {
        comboOwner != nil ? .comboBox : cell?.accessibilityRole() ?? super.accessibilityRole()
    }
    override func isAccessibilityExpanded() -> Bool {
        guard let node = comboOwner else { return super.isAccessibilityExpanded() }
        return node.props["accessibilityExpanded"] == "true"
    }
    override class var cellClass: AnyClass? { get { SearchFieldCell.self } set {} }
    override func becomeFirstResponder() -> Bool { focused(delegate) { super.becomeFirstResponder() } }
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
    if ok { owner?.showFieldFocus(true) }
    // The window's one field editor still has the last field's checking.
    if ok, let owner, let editor = owner.field?.currentEditor() as? NSTextView { owner.styleFieldEditor(editor) }
    if ok, let owner { owner.presenter?.focus(owner.id) }
    return ok
}
#endif

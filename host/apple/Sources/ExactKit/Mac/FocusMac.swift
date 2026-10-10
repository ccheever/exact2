// A node's focus as AppKit asks for it: who takes it and is a Tab stop, what
// taking and losing it tells the app, and the ring a focused pressable shows
// where Chrome's `:focus-visible` matches — after the keyboard, never after a
// pointer's press.
#if os(macOS)
import AppKit

extension NodeView {
    /// A node with focus, blur, or key handlers takes the focus (an input's
    /// field does by itself): the web's rule that only a focusable element
    /// hears these. A pressable is in the tab order the way a `<button>` is.
    /// A paragraph takes the focus too, for selection, but plain text is
    /// never a Tab stop on the web. An explicit `tabindex` makes any box
    /// focusable, and a Tab stop only when ≥ 0 (LLP 1088 D7.3).
    package override var acceptsFirstResponder: Bool {
        if formDisabled || inert || isHiddenOrHasHiddenAncestor || cssVisibilityHidden { return false }
        if field != nil || textArea != nil || isNativeButton { return false }
        return props["semanticTag"] == "dialog" || isParagraph || explicitTabIndex != nil || tabbable || isRadio
    }
    var tabbable: Bool {
        if let index = explicitTabIndex { return index >= 0 }
        return kind == "button" || isNativeButton || canvases?.wantsInput(id) == true || pressable || !handlers.isDisjoint(with: Self.focusEvents)
            || reorderKeys || radioTabStop // a grouped grip takes the keys (LLP 1094 D9); a radio group one stop (x2apps survey #2)
    }
    /// Sequential focus follows the web: a button is in the loop even when
    /// macOS "Keyboard navigation" is off (that setting would otherwise
    /// skip every non-field).
    package override var canBecomeKeyView: Bool { acceptsFirstResponder && tabbable }
    package override func becomeFirstResponder() -> Bool {
        guard !formDisabled else { return false }
        let ok = super.becomeFirstResponder()
        if ok { focusEntered() }
        return ok
    }
    package override func resignFirstResponder() -> Bool {
        let ok = super.resignFirstResponder()
        if ok { focusLeft() }
        return ok
    }
    // @ref LLP 1104 D6 — events are relayed from the node's single focus owner.
    func focusEntered() {
        focusVisible = !isNativeButton && presenter?.focusByPointer != true
        presenter?.collections.pinsChanged()
        presenter?.selection.focusEntered(self)
        if handlers.contains("focus") { presenter?.focus(id) }
    }
    func focusLeft() {
        focusVisible = false
        presenter?.selection.focusLeft()
        presenter?.collections.pinsChanged()
        if !isSurfaceControl { inputCanvas?.canvasInput?.blur() }
        if handlers.contains("blur") { presenter?.blur(id) }
    }
    var retainsFocus: Bool {
        sequence(first: self as NSView, next: \.superview).contains { ($0 as? NodeView)?.props["retainFocus"] == "true" }
    }
    /// A bare button is ringed even when only its ancestor handles the press.
    var ringsFocus: Bool { !isNativeButton && (isButton || pressable) }

    /// A focused pressable's ring, AppKit's exterior one around its rounded
    /// box, as Chrome outlines a focused `<button>` or `tabindex` box. AppKit
    /// draws no mask whose bounds are empty, which keeps it off a box that
    /// is not pressable, a field (its own ring) and a focus that is not
    /// visible.
    var bareFieldFocused: Bool {
        !isNativeTextControl && (field?.currentEditor() != nil || (textArea != nil && window?.firstResponder === textArea))
    }
    package override var focusRingMaskBounds: NSRect {
        bareFieldFocused || (field == nil && ringsFocus && focusVisible) ? bounds : .zero
    }
    package override func drawFocusRingMask() {
        guard bareFieldFocused || (field == nil && ringsFocus) else { return }
        roundedPath(in: bounds).fill()
    }
}

extension Presenter {
    /// A key that is not a shortcut chord (Tab and Shift-Tab included): the
    /// focus is visible from here, as Chrome's `:focus-visible` matches at a
    /// focus the keyboard moved, or used after a click had moved it.
    func keyboardUsed(_ event: NSEvent, in window: NSWindow?) {
        guard event.type == .keyDown, event.modifierFlags.intersection([.command, .control, .option]).isEmpty else { return }
        focusByPointer = false
        (window ?? event.window).flatMap { $0.firstResponder as? NodeView }?.focusVisible = true
    }
}
#endif

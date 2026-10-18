// A node's focus as AppKit asks for it: who takes it and is a Tab stop, whom
// a click gives it to (LLP 1115.000 D7.1: no button, no pressable, not the
// ground), what taking and losing it tells the app, and the ring a focused
// pressable shows after the keyboard moved the focus, never after a
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
        if field != nil || textArea != nil || isNativeButton || nativeValueControl != nil { return false }
        return props["semanticTag"] == "dialog" || isParagraph || explicitTabIndex != nil || tabbable || isRadio || ariaRadioGroup != nil
    }
    var tabbable: Bool {
        if let index = explicitTabIndex { return index >= 0 }
        if ariaRadioGroup != nil { return ariaRadioTabStop } // an authored radio group is one stop too
        return kind == "button" || isNativeButton || isDateOrSelect || canvases?.wantsInput(id) == true || pressable || !handlers.isDisjoint(with: Self.focusEvents)
            || reorderKeys || radioTabStop // a grouped grip takes the keys (LLP 1094 D9); a radio group one stop (x2apps survey #2)
    }
    /// Sequential focus honours macOS "Keyboard navigation" (LLP 1115.000
    /// D7.2; `tabStop`, KeyViewLoopMac.swift): off, a button is no key view,
    /// as an `NSButton` is none; a `tabindex` the author wrote still decides.
    package override var canBecomeKeyView: Bool { acceptsFirstResponder && tabStop }
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
        focusVisible = !isNativeButton && nativeValueControl == nil && presenter?.focusByPointer != true
        presenter?.collections.pinsChanged()
        presenter?.selection.focusEntered(self)
        inputCanvas?.canvasInput?.focused()
        presenter?.focus(id)
        // A row of an open menu taking the focus is its highlight, which
        // AppKit's menu tells as its selected child changing (Accessibility.swift).
        if isMenuItem, let menu = owningMenu, let presenter, presenter.shownMenus.contains(where: { $0 === menu }) {
            presenter.menusHeldFocus.add(menu)
            NSAccessibility.post(element: menu, notification: .selectedChildrenChanged)
        }
    }
    /// A shown `menu` the batch takes out of the window says it closed as it
    /// goes, while it is still an element to post on: once out, a post on it
    /// reaches no client (measured).
    package override func viewWillMove(toWindow newWindow: NSWindow?) {
        if newWindow == nil, window != nil, let presenter, let index = presenter.shownMenus.firstIndex(where: { $0 === self }) {
            presenter.shownMenus.remove(at: index)
            presenter.menuClosed(self)
        }
        super.viewWillMove(toWindow: newWindow)
    }
    func focusLeft() {
        focusVisible = false
        presenter?.selection.focusLeft()
        presenter?.collections.pinsChanged()
        if !isSurfaceControl { inputCanvas?.canvasInput?.blur() }
        presenter?.blur(id)
    }
    /// Dates and selects have the same single native focus owner as a field.
    /// An appearance:none control falls back to its authored node.
    var isDateOrSelect: Bool { kind == "control" && (props["type"] == "select" || ControlKinds.dates.contains(props["type"] ?? "")) }
    var nativeValueControl: NSControl? {
        guard isDateOrSelect, let control = presenter?.controls.controls[id], control.superview != nil else { return nil }
        return control
    }
    func acceptsNativeValueFocus(_ control: NSControl) -> Bool {
        control.isEnabled && !formDisabled && !inert && !control.isHiddenOrHasHiddenAncestor && !cssVisibilityHidden
    }
    /// Leave native editing keys to AppKit, but traverse Exact's explicit loop.
    func nativeValueTab(_ event: NSEvent, control: NSControl) -> Bool {
        guard Self.keyName(event) == "Tab", event.modifierFlags.intersection([.command, .control, .option]).isEmpty,
              let window = control.window else { return false }
        presenter?.tab(from: control, back: event.modifierFlags.contains(.shift), in: window)
        return true
    }
    var retainsFocus: Bool {
        sequence(first: self as NSView, next: \.superview).contains { ($0 as? NodeView)?.props["retainFocus"] == "true" }
    }
    /// Whether a click here moves the focus to this node (LLP 1115.000
    /// D7.1). On the Mac a click on a button, a radio or a box that is only
    /// pressable never does, whatever its `tabindex`: the first responder
    /// stays where it is, as it does for a clicked `NSButton`, and a field
    /// beside the button keeps its caret. What `retainFocus` asked for is so
    /// the default. A box the author made a taker of keys does take it, as
    /// AppKit focuses a clicked custom view that accepts first responder
    /// (and Safari a `tabindex` box): one that is no control with a
    /// `tabindex`, or with `focus`, `blur` or `key` handlers, or a canvas
    /// that takes input. There `retainFocus`, on it or above it, still keeps
    /// the focus where it was. An `autofocus` and an action's `focus()` move
    /// it to any of them.
    var takesClickFocus: Bool {
        acceptsFirstResponder && !retainsFocus
            && (explicitTabIndex != nil && !isButton && !isRadio && kind != "toggle" || takesKeysOfItsOwn)
    }
    /// How AppKit is told: before it hands a view the mouse-down, its window
    /// makes that view first responder when it accepts first responder and
    /// "needs its panel to become key" (measured on macOS 27.0: an `NSView`
    /// that accepts answers true, an `NSButton` false, which is why a click
    /// on a button moves no focus). A node answers as a button does unless a
    /// click gives it the focus. A paragraph that selects text, the one node
    /// that accepts and is asked here without taking keys, takes the focus
    /// itself as its selection begins (`mouseDown`).
    package override var needsPanelToBecomeKey: Bool { takesClickFocus }
    /// Whether a click on this node in a window that is not key is also a
    /// press (LLP 1115.000 D7.9). Every node used to answer true, a
    /// browser's rule. AppKit's answer, measured on macOS 27.0
    /// (`acceptsFirstMouse(for:)` of each): true for an `NSButton` (false
    /// once it has `hasDestructiveAction`, which a native `destructive`
    /// button keeps: `NativeButtonMac.acceptsFirstMouse`), an
    /// `NSSegmentedControl`, an `NSSwitch`, an `NSSlider`, an
    /// `NSPopUpButton`, an `NSTextField`, and for an `NSTableView`, an
    /// `NSOutlineView` and an `NSCollectionView`; false for an `NSView`, an
    /// `NSTextView`, an `NSImageView` and an `NSScrollView`. So a control
    /// clicks through, and a list's row does, as a table's row is selected
    /// by the click that brings its window forward; a card, a box, an image
    /// and the ground do not, and no text selection begins. A control here
    /// is a `button`, a native or surface control, or a node with a
    /// control's role, and what is inside one or inside a list's row.
    var clicksThrough: Bool {
        if selectsText { return false }
        var next: NSView? = self
        while let view = next {
            next = view.superview
            guard let node = view as? NodeView else { continue }
            if node.isButton || node.isSurfaceControl || node.kind == "control" || node.isListRow
                || Self.clickThroughRoles.contains(node.props["accessibilityRole"] ?? "") { return true }
        }
        return false
    }
    private static let clickThroughRoles: Set<String> = ["button", "tab", "checkbox", "radio", "switch", "menuitem", "menuitemcheckbox", "menuitemradio"]
    /// Every node takes the click that would activate its window: under the
    /// agent, whose window is never key and whose app is never active, so
    /// AppKit asks this of every click it sends. A test that sends a click
    /// through the application says so too, for the same reason.
    nonisolated(unsafe) static var everyClickCounts = ExactEnv.agentMode
    /// A bare button is ringed even when only its ancestor handles the press.
    var ringsFocus: Bool { !isNativeButton && (isButton || pressable) }

    /// A focused pressable's ring, AppKit's exterior one around its rounded
    /// box, as AppKit rings an `NSButton` the keyboard moved the focus to.
    /// AppKit draws no mask whose bounds are empty, which keeps it off a box
    /// that is not pressable, a field (its own ring) and a focus that is not
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
    /// A press acts on what the fields say: an edit a field has not reported
    /// yet (its `change`) is committed first, the field keeping its editor
    /// and its caret. A click on a button used to end the editing, and the
    /// blur committed it; now that the click leaves the first responder
    /// where it is (LLP 1115.000 D7.1) the commit is its own step, as a
    /// hand-built app commits its editors before a button acts
    /// (`NSEditor.commitEditing`). Text an input method is still composing
    /// is the input method's, and is left.
    func commitPendingEdit() {
        guard let id = edited, let node = views[id] else { return }
        let editor = node.field?.currentEditor() as? NSTextView ?? node.textArea
        if editor?.hasMarkedText() == true { return }
        guard let value = node.field?.stringValue ?? node.textArea?.string else { return }
        commitEdit(id, value, change: node.handlers.contains("change"))
    }

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

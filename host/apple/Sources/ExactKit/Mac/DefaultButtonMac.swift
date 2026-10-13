// Return and the default button in a dialog (LLP 1115.000 D7.3). A `dialog`
// with exactly one `filled`, non-destructive button has a default: AppKit
// draws it so, and Return presses it wherever the focus is in the dialog.
// With none, or with several, there is no default, and Return does only what
// the focused control does with it: emphasis alone never picks among
// actions. Space presses the focused control (NativeButtonsMac.swift,
// NodeViewMac.swift). The alert's default is the same rule (AlertsMac.swift).
// Two things the author does always win: a key the app prevented never
// reaches this (`Presenter.routeKey`), and a focus the pressed action set is
// kept, since Return's press moves none.
#if os(macOS)
import AppKit

extension DialogHost {
    /// The dialog's default button: its one `filled`, non-destructive
    /// button, when it has exactly one. A button the page hides, or one in a
    /// popover of the dialog's, is not among them.
    func defaultButton(of dialog: NodeView) -> NodeView? {
        // A modal route's confirming action is its sheet's own default button (RouteSheetMac.swift).
        if let confirm = entry(ofRoute: dialog)?.header?.confirm { return confirm }
        var buttons: [NodeView] = []
        func walk(_ node: NodeView) {
            for case let child as NodeView in node.container.subviews
            where !child.isHidden && child.style["display"]?.string != "none" && child.props["popover"] == nil {
                if child.isButton { buttons.append(child) } else { walk(child) }
            }
        }
        walk(dialog)
        return AlertHost.defaultRow(filled: buttons.map { $0.face?.style == "filled" },
                                    destructive: buttons.map { $0.props["destructive"] == "true" }).map { buttons[$0] }
    }

    /// AppKit draws the default: a push button whose key equivalent is
    /// Return is drawn as the default button of a key window. Kept current
    /// as the dialog opens, closes and changes.
    func syncDefaultButton() {
        // The sheet's own row is drawn as the default, not the authored button it stands for;
        // it arrives in its sheet without its key equivalent too, so it is given again.
        let row = entries.last?.header?.confirm
        if let confirm = entries.last?.sheet?.chrome?.confirm, !confirm.isHidden, confirm.keyEquivalent != "\r" { confirm.keyEquivalent = "\r" }
        let next = active.flatMap(defaultButton).flatMap { $0 === row ? nil : presenter?.controls.controls[$0.id] as? NativeButtonMac }
        if let old = drawnDefault, old !== next, old.keyEquivalent == "\r" { old.keyEquivalent = "" }
        if let next, next.keyEquivalent != "\r" { next.keyEquivalent = "\r" }
        drawnDefault = next
    }

    /// Return in the open dialog: its default button pressed, true when it
    /// took the key. What the focused control does with Return is its own
    /// and comes first: a textarea's new line, a link followed, a field's
    /// `submit` (a single-line field with a `submit` handler takes Return
    /// itself), and whatever a view that is no node's does (a web view's
    /// field). A field with no `submit` passes it here, its edit committed
    /// first, as AppKit ends the editing before the default button acts.
    func pressDefault(_ event: NSEvent, in dialog: NodeView) -> Bool {
        guard event.type == .keyDown, !event.isARepeat, NodeView.keyName(event) == "Enter",
              event.modifierFlags.intersection([.command, .control, .option, .shift]).isEmpty,
              let presenter, let window = dialog.window, let target = defaultButton(of: dialog), !target.disabled, !target.inert else { return false }
        let responder = window.firstResponder
        if let focus = presenter.keyTarget(responder) {
            if focus.textArea != nil || focus.props["href"] != nil || focus.canvasInput != nil { return false }
            if let field = focus.field {
                if focus.handlers.contains("submit") { return false }
                presenter.commitEdit(focus.id, field.stringValue, change: focus.handlers.contains("change"))
            }
        } else if let view = responder as? NSView, view !== presenter.viewport, view !== presenter.session?.view {
            return false
        }
        if let entry = entry(ofRoute: dialog), entry.header?.confirm === target, let row = entry.sheet?.chrome?.confirm { row.performClick(nil) }
        else if let button = presenter.controls.controls[target.id] as? NativeButtonMac { button.pressAsDefault() }
        else if target.pressable { presenter.press(target.id) }
        else { target.activateNative() }
        return true
    }
}
#endif

// The actions' element commands on AppKit (Presenter, PresenterMac.swift):
// a dialog's showModal and close, focus and blur by an element's id.
#if os(macOS)
import AppKit

extension Presenter {
    /// `showModal(id)`, `close(id)` from an action (LLP 1115 D6): a
    /// `dialog` by its id, in the session's top layer as its invoker's
    /// `command="show-modal"` puts it (DialogsMac).
    func dialogCommand(_ name: String, _ id: String) {
        guard let dialog = carrying("tag:dialog").first(where: { $0.props["id"] == id }) else {
            if name == "showModal" { session?.log("showModal \(id) refused: no dialog has that id") }
            return
        }
        if name == "showModal", dialogs.owns(dialog) || alerts.isOpen(dialog) {
            session?.log("showModal \(id) ignored: dialog is already open")
        } else if name == "showModal" { dialogs.show(dialog) } else { dialogs.close(dialog) }
    }

    /// The action's focus(html-id), delivered only after the batch is mounted.
    func focusElement(_ args: [Any], selectText: Bool = false) {
        guard args.count == 1, let name = args.first as? String,
              let target = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == name }) else { return }
        if !selectText, menus.focus(target) { return }
        // A search input the window's toolbar hosts, or a prompt's input lent to its
        // alert, has no box in the page (LLP 1115.002 D2): the caret goes to its field.
        if target.pageField != nil {
            if !target.formDisabled, toolbar.focusSearch(target) || alerts.focus(target), selectText { fieldSelections.selectAll(target) }
            return
        }
        guard let window = target.window, !target.formDisabled,
              target.bounds.width > 0, target.bounds.height > 0 else { return }
        var ancestor: NSView? = target
        while let view = ancestor {
            if view.isHidden || (view as? NodeView)?.inert == true { return }
            ancestor = view.superview
        }
        if target.kind == "native", !selectText { _ = session?.natives.focus(target); return }
        if selectText, target.textArea == nil, target.field == nil { return }
        if let field = target.field, window.firstResponder === field.currentEditor() {
            if selectText { fieldSelections.selectAll(target) }
            return
        }
        let responder = target.web ?? keyView(of: target)
        if responder.acceptsFirstResponder { window.makeFirstResponder(responder) }
        if selectText, window.firstResponder === target.textArea || target.field?.currentEditor() != nil { fieldSelections.selectAll(target) }
    }

    /// The action's blur(): drop the first responder; blur(html-id) only when
    /// that node holds it (a field's responder is its editor).
    func blurElement(_ args: [Any]) {
        // In the window the keyboard is in: an open dialog's sheet, else the page's.
        guard var window = frontWindow else { return }
        if let name = args.first as? String {
            guard let target = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == name }) else { return }
            window = target.window ?? window
            if target.kind == "native" {
                guard window.firstResponder === target || session?.natives.ownsFocus(target) == true else { return }
                window.makeFirstResponder(nil)
                return
            }
            let responder = keyView(of: target)
            guard window.firstResponder === responder || window.firstResponder === target.field?.currentEditor() else { return }
        }
        window.makeFirstResponder(nil)
    }
}
#endif

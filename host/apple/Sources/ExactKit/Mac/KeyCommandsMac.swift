// @ref LLP 1115.000 D7.9 — the keys the system's key bindings name, at a
// focused node: Shift with an arrow extends the reader's text selection.
#if os(macOS)
import AppKit

extension NodeView {
    /// The arrows, Home, End and the Page keys, which the system's key
    /// bindings turn into commands.
    private static let commandKeys: Set<String> = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"]

    /// A key down that no handler, control or press here took (`keyDown`).
    /// One of the command keys is named by the system's key bindings
    /// (`interpretKeyEvents`: the standard ones, and a person's own in
    /// `DefaultKeyBinding.dict`), and the command is performed here
    /// (`doCommand(by:)`). Returns whether it was taken; a key that was not
    /// goes on up the responder chain as before.
    func keyCommand(_ event: NSEvent) -> Bool {
        guard let presenter, Self.commandKeys.contains(NodeView.keyName(event)) else { return false }
        presenter.keyCommand = (self, false)
        defer { presenter.keyCommand = nil }
        interpretKeyEvents([event])
        return presenter.keyCommand?.taken ?? false
    }

    /// The command the key bindings named for the key `keyCommand` is
    /// interpreting: one that modifies a selection moves the reader's
    /// (`TextSelection.modify`), from a paragraph that has the focus. Any
    /// other is left untaken and not passed on, so the key itself goes up
    /// the chain; a command that did not come from there is AppKit's as ever.
    package override func doCommand(by selector: Selector) {
        guard let presenter, presenter.keyCommand?.source === self else { return super.doCommand(by: selector) }
        if isParagraph, selectsText, presenter.selection.modify(selector) { presenter.keyCommand?.taken = true }
    }
}
#endif

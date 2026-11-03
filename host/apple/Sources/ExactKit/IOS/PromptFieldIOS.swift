// A prompt's text field (LLP 1115 D6; `kernel/src/confirmation.rs`): an
// `alertdialog` of the alert's shape with an `input` among its rows is
// `UIAlertController` with a text field, as a hand-built rename prompt is.
// UIKit makes the field; it goes through the authored input's own field
// machinery as the bar's search field does (`NodeView.headerField`): the
// input's value writes reach it (`write`, from the input's props, so typing
// stands across batches that write nothing), its edits pass the input's own
// checks, a keystroke is its `input`, its focus and selection are the
// input's, and Return is the input's (its key, then its `submit` if it has
// one, and that is all) before it is the prompt's first action that is not
// destructive. UIKit owns the field's class, so its keys are heard with
// public API alone: a hardware key by a gesture recognizer on the field
// (`PromptKeys`), which UIKit gives a press before the field, and what a key
// does by the field's delegate, which refuses the edit or Return of a key a
// handler prevented. A software keyboard's Backspace is the delegate's
// deletion before the caret.
#if os(iOS) || os(tvOS)
import UIKit

final class PromptField: NSObject, UITextFieldDelegate {
    private(set) weak var node: NodeView?
    private weak var presenter: Presenter?
    private weak var field: UITextField?
    /// The key whose `key` handlers ran as it was pressed: its own edit or
    /// Return does not run them again (the agent's keys say theirs).
    var heard: String?
    /// Whether a handler prevented `heard`: its edit and Return are refused.
    private var prevented = false
    struct Snapshot {
        weak var node: NodeView?
        let incarnation: UInt64
        let value: String?
        let text: String
        let selection: NSRange?
    }
    private var restoring: Snapshot?
    private var relocating = false
    /// The alert's own delegate for the field, which answers what this does
    /// not.
    private weak var alertDelegate: UITextFieldDelegate?
    /// What Return chooses when the input itself does not take it: the
    /// prompt's first action that is not destructive (`MenuHost`).
    var returned: (() -> Void)?
    init(_ node: NodeView, presenter: Presenter) { self.node = node; self.presenter = presenter }
    func snapshot() -> Snapshot? {
        guard let node = live(), let field else { return nil }
        return Snapshot(node: node, incarnation: node.incarnation, value: node.props["value"],
                        text: field.text ?? "", selection: node.liveSelection)
    }
    func restore(_ state: Snapshot) {
        guard let node = live(), node === state.node, node.incarnation == state.incarnation,
              node.props["value"] == state.value, let field else { return }
        field.text = state.text
        restoring = state
    }
    func restoreSelection() {
        defer { restoring = nil }
        guard let state = restoring, let node = live(), node.incarnation == state.incarnation,
              node.props["value"] == state.value, field?.text == state.text, let selection = state.selection else { return }
        // UIKit focuses the prompt during presentation; setting its range earlier is lost.
        _ = node.showSelection(selection)
    }

    /// Give UIKit's field what the authored one says: its value, placeholder,
    /// name and keyboard (`type`, `inputmode`, `autocomplete`, `autocapitalize`,
    /// `enterkeyhint`, which the authored field holds as UIKit spells them).
    func attach(_ field: UITextField) {
        guard let node else { return }
        node.makeWaitingField()
        self.field = field
        field.text = node.fieldText.isEmpty ? node.props["value"] : node.fieldText
        if let own = node.field {
            field.placeholder = own.placeholder ?? own.attributedPlaceholder?.string
            field.isSecureTextEntry = own.isSecureTextEntry
            field.textContentType = own.textContentType
            field.keyboardType = own.keyboardType
            // What the author leaves unsaid stays the alert field's own:
            // no capitalization, no correction (so no QuickType echo of
            // its text), and the Return key ↵, not the sentences,
            // correction and Done an authored field takes. A type that
            // turns correction off says it.
            let said = { (name: String) in node.props[name] != nil }
            let plain = ["email", "url", "password"].contains((node.props["type"] ?? "").lowercased())
            if said("autocapitalize") || plain { field.autocapitalizationType = own.autocapitalizationType }
            if said("autocorrect") || plain {
                // Smart quotes and dashes go with correction (`inputSmartQuotes`).
                field.autocorrectionType = own.autocorrectionType
                field.smartQuotesType = own.smartQuotesType
                field.smartDashesType = own.smartDashesType
            }
            if said("spellcheck") { field.spellCheckingType = own.spellCheckingType }
            if said("enterKeyHint") { field.returnKeyType = own.returnKeyType }
        } else {
            field.placeholder = node.props["placeholder"]
        }
        field.accessibilityLabel = node.props["accessibilityLabel"]
        field.accessibilityIdentifier = node.props["testId"]
        field.isEnabled = !node.disabled
        node.headerField = field
        node.applyFieldDirection()
        alertDelegate = field.delegate
        field.delegate = self
        field.addGestureRecognizer(PromptKeys(self))
        field.addAction(UIAction { [weak self] _ in self?.typed() }, for: .editingChanged)
    }

    /// The input's props were written (`NodeView.applyProps`): its value, as
    /// into its own field (a composition stands), and its enablement.
    func write() {
        guard let node, let field, presenter?.views[node.id] === node else { return }
        if let value = node.props["value"] { node.writeValue(value, into: field) }
        field.isEnabled = !node.disabled
    }

    func textField(_ textField: UITextField, shouldChangeCharactersIn range: NSRange, replacementString string: String) -> Bool {
        guard let node = live(), !prevented else { return false }
        if string.isEmpty, heard != "Backspace", !node.disabled, Self.deletesBackward(textField, range),
           presenter?.keyDown(at: node, "Backspace") == true { return false }
        guard node.textField(textField, shouldChangeCharactersIn: range, replacementString: string) else { return false }
        return alertDelegate?.textField?(textField, shouldChangeCharactersIn: range, replacementString: string) ?? true
    }
    func textFieldShouldClear(_ textField: UITextField) -> Bool {
        guard live()?.textFieldShouldClear(textField) ?? false else { return false }
        return alertDelegate?.textFieldShouldClear?(textField) ?? true
    }
    func textFieldDidBeginEditing(_ textField: UITextField) {
        presenter?.keyboardPrompt = textField
        alertDelegate?.textFieldDidBeginEditing?(textField)
        if let node = live() { presenter?.focus(node.id) }
    }
    func textFieldDidChangeSelection(_ textField: UITextField) {
        alertDelegate?.textFieldDidChangeSelection?(textField)
        if let node = live() { presenter?.fieldSelections.changed(node) }
        live()?.applyFieldDirection() // a `direction` that came while it composed (LLP 1001 §1)
    }
    func textFieldDidEndEditing(_ textField: UITextField) {
        alertDelegate?.textFieldDidEndEditing?(textField)
        if !relocating { live()?.textFieldDidEndEditing(textField) } // the `change`, then the blur
    }
    /// Return, one rule on every host (docs/contract-for-agents.md): the
    /// input's `key` handlers first (prevented, nothing more); its `change`;
    /// then its `submit` if it has one, and that is all, as a hand-built
    /// field's delegate that handles Return; else the prompt's first action
    /// that is not destructive (`returned`), not UIKit's preferred action,
    /// which is the author's to draw.
    func textFieldShouldReturn(_ textField: UITextField) -> Bool {
        guard let node = live(), let presenter, !node.disabled, !prevented else { return false }
        if heard != "Enter", presenter.keyDown(at: node, "Enter") { return false }
        return returnHeard(textField)
    }
    /// Return once the input's `key` handlers have heard it unprevented (the
    /// agent's `key Enter` delivers those itself).
    func returnHeard(_ textField: UITextField) -> Bool {
        guard let node = live(), let presenter, !node.disabled else { return false }
        presenter.commitEdit(node.id, textField.text ?? "", change: node.handlers.contains("change"))
        if node.handlers.contains("submit") { presenter.submit(node.id); return false }
        if let returned { returned(); return false }
        return alertDelegate?.textFieldShouldReturn?(textField) ?? true
    }
    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || (alertDelegate?.responds(to: selector) ?? false)
    }
    override func forwardingTarget(for selector: Selector!) -> Any? {
        alertDelegate?.responds(to: selector) == true ? alertDelegate : nil
    }

    private func live() -> NodeView? {
        guard let node, presenter?.views[node.id] === node else { return nil }
        return node
    }
    private func typed() {
        guard let node = live(), let field else { return }
        node.typedText(field.text ?? "")
        // The page's value written while text was marked waited (`writeValue`).
        if field.markedTextRange == nil, let held = node.pendingValue { node.writeValue(held, into: field) }
        node.applyFieldDirection() // a `direction` that came while it composed (LLP 1001 §1)
    }

    /// A hardware key at the field, as `PromptKeys` hears it before UIKit
    /// does: its `key` or `keyup` handlers, and whether one prevented it.
    func pressed(_ presses: Set<UIPress>, phase: UIPress.Phase) {
        if phase != .began { heard = nil; prevented = false }
        guard let node = live() else { return }
        let remaining = node.pressedControls(presses, down: phase == .began)
        guard !remaining.isEmpty else { return }
        if phase == .began {
            heard = remaining.first?.key.map(NodeView.keyName)
            prevented = node.editorKeyDown(remaining)
        } else if phase == .ended { node.editorKeyUp(remaining) }
    }

    /// A Backspace whose key is already heard (the agent's): UIKit's deletion
    /// through the input's edit checks, which a keyboard's reaches through
    /// the delegate and `deleteBackward` alone does not.
    func deleteBackward(_ field: UITextField) {
        guard let node = live(), let selections = presenter?.fieldSelections else { return field.deleteBackward() }
        selections.nativeEdit(node, replacement: "", marked: field.markedTextRange != nil) { field.deleteBackward() }
    }

    /// Whether replacing `range` with nothing is Backspace's edit with no
    /// selection: the text just before the caret. A selection's deletion is
    /// not told apart from Cut's, so it is not taken for Backspace.
    private static func deletesBackward(_ field: UITextField, _ range: NSRange) -> Bool {
        guard range.length > 0, field.markedTextRange == nil, let selection = field.selectedTextRange, selection.isEmpty else { return false }
        return field.offset(from: field.beginningOfDocument, to: selection.start) == range.location + range.length
    }

    /// The alert is going: the edit commits, and the authored field is its
    /// own again.
    func end(relocating: Bool = false) {
        self.relocating = relocating
        heard = nil; prevented = false
        if !relocating, let node = live() { presenter?.commitEdit(node.id, node.fieldText, change: node.handlers.contains("change")) }
        if let node, node.headerField === field { node.headerField = nil }
    }
}

/// Hears a hardware key at a prompt's field before UIKit's field does, and
/// takes nothing from it: it recognizes no gesture, delays and cancels no
/// press, and prevents no other recognizer. A prevented key is refused where
/// it would act, at the field's delegate (`PromptField`).
final class PromptKeys: UIGestureRecognizer {
    private weak var prompt: PromptField?
    init(_ prompt: PromptField) {
        self.prompt = prompt
        super.init(target: nil, action: nil)
        cancelsTouchesInView = false
        delaysTouchesBegan = false
        delaysTouchesEnded = false
    }
    override func shouldReceive(_ event: UIEvent) -> Bool { event.type == .presses }
    override func canPrevent(_ preventedGestureRecognizer: UIGestureRecognizer) -> Bool { false }
    override func canBePrevented(by preventingGestureRecognizer: UIGestureRecognizer) -> Bool { false }
    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent) { prompt?.pressed(presses, phase: .began) }
    override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent) { prompt?.pressed(presses, phase: .ended) }
    override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent) { prompt?.pressed(presses, phase: .cancelled) }
}
#endif

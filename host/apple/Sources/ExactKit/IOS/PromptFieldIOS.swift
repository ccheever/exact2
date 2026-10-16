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
// destructive.
#if os(iOS) || os(tvOS)
import UIKit

final class PromptField: NSObject, UITextFieldDelegate {
    private(set) weak var node: NodeView?
    private weak var presenter: Presenter?
    private weak var field: UITextField?
    /// The alert's own delegate for the field, which answers what this does
    /// not.
    private weak var alertDelegate: UITextFieldDelegate?
    /// What Return chooses when the input itself does not take it: the
    /// prompt's first action that is not destructive (`MenuHost`).
    var returned: (() -> Void)?
    init(_ node: NodeView, presenter: Presenter) { self.node = node; self.presenter = presenter }

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
            field.autocapitalizationType = own.autocapitalizationType
            field.autocorrectionType = own.autocorrectionType
            field.spellCheckingType = own.spellCheckingType
            field.returnKeyType = own.returnKeyType
        } else {
            field.placeholder = node.props["placeholder"]
        }
        field.accessibilityLabel = node.props["accessibilityLabel"]
        field.accessibilityIdentifier = node.props["testId"]
        field.isEnabled = !node.disabled
        node.headerField = field
        alertDelegate = field.delegate
        field.delegate = self
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
        guard let node = live(), node.textField(textField, shouldChangeCharactersIn: range, replacementString: string) else { return false }
        return alertDelegate?.textField?(textField, shouldChangeCharactersIn: range, replacementString: string) ?? true
    }
    func textFieldShouldClear(_ textField: UITextField) -> Bool {
        guard live()?.textFieldShouldClear(textField) ?? false else { return false }
        return alertDelegate?.textFieldShouldClear?(textField) ?? true
    }
    func textFieldDidBeginEditing(_ textField: UITextField) {
        alertDelegate?.textFieldDidBeginEditing?(textField)
        if let node = live() { presenter?.focus(node.id) }
    }
    func textFieldDidChangeSelection(_ textField: UITextField) {
        alertDelegate?.textFieldDidChangeSelection?(textField)
        if let node = live() { presenter?.fieldSelections.changed(node) }
    }
    func textFieldDidEndEditing(_ textField: UITextField) {
        alertDelegate?.textFieldDidEndEditing?(textField)
        live()?.textFieldDidEndEditing(textField) // the `change`, then the blur
    }
    /// Return, one rule on every host (docs/contract-for-agents.md): the
    /// input's `key` handlers first (prevented, nothing more); its `change`;
    /// then its `submit` if it has one, and that is all, as a hand-built
    /// field's delegate that handles Return; else the prompt's first action
    /// that is not destructive (`returned`), not UIKit's preferred action,
    /// which is the author's to draw.
    func textFieldShouldReturn(_ textField: UITextField) -> Bool {
        guard let node = live(), let presenter, !node.disabled else { return false }
        if presenter.keyDown(at: node, "Enter") { return false }
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
    }

    /// The alert is going: the edit commits, and the authored field is its
    /// own again.
    func end() {
        if let node = live() { presenter?.commitEdit(node.id, node.fieldText, change: node.handlers.contains("change")) }
        if let node, node.headerField === field { node.headerField = nil }
    }
}
#endif

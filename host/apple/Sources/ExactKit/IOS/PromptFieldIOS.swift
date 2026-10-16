// A prompt's text field (LLP 1115 D6; `kernel/src/confirmation.rs`): an
// `alertdialog` of the alert's shape with an `input` among its rows is
// `UIAlertController` with a text field, as a hand-built rename prompt is.
// The field stands in for its authored `input` as the bar's search field does
// (`NodeView.headerField`): each batch writes the page's `value` into it as
// into the input's own field (a composition stands), its edits pass the
// input's own checks (`readonly`, `maxlength`, `beforeinput`, a number's
// characters), a keystroke is the input's `input` and the end of editing its
// `change`, so an action's press reads what was typed.
#if os(iOS) || os(tvOS)
import UIKit

final class PromptField: NSObject, UITextFieldDelegate {
    private weak var node: NodeView?
    private weak var presenter: Presenter?
    private weak var field: UITextField?
    /// The alert's own delegate for the field, which takes Return (the
    /// preferred action) and anything else this does not answer.
    private weak var alertDelegate: UITextFieldDelegate?
    init(_ node: NodeView, presenter: Presenter) { self.node = node; self.presenter = presenter }

    /// Give UIKit's field what the authored one says: its value, placeholder,
    /// name and keyboard (`type`, `inputmode`, `autocomplete`, `autocapitalize`,
    /// `enterkeyhint`, which the authored field holds as UIKit spells them).
    func attach(_ field: UITextField) {
        guard let node else { return }
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
        field.addAction(UIAction { [weak self] _ in self?.commit() }, for: .editingDidEnd)
    }

    /// After a batch: the page's value and enablement, as the input's own
    /// field takes them (`NodeView.writeValue`).
    func sync() {
        guard let node, let field, presenter?.views[node.id] === node else { return }
        if let value = node.props["value"] { node.writeValue(value, into: field) }
        field.isEnabled = !node.disabled
    }

    func textField(_ textField: UITextField, shouldChangeCharactersIn range: NSRange, replacementString string: String) -> Bool {
        guard let node, presenter?.views[node.id] === node,
              node.textField(textField, shouldChangeCharactersIn: range, replacementString: string) else { return false }
        return alertDelegate?.textField?(textField, shouldChangeCharactersIn: range, replacementString: string) ?? true
    }
    func textFieldShouldClear(_ textField: UITextField) -> Bool {
        guard node?.textFieldShouldClear(textField) ?? false else { return false }
        return alertDelegate?.textFieldShouldClear?(textField) ?? true
    }
    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || (alertDelegate?.responds(to: selector) ?? false)
    }
    override func forwardingTarget(for selector: Selector!) -> Any? {
        alertDelegate?.responds(to: selector) == true ? alertDelegate : nil
    }

    private func typed() {
        guard let node, let field, presenter?.views[node.id] === node else { return }
        node.typedText(field.text ?? "")
    }

    private func commit() {
        guard let node, let presenter, presenter.views[node.id] === node else { return }
        presenter.commitEdit(node.id, node.fieldText, change: node.handlers.contains("change"))
    }

    /// The alert is going: the edit commits, and the authored field is its
    /// own again.
    func end() {
        commit()
        if let node, node.headerField === field { node.headerField = nil }
    }
}
#endif

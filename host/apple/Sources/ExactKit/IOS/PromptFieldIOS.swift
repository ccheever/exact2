// A prompt's text field (LLP 1115 D6; `kernel/src/confirmation.rs`): an
// `alertdialog` of the alert's shape with an `input` among its rows is
// `UIAlertController` with that many text fields, as a hand-built rename
// prompt is. Each field stands in for its authored `input` as the bar's search
// field does (`NodeView.headerField`): the page's `value` is written into it,
// a keystroke is the input's `input` and the end of editing its `change`, so
// an action's press reads what was typed.
#if os(iOS) || os(tvOS)
import UIKit

final class PromptField {
    private weak var node: NodeView?
    private weak var presenter: Presenter?
    private weak var field: UITextField?
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
        field.addAction(UIAction { [weak self] _ in self?.typed() }, for: .editingChanged)
        field.addAction(UIAction { [weak self] _ in self?.commit() }, for: .editingDidEnd)
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

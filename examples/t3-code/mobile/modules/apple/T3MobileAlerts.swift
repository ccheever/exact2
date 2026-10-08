#if os(iOS)
// upstream 365aa87982 use-remote-environment-registry.ts and ConnectionsNewRouteScreen.
// @ref llp/1107.003-pairing-and-transport.decision.md#mobile-adaptations
import UIKit

final class T3MobileAlerts {
    private var shown: UIAlertController?
    private var complete: ((String, String) -> Void)?

    func present(title: String, message: String, buttons: [(String, String, UIAlertAction.Style)],
                 reply: @escaping (String) -> Void) throws {
        try show(title: title, message: message, buttons: buttons, initialValue: nil) { choice, _ in reply(choice) }
    }
    // upstream 365aa87982 useThreadListActions native Rename prompt.
    func prompt(title: String, initialValue: String, cancelLabel: String, submitLabel: String,
                reply: @escaping (String, String) -> Void) throws {
        try show(title: title, message: "", buttons: [("cancel", cancelLabel, .cancel), ("submit", submitLabel, .default)],
                 initialValue: initialValue, reply: reply)
    }
    private func show(title: String, message: String, buttons: [(String, String, UIAlertAction.Style)],
                      initialValue: String?, reply: @escaping (String, String) -> Void) throws {
        guard shown == nil else { throw ExactNativeRefusal("A mobile alert is already open.") }
        let roots = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
            .filter { $0.activationState == .foregroundActive }.flatMap { $0.windows }
        guard var presenter = roots.first(where: { $0.isKeyWindow })?.rootViewController else {
            throw ExactNativeRefusal("There is no active window for this alert.")
        }
        while let next = presenter.presentedViewController { presenter = next }
        let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
        if let initialValue { alert.addTextField { field in field.text = initialValue; field.accessibilityIdentifier = "home-rename-input" } }
        complete = reply; shown = alert
        for (value, label, style) in buttons {
            alert.addAction(UIAlertAction(title: label, style: style) { [weak self, weak alert] _ in
                guard let self, let alert, self.shown === alert else { return }
                self.finish(value, text: value == "submit" ? alert.textFields?.first?.text ?? "" : "")
            })
        }
        presenter.present(alert, animated: true)
    }
    private func finish(_ value: String, text: String = "") {
        let reply = complete; complete = nil; shown = nil; reply?(value, text)
    }
    func destroy() {
        shown?.dismiss(animated: false)
        finish("cancel")
    }
}
#endif

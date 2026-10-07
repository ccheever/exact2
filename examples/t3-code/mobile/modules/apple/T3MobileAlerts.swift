#if os(iOS)
// upstream 365aa87982 use-remote-environment-registry.ts and ConnectionsNewRouteScreen.
// @ref llp/1107.003-pairing-and-transport.decision.md#mobile-adaptations
import UIKit

final class T3MobileAlerts {
    private var shown: UIAlertController?
    private var complete: ((String) -> Void)?

    func present(title: String, message: String, buttons: [(String, String, UIAlertAction.Style)],
                 reply: @escaping (String) -> Void) throws {
        guard shown == nil else { throw ExactNativeRefusal("A mobile alert is already open.") }
        let roots = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
            .filter { $0.activationState == .foregroundActive }.flatMap { $0.windows }
        guard var presenter = roots.first(where: { $0.isKeyWindow })?.rootViewController else {
            throw ExactNativeRefusal("There is no active window for this alert.")
        }
        while let next = presenter.presentedViewController { presenter = next }
        let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
        complete = reply; shown = alert
        for (value, label, style) in buttons {
            alert.addAction(UIAlertAction(title: label, style: style) { [weak self] _ in self?.finish(value) })
        }
        presenter.present(alert, animated: true)
    }
    private func finish(_ value: String) {
        let reply = complete; complete = nil; shown = nil; reply?(value)
    }
    func destroy() {
        shown?.dismiss(animated: false)
        finish("cancel")
    }
}
#endif

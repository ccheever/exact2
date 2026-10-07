#if os(iOS)
// @ref llp/1107.003-pairing-and-transport.decision.md#mobile-adaptations
// App-local UIKit choices, spinner and explicit confirmations. No server/client ownership.
import UIKit

final class T3MobileScheduledControls: NSObject, UIPopoverPresentationControllerDelegate {
    private let agent: Bool
    private var shown: UIViewController?
    private var completion: (([String: Any]) -> Void)?
    private var generation = 0

    init(agent: Bool) { self.agent = agent; super.init() }

    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let requestedGeneration = request["generation"] as? Int ?? 0
        func refuse(_ message: String) {
            reply(["ok": false, "generation": requestedGeneration,
                   "error": ["kind": "refused", "message": message]])
        }
        guard !agent else { refuse("Scheduled-task native controls require a person in the app."); return }
        guard shown == nil else { refuse("A scheduled-task control is already open."); return }
        let windows = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
            .filter { $0.activationState == .foregroundActive }.flatMap { $0.windows }
        guard var presenter = windows.first(where: { $0.isKeyWindow })?.rootViewController else {
            refuse("There is no active window for this control."); return
        }
        while let next = presenter.presentedViewController { presenter = next }
        guard !(presenter is UIAlertController), !presenter.isBeingDismissed else {
            refuse("Close the current alert before opening this control."); return
        }
        let controller: UIViewController
        switch request["op"] as? String {
        case "mobileScheduledMenu":
            guard let rows = request["items"] as? [[String: Any]], !rows.isEmpty,
                  rows.allSatisfy({ ($0["id"] as? String)?.isEmpty == false && ($0["title"] as? String)?.isEmpty == false }),
                  Set(rows.compactMap { $0["id"] as? String }).count == rows.count else {
                refuse("This menu has no valid choices."); return
            }
            let menu = UIAlertController(title: request["title"] as? String, message: nil, preferredStyle: .actionSheet)
            for row in rows {
                let id = row["id"] as? String ?? ""
                let title = (row["selected"] as? Bool == true ? "✓ " : "") + (row["title"] as? String ?? "")
                let action = UIAlertAction(title: title, style: row["destructive"] as? Bool == true ? .destructive : .default) { [weak self] _ in
                    self?.dismissThenFinish(["choice": id, "cancelled": false])
                }
                action.isEnabled = row["disabled"] as? Bool != true
                menu.addAction(action)
            }
            menu.addAction(UIAlertAction(title: "Cancel", style: .cancel) { [weak self] _ in self?.finish(["cancelled": true]) })
            controller = menu
        case "mobileScheduledTime":
            guard let value = request["value"] as? String,
                  value.range(of: "^([01]?[0-9]|2[0-3]):[0-5][0-9]$", options: .regularExpression) != nil else {
                refuse("Choose a valid task time."); return
            }
            let picker = T3ScheduledTimeController(value: value) { [weak self] value in
                self?.shown?.dismiss(animated: true)
                self?.finish(value.map { ["value": $0, "cancelled": false] } ?? ["cancelled": true])
            }
            let navigation = UINavigationController(rootViewController: picker)
            navigation.modalPresentationStyle = .pageSheet
            if let sheet = navigation.sheetPresentationController {
                sheet.detents = [.custom(identifier: .init("scheduled-time")) { _ in 310 }]
                sheet.prefersGrabberVisible = true
            }
            controller = navigation
        case "mobileScheduledConfirm":
            let kind = request["kind"] as? String ?? ""
            let title: String, message: String, button: String, destructive: Bool
            switch kind {
            case "delete":
                title = "Delete task?"; message = request["taskTitle"] as? String ?? ""; button = "Delete"; destructive = true
            case "rotate":
                title = "Rotate URL?"; message = "The old URL will stop working immediately."; button = "Rotate URL"; destructive = true
            case "discard":
                title = "Discard changes?"; message = "Your unsaved changes will be lost."; button = "Discard changes"; destructive = true
            case "reset-credit":
                title = "Use a reset credit?"
                message = "This redeems one credit on your account and clears the current rate-limit windows. It cannot be undone."
                button = "Use credit"; destructive = false
            default: refuse("Unknown scheduled-task confirmation."); return
            }
            let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
            alert.addAction(UIAlertAction(title: kind == "discard" ? "Keep editing" : "Cancel", style: .cancel) { [weak self] _ in self?.finish(["confirmed": false, "cancelled": true]) })
            alert.addAction(UIAlertAction(title: button, style: destructive ? .destructive : .default) { [weak self] _ in self?.finish(["confirmed": true, "cancelled": false]) })
            controller = alert
        default: refuse("Unknown scheduled-task control."); return
        }
        generation = requestedGeneration; completion = reply; shown = controller
        if let popover = controller.popoverPresentationController {
            popover.sourceView = presenter.view
            // No tap rectangle crosses Contract's action boundary. Keep iPad anchoring explicit.
            let bounds = presenter.view.bounds.inset(by: presenter.view.safeAreaInsets)
            popover.sourceRect = CGRect(x: bounds.midX, y: bounds.maxY - 1, width: 1, height: 1)
            popover.permittedArrowDirections = .down; popover.delegate = self
        }
        presenter.present(controller, animated: true)
        controller.presentationController?.delegate = self
    }

    private func dismissThenFinish(_ value: [String: Any]) {
        guard let controller = shown else { return }
        controller.dismiss(animated: true) { [weak self] in self?.finish(value) }
    }

    private func finish(_ value: [String: Any]) {
        let reply = completion; completion = nil; shown = nil
        reply?(["ok": true, "generation": generation, "value": value])
    }
    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
        finish(["cancelled": true])
    }
    func destroy() {
        shown?.dismiss(animated: false)
        finish(["cancelled": true])
    }
}

private final class T3ScheduledTimeController: UIViewController {
    private let picker = UIDatePicker()
    private let initial: String
    private let completed: (String?) -> Void
    init(value: String, completed: @escaping (String?) -> Void) {
        initial = value; self.completed = completed; super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { nil }
    override func viewDidLoad() {
        super.viewDidLoad(); title = "Time"; view.backgroundColor = .systemBackground
        navigationItem.leftBarButtonItem = UIBarButtonItem(title: "Cancel", style: .plain, target: self, action: #selector(cancel))
        navigationItem.rightBarButtonItem = UIBarButtonItem(title: "Done", style: .done, target: self, action: #selector(done))
        picker.datePickerMode = .time; picker.preferredDatePickerStyle = .wheels
        let parts = initial.split(separator: ":").compactMap { Int($0) }
        if parts.count == 2, let date = Calendar.current.date(bySettingHour: parts[0], minute: parts[1], second: 0, of: Date()) { picker.date = date }
        picker.translatesAutoresizingMaskIntoConstraints = false; view.addSubview(picker)
        NSLayoutConstraint.activate([
            picker.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor),
            picker.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor),
            picker.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            picker.bottomAnchor.constraint(lessThanOrEqualTo: view.safeAreaLayoutGuide.bottomAnchor),
        ])
    }
    @objc private func cancel() { completed(nil) }
    @objc private func done() {
        let values = Calendar.current.dateComponents([.hour, .minute], from: picker.date)
        completed(String(format: "%02d:%02d", values.hour ?? 9, values.minute ?? 0))
    }
}
#endif

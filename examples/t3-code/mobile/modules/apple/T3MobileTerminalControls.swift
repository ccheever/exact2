#if os(iOS)
// Pinned365aa87982 ThreadTerminalRouteScreen toolbar actions; UIKit owns keyboard lifetime.
// @ref llp/1106.007-mobile-terminal.decision.md#native-renderer
import UIKit

final class T3MobileTerminalAccessory: UIView {
    let host: String
    private var modifiers: [String: UIButton] = [:]
    init(host: String, action: @escaping (String, String) -> Void) {
        self.host = host
        super.init(frame: CGRect(x: 0, y: 0, width: 390, height: 52))
        autoresizingMask = [.flexibleWidth]
        backgroundColor = .secondarySystemBackground
        let scroll = UIScrollView(); scroll.showsHorizontalScrollIndicator = false
        scroll.translatesAutoresizingMaskIntoConstraints = false; addSubview(scroll)
        let row = UIStackView(); row.axis = .horizontal; row.spacing = 4
        row.translatesAutoresizingMaskIntoConstraints = false; scroll.addSubview(row)
        var controls: [(String, String, String)] = [("esc", "input", "\u{1b}")]
        controls += host == "mac" ? [("cmd", "modifier", "meta"), ("ctrl", "modifier", "ctrl")] : [("ctrl", "modifier", "ctrl"), ("alt", "modifier", "meta")]
        controls += [("tab", "input", "\t"), ("paste", "paste", ""), ("clear", "clear", ""),
                     ("↑", "input", "\u{1b}[A"), ("↓", "input", "\u{1b}[B"), ("←", "input", "\u{1b}[D"), ("→", "input", "\u{1b}[C"),
                     ("~", "input", "~"), ("|", "input", "|"), ("/", "input", "/"), ("-", "input", "-")]
        for (label, operation, value) in controls {
            let button = UIButton(type: .system)
            button.setTitle(operation == "modifier" || operation == "clear" ? label.uppercased() : label, for: .normal)
            button.titleLabel?.font = .monospacedSystemFont(ofSize: 13, weight: .medium)
            button.accessibilityLabel = label
            button.addAction(UIAction { _ in action(operation, value) }, for: .touchUpInside)
            button.widthAnchor.constraint(greaterThanOrEqualToConstant: label.count > 1 ? 56 : 44).isActive = true
            row.addArrangedSubview(button)
            if operation == "modifier" { modifiers[value] = button }
        }
        let dismiss = UIButton(type: .system); dismiss.setImage(UIImage(systemName: "keyboard.chevron.compact.down"), for: .normal)
        dismiss.accessibilityLabel = "Dismiss keyboard"
        dismiss.addAction(UIAction { _ in action("hide-keyboard", "") }, for: .touchUpInside)
        dismiss.translatesAutoresizingMaskIntoConstraints = false; addSubview(dismiss)
        dismiss.widthAnchor.constraint(equalToConstant: 44).isActive = true
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: leadingAnchor), scroll.trailingAnchor.constraint(equalTo: dismiss.leadingAnchor),
            dismiss.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8), dismiss.centerYAnchor.constraint(equalTo: centerYAnchor), dismiss.heightAnchor.constraint(equalToConstant: 44),
            scroll.topAnchor.constraint(equalTo: topAnchor), scroll.bottomAnchor.constraint(equalTo: bottomAnchor),
            row.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 8),
            row.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -8),
            row.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 4),
            row.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -4),
            row.heightAnchor.constraint(equalTo: scroll.frameLayoutGuide.heightAnchor, constant: -8)])
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override var intrinsicContentSize: CGSize { CGSize(width: UIView.noIntrinsicMetric, height: 52) }
    func selectModifier(_ value: String) {
        for (key, button) in modifiers {
            button.isSelected = key == value
            button.backgroundColor = key == value ? tintColor.withAlphaComponent(0.15) : .clear
            button.accessibilityTraits = key == value ? [.button, .selected] : [.button]
        }
    }
}

/// Real native session chooser. Uses an action sheet until the shared navigation menu seam exists.
final class T3MobileTerminalMenu {
    private var shown: UIAlertController?
    private var completion: ((String) -> Void)?
    func present(source: String, anchor: UIView, reply: @escaping (String) -> Void) throws {
        guard shown == nil, let window = anchor.window, var presenter = window.rootViewController,
              let data = (try? JSONSerialization.jsonObject(with: Data(source.utf8))) as? [String: Any] else {
            throw ExactNativeRefusal("The terminal menu is unavailable.")
        }
        while let next = presenter.presentedViewController { presenter = next }
        let sheet = UIAlertController(title: "Terminal options", message: nil, preferredStyle: .actionSheet)
        let font = data["fontSize"] as? Double ?? 10.5, readOnly = data["readOnly"] as? Bool ?? true
        func item(_ title: String, _ choice: String, enabled: Bool = true, style: UIAlertAction.Style = .default) {
            let action = UIAlertAction(title: title, style: style) { [weak self] _ in self?.finish(choice) }
            action.isEnabled = enabled; sheet.addAction(action)
        }
        item(String(format: "A- %.1f pt", max(6, font - 0.5)), "font-decrease", enabled: font > 6)
        item(String(format: "A+ %.1f pt", min(14, font + 0.5)), "font-increase", enabled: font < 14)
        for tab in data["tabs"] as? [[String: Any]] ?? [] {
            let label = tab["label"] as? String ?? "Terminal", id = tab["id"] as? String ?? ""
            item((tab["selected"] as? Bool == true ? "✓ " : "") + label, "select:" + id)
        }
        item("Open new terminal", "new", enabled: !readOnly)
        item("Cancel", "", style: .cancel)
        sheet.popoverPresentationController?.sourceView = anchor
        sheet.popoverPresentationController?.sourceRect = CGRect(x: anchor.bounds.maxX - 22, y: 0, width: 1, height: 1)
        shown = sheet; completion = reply; presenter.present(sheet, animated: true)
    }
    private func finish(_ choice: String) { let reply = completion; completion = nil; shown = nil; reply?(choice) }
    func destroy() { shown?.dismiss(animated: false); finish("") }
}
#endif

#if os(iOS)
extension T3MobileTerminalAccessory {
    func colors(background: String, foreground: String, border: String) {
        backgroundColor = Self.color(background); tintColor = Self.color(foreground)
        layer.borderColor = Self.color(border).cgColor; layer.borderWidth = 0.5
    }
    private static func color(_ text: String) -> UIColor {
        let hex = text.hasPrefix("#") ? String(text.dropFirst()) : text
        guard hex.count == 6, let value = UInt32(hex, radix: 16) else { return .label }
        return UIColor(red: CGFloat((value >> 16) & 255) / 255, green: CGFloat((value >> 8) & 255) / 255, blue: CGFloat(value & 255) / 255, alpha: 1)
    }
}
#endif

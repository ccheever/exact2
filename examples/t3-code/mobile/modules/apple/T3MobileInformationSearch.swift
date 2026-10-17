#if os(iOS)
// Pinned OpenSourceLicenses bottom search; owns only its text input.
import UIKit
final class T3MobileInformationSearch: ExactNativeInstance {
    static let factory = ExactNativeFactory { props, events in let instance = T3MobileInformationSearch(events: events); try instance.setProps(props); return instance }
    private let root = UIView(), field = UISearchTextField(), close = UIButton(type: .system)
    private var trailing: NSLayoutConstraint?
    private var alive = true
    override var view: UIView { root }
    override init(events: ExactNativeEvents) {
        super.init(events: events)
        let effect: UIVisualEffect
        if #available(iOS 26.0, *) { let glass = UIGlassEffect(); glass.isInteractive = true; effect = glass } else { effect = UIBlurEffect(style: .systemMaterial) }
        let background = UIVisualEffectView(effect: effect); background.translatesAutoresizingMaskIntoConstraints = false; background.layer.cornerRadius = 24; background.clipsToBounds = true; root.addSubview(background)
        field.placeholder = "Search packages"; field.accessibilityLabel = "Search open-source licenses"; field.autocorrectionType = .no; field.autocapitalizationType = .none; field.returnKeyType = .search
        field.backgroundColor = .clear; field.borderStyle = .none; field.translatesAutoresizingMaskIntoConstraints = false; background.contentView.addSubview(field)
        field.addAction(UIAction { [weak self] _ in guard let self, alive else { return }; events.change(field.text ?? "") }, for: .editingChanged)
        field.addAction(UIAction { [weak self] _ in self?.editing(true) }, for: .editingDidBegin)
        field.addAction(UIAction { [weak self] _ in self?.editing(false) }, for: .editingDidEnd)
        close.setImage(UIImage(systemName: "xmark"), for: .normal); close.accessibilityLabel = "Dismiss search keyboard"; close.translatesAutoresizingMaskIntoConstraints = false; root.addSubview(close)
        close.isHidden = true; close.alpha = 0
        close.addAction(UIAction { [weak self] _ in guard let self, alive else { return }; field.resignFirstResponder() }, for: .touchUpInside)
        let trailing = background.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -16); self.trailing = trailing
        NSLayoutConstraint.activate([background.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 16), background.topAnchor.constraint(equalTo: root.topAnchor, constant: 6), background.heightAnchor.constraint(equalToConstant: 48),
            trailing, close.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -12), close.widthAnchor.constraint(equalToConstant: 44), close.centerYAnchor.constraint(equalTo: background.centerYAnchor), close.heightAnchor.constraint(equalToConstant: 44),
            field.leadingAnchor.constraint(equalTo: background.contentView.leadingAnchor, constant: 12), field.trailingAnchor.constraint(equalTo: background.contentView.trailingAnchor, constant: -12), field.topAnchor.constraint(equalTo: background.contentView.topAnchor), field.bottomAnchor.constraint(equalTo: background.contentView.bottomAnchor)])
    }
    private func editing(_ active: Bool) {
        guard alive else { return }
        trailing?.constant = active ? -64 : -16
        close.isHidden = false
        UIView.animate(withDuration: 0.2, delay: 0, options: [.beginFromCurrentState, .curveEaseInOut]) {
            self.close.alpha = active ? 1 : 0; self.root.layoutIfNeeded()
        } completion: { [weak self] finished in
            guard let self, alive, finished, field.isFirstResponder == active else { return }
            close.isHidden = !active
        }
    }
    override func setProps(_ props: [String: String]) throws { let query = props["query"] ?? ""; if field.text != query { field.text = query } }
    override func destroy() { alive = false; field.resignFirstResponder() }
}
#endif

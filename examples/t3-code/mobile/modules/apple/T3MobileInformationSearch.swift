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
        let toolbar = UIView(); toolbar.translatesAutoresizingMaskIntoConstraints = false; root.addSubview(toolbar)
        let background = UIVisualEffectView(effect: effect); background.translatesAutoresizingMaskIntoConstraints = false; background.layer.cornerRadius = 24; background.clipsToBounds = true; toolbar.addSubview(background)
        field.placeholder = "Search packages"; field.accessibilityLabel = "Search open-source licenses"; field.autocorrectionType = .no; field.autocapitalizationType = .none; field.spellCheckingType = .no
        let searchFont = UIFont.preferredFont(forTextStyle: .title3)
        field.font = searchFont; field.adjustsFontForContentSizeCategory = true; field.textColor = .label; field.tintColor = .label
        field.attributedPlaceholder = NSAttributedString(string: "Search packages", attributes: [.foregroundColor: UIColor.secondaryLabel, .font: searchFont])
        field.borderStyle = .none; field.translatesAutoresizingMaskIntoConstraints = false; background.contentView.addSubview(field)
        field.addAction(UIAction { [weak self] _ in guard let self, alive else { return }; events.change(field.text ?? "") }, for: .editingChanged)
        field.addAction(UIAction { [weak self] _ in self?.editing(true) }, for: .editingDidBegin)
        field.addAction(UIAction { [weak self] _ in self?.editing(false) }, for: .editingDidEnd)
        if #available(iOS 26.0, *) {
            var configuration = UIButton.Configuration.glass()
            configuration.cornerStyle = .capsule; configuration.image = UIImage(systemName: "xmark"); configuration.baseForegroundColor = .label
            close.configuration = configuration
        } else { close.setImage(UIImage(systemName: "xmark"), for: .normal) }
        close.accessibilityLabel = "Dismiss search keyboard"; close.translatesAutoresizingMaskIntoConstraints = false; toolbar.addSubview(close)
        close.isHidden = true; close.alpha = 0
        close.addAction(UIAction { [weak self] _ in guard let self, alive else { return }; field.resignFirstResponder() }, for: .touchUpInside)
        let trailing = background.trailingAnchor.constraint(equalTo: toolbar.trailingAnchor); self.trailing = trailing
        let widthCap = toolbar.widthAnchor.constraint(lessThanOrEqualTo: root.widthAnchor, constant: -12); widthCap.priority = UILayoutPriority(999)
        let widthMinimum = toolbar.widthAnchor.constraint(greaterThanOrEqualToConstant: 300); widthMinimum.priority = UILayoutPriority(998)
        let widthIdeal = toolbar.widthAnchor.constraint(equalTo: root.widthAnchor, constant: -36); widthIdeal.priority = UILayoutPriority(997)
        NSLayoutConstraint.activate([toolbar.centerXAnchor.constraint(equalTo: root.centerXAnchor), toolbar.bottomAnchor.constraint(equalTo: root.bottomAnchor, constant: -4), toolbar.heightAnchor.constraint(equalToConstant: 48),
            toolbar.widthAnchor.constraint(greaterThanOrEqualToConstant: 260), toolbar.widthAnchor.constraint(lessThanOrEqualToConstant: 560), widthCap, widthMinimum, widthIdeal,
            background.leadingAnchor.constraint(equalTo: toolbar.leadingAnchor), background.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor), background.heightAnchor.constraint(equalToConstant: 48), trailing,
            close.trailingAnchor.constraint(equalTo: toolbar.trailingAnchor), close.widthAnchor.constraint(equalToConstant: 48), close.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor), close.heightAnchor.constraint(equalToConstant: 48),
            field.leadingAnchor.constraint(equalTo: background.contentView.leadingAnchor, constant: 14), field.trailingAnchor.constraint(equalTo: background.contentView.trailingAnchor, constant: -14), field.centerYAnchor.constraint(equalTo: background.contentView.centerYAnchor), field.heightAnchor.constraint(equalToConstant: 42)])
    }
    private func editing(_ active: Bool) {
        guard alive else { return }
        trailing?.constant = active ? -56 : 0
        close.isHidden = false
        UIView.animate(withDuration: 0.2, delay: 0, options: [.beginFromCurrentState, .curveEaseInOut]) {
            self.close.alpha = active ? 1 : 0; self.root.layoutIfNeeded()
        } completion: { [weak self] finished in
            guard let self, alive, finished, field.isFirstResponder == active else { return }
            close.isHidden = !active
        }
    }
    override func setProps(_ props: [String: String]) throws {
        let query = props["query"] ?? ""; if field.text != query { field.text = query }
        if props["dismiss-keyboard"] == "true" { field.resignFirstResponder() }
    }
    override func destroy() { alive = false; field.resignFirstResponder() }
}
#endif

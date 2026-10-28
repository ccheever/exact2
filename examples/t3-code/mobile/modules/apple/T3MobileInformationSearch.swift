#if os(iOS)
import UIKit
final class T3MobileInformationSearch {
    private final class RouteRef { weak var value: ExactRoute?; init(_ value: ExactRoute) { self.value = value } }
    private final class PortRef { weak var value: T3MobileInformationSearchPort?; init(_ value: T3MobileInformationSearchPort) { self.value = value } }
    private var routes: [String: RouteRef] = [:]
    private var ports: [String: PortRef] = [:]
    private var alive = true

    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard alive else { throw ExactNativeRefusal("Search session has ended.") }
        let port = T3MobileInformationSearchPort(owner: self, events: events)
        try port.setProps(props); return port
    }
    func configure(_ route: ExactRoute) {
        guard alive, route.isLive else { return }
        routes[route.key] = RouteRef(route); ports[route.key]?.value?.attach(route)
    }
    func end(_ route: ExactRoute) {
        guard routes[route.key]?.value === route else { return }
        ports[route.key]?.value?.detach(); routes.removeValue(forKey: route.key)
    }
    fileprivate func bind(_ port: T3MobileInformationSearchPort, key: String) {
        guard alive else { return }
        if let previous = ports[key]?.value, previous !== port { previous.detach() }
        ports[key] = PortRef(port)
        if let route = routes[key]?.value { port.attach(route) }
    }
    fileprivate func unbind(_ port: T3MobileInformationSearchPort, key: String) {
        if ports[key]?.value === port { ports.removeValue(forKey: key) }
    }
    fileprivate func isCurrent(_ port: T3MobileInformationSearchPort, key: String, route: ExactRoute) -> Bool {
        alive && ports[key]?.value === port && routes[key]?.value === route
    }
    func destroy() {
        alive = false; for port in ports.values { port.value?.detach() }
        ports.removeAll(); routes.removeAll()
    }
}

private final class T3MobileInformationSearchPort: ExactNativeInstance {
    private weak var owner: T3MobileInformationSearch?
    private weak var route: ExactRoute?
    private weak var host: UIView?
    private let root = UIView()
    private var key = "", query = ""
    private var placeholder = "Search packages", searchLabel = "Search open-source licenses"
    private var dismissRequested = false, alive = true
    private var generation = 0
    private var toolbar: UIView?, field: UISearchTextField?, close: UIButton?
    private var constraints: [NSLayoutConstraint] = []
    private var trailing: NSLayoutConstraint?, resting: NSLayoutConstraint?, keyboard: NSLayoutConstraint?
    override var view: UIView { root }

    init(owner: T3MobileInformationSearch, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events)
        root.isUserInteractionEnabled = false; root.accessibilityElementsHidden = true
    }
    override func setProps(_ props: [String: String]) throws {
        guard alive, let nextKey = props["route-key"], !nextKey.isEmpty else {
            throw ExactNativeRefusal("Search needs its route key.")
        }
        if key != nextKey {
            detach(); owner?.unbind(self, key: key); key = nextKey
        }
        query = props["query"] ?? ""; dismissRequested = props["dismiss-keyboard"] == "true"
        placeholder = props["search-placeholder"] ?? "Search packages"
        searchLabel = props["search-label"] ?? "Search open-source licenses"
        owner?.bind(self, key: key)
        if let field {
            configureLabels(field)
            if field.text != query { field.text = query }
            if dismissRequested { field.resignFirstResponder() }
        }
    }
    fileprivate func attach(_ route: ExactRoute) {
        guard alive, route.isLive, route.key == key, owner?.isCurrent(self, key: key, route: route) == true else { return }
        if self.route === route, let host, route.controller.viewIfLoaded === host, toolbar?.superview === host { return }
        detach(); self.route = route
        install(in: route)
    }
    private func accepts(_ field: UISearchTextField, in route: ExactRoute) -> Bool {
        alive && self.field === field && self.route === route && route.isLive && owner?.isCurrent(self, key: key, route: route) == true
    }
    private func configureLabels(_ field: UISearchTextField) {
        field.accessibilityLabel = searchLabel
        if field.attributedPlaceholder?.string != placeholder {
            field.attributedPlaceholder = NSAttributedString(string: placeholder,
                attributes: [.foregroundColor: UIColor.secondaryLabel, .font: field.font ?? UIFont.preferredFont(forTextStyle: .title3)])
        }
    }
    private func install(in route: ExactRoute) {
        let host = route.controller.view!
        self.host = host
        let toolbar = UIView(); toolbar.translatesAutoresizingMaskIntoConstraints = false; toolbar.layer.zPosition = 1
        host.addSubview(toolbar); self.toolbar = toolbar
        route.owns(view: toolbar, "\(searchLabel) and keyboard dismiss", surface: true)
        let effect: UIVisualEffect
        if #available(iOS 26.0, *) { let glass = UIGlassEffect(); glass.isInteractive = true; effect = glass } else { effect = UIBlurEffect(style: .systemMaterial) }
        let background = UIVisualEffectView(effect: effect); background.translatesAutoresizingMaskIntoConstraints = false; background.layer.cornerRadius = 24; background.clipsToBounds = true; toolbar.addSubview(background)
        let field = UISearchTextField(); self.field = field
        field.autocorrectionType = .no; field.autocapitalizationType = .none; field.spellCheckingType = .no
        let searchFont = UIFont.preferredFont(forTextStyle: .title3)
        field.font = searchFont; field.adjustsFontForContentSizeCategory = true; field.textColor = .label; field.tintColor = .label
        configureLabels(field)
        field.borderStyle = .none; field.translatesAutoresizingMaskIntoConstraints = false; background.contentView.addSubview(field)
        field.text = query
        field.addAction(UIAction { [weak self, weak field, weak route] _ in
            guard let self, let field, let route, self.accepts(field, in: route) else { return }
            self.events.change(field.text ?? "")
        }, for: .editingChanged)
        field.addAction(UIAction { [weak self, weak field, weak route] _ in
            guard let self, let field, let route, self.accepts(field, in: route) else { return }
            self.editing(true, field: field, route: route)
        }, for: .editingDidBegin)
        field.addAction(UIAction { [weak self, weak field, weak route] _ in
            guard let self, let field, let route, self.accepts(field, in: route) else { return }
            self.editing(false, field: field, route: route)
        }, for: .editingDidEnd)
        let close = UIButton(type: .system); self.close = close
        if #available(iOS 26.0, *) {
            var configuration = UIButton.Configuration.glass()
            configuration.cornerStyle = .capsule; configuration.image = UIImage(systemName: "xmark"); configuration.baseForegroundColor = .label
            close.configuration = configuration
        } else { close.setImage(UIImage(systemName: "xmark"), for: .normal) }
        close.accessibilityLabel = "Dismiss search keyboard"; close.translatesAutoresizingMaskIntoConstraints = false; toolbar.addSubview(close)
        close.isHidden = true; close.alpha = 0
        close.addAction(UIAction { [weak self, weak field, weak route] _ in
            guard let self, let field, let route, self.accepts(field, in: route) else { return }
            field.resignFirstResponder()
        }, for: .touchUpInside)
        let trailing = background.trailingAnchor.constraint(equalTo: toolbar.trailingAnchor); self.trailing = trailing
        let widthCap = toolbar.widthAnchor.constraint(lessThanOrEqualTo: host.widthAnchor, constant: -12); widthCap.priority = UILayoutPriority(999)
        let widthMinimum = toolbar.widthAnchor.constraint(greaterThanOrEqualToConstant: 300); widthMinimum.priority = UILayoutPriority(998)
        let widthIdeal = toolbar.widthAnchor.constraint(equalTo: host.widthAnchor, constant: -36); widthIdeal.priority = UILayoutPriority(997)
        let resting = toolbar.bottomAnchor.constraint(equalTo: host.safeAreaLayoutGuide.bottomAnchor, constant: -4); resting.priority = .defaultHigh
        let keyboard = toolbar.bottomAnchor.constraint(equalTo: host.keyboardLayoutGuide.topAnchor, constant: -4); keyboard.priority = .defaultLow
        self.resting = resting; self.keyboard = keyboard
        constraints = [toolbar.centerXAnchor.constraint(equalTo: host.centerXAnchor), toolbar.heightAnchor.constraint(equalToConstant: 48),
            toolbar.bottomAnchor.constraint(lessThanOrEqualTo: host.safeAreaLayoutGuide.bottomAnchor, constant: -4), resting, keyboard,
            toolbar.widthAnchor.constraint(greaterThanOrEqualToConstant: 260), toolbar.widthAnchor.constraint(lessThanOrEqualToConstant: 560), widthCap, widthMinimum, widthIdeal,
            background.leadingAnchor.constraint(equalTo: toolbar.leadingAnchor), background.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor), background.heightAnchor.constraint(equalToConstant: 48), trailing,
            close.trailingAnchor.constraint(equalTo: toolbar.trailingAnchor), close.widthAnchor.constraint(equalToConstant: 48), close.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor), close.heightAnchor.constraint(equalToConstant: 48),
            field.leadingAnchor.constraint(equalTo: background.contentView.leadingAnchor, constant: 14), field.trailingAnchor.constraint(equalTo: background.contentView.trailingAnchor, constant: -14), field.centerYAnchor.constraint(equalTo: background.contentView.centerYAnchor), field.heightAnchor.constraint(equalToConstant: 42)]
        NSLayoutConstraint.activate(constraints)
        if dismissRequested { field.resignFirstResponder() }
    }
    private func editing(_ active: Bool, field: UISearchTextField, route: ExactRoute) {
        guard accepts(field, in: route), let close, let host else { return }
        resting?.priority = active ? .defaultLow : .defaultHigh
        keyboard?.priority = active ? .defaultHigh : .defaultLow
        trailing?.constant = active ? -56 : 0
        close.isHidden = false
        let capturedGeneration = generation
        UIView.animate(withDuration: 0.2, delay: 0, options: [.beginFromCurrentState, .curveEaseInOut]) {
            close.alpha = active ? 1 : 0; host.layoutIfNeeded()
        } completion: { [weak self, weak field, weak close, weak route] finished in
            guard let self, let field, let close, let route, self.alive, finished, self.generation == capturedGeneration,
                  self.accepts(field, in: route), field.isFirstResponder == active else { return }
            close.isHidden = !active
        }
    }
    fileprivate func detach() {
        generation += 1; route = nil
        let previousField = field; field = nil
        previousField?.resignFirstResponder()
        NSLayoutConstraint.deactivate(constraints); constraints.removeAll()
        toolbar?.removeFromSuperview(); toolbar = nil; close = nil
        trailing = nil; resting = nil; keyboard = nil; host = nil
    }
    override func destroy() {
        alive = false; detach(); owner?.unbind(self, key: key)
    }
}
#endif

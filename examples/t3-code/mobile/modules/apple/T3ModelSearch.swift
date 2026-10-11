#if os(iOS)
// Pinned365aa87982 ThreadSettingsSheet + RNS mail-search toolbar patch.
// @ref llp/1109.005-composer-and-transcript.decision.md#settings-ownership
import UIKit

/// The port is a zero-size configuration anchor. Only its own 48pt toolbar
/// enters the route's public UIKit view, so keyboard lifting keeps a real hit area.
final class T3ModelSearch {
    private final class RouteRef { weak var value: ExactRoute?; init(_ value: ExactRoute) { self.value = value } }
    private final class PortRef { weak var value: T3ModelSearchPort?; init(_ value: T3ModelSearchPort) { self.value = value } }
    private var routes: [String: RouteRef] = [:]
    private var ports: [String: PortRef] = [:]
    private var alive = true
    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard alive else { throw ExactNativeRefusal("Model search session has ended.") }
        let port = T3ModelSearchPort(owner: self, events: events)
        try port.setProps(props); return port
    }
    func configure(_ route: ExactRoute) {
        guard alive else { return }
        routes[route.key] = RouteRef(route); ports[route.key]?.value?.attach(route)
    }
    func end(_ route: ExactRoute) {
        guard routes[route.key]?.value === route else { return }
        ports[route.key]?.value?.detach(); routes.removeValue(forKey: route.key)
    }
    fileprivate func bind(_ port: T3ModelSearchPort, key: String) {
        guard alive else { return }
        ports[key] = PortRef(port)
        if let route = routes[key]?.value { port.attach(route) }
    }
    fileprivate func unbind(_ port: T3ModelSearchPort, key: String) {
        if ports[key]?.value === port { ports.removeValue(forKey: key) }
    }
    func destroy() {
        alive = false; for port in ports.values { port.value?.detach() }
        ports.removeAll(); routes.removeAll()
    }
}

private struct T3ModelSearchConfiguration: Decodable {
    struct Filter: Decodable { let id: String; let label: String; let selected: Bool; let disabled: Bool }
    let filters: [Filter]; let filter: String; let showLegacy: Bool; let hasLegacy: Bool
}
private final class T3ModelSearchAnchor: UIView {
    var changed: (() -> Void)?
    override func layoutSubviews() { super.layoutSubviews(); changed?() }
    override func didMoveToWindow() { super.didMoveToWindow(); changed?() }
}
private final class T3ModelSearchDelegate: NSObject, UITextFieldDelegate {
    var editing: ((UITextField, Bool) -> Void)?
    func textFieldDidBeginEditing(_ textField: UITextField) { editing?(textField, true) }
    func textFieldDidEndEditing(_ textField: UITextField) { editing?(textField, false) }
}
private final class T3ModelSearchPort: ExactNativeInstance {
    private weak var owner: T3ModelSearch?
    private weak var route: ExactRoute?
    private weak var host: UIView?
    private let root = T3ModelSearchAnchor()
    private let delegate = T3ModelSearchDelegate()
    private var key = "", identity = "", query = "", configurationText = ""
    private var configuration: T3ModelSearchConfiguration?
    private var visible = false, alive = true
    private var admission = 0, menuRevision = 0
    private var bar: UIView?, field: UISearchTextField?, filter: UIButton?, dismiss: UIButton?
    private var constraints: [NSLayoutConstraint] = []
    private var width: NSLayoutConstraint?, leading: NSLayoutConstraint?, trailing: NSLayoutConstraint?
    private var resting: NSLayoutConstraint?, keyboard: NSLayoutConstraint?
    private var foreground: UIColor = .label, muted: UIColor = .secondaryLabel
    private let inputID = UIAction.Identifier("t3-model-search-input")
    override var view: UIView { root }
    override var focusTarget: UIView? { admitted ? field : nil }

    init(owner: T3ModelSearch, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events)
        root.isUserInteractionEnabled = false; root.accessibilityElementsHidden = true
        root.changed = { [weak self] in self?.updateWidth() }
    }
    override func setProps(_ props: [String: String]) throws {
        guard alive, let nextKey = props["route-key"], !nextKey.isEmpty,
              let nextIdentity = props["owner"], nextIdentity.hasPrefix(nextKey + ":"),
              let text = props["configuration"], let data = text.data(using: .utf8),
              let next = try? JSONDecoder().decode(T3ModelSearchConfiguration.self, from: data) else {
            throw ExactNativeRefusal("Model search needs its route, owner and catalog configuration.")
        }
        let changedOwner = key != nextKey || identity != nextIdentity
        if changedOwner {
            detach(); owner?.unbind(self, key: key); key = nextKey; identity = nextIdentity
        }
        let wasVisible = visible
        visible = props["visible"] == "true"; query = props["query"] ?? ""
        let changedMenu = configurationText != text
        configurationText = text; configuration = next
        foreground = T3HomeChrome.color(props["search-foreground"] ?? "")
        muted = T3HomeChrome.color(props["search-muted"] ?? "")
        owner?.bind(self, key: key)
        if !visible {
            suspend()
            replaceQuery()
        } else {
            refresh()
            if !wasVisible || changedOwner { replaceQuery() }
        }
        if changedMenu { updateMenu() }
        updateColors()
    }
    fileprivate func attach(_ route: ExactRoute) {
        guard alive, route.key == key else { return }
        if self.route !== route { detach(); self.route = route }
        refresh()
    }
    private var admitted: Bool {
        guard alive, visible, let route, route.isLive, route.key == key,
              route.controller.navigationController?.topViewController === route.controller,
              route.controller.presentedViewController == nil,
              let host, route.controller.viewIfLoaded === host, host.window != nil,
              bar?.superview === host, bar?.isHidden == false else { return false }
        return true
    }
    private func accepts(_ token: Int, _ captured: String, _ route: ExactRoute?) -> Bool {
        admitted && token == admission && captured == identity && route === self.route
    }
    private func refresh() {
        guard alive, visible, let route, route.isLive else { return }
        if bar == nil, #available(iOS 26.0, *) { install(in: route) }
        guard let bar else { return }
        if bar.isHidden { bar.isHidden = false; activate() }
        updateWidth(); updateColors()
    }
    @available(iOS 26.0, *)
    private func install(in route: ExactRoute) {
        let host = route.controller.view!
        self.host = host
        let bar = UIView(); bar.translatesAutoresizingMaskIntoConstraints = false; bar.isHidden = true
        bar.layer.zPosition = 1
        host.addSubview(bar); self.bar = bar
        route.owns(view: bar, "Model search, filter and keyboard dismiss", surface: true)
        let effect = UIGlassEffect(style: .regular); effect.isInteractive = true
        let glass = UIVisualEffectView(effect: effect); glass.translatesAutoresizingMaskIntoConstraints = false
        glass.layer.cornerRadius = 24; glass.clipsToBounds = true; bar.addSubview(glass)
        func button(_ image: String, _ label: String, _ identifier: String) -> UIButton {
            var config = UIButton.Configuration.glass(); config.cornerStyle = .capsule
            config.image = UIImage(systemName: image)
            let button = UIButton(configuration: config); button.translatesAutoresizingMaskIntoConstraints = false
            button.accessibilityLabel = label; button.accessibilityIdentifier = identifier; bar.addSubview(button)
            constraints += [button.widthAnchor.constraint(equalToConstant: 48), button.heightAnchor.constraint(equalToConstant: 48), button.centerYAnchor.constraint(equalTo: bar.centerYAnchor)]
            return button
        }
        let filter = button("line.3.horizontal.decrease", "Filter models", "thread-settings-model-filter")
        filter.showsMenuAsPrimaryAction = true; self.filter = filter
        let dismiss = button("xmark", "Dismiss search keyboard", "thread-settings-model-search-dismiss")
        dismiss.isHidden = true; dismiss.alpha = 0; self.dismiss = dismiss
        let field = UISearchTextField(); field.translatesAutoresizingMaskIntoConstraints = false
        field.placeholder = "Find a model"; field.font = .preferredFont(forTextStyle: .title3)
        field.adjustsFontForContentSizeCategory = true; field.borderStyle = .none
        field.autocapitalizationType = .none; field.autocorrectionType = .no; field.spellCheckingType = .no
        field.accessibilityIdentifier = "thread-settings-model-search-text"
        field.delegate = delegate; glass.contentView.addSubview(field); self.field = field
        let leading = glass.leadingAnchor.constraint(equalTo: bar.leadingAnchor, constant: 56)
        let trailing = glass.trailingAnchor.constraint(equalTo: bar.trailingAnchor)
        let resting = bar.bottomAnchor.constraint(equalTo: host.safeAreaLayoutGuide.bottomAnchor, constant: -4); resting.priority = .defaultHigh
        let keyboard = bar.bottomAnchor.constraint(equalTo: host.keyboardLayoutGuide.topAnchor, constant: -4); keyboard.priority = .defaultLow
        let width = bar.widthAnchor.constraint(equalToConstant: 300)
        self.leading = leading; self.trailing = trailing; self.resting = resting; self.keyboard = keyboard; self.width = width
        constraints += [bar.centerXAnchor.constraint(equalTo: host.centerXAnchor), width, bar.heightAnchor.constraint(equalToConstant: 48),
            bar.bottomAnchor.constraint(lessThanOrEqualTo: host.safeAreaLayoutGuide.bottomAnchor, constant: -4), resting, keyboard,
            leading, trailing, glass.topAnchor.constraint(equalTo: bar.topAnchor), glass.bottomAnchor.constraint(equalTo: bar.bottomAnchor),
            filter.leadingAnchor.constraint(equalTo: bar.leadingAnchor), dismiss.trailingAnchor.constraint(equalTo: bar.trailingAnchor),
            field.leadingAnchor.constraint(equalTo: glass.contentView.leadingAnchor, constant: 14), field.trailingAnchor.constraint(equalTo: glass.contentView.trailingAnchor, constant: -14),
            field.centerYAnchor.constraint(equalTo: glass.contentView.centerYAnchor), field.heightAnchor.constraint(equalToConstant: 42)]
        NSLayoutConstraint.activate(constraints)
        replaceQuery()
    }
    private func activate() {
        admission += 1
        let token = admission, captured = identity
        weak var capturedRoute = route
        field?.addAction(UIAction(identifier: inputID) { [weak self, weak field] _ in
            guard let self, let field, self.field === field, self.accepts(token, captured, capturedRoute) else { return }
            self.events.change(field.text ?? "")
        }, for: .editingChanged)
        delegate.editing = { [weak self] field, active in
            guard let self, self.field === field, self.accepts(token, captured, capturedRoute) else { return }
            self.editing(active)
        }
        dismiss?.addAction(UIAction(identifier: inputID) { [weak self] _ in
            guard let self, self.accepts(token, captured, capturedRoute) else { return }
            self.field?.resignFirstResponder()
        }, for: .touchUpInside)
        updateMenu(); editing(false, animated: false)
    }
    private func updateMenu() {
        menuRevision += 1
        guard let configuration else { return }
        let token = admission, captured = identity, revision = menuRevision
        weak var capturedRoute = route
        func valid(_ port: T3ModelSearchPort) -> Bool {
            port.accepts(token, captured, capturedRoute) && revision == port.menuRevision
        }
        let providers = configuration.filters.map { choice in
            UIAction(title: choice.label, attributes: choice.disabled ? [.disabled] : [], state: choice.selected ? .on : .off) { [weak self] _ in
                guard let self, valid(self) else { return }; self.events.message(choice.id)
            }
        }
        var items: [UIMenuElement] = [UIMenu(title: "Provider", children: providers)]
        if configuration.hasLegacy {
            items.append(UIAction(title: "Show legacy models", state: configuration.showLegacy ? .on : .off) { [weak self] _ in
                guard let self, valid(self) else { return }; self.events.press()
            })
        }
        filter?.menu = UIMenu(title: "Model filters", children: items)
        filter?.setImage(UIImage(systemName: !configuration.filter.isEmpty || configuration.showLegacy ? "line.3.horizontal.decrease.circle.fill" : "line.3.horizontal.decrease"), for: .normal)
    }
    private func replaceQuery() {
        // Root echoes must not replace native in-progress text or its caret.
        if field?.text != query { field?.text = query }
    }
    private func updateColors() {
        field?.textColor = foreground; field?.tintColor = foreground
        field?.attributedPlaceholder = NSAttributedString(string: "Find a model", attributes: [.font: UIFont.preferredFont(forTextStyle: .title3), .foregroundColor: muted])
        filter?.tintColor = foreground; dismiss?.tintColor = foreground
    }
    private func updateWidth() {
        guard let host else { return }
        let resolved = min(min(560, max(300, host.bounds.width - 36)), max(260, host.bounds.width - 12))
        if width?.constant != resolved { width?.constant = resolved }
    }
    private func editing(_ active: Bool, animated: Bool = true) {
        keyboard?.priority = active ? .defaultHigh : .defaultLow
        resting?.priority = active ? .defaultLow : .defaultHigh
        leading?.constant = active ? 0 : 56; trailing?.constant = active ? -56 : 0
        filter?.isHidden = false; dismiss?.isHidden = false
        let token = admission
        let changes = { self.filter?.alpha = active ? 0 : 1; self.dismiss?.alpha = active ? 1 : 0; self.host?.layoutIfNeeded() }
        let completion: (Bool) -> Void = { [weak self] _ in
            guard let self, self.admission == token, self.field?.isFirstResponder == active else { return }
            self.filter?.isHidden = active; self.dismiss?.isHidden = !active
        }
        if animated { UIView.animate(withDuration: 0.2, delay: 0, options: [.beginFromCurrentState, .curveEaseInOut], animations: changes, completion: completion) }
        else { changes(); completion(true) }
    }
    private func suspend() {
        admission += 1; menuRevision += 1
        delegate.editing = nil
        field?.removeAction(identifiedBy: inputID, for: .editingChanged)
        dismiss?.removeAction(identifiedBy: inputID, for: .touchUpInside)
        field?.resignFirstResponder(); bar?.isHidden = true
        keyboard?.priority = .defaultLow; resting?.priority = .defaultHigh
    }
    fileprivate func detach() {
        suspend(); NSLayoutConstraint.deactivate(constraints); constraints.removeAll()
        bar?.removeFromSuperview(); bar = nil; field?.delegate = nil; field = nil; filter = nil; dismiss = nil
        width = nil; leading = nil; trailing = nil; resting = nil; keyboard = nil; host = nil; route = nil
    }
    override func agentInput(_ input: ExactNativeInput) throws {
        guard admitted, let field else { throw ExactNativeRefusal("Model search is not visible.") }
        switch input {
        case .text(let text): field.text = text; events.change(text)
        case .key(let key, let phase):
            guard key == "Escape" || key == "Enter" else { throw ExactNativeRefusal("Model search accepts Escape or Enter.") }
            if phase != "up" { field.resignFirstResponder() }
        }
    }
    override func destroy() { alive = false; detach(); owner?.unbind(self, key: key); root.changed = nil }
}
#endif

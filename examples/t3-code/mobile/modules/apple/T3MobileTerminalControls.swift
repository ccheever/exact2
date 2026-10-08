#if os(iOS)
// Pinned365aa87982 ThreadTerminalRouteScreen toolbar actions; UIKit owns keyboard lifetime.
// @ref llp/1109.007-mobile-terminal.decision.md#native-renderer
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

/// Same public route-hook ownership used by the accepted thread-header adapter.
final class T3MobileTerminalMenus {
    private final class RouteRef { weak var value: ExactRoute?; init(_ value: ExactRoute) { self.value = value } }
    private final class MenuRef { weak var value: T3MobileTerminalMenu?; init(_ value: T3MobileTerminalMenu) { self.value = value } }
    private var routes: [String: RouteRef] = [:], menus: [String: MenuRef] = [:]
    private var alive = true
    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard alive else { throw ExactNativeRefusal("The terminal session has ended.") }
        let menu = T3MobileTerminalMenu(owner: self, events: events); try menu.setProps(props); return menu
    }
    func configure(_ route: ExactRoute) {
        guard alive else { return }; routes[route.key] = RouteRef(route); menus[route.key]?.value?.attach(route)
    }
    func end(_ route: ExactRoute) {
        guard routes[route.key]?.value === route else { return }
        menus[route.key]?.value?.detach(); routes.removeValue(forKey: route.key)
    }
    fileprivate func bind(_ menu: T3MobileTerminalMenu, key: String) {
        guard alive else { return }
        if let old = menus[key]?.value, old !== menu { old.detach() }
        menus[key] = MenuRef(menu)
        if let route = routes[key]?.value { menu.attach(route) }
    }
    fileprivate func unbind(_ menu: T3MobileTerminalMenu, key: String) {
        if menus[key]?.value === menu { menus.removeValue(forKey: key) }
    }
    func destroy() { alive = false; for ref in menus.values { ref.value?.detach() }; menus.removeAll(); routes.removeAll() }
}

/// Pinned TerminalHeader menu on its actual header button. Actions carry the captured session.
final class T3MobileTerminalMenu: ExactNativeInstance {
    private weak var owner: T3MobileTerminalMenus?
    private weak var route: ExactRoute?
    private var routeKey = ""
    private var group: UIBarButtonItemGroup?
    private var rendered = "", renderedTint: UIColor?
    private let root = UIView()
    private let button = UIButton(type: .system)
    private var configuration = "", alive = true
    override var view: UIView { root }
    init(owner: T3MobileTerminalMenus, events: ExactNativeEvents) {
        self.owner = owner
        super.init(events: events)
        root.isUserInteractionEnabled = false; root.accessibilityElementsHidden = true
        button.showsMenuAsPrimaryAction = true
        button.setImage(UIImage(systemName: "terminal", withConfiguration: UIImage.SymbolConfiguration(pointSize: 20)), for: .normal)
        button.accessibilityLabel = "Terminal options"
        button.accessibilityIdentifier = "terminal-options"
    }
    override func setProps(_ props: [String: String]) throws {
        let key = props["route-key"] ?? ""
        guard !key.isEmpty else { throw ExactNativeRefusal("The terminal route is unavailable.") }
        if key != routeKey { detach(); owner?.unbind(self, key: routeKey); routeKey = key }
        let source = props["terminal-source"] ?? ""
        if source.isEmpty { configuration = ""; button.menu = nil; button.isEnabled = false; owner?.bind(self, key: routeKey); refresh(); return }
        guard let data = try JSONSerialization.jsonObject(with: Data(source.utf8)) as? [String: Any],
              let key = data["key"] as? String, !key.isEmpty else { throw ExactNativeRefusal("The terminal menu is unavailable.") }
        configuration = source; button.isEnabled = true
        button.tintColor = T3MobileTerminalAccessory.color(props["menu-tint"] ?? "")
        let size = data["fontSize"] as? Double ?? 10.5
        let font = size.isFinite ? max(6, min(14, size)) : 10.5
        let readOnly = data["readOnly"] as? Bool ?? true
        func item(_ title: String, _ operation: String, value: String = "", symbol: String = "", subtitle: String = "", enabled: Bool = true, selected: Bool = false) -> UIAction {
            let action = UIAction(title: title, image: UIImage(systemName: symbol), attributes: enabled ? [] : [.disabled], state: selected ? .on : .off) { [weak self] _ in
                guard let self, alive, configuration == source, let route, route.isLive,
                      route.key == routeKey, route.controller.navigationController?.topViewController === route.controller,
                      route.controller.viewIfLoaded?.window != nil, route.controller.presentedViewController == nil else { return }
                events.change(T3MobileTerminalView.json(["type": "menu", "key": key, "action": operation, "text": value]))
            }
            action.subtitle = subtitle.isEmpty ? nil : subtitle
            return action
        }
        let text = UIMenu(title: "Text size", image: UIImage(systemName: "textformat.size"), options: [.displayInline], children: [
            item(String(format: "A- %.1f pt", max(6, font - 0.5)), "font-decrease", enabled: font > 6),
            item(String(format: "A+ %.1f pt", min(14, font + 0.5)), "font-increase", enabled: font < 14)])
        var children: [UIMenuElement] = [text]
        for tab in data["tabs"] as? [[String: Any]] ?? [] {
            guard let id = tab["id"] as? String, !id.isEmpty else { continue }
            let status = tab["status"] as? String ?? "closed"
            let label = status == "running" ? (tab["running"] as? Bool == true ? "Task running" : "Ready") :
                ["starting": "Starting", "exited": "Exited", "error": "Error"][status] ?? "Not started"
            let cwd = Self.basename(tab["cwd"] as? String ?? "")
            children.append(item(tab["label"] as? String ?? "Terminal", "select", value: id, symbol: "terminal",
                subtitle: [label, cwd].filter { !$0.isEmpty }.joined(separator: " · "), selected: tab["selected"] as? Bool == true))
        }
        let workspace = Self.basename(data["workspaceRoot"] as? String ?? "")
        children.append(item("Open new terminal", "new", symbol: "plus",
            subtitle: "Start another shell in " + (workspace.isEmpty ? "this workspace" : workspace), enabled: !readOnly))
        button.menu = UIMenu(children: children)
        owner?.bind(self, key: routeKey); refresh()
    }
    fileprivate func attach(_ route: ExactRoute) {
        if self.route !== route { detach(); self.route = route }
        refresh()
    }
    private func refresh() {
        guard alive, let route, route.isLive else { return }
        guard !configuration.isEmpty else { removeItem(); return }
        if let group, rendered == configuration, renderedTint == button.tintColor {
            if !route.controller.navigationItem.trailingItemGroups.contains(where: { $0 === group }) {
                route.controller.navigationItem.trailingItemGroups = [group]
            }
            return
        }
        let item = UIBarButtonItem(image: UIImage(systemName: "terminal"), menu: button.menu)
        item.accessibilityLabel = "Terminal options"; item.accessibilityIdentifier = "terminal-options"
        item.isEnabled = button.isEnabled; item.tintColor = button.tintColor
        let next = UIBarButtonItemGroup.fixedGroup(representativeItem: nil, items: [item])
        group = next; rendered = configuration; renderedTint = button.tintColor
        route.controller.navigationItem.trailingItemGroups = [next]
    }
    private func removeItem() {
        if let item = route?.controller.navigationItem, let group {
            item.trailingItemGroups = item.trailingItemGroups.filter { $0 !== group }
        }
        group = nil; rendered = ""; renderedTint = nil
    }
    fileprivate func detach() { removeItem(); route = nil }
    private static func basename(_ path: String) -> String {
        guard !path.isEmpty else { return "" }
        let trimmed = path.replacingOccurrences(of: "/+$", with: "", options: .regularExpression)
        return trimmed.isEmpty ? "/" : String(trimmed.split(separator: "/").last ?? "")
    }
    override func destroy() { alive = false; configuration = ""; button.menu = nil; detach(); owner?.unbind(self, key: routeKey) }
}

/// Native keyboard visibility keeps the floating button out of the input accessory.
final class T3MobileTerminalChrome: UIView {
    private let keyboard = UIButton(type: .system), capture = UIButton(type: .system)
    private var observers: [NSObjectProtocol] = []
    private var keyboardFrame: CGRect = .null
    var readOnly = true { didSet { updateKeyboard() } }
    init(surface: UIView, action: @escaping (String) -> Void) {
        super.init(frame: .zero)
        surface.translatesAutoresizingMaskIntoConstraints = false; addSubview(surface)
        capture.setTitle("Attach visible output", for: .normal)
        capture.titleLabel?.font = UIFont(name: "DMSans-Regular", size: 14) ?? .systemFont(ofSize: 14)
        capture.contentHorizontalAlignment = .leading
        capture.accessibilityLabel = "Attach visible output"; capture.accessibilityIdentifier = "terminal-capture"
        capture.addAction(UIAction { _ in action("capture") }, for: .touchUpInside)
        capture.translatesAutoresizingMaskIntoConstraints = false; addSubview(capture)
        let effect: UIVisualEffect
        if #available(iOS 26.0, *) { effect = UIGlassEffect(style: .regular) }
        else { effect = UIBlurEffect(style: .systemMaterial) }
        let glass = UIVisualEffectView(effect: effect); glass.isUserInteractionEnabled = false
        glass.layer.cornerRadius = 24; glass.clipsToBounds = true
        glass.translatesAutoresizingMaskIntoConstraints = false; keyboard.addSubview(glass)
        keyboard.setImage(UIImage(systemName: "keyboard", withConfiguration: UIImage.SymbolConfiguration(pointSize: 20)), for: .normal)
        if let icon = keyboard.imageView { keyboard.bringSubviewToFront(icon) }
        keyboard.accessibilityLabel = "Show keyboard"; keyboard.accessibilityIdentifier = "terminal-keyboard"
        keyboard.addAction(UIAction { _ in action("show-keyboard") }, for: .touchUpInside)
        keyboard.addAction(UIAction { [weak keyboard] _ in keyboard?.alpha = 0.72 }, for: .touchDown)
        keyboard.addAction(UIAction { [weak keyboard] _ in keyboard?.alpha = 1 }, for: [.touchUpInside, .touchUpOutside, .touchCancel])
        keyboard.translatesAutoresizingMaskIntoConstraints = false; addSubview(keyboard)
        NSLayoutConstraint.activate([
            surface.topAnchor.constraint(equalTo: topAnchor), surface.leadingAnchor.constraint(equalTo: leadingAnchor), surface.trailingAnchor.constraint(equalTo: trailingAnchor),
            surface.bottomAnchor.constraint(equalTo: capture.topAnchor), capture.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            capture.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14), capture.bottomAnchor.constraint(equalTo: bottomAnchor), capture.heightAnchor.constraint(equalToConstant: 34),
            keyboard.widthAnchor.constraint(equalToConstant: 48), keyboard.heightAnchor.constraint(equalToConstant: 48),
            keyboard.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -16), keyboard.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -16),
            glass.leadingAnchor.constraint(equalTo: keyboard.leadingAnchor), glass.trailingAnchor.constraint(equalTo: keyboard.trailingAnchor),
            glass.topAnchor.constraint(equalTo: keyboard.topAnchor), glass.bottomAnchor.constraint(equalTo: keyboard.bottomAnchor)])
        for name in [UIResponder.keyboardWillChangeFrameNotification, UIResponder.keyboardWillHideNotification] {
            observers.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] note in
                self?.keyboardFrame = name == UIResponder.keyboardWillHideNotification ? .null : (note.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect ?? .null)
                self?.updateKeyboard()
            })
        }
        updateKeyboard()
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override func didMoveToWindow() { super.didMoveToWindow(); updateKeyboard() }
    override func layoutSubviews() { super.layoutSubviews(); updateKeyboard() }
    private func updateKeyboard() {
        guard let window else { keyboard.isHidden = true; return }
        let visible = !keyboardFrame.isNull && window.convert(keyboardFrame, from: window.screen.coordinateSpace).intersects(window.bounds)
        keyboard.isHidden = readOnly || visible
    }
    func colors(background: String, foreground: String) {
        backgroundColor = T3MobileTerminalAccessory.color(background)
        capture.setTitleColor(T3MobileTerminalAccessory.color(foreground), for: .normal)
        keyboard.tintColor = T3MobileTerminalAccessory.color(foreground)
    }
    func destroy() { observers.forEach(NotificationCenter.default.removeObserver); observers.removeAll() }
    deinit { destroy() }
}

#endif

#if os(iOS)
extension T3MobileTerminalAccessory {
    func colors(background: String, foreground: String, border: String) {
        backgroundColor = Self.color(background); tintColor = Self.color(foreground)
        layer.borderColor = Self.color(border).cgColor; layer.borderWidth = 0.5
    }
    static func color(_ text: String) -> UIColor {
        let hex = text.hasPrefix("#") ? String(text.dropFirst()) : text
        guard hex.count == 6, let value = UInt32(hex, radix: 16) else { return .label }
        return UIColor(red: CGFloat((value >> 16) & 255) / 255, green: CGFloat((value >> 8) & 255) / 255, blue: CGFloat(value & 255) / 255, alpha: 1)
    }
}
#endif

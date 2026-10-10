#if os(iOS)
// @ref llp/1109.002-design-system-parity.spec.md#semantic-colors
// @ref llp/1109.004-home-projection.decision.md#home-title-fitting
// Pinned T3 Code 365aa87982: HomeHeader, WorkspaceConnectionTitle,
// ThreadNavigationSidebar, and patches/react-native-screens@4.28.0.patch.
import UIKit

/// Per-session coordinator. Exact owns the route, list and navigation stack.
/// This app owns UINavigationItem content and the app-specific search toolbar.
final class T3HomeChrome {
    private let swipes: T3MobileHomeSwipes
    init(swipes: T3MobileHomeSwipes) { self.swipes = swipes }
    let rowMenus = T3MobileHomeMenus()
    let customSnooze = T3MobileCustomSnooze()
    static func color(_ value: String) -> UIColor { T3ChromeTitle.color(value) }
    private final class RouteRef { weak var value: ExactRoute?; init(_ value: ExactRoute) { self.value = value } }
    private final class ViewRef { weak var value: T3HomeChromeView?; init(_ value: T3HomeChromeView) { self.value = value } }
    private var routes: [String: RouteRef] = [:]
    private var views: [String: ViewRef] = [:]
    private var hiddenSidebars = Set<String>()

    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        let instance = T3HomeChromeView(owner: self, events: events)
        try instance.setProps(props)
        return instance
    }
    func configure(_ route: ExactRoute) {
        rowMenus.configure(route)
        routes[route.key] = RouteRef(route)
        views[route.key]?.value?.attach(route)
    }
    func end(_ route: ExactRoute) {
        rowMenus.end(route)
        customSnooze.routeEnded(route)
        guard routes[route.key]?.value === route else { return }
        views[route.key]?.value?.detach()
        routes.removeValue(forKey: route.key)
        hiddenSidebars.remove(route.key)
    }
    /// The public workspace holder hides only the app-owned sidebar search.
    /// Keep its query and route attachment; never end editing in the workspace.
    func hideSidebar(key: String) {
        if key == "t3-workspace-sidebar" { rowMenus.setVisible(false); customSnooze.setSidebarVisible(false); swipes.setSidebarVisible(false) }
        hiddenSidebars.insert(key); views[key]?.value?.setWorkspaceHidden(true)
    }
    func showSidebar(key: String) {
        if key == "t3-workspace-sidebar" { rowMenus.setVisible(true); customSnooze.setSidebarVisible(true); swipes.setSidebarVisible(true) }
        hiddenSidebars.remove(key); views[key]?.value?.setWorkspaceHidden(false)
    }
    fileprivate func bind(_ view: T3HomeChromeView, key: String) {
        views[key] = ViewRef(view)
        view.setWorkspaceHidden(hiddenSidebars.contains(key))
        if let route = routes[key]?.value { view.attach(route) }
    }
    fileprivate func unbind(_ view: T3HomeChromeView, key: String) {
        if views[key]?.value === view { views.removeValue(forKey: key) }
    }
}

private struct T3ChromeConfiguration: Decodable {
    struct Environment: Decodable { let environmentId: String; let label: String }
    struct Project: Decodable { let key: String; let label: String }
    struct Status: Decodable { let label: String; let progress: Bool }
    struct Colors: Decodable { let foreground: String; let muted: String; let iconMuted: String; let subtle: String }
    let routeKey: String; let layout: String; let query: String
    let environmentId: String; let projectKey: String
    let environments: [Environment]; let projects: [Project]
    let status: Status; let colors: Colors
    var filtered: Bool { !environmentId.isEmpty || !projectKey.isEmpty }
    var filterIcon: String { filtered ? "line.3.horizontal.decrease.circle.fill" : "line.3.horizontal.decrease" }
    var sidebarFilterIcon: String { filtered ? "line.3.horizontal.decrease.circle.fill" : "line.3.horizontal.decrease.circle" }
}

private final class T3ChromeOverlay: UIView {
    var layoutChanged: (() -> Void)?
    override func layoutSubviews() { super.layoutSubviews(); layoutChanged?() }
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        let hit = super.hitTest(point, with: event)
        return hit === self ? nil : hit
    }
}

private final class T3SearchDelegate: NSObject, UISearchResultsUpdating, UISearchBarDelegate {
    var changed: ((String) -> Void)?
    func updateSearchResults(for searchController: UISearchController) { changed?(searchController.searchBar.text ?? "") }
    func searchBarCancelButtonClicked(_ searchBar: UISearchBar) { changed?("") }
}

private final class T3FieldDelegate: NSObject, UITextFieldDelegate {
    var editing: ((Bool) -> Void)?
    func textFieldDidBeginEditing(_ textField: UITextField) { editing?(true) }
    func textFieldDidEndEditing(_ textField: UITextField) { editing?(false) }
}

private final class T3HomeChromeView: ExactNativeInstance {
    private weak var owner: T3HomeChrome?
    private weak var route: ExactRoute?
    private let root = T3ChromeOverlay()
    private var config: T3ChromeConfiguration?
    private var key = ""
    private var bar: UIView?
    private var field: UISearchTextField?
    private var filter: UIButton?
    private var compose: UIButton?
    private var dismiss: UIButton?
    private var glassLeading: NSLayoutConstraint?
    private var glassTrailing: NSLayoutConstraint?
    private var keyboardBottom: NSLayoutConstraint?
    private var restingBottom: NSLayoutConstraint?
    private var width: NSLayoutConstraint?
    private let searchDelegate = T3SearchDelegate()
    private let fieldDelegate = T3FieldDelegate()
    private var searchController: UISearchController?
    private var settingQuery = false
    private var workspaceHidden = false
    private var statusWork: DispatchWorkItem?
    private var statusStarted = false
    private var statusShown = false
    private var alive = true
    private var attachedLayout = ""
    private var installedTitle: UIView?
    private var installedRight: [UIBarButtonItem] = []
    private var installedToolbar: [UIBarButtonItem] = []
    override var view: UIView { root }
    override var focusTarget: UIView? { field ?? searchController?.searchBar.searchTextField }

    init(owner: T3HomeChrome, events: ExactNativeEvents) {
        self.owner = owner
        super.init(events: events)
        root.backgroundColor = .clear
        fieldDelegate.editing = { [weak self] active in self?.editing(active) }
        root.layoutChanged = { [weak self] in self?.updateWidth() }
        searchDelegate.changed = { [weak self] text in
            guard let self, !self.settingQuery, !self.workspaceHidden else { return }
            self.emit("search", text)
        }
    }
    override func setProps(_ props: [String: String]) throws {
        guard let raw = props["configuration"], let data = raw.data(using: .utf8),
              let next = try? JSONDecoder().decode(T3ChromeConfiguration.self, from: data),
              ["compact", "sidebar"].contains(next.layout), !next.routeKey.isEmpty else {
            throw ExactNativeRefusal("Home chrome needs a valid configuration and navigation route key.")
        }
        if key != next.routeKey {
            detach(); owner?.unbind(self, key: key); key = next.routeKey
        }
        config = next
        owner?.bind(self, key: key)
        refresh()
    }
    fileprivate func attach(_ route: ExactRoute) {
        if self.route !== route { detach(); self.route = route }
        refresh()
    }
    private func emit(_ kind: String, _ value: String = "") {
        guard alive, !(config?.layout == "sidebar" && workspaceHidden), let data = try? JSONSerialization.data(withJSONObject: ["kind": kind, "value": value]) else { return }
        events.change(String(decoding: data, as: UTF8.self))
    }
    private func menu() -> UIMenu {
        guard let config else { return UIMenu() }
        func action(_ title: String, subtitle: String? = nil, kind: String, value: String, selected: Bool) -> UIAction {
            UIAction(title: title, subtitle: subtitle, state: selected ? .on : .off) { [weak self] _ in self?.emit(kind, value) }
        }
        let environments = [action("All environments", subtitle: "Show threads from every environment", kind: "environment", value: "", selected: config.environmentId.isEmpty)]
            + config.environments.map { action($0.label, kind: "environment", value: $0.environmentId, selected: config.environmentId == $0.environmentId) }
        var children: [UIMenuElement] = [UIMenu(title: "Environment", children: environments)]
        if !config.projects.isEmpty {
            let projects = [action("All projects", subtitle: "Show threads from every project", kind: "project", value: "", selected: config.projectKey.isEmpty)]
                + config.projects.map { action($0.label, kind: "project", value: $0.key, selected: config.projectKey == $0.key) }
            children.append(UIMenu(title: "Project", children: projects))
        }
        return UIMenu(title: "Thread list options", children: children)
    }
    private func refresh() {
        guard alive, let config, let route, route.isLive else { return }
        let glass: Bool
        if #available(iOS 26.0, *) { glass = config.layout == "compact" } else { glass = false }
        let mode = glass ? "glass" : config.layout
        if attachedLayout != mode {
            removeSearch()
            attachedLayout = mode
            if glass { if #available(iOS 26.0, *) { makeGlassToolbar() } }
            else { makeHeaderSearch(route) }
        }
        let item = route.controller.navigationItem
        item.largeTitleDisplayMode = .never
        item.backButtonDisplayMode = .minimal
        // Pinned Stack.tsx GLASS_HEADER_OPTIONS uses the leading editor title.
        if #available(iOS 26.0, *) { item.style = .editor }
        let settings = UIBarButtonItem(image: UIImage(systemName: config.layout == "sidebar" ? "gearshape" : "ellipsis"), primaryAction: UIAction { [weak self] _ in self?.emit("settings") })
        settings.accessibilityLabel = "Open settings"; settings.accessibilityIdentifier = "home-settings"
        if config.layout == "sidebar" {
            let filter = UIBarButtonItem(image: UIImage(systemName: config.sidebarFilterIcon), menu: menu())
            filter.accessibilityLabel = "Filter threads"; filter.accessibilityIdentifier = "home-filter"
            // UIKit orders rightBarButtonItems from the trailing edge inward.
            installedRight = [settings, filter]
        } else { installedRight = [settings] }
        item.rightBarButtonItems = installedRight
        if !glass && config.layout == "compact" {
            let filter = UIBarButtonItem(image: UIImage(systemName: config.sidebarFilterIcon), menu: menu())
            filter.accessibilityLabel = "Filter threads"; filter.accessibilityIdentifier = "home-filter"
            let compose = UIBarButtonItem(image: UIImage(systemName: "square.and.pencil"), primaryAction: UIAction { [weak self] _ in self?.emit("compose") })
            compose.accessibilityLabel = "New task"; compose.accessibilityIdentifier = "home-new-task"
            installedToolbar = [filter, UIBarButtonItem(systemItem: .flexibleSpace), compose]
            route.controller.setToolbarItems(installedToolbar, animated: false)
            route.controller.navigationController?.setToolbarHidden(false, animated: false)
        }
        if field?.text != config.query { field?.text = config.query }
        if searchController?.searchBar.text != config.query {
            settingQuery = true; searchController?.searchBar.text = config.query; settingQuery = false
        }
        filter?.setImage(UIImage(systemName: config.filterIcon), for: .normal)
        filter?.menu = menu()
        updateStatus(); updateWidth()
    }
    private func makeHeaderSearch(_ route: ExactRoute) {
        let search = UISearchController(searchResultsController: nil)
        search.obscuresBackgroundDuringPresentation = false
        search.hidesNavigationBarDuringPresentation = false
        search.searchResultsUpdater = searchDelegate; search.searchBar.delegate = searchDelegate
        search.searchBar.placeholder = "Search"; search.searchBar.autocapitalizationType = .none
        search.searchBar.searchTextField.accessibilityIdentifier = "home-search-text"
        route.controller.navigationItem.searchController = search
        route.controller.navigationItem.hidesSearchBarWhenScrolling = false
        route.controller.definesPresentationContext = true
        searchController = search
    }
    @available(iOS 26.0, *)
    private func makeGlassToolbar() {
        let bar = UIView(); bar.translatesAutoresizingMaskIntoConstraints = false
        root.addSubview(bar); self.bar = bar
        let effect = UIGlassEffect(style: .regular); effect.isInteractive = true
        let glass = UIVisualEffectView(effect: effect)
        glass.translatesAutoresizingMaskIntoConstraints = false; glass.clipsToBounds = true; glass.layer.cornerRadius = 24
        bar.addSubview(glass)
        func button(_ image: String, label: String, id: String, action: @escaping () -> Void) -> UIButton {
            var configuration = UIButton.Configuration.glass()
            configuration.cornerStyle = .capsule; configuration.image = UIImage(systemName: image); configuration.baseForegroundColor = .label
            let button = UIButton(configuration: configuration, primaryAction: UIAction { _ in action() })
            button.translatesAutoresizingMaskIntoConstraints = false
            button.accessibilityLabel = label; button.accessibilityIdentifier = id; bar.addSubview(button)
            NSLayoutConstraint.activate([button.widthAnchor.constraint(equalToConstant: 48), button.heightAnchor.constraint(equalToConstant: 48), button.centerYAnchor.constraint(equalTo: bar.centerYAnchor)])
            return button
        }
        let filter = button("line.3.horizontal.decrease", label: "Filter threads", id: "home-filter") {}
        filter.menu = menu(); filter.showsMenuAsPrimaryAction = true
        let compose = button("square.and.pencil", label: "New task", id: "home-new-task") { [weak self] in self?.emit("compose") }
        let dismiss = button("xmark", label: "Dismiss search keyboard", id: "home-search-dismiss") { [weak self] in self?.field?.resignFirstResponder() }
        dismiss.alpha = 0; dismiss.isHidden = true
        self.filter = filter; self.compose = compose; self.dismiss = dismiss
        let field = UISearchTextField(); field.translatesAutoresizingMaskIntoConstraints = false
        field.placeholder = "Search"; field.font = .preferredFont(forTextStyle: .title3); field.adjustsFontForContentSizeCategory = true
        field.attributedPlaceholder = NSAttributedString(string: "Search", attributes: [.font: UIFont.preferredFont(forTextStyle: .title3), .foregroundColor: UIColor.secondaryLabel])
        field.borderStyle = .none; field.textColor = .label; field.tintColor = .label
        field.autocapitalizationType = .none; field.autocorrectionType = .no; field.spellCheckingType = .no
        field.accessibilityIdentifier = "home-search-text"; field.delegate = fieldDelegate
        field.addAction(UIAction { [weak self] _ in self?.queryChanged() }, for: .editingChanged)
        glass.contentView.addSubview(field); self.field = field
        let leading = glass.leadingAnchor.constraint(equalTo: bar.leadingAnchor, constant: 56)
        let trailing = glass.trailingAnchor.constraint(equalTo: bar.trailingAnchor, constant: -56)
        let resting = bar.bottomAnchor.constraint(equalTo: root.safeAreaLayoutGuide.bottomAnchor, constant: -4); resting.priority = .defaultHigh
        let keyboard = bar.bottomAnchor.constraint(equalTo: root.keyboardLayoutGuide.topAnchor, constant: -4); keyboard.priority = .defaultLow
        let width = bar.widthAnchor.constraint(equalToConstant: 300)
        glassLeading = leading; glassTrailing = trailing; restingBottom = resting; keyboardBottom = keyboard; self.width = width
        NSLayoutConstraint.activate([bar.centerXAnchor.constraint(equalTo: root.centerXAnchor), width, bar.heightAnchor.constraint(equalToConstant: 48),
            bar.bottomAnchor.constraint(lessThanOrEqualTo: root.safeAreaLayoutGuide.bottomAnchor, constant: -4), resting, keyboard,
            leading, trailing, glass.topAnchor.constraint(equalTo: bar.topAnchor), glass.bottomAnchor.constraint(equalTo: bar.bottomAnchor),
            filter.leadingAnchor.constraint(equalTo: bar.leadingAnchor), compose.trailingAnchor.constraint(equalTo: bar.trailingAnchor), dismiss.trailingAnchor.constraint(equalTo: bar.trailingAnchor),
            field.leadingAnchor.constraint(equalTo: glass.contentView.leadingAnchor, constant: 14), field.trailingAnchor.constraint(equalTo: glass.contentView.trailingAnchor, constant: -14),
            field.centerYAnchor.constraint(equalTo: glass.contentView.centerYAnchor), field.heightAnchor.constraint(equalToConstant: 42)])
    }
    private func queryChanged() { emit("search", field?.text ?? "") }
    private func editing(_ active: Bool) {
        keyboardBottom?.priority = active ? .defaultHigh : .defaultLow
        restingBottom?.priority = active ? .defaultLow : .defaultHigh
        glassLeading?.constant = active ? 0 : 56
        glassTrailing?.constant = -56
        filter?.isHidden = false; compose?.isHidden = false; dismiss?.isHidden = false
        UIView.animate(withDuration: 0.2, delay: 0, options: [.beginFromCurrentState, .curveEaseInOut]) {
            self.filter?.alpha = active ? 0 : 1; self.compose?.alpha = active ? 0 : 1; self.dismiss?.alpha = active ? 1 : 0
            self.root.layoutIfNeeded()
        } completion: { [weak self] _ in
            guard let self, self.field?.isFirstResponder == active else { return }
            self.filter?.isHidden = active; self.compose?.isHidden = active; self.dismiss?.isHidden = !active
        }
    }
    private func updateWidth() {
        let hostWidth = root.bounds.width
        let resolved = min(min(560, max(300, hostWidth - 36)), max(260, hostWidth - 12))
        if width?.constant != resolved { width?.constant = resolved }
        if let title = installedTitle as? T3ChromeTitle, hostWidth > 0 {
            title.availableWidth = max(0, hostWidth - 64 - 44 * (config?.layout == "sidebar" ? 2 : 1))
        }
    }
    private func updateStatus() {
        guard let config else { return }
        if config.status.label.isEmpty {
            statusWork?.cancel(); statusWork = nil; statusStarted = false; statusShown = false
        } else if !statusStarted {
            statusStarted = true
            let work = DispatchWorkItem { [weak self] in
                guard let self, self.alive, self.config?.status.label.isEmpty == false else { return }
                self.statusShown = true; self.renderTitle(animated: true)
            }
            statusWork = work; DispatchQueue.main.asyncAfter(deadline: .now() + 0.8, execute: work)
        }
        renderTitle(animated: false)
    }
    private func renderTitle(animated: Bool) {
        guard let config, let route else { return }
        let title = T3ChromeTitle(config: config, status: statusShown, action: { [weak self] in self?.emit("environments") })
        title.alpha = animated ? 0 : 1
        title.availableWidth = max(0, route.controller.view.bounds.width - 64 - 44 * (config.layout == "sidebar" ? 2 : 1))
        installedTitle = title
        route.controller.navigationItem.titleView = title
        if animated { UIView.animate(withDuration: 0.25) { title.alpha = 1 } }
    }
    fileprivate func setWorkspaceHidden(_ hidden: Bool) {
        guard workspaceHidden != hidden else { return }
        workspaceHidden = hidden
        guard config?.layout == "sidebar" else { return }
        if !hidden {
            settingQuery = true
            searchController?.searchBar.text = config?.query
            field?.text = config?.query
            settingQuery = false
            return
        }
        settingQuery = true
        searchController?.searchBar.searchTextField.resignFirstResponder()
        searchController?.isActive = false
        field?.resignFirstResponder()
        settingQuery = false
    }
    private func removeSearch() {
        if let searchController, route?.controller.navigationItem.searchController === searchController {
            route?.controller.navigationItem.searchController = nil
        }
        searchController?.searchResultsUpdater = nil; searchController?.searchBar.delegate = nil
        searchController = nil; field?.delegate = nil; field?.resignFirstResponder(); field = nil
        bar?.removeFromSuperview(); bar = nil; filter = nil; compose = nil; dismiss = nil
        width = nil; keyboardBottom = nil; restingBottom = nil; glassLeading = nil; glassTrailing = nil
    }
    fileprivate func detach() {
        statusWork?.cancel(); statusWork = nil; statusStarted = false; statusShown = false
        removeSearch()
        if let route {
            let item = route.controller.navigationItem
            if item.titleView === installedTitle { item.titleView = nil }
            if item.rightBarButtonItems == installedRight { item.rightBarButtonItems = nil }
            if route.controller.toolbarItems == installedToolbar && !installedToolbar.isEmpty {
                route.controller.setToolbarItems(nil, animated: false)
                route.controller.navigationController?.setToolbarHidden(true, animated: false)
            }
        }
        route = nil; installedTitle = nil; installedRight = []; installedToolbar = []; attachedLayout = ""
    }
    override func agentInput(_ input: ExactNativeInput) throws {
        switch input {
        case .text(let text):
            if let field { field.text = text; queryChanged() }
            else if let searchController { searchController.searchBar.text = text; emit("search", text) }
            else { throw ExactNativeRefusal("Home search is not attached to a route.") }
        case .key(let key, let phase):
            guard key == "Escape" || key == "Enter" else { throw ExactNativeRefusal("Home search accepts Escape or Enter.") }
            if phase != "up" { focusTarget?.resignFirstResponder() }
        }
    }
    override func destroy() { alive = false; detach(); owner?.unbind(self, key: key); root.layoutChanged = nil }
}

private final class T3ChromeTitle: UIControl {
    private let stack = UIStackView()
    private let action: () -> Void
    private let offset: CGFloat
    var availableWidth: CGFloat = .greatestFiniteMagnitude {
        didSet {
            if oldValue != availableWidth { invalidateIntrinsicContentSize(); sizeToFit() }
        }
    }
    init(config: T3ChromeConfiguration, status: Bool, action: @escaping () -> Void) {
        self.action = action
        offset = UIDevice.current.userInterfaceIdiom == .pad ? 10 : 0
        super.init(frame: .zero)
        stack.axis = .horizontal; stack.alignment = .center; stack.spacing = status ? 7 : 5.25
        stack.isUserInteractionEnabled = false; addSubview(stack)
        let muted = Self.color(config.colors.muted)
        func label(_ text: String, size: CGFloat, weight: String, spacing: CGFloat = 0) -> UILabel {
            let label = UILabel()
            let font = UIFont(name: "DMSans-\(weight)", size: size) ?? .systemFont(ofSize: size, weight: weight == "Bold" ? .bold : .medium)
            label.font = UIFontMetrics.default.scaledFont(for: font); label.adjustsFontForContentSizeCategory = true
            label.attributedText = NSAttributedString(string: text, attributes: [.kern: spacing, .foregroundColor: muted])
            return label
        }
        if status {
            if config.status.progress {
                let spinner = UIActivityIndicatorView(style: .medium); spinner.color = Self.color(config.colors.iconMuted); spinner.startAnimating(); stack.addArrangedSubview(spinner)
            } else {
                let icon = UIImageView(image: UIImage(systemName: "wifi.slash", withConfiguration: UIImage.SymbolConfiguration(pointSize: 15)))
                icon.tintColor = Self.color(config.colors.iconMuted); icon.setContentHuggingPriority(.required, for: .horizontal); stack.addArrangedSubview(icon)
            }
            let text = label(config.status.label, size: 16, weight: "Bold")
            text.lineBreakMode = .byTruncatingTail; stack.addArrangedSubview(text)
            accessibilityLabel = config.status.label; accessibilityHint = "Opens environment settings"
            accessibilityTraits = .button
            addTarget(self, action: #selector(activate), for: .touchUpInside)
        } else {
            let mark = UIImageView(image: Self.wordmark(color: Self.color(config.colors.foreground)))
            mark.setContentHuggingPriority(.required, for: .horizontal); stack.addArrangedSubview(mark)
            stack.addArrangedSubview(label("Code", size: 21, weight: "Medium", spacing: -0.5))
            let stage = label("ALPHA", size: 9, weight: "Bold", spacing: 0.9)
            let pill = UIView(); pill.backgroundColor = Self.color(config.colors.subtle)
            pill.layer.cornerRadius = (stage.intrinsicContentSize.height + 3.5) / 2
            stage.translatesAutoresizingMaskIntoConstraints = false; pill.addSubview(stage)
            NSLayoutConstraint.activate([stage.leadingAnchor.constraint(equalTo: pill.leadingAnchor, constant: 5.25), stage.trailingAnchor.constraint(equalTo: pill.trailingAnchor, constant: -5.25), stage.topAnchor.constraint(equalTo: pill.topAnchor, constant: 1.75), stage.bottomAnchor.constraint(equalTo: pill.bottomAnchor, constant: -1.75)])
            stack.addArrangedSubview(pill)
            accessibilityLabel = "T3 Code, Threads"; accessibilityTraits = .header
            isUserInteractionEnabled = false
        }
        isAccessibilityElement = true
    }
    required init?(coder: NSCoder) { nil }
    // UINavigationItem needs a fitting size as well as an intrinsic size.
    // UIView's default returns the initial zero frame, leaving the title absent.
    override func sizeThatFits(_ size: CGSize) -> CGSize { intrinsicContentSize }

    override var intrinsicContentSize: CGSize {
        let size = stack.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize)
        return CGSize(width: min(availableWidth, size.width + offset), height: max(32, size.height))
    }
    override func layoutSubviews() {
        super.layoutSubviews()
        stack.frame = CGRect(x: offset, y: 0, width: max(0, bounds.width - offset), height: bounds.height)
    }
    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool { bounds.insetBy(dx: -8, dy: -8).contains(point) }
    @objc private func activate() { action() }
    static func color(_ value: String) -> UIColor {
        let text = value.trimmingCharacters(in: .whitespacesAndNewlines)
        if text.hasPrefix("#"), let hex = UInt64(text.dropFirst(), radix: 16) {
            let alpha = text.count == 9 ? CGFloat(hex & 255) / 255 : 1
            let rgb = text.count == 9 ? hex >> 8 : hex
            return UIColor(red: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255, blue: CGFloat(rgb & 255) / 255, alpha: alpha)
        }
        if text.hasPrefix("rgb"), let opening = text.firstIndex(of: "("), let closing = text.lastIndex(of: ")") {
            let values = text[text.index(after: opening)..<closing].split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespaces)) }
            if values.count >= 3 { return UIColor(red: values[0] / 255, green: values[1] / 255, blue: values[2] / 255, alpha: values.count == 4 ? values[3] : 1) }
        }
        return .label
    }
    private static func wordmark(color: UIColor) -> UIImage {
        let size = CGSize(width: 15 * (94.3941 / 56.96), height: 15)
        return UIGraphicsImageRenderer(size: size).image { context in
            context.cgContext.scaleBy(x: 15 / 56.96, y: 15 / 56.96)
            context.cgContext.translateBy(x: -15.5309, y: -37)
            let path = CGMutablePath()
            path.move(to: CGPoint(x: 33.4509, y: 93))
            path.addLine(to: CGPoint(x: 33.4509, y: 47.56))
            path.addLine(to: CGPoint(x: 15.5309, y: 47.56))
            path.addLine(to: CGPoint(x: 15.5309, y: 37))
            path.addLine(to: CGPoint(x: 64.3309, y: 37))
            path.addLine(to: CGPoint(x: 64.3309, y: 47.56))
            path.addLine(to: CGPoint(x: 46.4109, y: 47.56))
            path.addLine(to: CGPoint(x: 46.4109, y: 93))
            path.addLine(to: CGPoint(x: 33.4509, y: 93))
            path.closeSubpath()
            path.move(to: CGPoint(x: 86.7253, y: 93.96))
            path.addCurve(to: CGPoint(x: 75.1253, y: 92.44), control1: CGPoint(x: 82.832, y: 93.96), control2: CGPoint(x: 78.9653, y: 93.4533))
            path.addCurve(to: CGPoint(x: 65.3653, y: 87.96), control1: CGPoint(x: 71.2853, y: 91.3733), control2: CGPoint(x: 68.032, y: 89.88))
            path.addLine(to: CGPoint(x: 70.4053, y: 78.04))
            path.addCurve(to: CGPoint(x: 77.8453, y: 81.72), control1: CGPoint(x: 72.5386, y: 79.5867), control2: CGPoint(x: 75.0186, y: 80.8133))
            path.addCurve(to: CGPoint(x: 86.4053, y: 83.08), control1: CGPoint(x: 80.672, y: 82.6267), control2: CGPoint(x: 83.5253, y: 83.08))
            path.addCurve(to: CGPoint(x: 94.0853, y: 81.16), control1: CGPoint(x: 89.6586, y: 83.08), control2: CGPoint(x: 92.2186, y: 82.44))
            path.addCurve(to: CGPoint(x: 96.8853, y: 75.88), control1: CGPoint(x: 95.952, y: 79.88), control2: CGPoint(x: 96.8853, y: 78.12))
            path.addCurve(to: CGPoint(x: 94.4053, y: 70.84), control1: CGPoint(x: 96.8853, y: 73.7467), control2: CGPoint(x: 96.0586, y: 72.0667))
            path.addCurve(to: CGPoint(x: 86.4053, y: 69), control1: CGPoint(x: 92.752, y: 69.6133), control2: CGPoint(x: 90.0853, y: 69))
            path.addLine(to: CGPoint(x: 80.4853, y: 69))
            path.addLine(to: CGPoint(x: 80.4853, y: 60.44))
            path.addLine(to: CGPoint(x: 96.0853, y: 42.76))
            path.addLine(to: CGPoint(x: 97.5253, y: 47.4))
            path.addLine(to: CGPoint(x: 68.1653, y: 47.4))
            path.addLine(to: CGPoint(x: 68.1653, y: 37))
            path.addLine(to: CGPoint(x: 107.365, y: 37))
            path.addLine(to: CGPoint(x: 107.365, y: 45.4))
            path.addLine(to: CGPoint(x: 91.8453, y: 63.08))
            path.addLine(to: CGPoint(x: 85.2853, y: 59.32))
            path.addLine(to: CGPoint(x: 89.0453, y: 59.32))
            path.addCurve(to: CGPoint(x: 104.645, y: 63.96), control1: CGPoint(x: 95.9253, y: 59.32), control2: CGPoint(x: 101.125, y: 60.8667))
            path.addCurve(to: CGPoint(x: 109.925, y: 75.88), control1: CGPoint(x: 108.165, y: 67.0533), control2: CGPoint(x: 109.925, y: 71.0267))
            path.addCurve(to: CGPoint(x: 107.445, y: 84.76), control1: CGPoint(x: 109.925, y: 79.0267), control2: CGPoint(x: 109.099, y: 81.9867))
            path.addCurve(to: CGPoint(x: 99.8453, y: 91.4), control1: CGPoint(x: 105.792, y: 87.48), control2: CGPoint(x: 103.259, y: 89.6933))
            path.addCurve(to: CGPoint(x: 86.7253, y: 93.96), control1: CGPoint(x: 96.432, y: 93.1067), control2: CGPoint(x: 92.0586, y: 93.96))
            path.closeSubpath()
            context.cgContext.addPath(path); context.cgContext.setFillColor(color.cgColor); context.cgContext.fillPath()
        }
    }
}
#endif

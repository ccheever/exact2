#if os(iOS)
// @ref llp/1109.004-home-projection.decision.md#decision
// Pinned365aa87982 ArchivedThreadsHeader, ScreenHeader and RNS mail-search toolbar patch.
import UIKit

/// Archive-only navigation adornments. Exact owns routes; the root owns query/filter state.
final class T3ArchiveChrome {
    private final class RouteRef { weak var value: ExactRoute?; init(_ value: ExactRoute) { self.value = value } }
    private final class PortRef { weak var value: T3ArchiveChromePort?; init(_ value: T3ArchiveChromePort) { self.value = value } }
    private var routes: [String: RouteRef] = [:]
    private var ports: [String: PortRef] = [:]
    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        let port = T3ArchiveChromePort(owner: self, events: events); try port.setProps(props); return port
    }
    func configure(_ route: ExactRoute) { routes[route.key] = RouteRef(route); ports[route.key]?.value?.attach(route) }
    func end(_ route: ExactRoute) {
        guard routes[route.key]?.value === route else { return }
        ports[route.key]?.value?.detach(); routes.removeValue(forKey: route.key)
    }
    fileprivate func bind(_ port: T3ArchiveChromePort, key: String) { ports[key] = PortRef(port); if let route = routes[key]?.value { port.attach(route) } }
    fileprivate func unbind(_ port: T3ArchiveChromePort, key: String) { if ports[key]?.value === port { ports.removeValue(forKey: key) } }
}
private struct T3ArchiveChromeConfig: Decodable {
    struct Environment: Decodable { let id: String; let label: String }
    let routeKey: String; let compact: Bool; let query: String; let environmentId: String; let sortOrder: String
    let environments: [Environment]
    var filterIcon: String { !environmentId.isEmpty || sortOrder != "newest" ? "line.3.horizontal.decrease.circle.fill" : "line.3.horizontal.decrease.circle" }
}
private final class T3ArchiveChromeOverlay: UIView {
    var resized: (() -> Void)?
    override func layoutSubviews() { super.layoutSubviews(); resized?() }
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? { let hit = super.hitTest(point, with: event); return hit === self ? nil : hit }
}
private final class T3ArchiveSearchDelegate: NSObject, UISearchResultsUpdating, UISearchBarDelegate, UITextFieldDelegate {
    var changed: ((String) -> Void)?
    var editing: ((Bool) -> Void)?
    func updateSearchResults(for searchController: UISearchController) { changed?(searchController.searchBar.text ?? "") }
    func searchBarCancelButtonClicked(_ searchBar: UISearchBar) { changed?("") }
    func textFieldDidBeginEditing(_ textField: UITextField) { editing?(true) }
    func textFieldDidEndEditing(_ textField: UITextField) { editing?(false) }
}
private final class T3ArchiveChromePort: ExactNativeInstance {
    private weak var owner: T3ArchiveChrome?
    private weak var route: ExactRoute?
    private let root = T3ArchiveChromeOverlay()
    private let delegate = T3ArchiveSearchDelegate()
    private var config: T3ArchiveChromeConfig?
    private var key = ""
    private var mode = ""
    private var alive = true
    private var settingQuery = false
    private var searchController: UISearchController?
    private var field: UISearchTextField?
    private var filter: UIButton?
    private var dismiss: UIButton?
    private var bar: UIView?
    private var width: NSLayoutConstraint?
    private var leading: NSLayoutConstraint?
    private var trailing: NSLayoutConstraint?
    private var resting: NSLayoutConstraint?
    private var keyboard: NSLayoutConstraint?
    private var installedRight: [UIBarButtonItem] = []
    private var oldPlacement: UINavigationItem.SearchBarPlacement?
    override var view: UIView { root }
    override var focusTarget: UIView? { field ?? searchController?.searchBar.searchTextField }
    init(owner: T3ArchiveChrome, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events)
        root.backgroundColor = .clear
        root.resized = { [weak self] in self?.updateWidth() }
        delegate.changed = { [weak self] value in guard let self, !self.settingQuery else { return }; self.emit("search", value) }
        delegate.editing = { [weak self] active in self?.editing(active) }
    }
    override func setProps(_ props: [String: String]) throws {
        guard let data = props["configuration"]?.data(using: .utf8), let next = try? JSONDecoder().decode(T3ArchiveChromeConfig.self, from: data),
              !next.routeKey.isEmpty, ["newest", "oldest"].contains(next.sortOrder) else { throw ExactNativeRefusal("Archive chrome needs a route and valid configuration.") }
        if key != next.routeKey { detach(); owner?.unbind(self, key: key); key = next.routeKey }
        config = next; owner?.bind(self, key: key); refresh()
    }
    fileprivate func attach(_ route: ExactRoute) { if self.route !== route { detach(); self.route = route }; refresh() }
    private func emit(_ kind: String, _ value: String) {
        guard alive, route?.isLive == true, let data = try? JSONSerialization.data(withJSONObject: ["routeKey": key, "kind": kind, "value": value]) else { return }
        events.change(String(decoding: data, as: UTF8.self))
    }
    private func menu() -> UIMenu {
        guard let config else { return UIMenu() }
        let ownerKey = key
        func action(_ title: String, _ kind: String, _ value: String, _ selected: Bool) -> UIAction {
            UIAction(title: title, state: selected ? .on : .off) { [weak self] _ in guard self?.key == ownerKey else { return }; self?.emit(kind, value) }
        }
        let environments = [action("All environments", "environment", "", config.environmentId.isEmpty)]
            + config.environments.map { action($0.label, "environment", $0.id, config.environmentId == $0.id) }
        return UIMenu(title: "Archived thread options", children: [UIMenu(title: "Environment", children: environments),
            UIMenu(title: "Sort by archived date", children: [action("Newest first", "sort", "newest", config.sortOrder == "newest"), action("Oldest first", "sort", "oldest", config.sortOrder == "oldest")])])
    }
    private func refresh() {
        guard alive, let config, let route, route.isLive else { return }
        let glass: Bool
        if #available(iOS 26.0, *) { glass = config.compact } else { glass = false }
        let nextMode = glass ? "glass" : "header"
        if mode != nextMode {
            removeSearch(); mode = nextMode
            if glass { if #available(iOS 26.0, *) { makeGlass() } } else { makeHeader(route) }
        }
        if !glass {
            let item = UIBarButtonItem(image: UIImage(systemName: config.filterIcon), menu: menu())
            item.accessibilityLabel = "Archived thread options"; item.accessibilityIdentifier = "archive-filter"
            installedRight = [item]; route.controller.navigationItem.rightBarButtonItems = installedRight
        }
        filter?.setImage(UIImage(systemName: config.filterIcon), for: .normal); filter?.menu = menu()
        settingQuery = true
        if field?.text != config.query { field?.text = config.query }
        if searchController?.searchBar.text != config.query { searchController?.searchBar.text = config.query }
        settingQuery = false; updateWidth()
    }
    private func makeHeader(_ route: ExactRoute) {
        let search = UISearchController(searchResultsController: nil)
        search.obscuresBackgroundDuringPresentation = false; search.hidesNavigationBarDuringPresentation = false
        search.searchResultsUpdater = delegate; search.searchBar.delegate = delegate
        search.searchBar.placeholder = "Search archived threads"; search.searchBar.autocapitalizationType = .none
        search.searchBar.searchTextField.accessibilityIdentifier = "archive-search-text"
        let item = route.controller.navigationItem
        oldPlacement = item.preferredSearchBarPlacement
        if #available(iOS 26.0, *) { item.preferredSearchBarPlacement = .integratedButton }
        item.searchController = search; route.controller.definesPresentationContext = true
        searchController = search
    }
    @available(iOS 26.0, *)
    private func makeGlass() {
        let bar = UIView(); bar.translatesAutoresizingMaskIntoConstraints = false; root.addSubview(bar); self.bar = bar
        let effect = UIGlassEffect(style: .regular); effect.isInteractive = true
        let glass = UIVisualEffectView(effect: effect); glass.translatesAutoresizingMaskIntoConstraints = false
        glass.clipsToBounds = true; glass.layer.cornerRadius = 24; bar.addSubview(glass)
        func button(_ image: String, _ label: String, _ id: String, _ action: @escaping () -> Void) -> UIButton {
            var style = UIButton.Configuration.glass(); style.cornerStyle = .capsule; style.image = UIImage(systemName: image); style.baseForegroundColor = .label
            let button = UIButton(configuration: style, primaryAction: UIAction { _ in action() }); button.translatesAutoresizingMaskIntoConstraints = false
            button.accessibilityLabel = label; button.accessibilityIdentifier = id; bar.addSubview(button)
            NSLayoutConstraint.activate([button.widthAnchor.constraint(equalToConstant: 48), button.heightAnchor.constraint(equalToConstant: 48), button.centerYAnchor.constraint(equalTo: bar.centerYAnchor)])
            return button
        }
        let filter = button("line.3.horizontal.decrease.circle", "Archived thread options", "archive-filter") {}
        filter.menu = menu(); filter.showsMenuAsPrimaryAction = true; self.filter = filter
        let dismiss = button("xmark", "Dismiss search keyboard", "archive-search-dismiss") { [weak self] in self?.field?.resignFirstResponder() }
        dismiss.alpha = 0; dismiss.isHidden = true; self.dismiss = dismiss
        let field = UISearchTextField(); field.translatesAutoresizingMaskIntoConstraints = false
        field.placeholder = "Search"; field.font = .preferredFont(forTextStyle: .title3); field.adjustsFontForContentSizeCategory = true
        field.attributedPlaceholder = NSAttributedString(string: "Search", attributes: [.font: UIFont.preferredFont(forTextStyle: .title3), .foregroundColor: UIColor.secondaryLabel])
        field.borderStyle = .none; field.textColor = .label; field.tintColor = .label
        field.autocapitalizationType = .none; field.autocorrectionType = .no; field.spellCheckingType = .no
        field.accessibilityIdentifier = "archive-search-text"; field.delegate = delegate
        field.addAction(UIAction { [weak self] _ in guard let self else { return }; self.emit("search", self.field?.text ?? "") }, for: .editingChanged)
        glass.contentView.addSubview(field); self.field = field
        let leading = glass.leadingAnchor.constraint(equalTo: bar.leadingAnchor, constant: 56)
        let trailing = glass.trailingAnchor.constraint(equalTo: bar.trailingAnchor)
        let resting = bar.bottomAnchor.constraint(equalTo: root.safeAreaLayoutGuide.bottomAnchor, constant: -4); resting.priority = .defaultHigh
        let keyboard = bar.bottomAnchor.constraint(equalTo: root.keyboardLayoutGuide.topAnchor, constant: -4); keyboard.priority = .defaultLow
        let width = bar.widthAnchor.constraint(equalToConstant: 300)
        self.leading = leading; self.trailing = trailing; self.resting = resting; self.keyboard = keyboard; self.width = width
        NSLayoutConstraint.activate([bar.centerXAnchor.constraint(equalTo: root.centerXAnchor), width, bar.heightAnchor.constraint(equalToConstant: 48),
            bar.bottomAnchor.constraint(lessThanOrEqualTo: root.safeAreaLayoutGuide.bottomAnchor, constant: -4), resting, keyboard, leading, trailing,
            glass.topAnchor.constraint(equalTo: bar.topAnchor), glass.bottomAnchor.constraint(equalTo: bar.bottomAnchor),
            filter.leadingAnchor.constraint(equalTo: bar.leadingAnchor), dismiss.trailingAnchor.constraint(equalTo: bar.trailingAnchor),
            field.leadingAnchor.constraint(equalTo: glass.contentView.leadingAnchor, constant: 14), field.trailingAnchor.constraint(equalTo: glass.contentView.trailingAnchor, constant: -14),
            field.centerYAnchor.constraint(equalTo: glass.contentView.centerYAnchor), field.heightAnchor.constraint(equalToConstant: 42)])
    }
    private func editing(_ active: Bool) {
        keyboard?.priority = active ? .defaultHigh : .defaultLow; resting?.priority = active ? .defaultLow : .defaultHigh
        leading?.constant = active ? 0 : 56; trailing?.constant = active ? -56 : 0
        filter?.isHidden = false; dismiss?.isHidden = false
        UIView.animate(withDuration: 0.2, delay: 0, options: [.beginFromCurrentState, .curveEaseInOut]) {
            self.filter?.alpha = active ? 0 : 1; self.dismiss?.alpha = active ? 1 : 0; self.root.layoutIfNeeded()
        } completion: { [weak self] finished in
            guard let self, finished, self.field?.isFirstResponder == active else { return }
            self.filter?.isHidden = active; self.dismiss?.isHidden = !active
        }
    }
    private func updateWidth() { let host = root.bounds.width; width?.constant = min(min(560, max(300, host - 36)), max(260, host - 12)) }
    private func removeSearch() {
        settingQuery = true
        if let searchController, let route, route.controller.navigationItem.searchController === searchController {
            searchController.isActive = false; route.controller.navigationItem.searchController = nil
            if let oldPlacement { route.controller.navigationItem.preferredSearchBarPlacement = oldPlacement }
        }
        if let route, !installedRight.isEmpty, route.controller.navigationItem.rightBarButtonItems == installedRight { route.controller.navigationItem.rightBarButtonItems = nil }
        field?.resignFirstResponder(); bar?.removeFromSuperview()
        searchController = nil; oldPlacement = nil; installedRight = []; field = nil; bar = nil; filter = nil; dismiss = nil
        width = nil; leading = nil; trailing = nil; resting = nil; keyboard = nil; settingQuery = false
    }
    fileprivate func detach() { removeSearch(); route = nil; mode = "" }
    override func agentInput(_ input: ExactNativeInput) throws {
        switch input {
        case .text(let text):
            guard route?.isLive == true else { throw ExactNativeRefusal("Archive search is not attached.") }
            if let field { field.text = text }
            else if let searchController { settingQuery = true; searchController.searchBar.text = text; settingQuery = false }
            else { throw ExactNativeRefusal("Archive search is not attached.") }
            emit("search", text)
        case .key(let key, let phase):
            guard key == "Escape" || key == "Enter" else { throw ExactNativeRefusal("Archive search accepts Escape or Enter.") }
            if phase != "up" { focusTarget?.resignFirstResponder() }
        }
    }
    override func destroy() { alive = false; detach(); owner?.unbind(self, key: key); root.resized = nil }
}
#endif

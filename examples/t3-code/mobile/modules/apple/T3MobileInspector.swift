// @ref llp/1109.011-responsive-workspace.decision.md#composition-decision
// Pinned365aa87982 thread-file-navigator-pane, ReviewSheet and GitOverviewSheet.
#if os(iOS)
import UIKit

final class T3MobileInspector {
    private final class RouteRef { weak var value: ExactRoute?; init(_ value: ExactRoute) { self.value = value } }
    private final class ViewRef { weak var value: T3InspectorChromeView?; init(_ value: T3InspectorChromeView) { self.value = value } }
    private var active = true
    private var routes: [String: RouteRef] = [:]
    private var views: [String: ViewRef] = [:]
    private var visibility: [String: Bool] = [:]
    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard active else { throw ExactNativeRefusal("The inspector session has ended.") }
        let instance = T3InspectorChromeView(owner: self, events: events)
        try instance.setProps(props); return instance
    }
    func configure(_ route: ExactRoute) {
        guard active, route.key == "t3-workspace-inspector" else { return }
        routes[route.key] = RouteRef(route); views[route.key]?.value?.attach(route)
    }
    func end(_ route: ExactRoute) {
        guard routes[route.key]?.value === route else { return }
        views[route.key]?.value?.detach(); routes.removeValue(forKey: route.key)
    }
    func setVisible(key: String, visible: Bool) {
        visibility[key] = visible; views[key]?.value?.setVisible(visible)
    }
    fileprivate func bind(_ view: T3InspectorChromeView, key: String) {
        guard active else { return }
        views[key] = ViewRef(view); view.setVisible(visibility[key] ?? false)
        if let route = routes[key]?.value { view.attach(route) }
    }
    fileprivate func unbind(_ view: T3InspectorChromeView, key: String) {
        if views[key]?.value === view { views.removeValue(forKey: key) }
    }
    func destroy() {
        active = false
        for view in views.values { view.value?.detach() }
        views.removeAll(); routes.removeAll(); visibility.removeAll()
    }
}
private struct T3InspectorConfiguration: Decodable {
    let routeKey: String; let owner: String; let kind: String
    let title: String; let subtitle: String; let query: String; let visible: Bool
    let foreground: String; let muted: String; let sheet: String; let border: String
}
private final class T3InspectorSearch: NSObject, UISearchResultsUpdating, UISearchBarDelegate {
    var changed: ((String) -> Void)?
    func updateSearchResults(for searchController: UISearchController) { changed?(searchController.searchBar.text ?? "") }
    func searchBarCancelButtonClicked(_ searchBar: UISearchBar) { changed?("") }
}
private final class T3InspectorChromeView: ExactNativeInstance {
    private weak var owner: T3MobileInspector?
    private weak var route: ExactRoute?
    private let root = UIView()
    private var config: T3InspectorConfiguration?
    private var key = ""
    private var visible = false
    private var active = true
    private var settingQuery = false
    private var search: UISearchController?
    private var searchDelegate: T3InspectorSearch?
    private var close: UIBarButtonItem?
    private var installedTitle: UIView?
    override var view: UIView { root }
    override var focusTarget: UIView? { search?.searchBar.searchTextField }
    init(owner: T3MobileInspector, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events)
        root.isUserInteractionEnabled = false; root.accessibilityElementsHidden = true
    }
    override func setProps(_ props: [String: String]) throws {
        guard active, let text = props["inspector-configuration"], let bytes = text.data(using: .utf8),
              let next = try? JSONDecoder().decode(T3InspectorConfiguration.self, from: bytes),
              next.routeKey == "t3-workspace-inspector", ["files", "changes", "git"].contains(next.kind) else {
            throw ExactNativeRefusal("Inspector chrome requires its static route and a supported content configuration.")
        }
        if config?.kind != next.kind || key != next.routeKey { removeSearch(); close = nil }
        else if config?.owner != next.owner {
            // Preserve an active Files search across a visible file replacement.
            // Retire the prior owner's callback while retaining the actual field.
            searchDelegate?.changed = nil; close = nil
            if let search { connectSearch(search, capturedOwner: next.owner) }
        }
        if key != next.routeKey { detach(); owner?.unbind(self, key: key); key = next.routeKey }
        config = next; owner?.bind(self, key: key); refresh()
    }
    fileprivate func attach(_ route: ExactRoute) {
        if self.route !== route { detach(); self.route = route }
        refresh()
    }
    fileprivate func setVisible(_ value: Bool) {
        visible = value
        close?.isEnabled = value && config?.visible == true
        guard value else { suspendSearch(); return }
        if let config, search?.searchBar.text != config.query {
            settingQuery = true; search?.searchBar.text = config.query; settingQuery = false
        }
    }
    private func emit(_ kind: String, value: String = "", capturedOwner: String) {
        guard active, visible, route?.isLive == true, let config, config.visible, config.owner == capturedOwner,
              let bytes = try? JSONSerialization.data(withJSONObject: ["kind": kind, "owner": capturedOwner, "value": value]) else { return }
        events.change(String(decoding: bytes, as: UTF8.self))
    }
    private func refresh() {
        guard active, let route, route.isLive, let config else { return }
        let item = route.controller.navigationItem
        let foreground = T3InspectorColor.parse(config.foreground)
        item.largeTitleDisplayMode = .never; item.hidesBackButton = true; item.style = .editor
        item.title = config.kind == "files" ? "Files" : config.kind == "changes" ? "Changed files" : config.title
        let appearance = UINavigationBarAppearance(); appearance.configureWithTransparentBackground()
        appearance.titleTextAttributes = [.font: UIFont.systemFont(ofSize: 17, weight: .bold), .foregroundColor: foreground]
        appearance.shadowColor = T3InspectorColor.parse(config.border)
        item.standardAppearance = appearance; item.scrollEdgeAppearance = appearance; item.compactAppearance = appearance
        if #available(iOS 26.0, *) {
            item.subtitle = config.subtitle.isEmpty ? nil : config.subtitle
            item.titleView = nil
        } else {
            let title = UILabel(); title.text = item.title; title.font = .systemFont(ofSize: 17, weight: .bold); title.textColor = foreground
            let subtitle = UILabel(); subtitle.text = config.subtitle; subtitle.font = .systemFont(ofSize: 12); subtitle.textColor = T3InspectorColor.parse(config.muted)
            subtitle.isHidden = config.subtitle.isEmpty
            let stack = UIStackView(arrangedSubviews: [title, subtitle]); stack.axis = .vertical; stack.alignment = .leading
            installedTitle = stack; item.titleView = stack
        }
        if config.kind == "files" {
            if search == nil { installSearch(route: route, config: config) }
            if close == nil {
                let capturedOwner = config.owner
                close = UIBarButtonItem(image: UIImage(systemName: "xmark"), primaryAction: UIAction { [weak self] _ in
                    self?.emit("close", capturedOwner: capturedOwner)
                })
                close?.accessibilityLabel = "Close files"; close?.accessibilityIdentifier = "thread-file-navigator-close"
                if #available(iOS 26.0, *) { close?.hidesSharedBackground = true }
            }
            close?.tintColor = foreground; close?.isEnabled = config.visible && visible
            item.rightBarButtonItems = close.map { [$0] } ?? []
            search?.searchBar.barTintColor = T3InspectorColor.parse(config.sheet)
            search?.searchBar.tintColor = foreground; search?.searchBar.searchTextField.textColor = foreground
            if search?.searchBar.text != config.query { settingQuery = true; search?.searchBar.text = config.query; settingQuery = false }
        } else { removeSearch(); item.rightBarButtonItems = [] }
        if !config.visible || !visible { suspendSearch() }
    }
    private func installSearch(route: ExactRoute, config: T3InspectorConfiguration) {
        let search = UISearchController(searchResultsController: nil)
        search.obscuresBackgroundDuringPresentation = false; search.hidesNavigationBarDuringPresentation = false
        connectSearch(search, capturedOwner: config.owner)
        search.searchBar.autocapitalizationType = .none; search.searchBar.placeholder = "Search files"
        search.searchBar.searchTextField.accessibilityIdentifier = "inspector-search-files"
        route.controller.definesPresentationContext = true
        let item = route.controller.navigationItem
        item.searchController = search; item.hidesSearchBarWhenScrolling = false
        if #available(iOS 26.0, *) {
            item.preferredSearchBarPlacement = .integratedButton
            item.searchBarPlacementAllowsToolbarIntegration = true
        }
        self.search = search
    }
    private func connectSearch(_ search: UISearchController, capturedOwner: String) {
        let delegate = T3InspectorSearch()
        delegate.changed = { [weak self] value in
            guard let self, !self.settingQuery else { return }; self.emit("search", value: value, capturedOwner: capturedOwner)
        }
        search.searchResultsUpdater = delegate; search.searchBar.delegate = delegate; searchDelegate = delegate
    }
    private func suspendSearch() {
        settingQuery = true; search?.searchBar.searchTextField.resignFirstResponder(); search?.isActive = false; settingQuery = false
    }
    private func removeSearch() {
        suspendSearch()
        if route?.controller.navigationItem.searchController === search { route?.controller.navigationItem.searchController = nil }
        search?.searchResultsUpdater = nil; search?.searchBar.delegate = nil
        searchDelegate?.changed = nil; searchDelegate = nil; search = nil
    }
    fileprivate func detach() {
        removeSearch()
        if let item = route?.controller.navigationItem {
            if let close, item.rightBarButtonItems?.contains(where: { $0 === close }) == true { item.rightBarButtonItems = nil }
            if item.titleView === installedTitle { item.titleView = nil }
        }
        close = nil; installedTitle = nil; route = nil
    }
    override func destroy() { active = false; detach(); owner?.unbind(self, key: key) }
}
private enum T3InspectorColor {
    static func parse(_ text: String) -> UIColor {
        if text.hasPrefix("#"), let hex = UInt64(text.dropFirst(), radix: 16) {
            let rgb = text.count == 9 ? hex >> 8 : hex
            return UIColor(red: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255,
                blue: CGFloat(rgb & 255) / 255, alpha: text.count == 9 ? CGFloat(hex & 255) / 255 : 1)
        }
        if let start = text.firstIndex(of: "("), let end = text.lastIndex(of: ")") {
            let parts = text[text.index(after: start)..<end].split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespaces)) }
            if parts.count >= 3 { return UIColor(red: parts[0] / 255, green: parts[1] / 255, blue: parts[2] / 255, alpha: parts.count > 3 ? parts[3] : 1) }
        }
        return .label
    }
}
#endif

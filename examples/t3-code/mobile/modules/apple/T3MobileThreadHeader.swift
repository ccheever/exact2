// @ref llp/1109.005-composer-and-transcript.decision.md#native-thread-header
// T3 Code365aa87982 useThreadHeaderOptions.tsx and ThreadGitControls.tsx.
#if os(iOS)
import UIKit

struct T3ThreadHeaderItem: Decodable, Equatable {
    let id: String; let title: String; let subtitle: String; let symbol: String; let disabled: Bool
}
struct T3ThreadHeaderConfiguration: Decodable, Equatable {
    let owner: String; let routeKey: String; let version: String; let focused: Bool
    let title: String; let subtitle: String; let split: Bool; let sidebarVisible: Bool
    let canGoBack: Bool; let returnToChat: Bool; let canOpenFiles: Bool; let canOpenTerminal: Bool
    let foreground: String; let gitItems: [T3ThreadHeaderItem]; let terminalItems: [T3ThreadHeaderItem]

    func presentation(nativeGlass: Bool) -> T3ThreadHeaderPresentation {
        // The compact pre26 fallback's later layout effect replaces the right
        // factory. Its direct Files/auxiliary controls both require split layout.
        if !nativeGlass && !split {
            return T3ThreadHeaderPresentation(groups: T3ThreadHeaderPresentation.trailingGroups(["terminal", "git"], nativeGlass: false), nativeGlass: false,
                gitTitle: "Git controls", gitLabel: "Git controls", terminalTitle: "",
                gitItems: gitItems.filter { ["git:branch", "git:quick", "git:review", "git:more"].contains($0.id) })
        }
        let controls = split ? ["files", "git", "terminal"] : ["git", "files", "terminal"]
        return T3ThreadHeaderPresentation(groups: T3ThreadHeaderPresentation.trailingGroups(controls, nativeGlass: nativeGlass), nativeGlass: nativeGlass,
            gitTitle: "Git", gitLabel: "Git actions", terminalTitle: "Terminal", gitItems: gitItems)
    }

    func allows(_ id: String) -> Bool {
        guard focused, !id.isEmpty, !owner.isEmpty, !routeKey.isEmpty else { return false }
        switch id {
        case "files": return canOpenFiles
        case "sidebar", "new-task": return split
        case "home": return !split && !canGoBack
        case "return-chat": return split && returnToChat
        default:
            if let item = gitItems.first(where: { $0.id == id }) { return !item.disabled }
            if let item = terminalItems.first(where: { $0.id == id }) { return canOpenTerminal && !item.disabled }
            return false
        }
    }
    func accepts(_ captured: Self, itemID: String) -> Bool {
        owner == captured.owner && routeKey == captured.routeKey && version == captured.version
            && allows(itemID) && captured.allows(itemID)
    }
}

struct T3ThreadHeaderPresentation {
    let groups: [[String]]; let nativeGlass: Bool
    // Native-stack reverses right items before the screens patch groups them.
    // Mirror that adapter step for every trailing policy; leading stays ordered.
    static func trailingGroups(_ controls: [String], nativeGlass: Bool) -> [[String]] {
        group(Array(controls.reversed()), nativeGlass: nativeGlass)
    }
    // The pinned native screens patch enables background sharing only on iOS 26+.
    static func group<T>(_ controls: [T], nativeGlass: Bool) -> [[T]] {
        guard !controls.isEmpty else { return [] }
        return nativeGlass ? [controls] : controls.map { [$0] }
    }
    let gitTitle: String; let gitLabel: String; let terminalTitle: String
    let gitItems: [T3ThreadHeaderItem]
}

final class T3MobileThreadHeader {
    private final class RouteRef { weak var value: ExactRoute?; init(_ value: ExactRoute) { self.value = value } }
    private final class ViewRef { weak var value: T3ThreadHeaderView?; init(_ value: T3ThreadHeaderView) { self.value = value } }
    private var routes: [String: RouteRef] = [:]
    private var views: [String: ViewRef] = [:]
    private var active = true
    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard active else { throw ExactNativeRefusal("The thread header session has ended.") }
        let result = T3ThreadHeaderView(owner: self, events: events)
        try result.setProps(props); return result
    }
    func configure(_ route: ExactRoute) {
        guard active else { return }
        routes[route.key] = RouteRef(route); views[route.key]?.value?.attach(route)
    }
    func end(_ route: ExactRoute) {
        guard routes[route.key]?.value === route else { return }
        views[route.key]?.value?.detach(); routes.removeValue(forKey: route.key)
    }
    fileprivate func bind(_ view: T3ThreadHeaderView, key: String) {
        guard active else { return }
        if let old = views[key]?.value, old !== view { old.detach() }
        views[key] = ViewRef(view)
        if let route = routes[key]?.value { view.attach(route) }
    }
    fileprivate func unbind(_ view: T3ThreadHeaderView, key: String) {
        if views[key]?.value === view { views.removeValue(forKey: key) }
    }
    func destroy() {
        active = false
        for reference in views.values { reference.value?.detach() }
        views.removeAll(); routes.removeAll()
    }
}

private final class T3ThreadHeaderView: ExactNativeInstance {
    private let root = UIView()
    private weak var owner: T3MobileThreadHeader?
    private weak var route: ExactRoute?
    private var config: T3ThreadHeaderConfiguration?
    private var active = true
    private var trailing: [UIBarButtonItemGroup] = []
    private var leading: [UIBarButtonItemGroup] = []
    private var appearance: UINavigationBarAppearance?
    private var rendered: T3ThreadHeaderConfiguration?
    override var view: UIView { root }
    init(owner: T3MobileThreadHeader, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events)
        root.isUserInteractionEnabled = false; root.accessibilityElementsHidden = true
    }
    override func setProps(_ props: [String: String]) throws {
        guard active, let bytes = props["configuration"]?.data(using: .utf8),
              let next = try? JSONDecoder().decode(T3ThreadHeaderConfiguration.self, from: bytes),
              !next.routeKey.isEmpty else { throw ExactNativeRefusal("Invalid thread header configuration.") }
        if config?.routeKey != next.routeKey {
            let oldKey = config?.routeKey ?? ""
            detach(); owner?.unbind(self, key: oldKey)
        }
        config = next; owner?.bind(self, key: next.routeKey)
    }
    fileprivate func attach(_ route: ExactRoute) {
        if self.route !== route { detach(); self.route = route }
        refresh()
    }
    private func emit(_ id: String, captured: T3ThreadHeaderConfiguration) {
        guard active, let config, config.accepts(captured, itemID: id), let route,
              route.isLive, route.key == captured.routeKey,
              route.controller.navigationController?.topViewController === route.controller,
              route.controller.viewIfLoaded?.window != nil,
              route.controller.presentedViewController == nil,
              let bytes = try? JSONSerialization.data(withJSONObject: ["owner": captured.owner,
                "routeKey": captured.routeKey, "version": captured.version, "itemID": id]) else { return }
        events.change(String(decoding: bytes, as: UTF8.self))
    }
    private func action(_ id: String, title: String, symbol: String, captured: T3ThreadHeaderConfiguration) -> UIBarButtonItem {
        let item = UIBarButtonItem(image: UIImage(systemName: symbol), primaryAction: UIAction { [weak self] _ in
            self?.emit(id, captured: captured)
        })
        item.accessibilityLabel = title; item.accessibilityIdentifier = "thread-header-\(id)"
        item.isEnabled = captured.allows(id); item.tintColor = color(captured.foreground)
        return item
    }
    private func menu(_ items: [T3ThreadHeaderItem], title: String, symbol: String,
                      label: String, identifier: String, enabled: Bool, captured: T3ThreadHeaderConfiguration) -> UIBarButtonItem {
        let children = items.map { row -> UIAction in
            let action = UIAction(title: row.title, image: UIImage(systemName: row.symbol),
                                  identifier: UIAction.Identifier(row.id), attributes: row.disabled ? [.disabled] : []) { [weak self] _ in
                self?.emit(row.id, captured: captured)
            }
            action.subtitle = row.subtitle.isEmpty ? nil : row.subtitle
            return action
        }
        let item = UIBarButtonItem(title: title, image: UIImage(systemName: symbol), primaryAction: nil,
                                  menu: UIMenu(title: title, children: children))
        item.accessibilityLabel = label; item.accessibilityIdentifier = identifier
        item.isEnabled = enabled && captured.focused; item.tintColor = color(captured.foreground)
        return item
    }
    private func refresh() {
        guard active, let config, let route, route.isLive, route.key == config.routeKey else { return }
        let item = route.controller.navigationItem
        item.title = config.title; item.largeTitleDisplayMode = .never
        item.backButtonDisplayMode = .minimal; item.hidesBackButton = config.split || !config.canGoBack
        if rendered != config {
            let style = item.standardAppearance?.copy() as? UINavigationBarAppearance ?? UINavigationBarAppearance()
            if #available(iOS 26.0, *) {
                style.configureWithTransparentBackground(); item.style = .editor
                item.subtitle = config.subtitle.isEmpty ? nil : config.subtitle
                style.titleTextAttributes[.font] = UIFont.systemFont(ofSize: 17, weight: .heavy)
            } else {
                style.configureWithDefaultBackground()
                style.titleTextAttributes.removeValue(forKey: .font)
            }
            style.titleTextAttributes[.foregroundColor] = color(config.foreground); style.shadowColor = .clear
            appearance = style; item.standardAppearance = style; item.scrollEdgeAppearance = style; item.compactAppearance = style
            let presentation: T3ThreadHeaderPresentation
            if #available(iOS 26.0, *) { presentation = config.presentation(nativeGlass: true) }
            else { presentation = config.presentation(nativeGlass: false) }
            let git = menu(presentation.gitItems, title: presentation.gitTitle,
                           symbol: "point.topleft.down.curvedto.point.bottomright.up", label: presentation.gitLabel,
                           identifier: "thread-right-git", enabled: true, captured: config)
            let files = action("files", title: "Open files", symbol: "folder", captured: config)
            let terminal = menu(config.terminalItems, title: presentation.terminalTitle, symbol: "terminal", label: "Open terminal",
                                identifier: "thread-right-terminal", enabled: config.canOpenTerminal, captured: config)
            let buttons = ["git": git, "files": files, "terminal": terminal]
            trailing = presentation.groups.compactMap { ids in
                let items = ids.compactMap { buttons[$0] }
                return items.isEmpty ? nil : .fixedGroup(representativeItem: nil, items: items)
            }
            var left: [UIBarButtonItemGroup] = []
            if config.split {
                let spacing = UIBarButtonItem(barButtonSystemItem: .fixedSpace, target: nil, action: nil); spacing.width = 18
                left.append(.fixedGroup(representativeItem: nil, items: [spacing]))
                var controls: [UIBarButtonItem] = []
                if config.returnToChat { controls.append(action("return-chat", title: "Return to chat", symbol: "chevron.left", captured: config)) }
                controls.append(action("sidebar", title: config.sidebarVisible ? "Maximize content" : "Show thread sidebar",
                    symbol: config.sidebarVisible ? "arrow.up.left.and.arrow.down.right" : "sidebar.left", captured: config))
                controls.append(action("new-task", title: "New task", symbol: "square.and.pencil", captured: config))
                left.append(contentsOf: T3ThreadHeaderPresentation.group(controls, nativeGlass: presentation.nativeGlass)
                    .map { .fixedGroup(representativeItem: nil, items: $0) })
            } else if !config.canGoBack {
                left.append(.fixedGroup(representativeItem: nil, items: [action("home", title: "Go to threads list", symbol: "list.bullet", captured: config)]))
            }
            leading = left; rendered = config
        }
        if #available(iOS 26.0, *) { item.style = .editor; item.subtitle = config.subtitle.isEmpty ? nil : config.subtitle }
        item.standardAppearance = appearance; item.scrollEdgeAppearance = appearance; item.compactAppearance = appearance
        if !sameGroups(item.trailingItemGroups, trailing) { item.trailingItemGroups = trailing }
        if !sameGroups(item.leadingItemGroups, leading) { item.leadingItemGroups = leading }
    }
    private func sameGroups(_ lhs: [UIBarButtonItemGroup], _ rhs: [UIBarButtonItemGroup]) -> Bool {
        lhs.count == rhs.count && zip(lhs, rhs).allSatisfy { $0 === $1 }
    }
    fileprivate func detach() {
        if let item = route?.controller.navigationItem {
            if item.trailingItemGroups.contains(where: { current in trailing.contains(where: { $0 === current }) }) { item.trailingItemGroups = [] }
            if item.leadingItemGroups.contains(where: { current in leading.contains(where: { $0 === current }) }) { item.leadingItemGroups = [] }
            if let appearance, item.standardAppearance === appearance {
                item.standardAppearance = nil; item.scrollEdgeAppearance = nil; item.compactAppearance = nil
            }
        }
        trailing = []; leading = []; appearance = nil; rendered = nil; route = nil
    }
    override func destroy() {
        active = false; detach(); owner?.unbind(self, key: config?.routeKey ?? ""); config = nil
    }
    private func color(_ text: String) -> UIColor {
        let raw = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if raw.hasPrefix("#"), [7, 9].contains(raw.count), let value = UInt64(raw.dropFirst(), radix: 16) {
            let rgb = raw.count == 9 ? value >> 8 : value
            return UIColor(red: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255,
                           blue: CGFloat(rgb & 255) / 255, alpha: raw.count == 9 ? CGFloat(value & 255) / 255 : 1)
        }
        if raw.hasPrefix("rgb"), let start = raw.firstIndex(of: "("), let end = raw.lastIndex(of: ")") {
            let values = raw[raw.index(after: start)..<end].split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespaces)) }
            if values.count >= 3 { return UIColor(red: values[0] / 255, green: values[1] / 255, blue: values[2] / 255, alpha: values.count > 3 ? values[3] : 1) }
        }
        return .label
    }
}
#endif

// @ref llp/1107.004-home-projection.decision.md#ordinary-row-actions
// @ref llp/1107.011-responsive-workspace.decision.md#composition-decision
// GAP 007: the visible sidebar is an inactive tab for Exact's authored menus.
// The public element hatch adds only an app-owned interaction; root keeps actions.
import Foundation

struct T3HomeMenuConfiguration: Decodable, Equatable {
    struct Item: Decodable, Equatable {
        let id: String, parentId: String, label: String, operation: String, value: String, symbol: String, subtitle: String
        let destructive: Bool, checked: Bool, disabled: Bool
        func isDisabled(under parent: Item? = nil) -> Bool { disabled || parent?.disabled == true }
    }
    let identity: String, requestRoute: String
    let enabled: Bool
    let items: [Item]

    static func decode(_ text: String) -> Self? {
        guard let result = try? JSONDecoder().decode(Self.self, from: Data(text.utf8)),
              !result.identity.isEmpty, !result.requestRoute.isEmpty, !result.items.isEmpty,
              Set(result.items.map(\.id)).count == result.items.count,
              result.items.allSatisfy({ !$0.id.isEmpty && !$0.label.isEmpty }) else { return nil }
        let parents = result.items.filter { $0.parentId.isEmpty && $0.operation.isEmpty }
        guard parents.allSatisfy({ parent in result.items.contains { $0.parentId == parent.id } }),
              result.items.allSatisfy({ item in
                  item.parentId.isEmpty || (!item.operation.isEmpty && parents.contains { $0.id == item.parentId })
              }) else { return nil }
        return result
    }
}

#if os(iOS)
import UIKit

final class T3MobileHomeMenus {
    private weak var route: ExactRoute?
    private var bindings: [ObjectIdentifier: T3HomeMenuBinding] = [:]
    private var visible = false
    private var alive = true

    func configure(_ route: ExactRoute) {
        guard alive, route.key == "t3-workspace-sidebar" else { return }
        if self.route !== route { bindings.values.forEach { $0.invalidate() }; self.route = route }
    }
    func end(_ route: ExactRoute) {
        guard self.route === route else { return }
        bindings.values.forEach { $0.invalidate() }; self.route = nil
    }
    func setVisible(_ visible: Bool) {
        self.visible = visible
        if !visible { bindings.values.forEach { $0.invalidate() } }
    }
    func configure(_ element: ExactElement) {
        let key = ObjectIdentifier(element)
        guard alive, let view = element.view,
              let raw = element.data[.mobileHomeMenu], let configuration = T3HomeMenuConfiguration.decode(raw),
              let menuId = element.data[.mobileHomeMenuId], menuId.hasPrefix("home-menu-sidebar-list-") else {
            bindings.removeValue(forKey: key)?.end(); element.reusable = true; return
        }
        if let binding = bindings[key], binding.matches(element, view: view) {
            binding.update(configuration, menuId: menuId)
        } else {
            bindings.removeValue(forKey: key)?.end()
            bindings[key] = T3HomeMenuBinding(owner: self, element: element, view: view, configuration: configuration, menuId: menuId)
        }
        element.reusable = true
    }
    func end(_ element: ExactElement) {
        bindings.removeValue(forKey: ObjectIdentifier(element))?.end(); element.reusable = true
    }
    fileprivate func currentRoute() -> ExactRoute? {
        guard alive, visible, let route, route.isLive, route.controller.viewIfLoaded?.window != nil else { return nil }
        return route
    }
    func destroy() {
        alive = false; visible = false
        bindings.values.forEach { $0.end() }; bindings.removeAll(); route = nil
    }
}

private final class T3HomeMenuBinding: NSObject, UIContextMenuInteractionDelegate {
    private weak var owner: T3MobileHomeMenus?
    private weak var element: ExactElement?
    private weak var view: UIView?
    private var configuration: T3HomeMenuConfiguration
    private var menuId: String
    private var interaction: UIContextMenuInteraction?
    private var addedInteractions: [UIInteraction] = []
    private var addedRecognizers: [UIGestureRecognizer] = []
    private var opening: UUID?
    private var active = true

    init(owner: T3MobileHomeMenus, element: ExactElement, view: UIView, configuration: T3HomeMenuConfiguration, menuId: String) {
        self.owner = owner; self.element = element; self.view = view; self.configuration = configuration; self.menuId = menuId
        super.init()
        let priorInteractions = Set(view.interactions.map(ObjectIdentifier.init))
        let priorRecognizers = Set((view.gestureRecognizers ?? []).map(ObjectIdentifier.init))
        let interaction = UIContextMenuInteraction(delegate: self)
        self.interaction = interaction; view.addInteraction(interaction)
        addedInteractions = view.interactions.filter { !priorInteractions.contains(ObjectIdentifier($0)) }
        addedRecognizers = (view.gestureRecognizers ?? []).filter { !priorRecognizers.contains(ObjectIdentifier($0)) }
    }
    func matches(_ element: ExactElement, view: UIView) -> Bool { active && self.element === element && self.view === view }
    func update(_ next: T3HomeMenuConfiguration, menuId: String) {
        if configuration != next || self.menuId != menuId { invalidate(); configuration = next; self.menuId = menuId }
    }
    func invalidate() { opening = nil; interaction?.dismissMenu() }
    func end() {
        guard active else { return }; active = false; invalidate()
        if let view {
            for interaction in addedInteractions where interaction.view === view { view.removeInteraction(interaction) }
            for recognizer in addedRecognizers where recognizer.view === view { view.removeGestureRecognizer(recognizer) }
        }
        addedInteractions.removeAll(); addedRecognizers.removeAll(); interaction = nil; element = nil; view = nil; owner = nil
    }
    private func current(_ token: UUID? = nil, route: ExactRoute? = nil) -> Bool {
        guard active, configuration.enabled, let element, element.isLive, let view, element.view === view,
              view.window != nil, let currentRoute = owner?.currentRoute(),
              token == nil || opening == token, route == nil || currentRoute === route else { return false }
        var ancestor: UIView? = view
        while let current = ancestor {
            if current.isHidden || current.alpha == 0 || !current.isUserInteractionEnabled { return false }
            ancestor = current.superview
        }
        return true
    }
    func contextMenuInteraction(_ interaction: UIContextMenuInteraction, configurationForMenuAtLocation location: CGPoint) -> UIContextMenuConfiguration? {
        guard current(), let route = owner?.currentRoute() else { return nil }
        let token = UUID(), captured = configuration, prefix = menuId
        opening = token
        return UIContextMenuConfiguration(identifier: token.uuidString as NSString, previewProvider: nil) { [weak self, weak route] _ in
            guard let self, let route, self.current(token, route: route), self.configuration == captured else { return UIMenu() }
            func leaf(_ item: T3HomeMenuConfiguration.Item, parent: T3HomeMenuConfiguration.Item? = nil) -> UIAction {
                let disabled = item.isDisabled(under: parent)
                var attributes = UIMenuElement.Attributes()
                if disabled { attributes.insert(.disabled) }
                if item.destructive { attributes.insert(.destructive) }
                // Keep this wrapper: route.element returns a new object with a weak route.
                let target = route.element("\(prefix)-\(item.id)")
                if target == nil { attributes.insert(.disabled) }
                return UIAction(title: item.label, subtitle: item.subtitle.isEmpty ? nil : item.subtitle,
                                image: item.symbol.isEmpty ? nil : UIImage(systemName: item.symbol),
                                attributes: attributes, state: item.checked ? .on : .off) { [weak self, weak route] _ in
                    guard let self, let route, self.current(token, route: route), self.configuration == captured,
                          !disabled, let target, target.isLive else { return }
                    // Consume the displayed menu before the queued root press; duplicates refuse.
                    self.opening = nil
                    target.click()
                }
            }
            let children: [UIMenuElement] = captured.items.filter { $0.parentId.isEmpty }.map { item in
                if !item.operation.isEmpty { return leaf(item) }
                let children = captured.items.filter { $0.parentId == item.id }.map { leaf($0, parent: item) }
                return UIMenu(title: item.label, image: item.symbol.isEmpty ? nil : UIImage(systemName: item.symbol), children: children)
            }
            return UIMenu(children: children)
        }
    }
    func contextMenuInteraction(_ interaction: UIContextMenuInteraction, willEndFor configuration: UIContextMenuConfiguration, animator: UIContextMenuInteractionAnimating?) {
        let ended = configuration.identifier as? String
        let finish = { [weak self] in
            if self?.opening?.uuidString == ended { self?.opening = nil }
        }
        if let animator { animator.addCompletion(finish) } else { finish() }
    }
}
#endif

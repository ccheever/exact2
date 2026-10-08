#if os(iOS)
// @ref llp/1107.004-home-projection.decision.md#ordinary-row-actions
// GAP 007: app-owned native menu on the visible inactive sidebar tab.
import UIKit

final class T3HomeSwipeSnooze {
    private weak var owner: T3MobileHomeSwipes?
    private weak var element: ExactElement?
    private weak var view: UIView?
    private let button = UIButton(type: .custom)
    private var identity: T3HomeSwipeRowIdentity
    private var menuID = ""
    private var items: [T3HomeMenuConfiguration.Item] = []
    private var open = false
    private var active = true
    private var revision = UUID()
    var ownerKey: String { identity.owner }
    init(owner: T3MobileHomeSwipes, element: ExactElement, view: UIView, identity: T3HomeSwipeRowIdentity) {
        self.owner = owner; self.element = element; self.view = view; self.identity = identity
        button.frame = view.bounds; button.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        button.showsMenuAsPrimaryAction = true
        button.isAccessibilityElement = false
        view.addSubview(button)
        update(element, identity: identity)
    }
    func matches(_ element: ExactElement, _ view: UIView) -> Bool { active && self.element === element && self.view === view }
    func update(_ element: ExactElement, identity: T3HomeSwipeRowIdentity) {
        let id = element.data[.mobileHomeMenuId] ?? ""
        let open = element.data[.mobileSwipeOpen] == "true"
        let json = element.data[.mobileHomeMenu] ?? ""
        let object = (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [String: Any]
        let raw = object?["swipeItems"] as? [[String: Any]] ?? []
        let data = try? JSONSerialization.data(withJSONObject: raw)
        let items = data.flatMap { try? JSONDecoder().decode([T3HomeMenuConfiguration.Item].self, from: $0) } ?? []
        guard identity != self.identity || id != menuID || open != self.open || items != self.items || button.menu == nil else { return }
        invalidate(); self.identity = identity; menuID = id; self.open = open; self.items = items
        button.isUserInteractionEnabled = open && identity.enabled && !items.isEmpty
        guard !id.isEmpty, Set(items.map(\.id)).count == items.count else { button.menu = nil; return }
        let captured = revision
        button.menu = UIMenu(title: "Snooze until", children: [UIDeferredMenuElement.uncached { [weak self] completion in
            guard let self, self.current(captured), let bridge = self.owner?.bridge(for: self.identity.owner),
                  let route = self.owner?.route(for: bridge) else { completion([]); return }
            let actions: [UIMenuElement] = self.items.map { item in
                let target = route.element("\(self.menuID)-\(item.id)")
                let disabled = item.disabled || target == nil
                return UIAction(title: item.label, subtitle: item.subtitle.isEmpty ? nil : item.subtitle,
                    image: item.symbol.isEmpty ? nil : UIImage(systemName: item.symbol),
                    attributes: (disabled ? UIMenuElement.Attributes.disabled : []).union(item.destructive ? .destructive : []), state: item.checked ? .on : .off) { [weak self, weak route, weak bridge] _ in
                    guard let self, let route, let bridge, self.current(captured), !disabled, route.isLive,
                          self.owner?.route(for: bridge) === route, let target, target.isLive else { return }
                    self.revision = UUID(); target.click()
                }
            }
            completion(actions)
        }])
    }
    private func current(_ captured: UUID) -> Bool {
        guard active, captured == revision, open, identity.enabled, element?.isLive == true,
              let view, element?.view === view, view.window != nil,
              owner?.bridge(for: identity.owner) != nil else { return false }
        var at: UIView? = view
        while let next = at { if next.isHidden || next.alpha <= 0 || !next.isUserInteractionEnabled { return false }; at = next.superview }
        return true
    }
    func invalidate() { revision = UUID(); button.contextMenuInteraction?.dismissMenu() }
    func end() {
        guard active else { return }; invalidate(); active = false
        button.menu = nil; button.removeFromSuperview(); element = nil; view = nil; owner = nil
    }
}
#endif

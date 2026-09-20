// @ref LLP 1038 D6; LLP 1035.001 D1/D6 — the web's hide-and-inert rule.
#if os(macOS)
import AppKit

final class NavigationHost {
    unowned let presenter: Presenter
    private var refused: [UInt32: String] = [:]
    private var hadChrome = false
    init(presenter: Presenter) { self.presenter = presenter }
    func reset() { refused.removeAll(); hadChrome = false }

    func sync() {
        refused = refused.filter { presenter.views[$0.key] != nil }
        for nav in presenter.carrying("navigationBack") {
            let routes = nav.container.subviews.compactMap { $0 as? NodeView }.filter { $0.props["navigationKey"] != nil }
            let key = nav.props["navigationKey"] ?? ""
            guard let prefix = NavigationRules.stack(routeKeys: routes.map { $0.props["navigationKey"] ?? "" }, selected: key) else {
                if refused[nav.id] != key {
                    refused[nav.id] = key
                    presenter.session?.log("navigationKey \"\(key)\" matches no route; the stack is unchanged")
                }
                continue
            }
            refused.removeValue(forKey: nav.id)
            let selected = prefix.upperBound - 1
            let modal = routes[selected].props["navigationPresentation"] == "modal"
            for (index, route) in routes.enumerated() {
                route.isHidden = index != selected && !(modal && index == selected - 1)
                route.routeInert = index != selected
            }
        }
        // Reuse the presenter's ancestor inert gate for focus/input. AppKit's
        // accessibility subtree is suppressed as HTML inert suppresses it.
        // Only a route, an `inert` prop, a dialog, a popover, a tab list or the
        // window toolbar ever hides a view or makes one inert; with none
        // mounted this pass restates what `applyProps` already set. One more
        // pass after the last of them goes clears what it left behind.
        let chrome = presenter.chrome.hidesOrInerts
        defer { hadChrome = chrome }
        guard chrome || hadChrome else { return }
        for node in presenter.views.values {
            let inert = node.inert
            node.setAccessibilityHidden(inert || node.isHiddenOrHasHiddenAncestor)
            node.field?.isEnabled = !node.disabled && !inert
            node.textArea?.isEditable = !node.disabled && !inert && node.props["editable"] != "false"
            if (inert || node.isHiddenOrHasHiddenAncestor), let responder = node.window?.firstResponder as? NSView,
               responder === node || responder.isDescendant(of: node) || node.field?.currentEditor() === responder {
                node.window?.makeFirstResponder(nil)
            }
        }
    }
}
#endif

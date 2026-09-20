// @ref LLP 1038 D6; LLP 1035.001 D1/D6 — the web's hide-and-inert rule.
#if os(macOS)
import AppKit

final class NavigationHost {
    unowned let presenter: Presenter
    private var refused: [UInt32: String] = [:]
    init(presenter: Presenter) { self.presenter = presenter }
    func reset() { refused.removeAll() }

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
        for node in presenter.views.values {
            let inert = node.inert
            let hidden = inert || node.isHiddenOrHasHiddenAncestor
            // The AX getter can traverse the legacy unsupported-attribute path
            // for non-elements; writing this property is cheaper than querying it.
            node.setAccessibilityHidden(hidden)
            if let field = node.field {
                let enabled = !node.disabled && !inert
                if field.isEnabled != enabled { field.isEnabled = enabled }
            }
            if let area = node.textArea {
                let editable = !node.disabled && !inert && node.props["editable"] != "false"
                if area.isEditable != editable { area.isEditable = editable }
            }
            if hidden, let responder = node.window?.firstResponder as? NSView,
               responder === node || responder.isDescendant(of: node) || node.field?.currentEditor() === responder {
                node.window?.makeFirstResponder(nil)
            }
        }
    }
}
#endif

// @ref LLP 1038 D6; LLP 1035.001 D1/D6 — the web's hide-and-inert rule.
#if os(macOS)
import AppKit

final class NavigationHost {
    unowned let presenter: Presenter
    private var refused: [UInt32: String] = [:]
    private var gates: [UInt32: [Bool]] = [:]
    /// The ops that can change what hides or disables a node: a frame, a
    /// paint or a presentation value (every frame of an animation) cannot,
    /// and each write posts an accessibility notification.
    private static let gating: Set<BatchOp.Kind> = [.create, .props, .style, .children, .roots]
    init(presenter: Presenter) { self.presenter = presenter }
    func reset() { refused.removeAll(); gates.removeAll() }

    func sync(_ batch: Batch, reparented: Set<UInt32> = []) {
        var touched = Set<UInt32>()
        var subtrees = reparented
        for op in batch.ops {
            if let id = op.nodeID {
                if op.op == .destroy { gates.removeValue(forKey: id); refused.removeValue(forKey: id) }
                else if Self.gating.contains(op.op) { touched.insert(id) }
            }
            // A children op names retained siblings too. Only changed ancestry
            // can change their inherited gates; a list append must not revisit
            // every paragraph and cell already mounted in the window.
            if op.op == .roots {
                subtrees.formUnion(op.ids)
            }
        }
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
                let hidden = index != selected && !(modal && index == selected - 1)
                let inert = index != selected
                if route.isHidden != hidden || route.routeInert != inert { subtrees.insert(route.id) }
                route.isHidden = hidden
                route.routeInert = inert
            }
        }
        // Reuse the presenter's ancestor inert gate for focus/input. AppKit's
        // accessibility subtree is suppressed as HTML inert suppresses it.
        for id in touched {
            guard let node = presenter.views[id] else { continue }
            let gate = [node.isHidden, node.routeInert, node.props["inert"] == "true", node.props["accessibilityElementsHidden"] == "true"]
            if let old = gates[id], old != gate { subtrees.insert(id) }
            gates[id] = gate
        }
        func descendants(_ view: NSView) {
            if let node = view as? NodeView { touched.insert(node.id) }
            for child in view.subviews { descendants(child) }
        }
        for id in subtrees { if let node = presenter.views[id] { descendants(node) } }
        for id in touched {
            guard let node = presenter.views[id] else { continue }
            gates[id] = [node.isHidden, node.routeInert, node.props["inert"] == "true", node.props["accessibilityElementsHidden"] == "true"]
        }
        refreshInputGates(touched.compactMap { presenter.views[$0] })
    }

    /// Top-layer ownership changes the same native input/AX gates as inert.
    func refreshInputGates(_ nodes: [NodeView]) {
        for node in nodes {
            let inert = node.inert
            let hidden = inert || node.isHiddenOrHasHiddenAncestor || node.accessibilityHiddenByProp
            // The AX getter can traverse the legacy unsupported-attribute path
            // for non-elements; writing this property is cheaper than querying it.
            node.setAccessibilityHidden(hidden)
            let enabled = !node.disabled && !inert
            if let field = node.field, field.isEnabled != enabled { field.isEnabled = enabled }
            let editable = enabled && node.props["editable"] != "false"
            if let area = node.textArea, area.isEditable != editable { area.isEditable = editable }
            if hidden, let responder = node.window?.firstResponder as? NSView,
               responder === node || responder.isDescendant(of: node) || node.field?.currentEditor() === responder {
                node.window?.makeFirstResponder(nil)
            }
        }
    }
}
#endif

// @ref LLP 1038 D6; LLP 1035.001 D1/D6 — the web's hide-and-inert rule.
#if os(macOS)
import AppKit

final class NavigationHost {
    unowned let presenter: Presenter
    private var refused: [UInt32: String] = [:]
    private var gates: [UInt32: [Bool]] = [:]
    /// The tabpanels a root last hid or showed: one it stops naming shows again.
    private var panels: Set<UInt32> = []
    /// Each root's stack as last shown, its keys through its selected route.
    private var shown: [UInt32: [String]] = [:]
    /// The ops that can change what hides or disables a node: a frame, a
    /// paint or a presentation value (every frame of an animation) cannot,
    /// and each write posts an accessibility notification.
    private static let gating: Set<BatchOp.Kind> = [.create, .props, .style, .children, .roots]
    init(presenter: Presenter) { self.presenter = presenter }
    /// LLP 1080.001 D3: a route or tabpanel this host hid (the web's
    /// hide-and-inert rule), which it inerts with it.
    func hides(_ node: NodeView) -> Bool { node.routeInert && (panels.contains(node.id) || node.props["navigationKey"] != nil) }
    /// The first navigation root, the stack that holds the route it shows
    /// and that route's place in it, as of the last batch: what the
    /// platform's Back leaves (BackMac.swift).
    private(set) var selection: (root: UInt32, stack: [UInt32], at: Int)?
    /// Whether a route mounted under that root, in any of its stacks, shows
    /// the platform's bar (does not say `navigationBar="none"`): what gives
    /// the window its toolbar (ToolbarMac.swift).
    private(set) var barRoute = false
    func reset() { refused.removeAll(); gates.removeAll(); panels.removeAll(); shown.removeAll(); selection = nil; barRoute = false }

    func sync(_ batch: Batch, reparented: Set<UInt32> = []) {
        var touched = Set<UInt32>()
        var subtrees = reparented
        for op in batch.ops {
            if let id = op.nodeID {
                if op.op == .destroy { gates.removeValue(forKey: id); refused.removeValue(forKey: id); shown.removeValue(forKey: id) }
                else if Self.gating.contains(op.op) { touched.insert(id) }
            }
            // A children op names retained siblings too. Only changed ancestry
            // can change their inherited gates; a list append must not revisit
            // every paragraph and cell already mounted in the window.
            if op.op == .roots {
                subtrees.formUnion(op.ids)
            }
        }
        func gate(_ node: NodeView, hidden: Bool, inert: Bool) {
            if node.isHidden != hidden || node.routeInert != inert { subtrees.insert(node.id) }
            node.isHidden = hidden
            node.routeInert = inert
        }
        var managed = Set<UInt32>()
        let roots = presenter.carrying("navigationBack")
        selection = nil
        barRoute = false
        for nav in roots {
            // @ref LLP 1075.003 §3.7 — with tabs, each panel is a stack.
            let tabs = NavigationTabs.of(nav, presenter)
            let locate = { (key: String) -> (at: Int?, stacks: [[NodeView]]) in
                tabs?.stacks(selecting: key) ?? {
                    let routes = NavigationTabs.routes(in: nav)
                    return (routes.contains { $0.props["navigationKey"] == key } ? 0 : nil, [routes])
                }()
            }
            var key = nav.props["navigationKey"] ?? ""
            var (found, stacks) = locate(key)
            if found == nil {
                if refused[nav.id] != key {
                    refused[nav.id] = key
                    presenter.session?.log("navigationKey \"\(key)\" matches no route among the root's children or those of the tabpanels its tablist names; the stack is unchanged")
                }
                // D1: left alone, less the routes that left the tree (a closed sheet's).
                let declared = stacks.map { $0.map { $0.props["navigationKey"] ?? "" } }
                if let survivor = NavigationRules.survivor(of: shown[nav.id] ?? [], declared: declared) {
                    key = survivor
                    (found, stacks) = locate(key)
                }
            } else {
                refused.removeValue(forKey: nav.id)
            }
            guard let at = found else {
                managed.formUnion((tabs?.panels ?? []).map(\.id))
                continue
            }
            for (index, panel) in (tabs?.panels ?? []).enumerated() {
                gate(panel, hidden: index != at, inert: index != at)
                managed.insert(panel.id)
            }
            for (stack, routes) in stacks.enumerated() {
                // The selected stack shows the route the root names; another
                // keeps its top laid out under its hidden panel.
                let selected = stack == at ? routes.firstIndex { $0.props["navigationKey"] == key } ?? 0 : routes.count - 1
                if stack == at { shown[nav.id] = routes.prefix(selected + 1).map { $0.props["navigationKey"] ?? "" } }
                if stack == at, nav === roots.first { selection = (nav.id, routes.map(\.id), selected) }
                if nav === roots.first, routes.contains(where: { $0.props["navigationBar"] != "none" }) { barRoute = true }
                let modal = routes.indices.contains(selected) && routes[selected].props["navigationPresentation"] == "modal"
                for (index, route) in routes.enumerated() {
                    gate(route, hidden: index != selected && !(modal && index == selected - 1), inert: index != selected)
                }
            }
        }
        for id in panels.subtracting(managed) {
            if let panel = presenter.views[id] { gate(panel, hidden: false, inert: false) }
        }
        panels = managed
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

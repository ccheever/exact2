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
    /// The first navigation root as of the last batch, its tabs as this
    /// host's views name them (`NavigationTabs`) and the tab whose stack
    /// holds the route it shows: what the sidebar reads (SidebarMac.swift).
    private(set) var root: (node: NodeView, tabs: NavigationTabs?, at: Int?)?
    /// @ref LLP 1115.002 D7.1 — the ground under each modal route that
    /// paints none of its own, by the route's id (`ModalGround`).
    private var grounds: [UInt32: ModalGround] = [:]
    /// @ref LLP 1115.002 D7.4 — the modal routes that are sheets as of the
    /// last batch, bottom to top, with what each sheet shows of its header
    /// (RouteSheetMac.swift), and why a route that is not one stays painted.
    private(set) var sheets: [(route: NodeView, header: SheetHeader?)] = []
    var keptSheets: [UInt32: String] = [:]
    /// A batch that can have changed a header came since the sheets last showed theirs.
    var sheetsRead = false
    /// @ref LLP 1115.002 D4 — the selected stack as a list beside its detail (SplitMac.swift).
    private(set) lazy var split = SplitHost(presenter)
    /// The modal route the root shows painted in the window, on its ground.
    var paintedModal: NodeView? {
        guard let selection, selection.at > 0, selection.stack.indices.contains(selection.at), let route = presenter.views[selection.stack[selection.at]],
              route.props["navigationPresentation"] == "modal", !sheets.contains(where: { $0.route === route }) else { return nil }
        return route
    }
    /// What is shown, read again with no batch: a sheet left, a window came.
    func resync() { sync(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil), concerned: true) }
    /// LLP 1080.001 D3: a modal route's ground is this host's view.
    func inspectionOwns(_ view: NSView) -> Bool { view is ModalGround }
    /// The page's ground changed colour: the grounds are painted again.
    func groundChanged() { for ground in grounds.values { ground.needsDisplay = true }; split.groundChanged() }
    func reset() {
        refused.removeAll(); gates.removeAll(); panels.removeAll(); shown.removeAll(); selection = nil; barRoute = false; root = nil
        sheets = []; keptSheets.removeAll(); split.reset()
        for ground in grounds.values { ground.removeFromSuperview() }
        grounds.removeAll()
    }

    /// A modal route painted in the window covers the route beneath it,
    /// which stays shown under it as a page stays under a sheet: where the
    /// route paints no background of its own, the page's ground is its
    /// surface. What the author wrote on the route (a colour, a gradient)
    /// is the surface. One in its sheet stands on the sheet's.
    private func ground(_ routes: [NodeView], grounded: inout Set<UInt32>) {
        for route in routes where route.props["navigationPresentation"] == "modal" {
            let painted = (route.cgColor("background_color")?.alpha ?? 0) > 0 || route.style["background_image"] != nil
            // In its sheet the route stands on the sheet's own ground (D7.4).
            guard !painted, !sheets.contains(where: { $0.route === route }), !presenter.dialogs.owns(route) else { continue }
            grounded.insert(route.id)
            let ground = grounds[route.id] ?? ModalGround(presenter: presenter)
            grounds[route.id] = ground
            if ground.superview !== route {
                ground.frame = route.bounds
                route.addSubview(ground, positioned: .below, relativeTo: nil)
            }
        }
    }

    func sync(_ batch: Batch, reparented: Set<UInt32> = [], concerned: Bool? = nil) {
        var touched = Set<UInt32>()
        var subtrees = reparented
        // Read again with no batch (`resync`): the split's rule is asked again too.
        let resynced = concerned != nil
        // Whether a header can have changed: what the window's bar reads again for.
        let concerned = concerned ?? presenter.toolbar.concerns(batch)
        if concerned { sheetsRead = true }
        var sheeted: [(route: NodeView, header: SheetHeader?)] = []
        for op in batch.ops {
            if let id = op.nodeID {
                if op.op == .destroy { gates.removeValue(forKey: id); refused.removeValue(forKey: id); shown.removeValue(forKey: id); keptSheets.removeValue(forKey: id) }
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
        var grounded = Set<UInt32>()
        let roots = presenter.carrying("navigationBack")
        selection = nil
        barRoute = false
        root = nil
        for nav in roots {
            // @ref LLP 1075.003 §3.7 — with tabs, each panel is a stack.
            let tabs = NavigationTabs.of(nav, presenter)
            // A stack's routes as authored: one in its sheet's window has left its parent's views.
            let routes = { [presenter] (node: NodeView) in presenter.blocks.authored(NavigationTabs.routes(in: node), in: node.container) }
            let locate = { (key: String) -> (at: Int?, stacks: [[NodeView]]) in
                let stacks = tabs.map { $0.panels.map(routes) } ?? [routes(nav)]
                return (stacks.firstIndex { $0.contains { $0.props["navigationKey"] == key } }, stacks)
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
            if nav === roots.first {
                if let at = found, stacks.indices.contains(at) {
                    let routes = stacks[at], selected = routes.firstIndex { $0.props["navigationKey"] == key } ?? 0
                    sheeted = sheetRoutes(routes, selected: selected, root: nav, concerned: concerned)
                    split.read(root: nav, routes: routes, selected: selected, batch: resynced ? nil : batch)
                } else { split.readNone() }
                sheets = sheeted
            }
            for routes in stacks { ground(routes, grounded: &grounded) }
            if nav === roots.first { root = (nav, tabs, tabs == nil ? nil : found) }
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
                // The lowest route in a sheet: it and the sheets on it are shown, and the route they stand over.
                let lowest = stack == at && nav === roots.first ? sheeted.first.flatMap { low in routes.firstIndex { $0 === low.route } } : nil
                // A stack that splits (D4) shows its list and the route its pane shows, and the list stays live beside its detail.
                let splits = stack == at && nav === roots.first && split.active, beside = splits ? split.shown?.detail : nil
                let covered = routes.indices.contains(selected) && ["modal", "fullscreen"].contains(routes[selected].props["navigationPresentation"] ?? "")
                for (index, route) in routes.enumerated() {
                    let under = lowest.map { index >= $0 - 1 && index <= selected } ?? false
                    // A route still in its sheet is shown until the sheet has gone (`DialogHost.syncRoutes`).
                    let leaving = presenter.dialogs.owns(route) && !sheeted.contains { $0.route === route }
                    let column = splits && (index == 0 || route === beside)
                    // In the detail's pane, out of its hidden panel's views: hidden and inert by name.
                    let away = stack != at && split.holds(route)
                    gate(route, hidden: leaving ? route.isHidden : away || (index != selected && !(modal && index == selected - 1) && !under && !column),
                         inert: away || (index != selected && !(splits && index == 0 && !covered)))
                }
            }
        }
        if roots.isEmpty { sheets = []; split.readNone() }
        // A sheet arrives and leaves after the batch: AppKit turns the main queue while it does.
        if !sheets.isEmpty || presenter.dialogs.entries.contains(where: \.route) { presenter.afterBatch { [weak presenter] in presenter?.dialogs.syncRoutes() } }
        for id in panels.subtracting(managed) {
            if let panel = presenter.views[id] { gate(panel, hidden: false, inert: false) }
        }
        panels = managed
        for id in Array(grounds.keys) where !grounded.contains(id) { grounds.removeValue(forKey: id)?.removeFromSuperview() }
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
            let hidden = node.accessibilityInert || node.isHiddenOrHasHiddenAncestor || node.accessibilityHiddenByProp
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

/// @ref LLP 1115.002 D7.1 — the opaque surface of a modal route that paints
/// none: the page's ground, under everything the route holds (a child's
/// negative `z-index` included), at the route's size. It is no hit: a click
/// on it is a click on the route. A modal route is a sheet with a window of
/// its own (D7.4, RouteSheetMac.swift); this is the ground of one painted
/// in the window all the same (the driver's own chrome, a header its sheet
/// cannot take the title out of, no window) when the compiler gave it none
/// either (LLP 1116.009 D3 gives a route whose presentation it can read
/// the page's).
final class ModalGround: NSView {
    private weak var presenter: Presenter?
    init(presenter: Presenter) {
        self.presenter = presenter
        super.init(frame: .zero)
        autoresizingMask = [.width, .height]
        wantsLayer = true
        layer?.zPosition = -1
    }
    required init?(coder: NSCoder) { nil }
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        (presenter?.pageBackground ?? .windowBackgroundColor).setFill()
        // A capture's dirty rectangle is the captured view's, not this one's.
        bounds.intersection(dirtyRect).fill()
    }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}
#endif

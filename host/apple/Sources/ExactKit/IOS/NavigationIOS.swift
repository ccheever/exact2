// @ref LLP 1008 §9 — Contract routes projected into UIKit's navigation
// controller. UIKit owns recognition, arbitration, progress and cancellation.
// Only a completed pop invokes the Contract back control. The bar a stack
// shows, what a route projects into it and when the app module's hatches run
// are LLP 1075.003's (NavigationBarIOS.swift, NativeHatches.swift).
#if os(iOS) || os(tvOS)
import UIKit

final class RouteController: UIViewController {
    let node: NodeView
    var key: String { node.props["navigationKey"] ?? "" }
    /// What Exact last projected into the navigation item, and that plus
    /// what the route hatch last saw; whether the hatch has run; the header
    /// lifted into the bar; the content scroll view a hatch was handed.
    var projectedSource: BarSource?, projected: HatchSource?, backSource: String?
    var hatched = false
    weak var lifted: NodeView?
    weak var host: NavigationHost?
    weak var ownedScroll: NodeView?
    /// The scroller a large title collapses with (LLP 1075.003 Stage 3).
    weak var collapseScroll: NodeView?
    /// The scroller that is the route's last in-flow child, which reaches
    /// the screen's bottom edge (LLP 1116 D2).
    weak var edgeScroll: NodeView?
    /// The targets of the bar items projected from its header.
    var barPresses: [BarPress] = []
    /// The header's search field as UIKit's search controller (§9.6).
    var search: HeaderSearch?
    /// The title Exact drew from the heading's group, and the subtitle it
    /// wrote; whether the root's tablist was hidden when it last looked (§9.10).
    var titleView: HeaderTitleView?, subtitle: String?, tablistHidden: Bool?
    init(_ node: NodeView) {
        self.node = node
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { nil }
    // The bar's height changed (a push to an inline title, a rotation, a
    // hatch): the route's content area follows (LLP 1075.003 §3.5).
    override func viewSafeAreaInsetsDidChange() {
        super.viewSafeAreaInsetsDidChange()
        host?.coversChanged()
    }
    override func loadView() {
        view = UIView()
        paintBackdrop()
        view.addSubview(node)
    }

    /// The backdrop's colours last written, light and dark.
    private var backdropKey: [[Double]?]?
    /// The controller's view paints what shows behind the route. The sheet
    /// supplies its surface behind transparent authored corners; dimming
    /// belongs outside that surface, to UIKit's presentation. Otherwise a
    /// dynamic colour: the route's backdrop (`backdrop`) resolved for the
    /// controller's current appearance, re-resolved by UIKit when it changes
    /// (a route loaded before its window has a trait collection would
    /// otherwise keep the light colour in dark mode), and written again when
    /// a batch changes it. A route under no background shows the system
    /// background, not white.
    func paintBackdrop() {
        guard isViewLoaded else { return }
        let modal = node.props["navigationPresentation"] == "modal"
        let key = modal ? nil : [backdrop(dark: false), backdrop(dark: true)]
        guard view.backgroundColor == nil || key != backdropKey else { return }
        backdropKey = key
        #if os(tvOS)
        // tvOS has no system backgrounds; white stands in, as the viewport's.
        let system = UIColor.white
        #else
        // A sheet's surface is the platform's (LLP 1115 D2): the route
        // paints its own background, if any, over it.
        let system = UIColor.systemBackground
        #endif
        view.backgroundColor = modal ? system : UIColor { [weak self] traits in
            self?.backdrop(dark: traits.userInterfaceStyle == .dark).map { TextEngine.color($0) } ?? system
        }
    }

    /// What shows behind the route where it paints nothing: its own
    /// background, else the nearest one its logical ancestors paint — its
    /// tabpanel's, then the boxes up to the root — as the web shows them
    /// through a transparent route (LLP 1116 D2: a root's
    /// `-exact-grouped-background` under every screen).
    func backdrop(dark: Bool) -> [Double]? {
        let painted = { (n: NodeView) in n.channels("background_color", dark: dark).flatMap { $0.count < 4 || $0[3] > 0 ? $0 : nil } }
        if let own = painted(node) { return own }
        guard let host, let root = host.container else { return nil }
        var next: UIView? = host.logicalParent(of: node)
        while let v = next {
            if let n = v as? NodeView, let colour = painted(n) { return colour }
            if v === root { break }
            next = v.superview
        }
        return nil
    }
    func mount() {
        loadViewIfNeeded()
        if node.superview !== view { view.addSubview(node) }
    }
    // A button can remove a route before UIKit starts its pop. Preserve its
    // outgoing pixels for that transition; an interactive pop uses live views.
    func freeze() {
        guard isViewLoaded, let snapshot = view.snapshotView(afterScreenUpdates: false) else { return }
        snapshot.frame = view.bounds
        // Pinned to the top, never stretched: the viewport can grow under it
        // as the pop starts (a keyboard going away, see NavigationHost.sync).
        snapshot.autoresizingMask = [.flexibleWidth, .flexibleBottomMargin]
        view.addSubview(snapshot)
        snapshot.setPaintForeground()
    }
}

final class NavigationHost: NSObject, UINavigationControllerDelegate, UIGestureRecognizerDelegate {
    unowned let presenter: Presenter
    var primaryNavigation: UINavigationController?
    private(set) var presentedNavigations: [UINavigationController] = []
    var modalNavigation: UINavigationController? { presentedNavigations.last }
    private var navigation: UINavigationController? { modalNavigation ?? primaryNavigation }
    /// The route on top of the stack in front: the one the person sees.
    var activeRoute: RouteController? { navigation?.topViewController as? RouteController }
    private(set) var syncing = false
    private var mounting = false
    private(set) weak var container: NodeView?
    private var routeIDs: [UInt32] = []
    private(set) var controllers: [UInt32: RouteController] = [:]
    /// Each route owner's logical children (the root's, each tabpanel's),
    /// while UIKit containment holds the route views (LLP 1075.003 §3.7).
    var logicalChildren: [UInt32: [UInt32]] = [:]
    /// Exact's tab container and its stacks by panel id; the tablist whose
    /// place its bar takes; the tab delegate Exact keeps and forwards.
    var tabController: UITabBarController?
    /// What holds the tabs: Exact's tab controller, or a container the app's
    /// `tabContainer` hatch returned (LLP 1075.003 §3.6).
    var tabOwner: UIViewController?
    var tabsHatched = false, tabContainerAsked = false, routerTab = -1
    var tabNavigations: [UInt32: UINavigationController] = [:]
    var tabPanels: [UInt32] = []
    /// How many routes the selected stack declared at the last projection.
    private var selectedRouteCount = 0
    var adoptedTablist: UInt32?
    /// The root whose tablist last decided the bar, and whether Exact hid the
    /// bar for a root's hidden tablist (LLP 1075.003 §3.7, amended).
    weak var tablistRoot: RouteController?
    var tablistHidBar = false
    var tabItems: [String] = []
    /// The bar's tint as last written: the tablist's accent, light and dark.
    var tabTint: [[Double]?]?
    let tabProxy = TabDelegateProxy()
    private(set) var changing = false
    /// A context menu's commit pushes without the stack's animation: UIKit
    /// animates it (`.pop`, LLP 1021 §5.1). The selected route's key tells
    /// whether its press navigated.
    var unanimated = false
    var activeKey: String? { container?.props["navigationKey"] }
    /// While a push or pop runs: paints what each frame newly reveals.
    /// LLP 1075.003: each stack Exact built, by controller; what each shown
    /// bar covers of its route; the ownership changes already journaled;
    /// whether the hatches are being replayed for a cold launch's objects.
    var stacks: [ObjectIdentifier: NavigationStack] = [:]
    var stackCount = 0
    var covers: [UInt32: HostCover] = [:]
    var ownedReported: Set<String> = []
    var replayingHatches = false
    private var coversPending = false
    /// Whether the session's view last took the whole of its own for a bar.
    var tookWholeView = false
    private var pendingSync = false
    /// A sync was deferred (a transition ran, or there was no window yet)
    /// and is still owed, or UIKit moved a stack itself (its back button, a
    /// swipe) since the last one: the next batch syncs, whatever it holds.
    var syncOwed: Bool {
        pendingSync || changing || windowless || nativeMoved || !settled
            || ([primaryNavigation].compactMap { $0 } + presentedNavigations).contains { $0.transitionCoordinator != nil }
    }
    /// No window yet; UIKit moved a stack since the last sync; the last
    /// sync ran to its end (one that stopped early, waiting for the first
    /// draw, a transition or a presentation it could not make yet, is owed).
    private var windowless = false, nativeMoved = false, settled = false
    /// Syncs asked for, for tests.
    private(set) var syncCalls = 0
    private var interactiveSource: (node: NodeView, key: String)?
    /// The stack's depth when the interactive pop began, source included.
    private var interactiveDepth = 0
    /// The root key last journaled as matching no route, so a refusal is one
    /// line, not one per batch (LLP 1035.001 D6).
    private var refusedKey: String?

    init(presenter: Presenter) { self.presenter = presenter }

    /// The node a route is a child of in the tree, where UIKit holds its
    /// view elsewhere: the root or, with tabs, its tabpanel.
    func logicalParent(of route: NodeView) -> NodeView? {
        guard let owner = logicalChildren.first(where: { $0.value.contains(route.id) })?.key else { return nil }
        return owner == container?.id ? container : presenter.views[owner]
    }

    /// Logical child-list edits leave declared, retained routes inside their
    /// controllers; newly added or no-longer-declared nodes use normal mounting.
    /// A route's logical parent is the root or, with tabs, its tabpanel.
    func ownsContainment(of node: NodeView, under parent: NodeView) -> Bool {
        (parent === container || (parent !== container && logicalChildren[parent.id] != nil)) &&
            container?.props["navigationBack"] != nil &&
            node.props["navigationKey"] != nil && controllers[node.id]?.node === node
    }

    /// One batch's projection: the root, each stack's routes — one a tab
    /// (LLP 1075.003 §3.7), or the root's own — which stack holds the route
    /// the root names and where, and each stack's wanted controllers (the
    /// selected stack's prefix through that route; another tab's whole).
    struct Projection {
        let root: NodeView
        let tabs: NavigationTabs?
        let stacks: [[NodeView]]
        let at: Int, selected: Int
        let wanted: [[RouteController]]
        var routes: [NodeView] { stacks[at] }
        var chosen: [RouteController] { wanted[at] }
    }

    /// Put each route node back under its logical parent, before native
    /// ownership retires, so surviving content stays in the window.
    private func restoreLogicalChildren() {
        for (owner, ids) in logicalChildren {
            guard let parent = owner == container?.id ? container : presenter.views[owner] else { continue }
            for (index, node) in ids.compactMap({ presenter.views[$0] }).enumerated() where presenter.views[node.id] === node {
                parent.container.insertSubview(node, at: index)
            }
        }
    }

    func prepare(_ batch: Batch) {
        checkOwned()
        presenter.elements.checkOwned()
        guard let top = navigation?.topViewController as? RouteController else { return }
        if batch.ops.contains(where: { $0.op == .destroy && $0.id == top.node.id }) {
            top.freeze()
        }
    }

    /// Resolve the declared route stacks once for both structural installation
    /// and later presentation. UIKit containment never becomes route state.
    private func projection(_ batch: Batch) -> Projection? {
        guard let root = presenter.root.subviews.first as? NodeView,
              root.props["navigationBack"] != nil else {
            // Retiring native ownership must not remove surviving content.
            // Restore the logical children before detaching the controller so
            // an editor remains in the same window through the handoff.
            if container != nil { restoreLogicalChildren() }
            reset(clearFocus: container != nil)
            return nil
        }
        if container !== root {
            // A fresh owner after unmount must preserve focus requested while
            // offscreen. Replacing an existing root still retires its command.
            reset(clearFocus: container != nil)
            container = root
            logicalChildren = [root.id: root.container.subviews.compactMap { ($0 as? NodeView)?.id }]
        }
        for op in batch.ops where op.op == .children && logicalChildren[op.id] != nil {
            logicalChildren[op.id] = op.ids
        }
        // With tabs, each panel holds a stack; a panel seen for the first
        // time still holds its rows, and its children ops say the rest.
        let tabs = NavigationTabs.of(root, presenter)
        for panel in tabs?.panels ?? [] where logicalChildren[panel.id] == nil {
            logicalChildren[panel.id] = panel.container.subviews.compactMap { ($0 as? NodeView)?.id }
        }
        let owners = tabs?.panels ?? [root]
        let stacks = owners.map { owner in
            (logicalChildren[owner.id] ?? []).compactMap { presenter.views[$0] }.filter { $0.props["navigationKey"] != nil }
        }
        routeIDs = stacks.flatMap { $0.map(\.id) }
        // Every route left the tree (the root shows something else now): the
        // containers go with them, or their views — a removed route's frozen
        // snapshot among them — stay over the new content (mail F21).
        if routeIDs.isEmpty, primaryOwner != nil || !presentedNavigations.isEmpty {
            presenter.session?.log("navigation: the root has no routes now; its containers are retired")
            reset(clearFocus: false)
            return nil
        }
        // D1: the stack is the prefix through the route the root names; a
        // key that names none leaves the stack alone, and says so once.
        let rootKey = root.props["navigationKey"] ?? ""
        let keys = { (routes: [NodeView]) in routes.map { $0.props["navigationKey"] ?? "" } }
        guard let at = stacks.firstIndex(where: { NavigationRules.stack(routeKeys: keys($0), selected: rootKey) != nil }),
              let range = NavigationRules.stack(routeKeys: keys(stacks[at]), selected: rootKey) else {
            if refusedKey != rootKey {
                refusedKey = rootKey
                presenter.session?.log("navigationKey \"\(rootKey)\" matches no route among the root's children or those of the tabpanels its tablist names; the stack is unchanged")
            }
            return nil
        }
        refusedKey = nil
        selectedRouteCount = stacks[at].count
        let wanted = stacks.enumerated().map { index, routes in
            (index == at ? Array(routes[range]) : routes).map { node -> RouteController in
                let c = controllers[node.id] ?? RouteController(node)
                c.host = self
                controllers[node.id] = c
                c.mount()
                return c
            }
        }
        return Projection(root: root, tabs: tabs, stacks: stacks, at: at, selected: range.upperBound - 1, wanted: wanted)
    }

    private func installPrimary(_ p: Projection, first: [RouteController]) {
        guard primaryNavigation == nil else { return }
        // Find the containing controller before installing our child.
        var responder: UIResponder? = p.root
        while responder != nil && !(responder is UIViewController) { responder = responder?.next }
        guard let parent = responder as? UIViewController else { return }
        if let tabs = p.tabs {
            installTabs(p, tabs, first: first, in: parent)
            return
        }
        let nav = makeNavigation(first: first.first?.node)
        parent.addChild(nav)
        p.root.addSubview(nav.view)
        nav.view.setPaintForeground(aboveAuthored: false)
        nav.view.frame = p.root.bounds
        nav.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        nav.didMove(toParent: parent)
        primaryNavigation = nav
        prepareRoutes(first, in: nav)
        nav.setViewControllers(first, animated: false)
        recordOwned(nav)
        watchPops(nav)
    }

    /// A hidden navigation bar needs its availability check here. The
    /// recognizer, competing scroll views and transition stay UIKit's.
    /// UIKit gives both recognizers its own delegate when the controller's
    /// view loads: a tab's stack, built before its tab controller shows it,
    /// is loaded first, or the swipe never asks Exact.
    func watchPops(_ nav: UINavigationController) {
        nav.loadViewIfNeeded()
        // tvOS has no interactive pop gesture.
        #if !os(tvOS)
        nav.interactivePopGestureRecognizer?.delegate = self
        if #available(iOS 26.0, tvOS 26.0, *) { nav.interactiveContentPopGestureRecognizer?.delegate = self }
        #endif
    }

    /// Initial containment is ready before child frames. It does not present
    /// a sheet or flush focus commands from the partially applied batch.
    func installInitialOwner(_ batch: Batch) {
        guard primaryNavigation == nil, !syncing, presenter.session?.view?.window != nil else { return }
        syncing = true
        defer { syncing = false }
        guard let p = projection(batch) else { return }
        let parts = NavigationRules.segments(presentations: p.routes[...p.selected].map { $0.props["navigationPresentation"] })
        installPrimary(p, first: Array(p.chosen[parts[0]]))
        primaryOwner?.view.layoutIfNeeded()
        // The content area the bar leaves reaches layout in this turn, before
        // the first frame (LLP 1075.003 Q3 (c)).
        reportCovers()
        refitForBars()
    }

    /// A container that starts or stops showing its bars changes the view
    /// the session takes (ExactViewIOS `fit`): lay it out again this turn.
    func refitForBars() {
        guard wantsWholeView != tookWholeView else { return }
        tookWholeView = wantsWholeView
        presenter.session?.view?.setNeedsLayout()
    }

    func sync(_ batch: Batch) {
        // Installing or moving a controller can synchronously cause layout.
        // That layout must not start another containment handoff inside this one.
        syncCalls += 1
        guard !syncing else { return }
        let window = presenter.session?.view?.window
        windowless = presenter.session != nil && window == nil
        // With no session (a presenter on its own) nothing is projected or owed.
        if presenter.session == nil { settled = true }
        guard window != nil else { return }
        nativeMoved = false; settled = false
        syncing = true
        defer { syncing = false; presenter.flushPendingFocus() }
        guard let p = projection(batch) else { settled = true; return }
        let root = p.root, wanted = p.chosen
        let parts = NavigationRules.segments(presentations: p.routes[...p.selected].map { $0.props["navigationPresentation"] })
        reshape(p)
        installPrimary(p, first: Array(wanted[parts[0]]))
        // The other tabs' stacks, the selected tab and its items (LLP 1075.003 §3.7).
        if p.tabs != nil { syncTabs(p) }
        // Initial content draws before presentation takes over its viewport.
        if parts.count > 1, let session = presenter.session, session.firstDrawMs == nil { return }
        guard !changing, !presenter.modals.inTransition else { pendingSync = true; return }
        pendingSync = false
        let boundaries = parts.dropFirst().map { wanted[$0.lowerBound].node }
        let mounted = presenter.modals.routes
        let common = zip(boundaries, mounted).prefix {
            $0.0 === $0.1.node && $0.0.props["navigationPresentation"] == $0.1.kind
        }.count
        // Mount the destination under a departing presentation before starting
        // dismissal, so its editor can accept the batch's focus handoff.
        let owners = [primaryNavigation].compactMap { $0 } + presentedNavigations
        // A press that changes a stack with the keyboard up resigns the editor
        // inside this batch, and the viewport grows only once the batch ends
        // (Presenter.applyKeyboard), in a resize batch of its own. UIKit fixes
        // the routes' frames as its transition starts, so the route a pop
        // revealed was laid out only down to the keyboard's top until the
        // transition ended: a list's top half, then the rest. The change
        // waits one turn, for that growth (a swipe drops the keyboard first:
        // dropKeyboard).
        // Only the change UIKit animates (the top stack's, with no
        // presentation opening or closing in the same batch) waits: a
        // closing sheet's cleanup and the focus it hands over stay in order.
        let top = owners.count - 1
        let animatedChange = top >= 0 && top <= common && mounted.count == boundaries.count && !unanimated
            && owners[top].view.window != nil
            && !owners[top].viewControllers.elementsEqual(wanted[parts[top]], by: { $0 === $1 })
        // A presentation opening or closing meanwhile supersedes a wait
        // already begun: its cleanup and focus handoff go now.
        if (awaitingKeyboardViewport && mounted.count == boundaries.count) || NavigationRules.waitsForKeyboardViewport(
            applying: presenter.applying, keyboardShown: presenter.keyboardTop != nil,
            editing: presenter.hasKeyboardEditor, agentFreezes: ExactEnv.agentFreezes, stackChanges: animatedChange) {
            pendingSync = true
            if !awaitingKeyboardViewport {
                awaitingKeyboardViewport = true
                DispatchQueue.main.async { [weak self] in
                    guard let self else { return }
                    // A no-duration hide waits 80 ms to be applied; apply it
                    // now, while the resize batch's own sync still waits.
                    presenter.flushKeyboardResize()
                    awaitingKeyboardViewport = false
                    guard pendingSync, !changing, !presenter.modals.inTransition else { return }
                    pendingSync = false
                    sync(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil))
                    // The batches that ran meanwhile reconciled autofocus
                    // with the destination still out of the window. Retry it
                    // once the transition is over: a keyboard rising as it
                    // starts would shrink the viewport under frozen frames.
                    if changing { autofocusOwed = true } else { presenter.syncAccessibility() }
                }
            }
            return
        }
        for index in 0...common where index < owners.count {
            let nav = owners[index], stack = Array(wanted[parts[index]])
            prepareRoutes(stack, in: nav)
            let same = nav.viewControllers.count == stack.count && zip(nav.viewControllers, stack).allSatisfy { $0 === $1 }
            if !same {
                // @ref LLP 1038 D6 — a tab change swaps immediately. A replacement within one stack
                // whose new top was not on it (a finished screen giving way to its result) arrives
                // as UIKit's own push, rather than cutting.
                let pushOrPop = NavigationRules.isPushOrPop(from: nav.viewControllers.map(ObjectIdentifier.init), to: stack.map(ObjectIdentifier.init))
                let arrives = stack.count > 1 && nav.viewControllers.first === stack.first
                    && !nav.viewControllers.contains { $0 === stack.last }
                nav.setViewControllers(stack, animated: (pushOrPop || arrives) && index == owners.count - 1 && mounted.count == boundaries.count && !ExactEnv.agentFreezes && !unanimated && nav.view.window != nil)
                recordOwned(nav)
            }
            nav.view.layoutIfNeeded()
        }
        unanimated = false
        if mounted.count > common {
            pendingSync = true
            presenter.modals.closeTop()
            return
        }
        if boundaries.count > mounted.count, let background = modalNavigation ?? primaryOwner, let owner {
            let source = navigation?.topViewController as? RouteController
            let part = parts[mounted.count + 1], route = wanted[part.lowerBound].node
            guard presenter.modals.canPresent(from: owner, route: route) else { return }
            let nav = makeNavigation(first: route)
            owner.addChild(nav)
            root.addSubview(nav.view)
            nav.view.setPaintForeground(aboveAuthored: false)
            nav.view.frame = root.bounds
            nav.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            nav.didMove(toParent: owner)
            presentedNavigations.append(nav)
            prepareRoutes(Array(wanted[part]), in: nav)
            nav.setViewControllers(Array(wanted[part]), animated: false)
            recordOwned(nav)
            watchPops(nav)
            nav.view.layoutIfNeeded()
            pendingSync = boundaries.count > mounted.count + 1
            let preceding = part.lowerBound > 0 ? wanted[part.lowerBound - 1].node : nil
            presenter.modals.present(route, navigation: nav, from: background, node: source?.node,
                                     preceding: preceding, owner: owner)
        }
        if let top = modalNavigation ?? primaryOwner {
            top.view.frame = root.bounds
            if top === primaryOwner { placeOwner() } else { root.bringSubviewToFront(top.view) }
            top.view.layoutIfNeeded()
        }
        for (id, c) in controllers where !routeIDs.contains(id) { end(c) }
        controllers = controllers.filter { routeIDs.contains($0.key) }
        presenter.modals.updatePermissions()
        if p.tabs != nil { replayTabs() }
        reportCovers()
        refitForBars()
        settled = true
    }

    /// The container standing in for the routes paints where they are in
    /// the root's children, as CSS paints siblings in tree order (LLP
    /// 1083.000): above the children before them, under those after them —
    /// an authored tablist, a toast. On top of every child it hid a sibling
    /// tablist and a root overlay, and took their taps (shop F21, recipes F23).
    func placeOwner() {
        guard let root = container, let owner = primaryOwner?.view, owner.superview === root else { return }
        let ids = logicalChildren[root.id] ?? []
        let panels = tabPanels.compactMap { presenter.views[$0] }
        let region = ids.firstIndex { id in
            routeIDs.contains(id) || presenter.views[id].map { child in panels.contains { $0 === child || $0.isDescendant(of: child) } } == true
        }
        let after = Set(region.map { ids[($0 + 1)...] } ?? [])
        guard let next = root.subviews.first(where: { ($0 as? NodeView).map { after.contains($0.id) } == true }) else {
            if root.subviews.last !== owner { root.bringSubviewToFront(owner) }
            return
        }
        let at = root.subviews.firstIndex { $0 === next }!
        if at == 0 || root.subviews[at - 1] !== owner { root.insertSubview(owner, belowSubview: next) }
    }

    /// A route's controller leaves for good: its header paints again and the
    /// route hatch hears `routeEnded` (LLP 1075.003 §3.2).
    private func end(_ c: RouteController) {
        if let header = c.lifted { header.isHidden = false; c.lifted = nil }
        guard c.hatched else { return }
        c.hatched = false
        presenter.session?.natives.routeHatch(.ended, controller: c, navigation: nil, scroll: nil, key: c.key, dataset: c.node.props["dataset"])
    }

    /// At a cold launch the module loads a turn after first pixel (LLP
    /// 1075.003 Q3 (c)): each hatch runs once for every object already live,
    /// stacks first, in the order they were built.
    func replayHatches() {
        guard presenter.session?.natives.hatchesConnected == true else { return }
        replayingHatches = true
        defer { replayingHatches = false }
        let navs = allNavigations
        for nav in navs { hatchNavigation(nav) }
        replayTabs()
        for nav in navs {
            prepareRoutes(nav.viewControllers.compactMap { $0 as? RouteController }, in: nav)
            nav.view.layoutIfNeeded()
        }
        reportCovers()
        refitForBars()
    }

    /// A route's safe area moved: report its cover once this turn is done.
    func coversChanged() {
        guard !coversPending else { return }
        coversPending = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            coversPending = false
            reportCovers()
        }
    }

    /// What presents a sheet: the last sheet, else the primary owner's
    /// parent — with tabs the tab controller's, never a tab's own stack.
    var owner: UIViewController? { presenter.modals.owner ?? primaryOwner?.parent }
    /// The controller Exact installed under the root: the tab container, or
    /// the one stack.
    var primaryOwner: UIViewController? { tabOwner ?? primaryNavigation }

    func willMount() { mounting = true }

    func mounted() {
        mounting = false
        sync(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil))
    }

    func unmounted() {
        mounting = false
        let root = container, logical = logicalChildren
        reset(clearFocus: false)
        // Release controller containment while preserving the actual nodes.
        // Offscreen batches now reconcile their ordinary logical child list.
        guard let root, presenter.views[root.id] === root else { return }
        for (owner, ids) in logical {
            guard let parent = owner == root.id ? root : presenter.views[owner] else { continue }
            for (index, node) in ids.compactMap({ presenter.views[$0] }).enumerated() where presenter.views[node.id] === node {
                parent.container.insertSubview(node, at: index)
            }
        }
    }

    func modalDidDismiss() {
        guard pendingSync else { return }
        pendingSync = false
        sync(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil))
        presenter.syncModal()
    }

    /// Retire only the named owner; a late completion cannot remove its
    /// successor or the retained owner beneath it.
    func retireNavigation(_ controller: UIViewController, preserving: Bool) {
        guard let nav = controller as? UINavigationController,
              presentedNavigations.contains(where: { $0 === nav }) else { return }
        presentedNavigations.removeAll { $0 === nav }
        // A presentation gone (UIKit's own dismissal included): the next
        // batch syncs, whatever it holds, so a route still declared returns.
        nativeMoved = true
        retireStack(nav)
        nav.delegate = nil
        if !preserving {
            nav.willMove(toParent: nil)
            nav.view.removeFromSuperview()
            nav.removeFromParent()
        }
    }

    /// The removed editor stays in its outgoing presentation while the
    /// viewport returns independently to the destination owner.
    func preserveModalContent(in parent: UIViewController) -> UIViewController? {
        guard let nav = presentedNavigations.first(where: { $0.parent === parent }) else { return nil }
        let frame = nav.view.convert(nav.view.bounds, to: parent.view)
        parent.view.insertSubview(nav.view, at: 0)
        nav.view.setPaintForeground(false)
        nav.view.frame = frame
        nav.view.accessibilityElementsHidden = true
        return nav
    }

    /// Focus waits through controller installation as well as UIKit's push/pop.
    /// `clock settle` observes the asynchronous part under platform timing.
    var defersFocus: Bool { changing || syncing || mounting || awaitingKeyboardViewport }
    var inTransition: Bool { defersFocus || pendingSync }
    /// How the last transition ended — `completed` (the Back control was
    /// pressed), `cancelled` (an interactive pop returned), `idle` (a
    /// programmatic change, or nothing yet) — for `state.navigation`.
    private var lastTransition = "idle"
    private var interactiveTransition = false
    /// A stack moving: Exact's own push or pop, or the person's swipe; and
    /// (`started`) one begun by Exact or the finger, without the coordinator
    /// a finished transition is still winding down in `didShow`.
    var transitioning: Bool {
        // A rotation's or resize's coordinator moves no controller: not one.
        started || ([primaryNavigation].compactMap { $0 } + presentedNavigations).contains { $0.transitionCoordinator?.viewController(forKey: .from) != nil }
    }
    var started: Bool { changing || interactiveTransition }
    /// A back swipe began with the keyboard up and put it away (see willShow):
    /// its hide is real, and the viewport follows it.
    private var keyboardDropped = false
    /// A stack change waits a turn for the viewport a leaving keyboard
    /// frees (see sync).
    private var awaitingKeyboardViewport = false
    /// The editor a back swipe put away, focused again if the swipe cancels.
    private weak var droppedEditor: UIView?
    /// Autofocus to retry when the transition a keyboard waited for ends.
    private var autofocusOwed = false
    /// A cancelled swipe's editor is focused again as didShow settles.
    private var restoresEditor = false

    /// For `state.navigation` (LLP 1035.002 D2): the route the root names,
    /// UIKit's stack by key, and the transition's phase — observations.
    func observation() -> [String: Any] {
        let stack = (navigation?.viewControllers ?? []).compactMap { ($0 as? RouteController)?.key }
        let transition: [String: Any] = ["interactive": navigation?.transitionCoordinator?.isInteractive ?? false,
                                         "phase": changing || pendingSync || presenter.modals.inTransition ? "in-progress" : lastTransition]
        let root = container ?? presenter.root.subviews.first as? NodeView
        return ["route": root?.props["navigationKey"] ?? NSNull(), "stack": stack, "transition": transition]
    }

    func move(to parent: UIViewController, mount: () -> Void) {
        guard let nav = modalNavigation ?? primaryOwner, nav.parent !== parent else { mount(); return }
        let container = nav.view.superview
        nav.willMove(toParent: nil)
        nav.view.removeFromSuperview()
        nav.removeFromParent()
        parent.addChild(nav)
        mount()
        container?.addSubview(nav.view)
        nav.didMove(toParent: parent)
    }

    func invokeBack(from source: NodeView) {
        guard presenter.session?.view?.window != nil,
              presenter.views[source.id] === source, routeIDs.contains(source.id),
              source.props["navigationKey"] == container?.props["navigationKey"] else { return }
        goBack()
    }

    /// Whether the platform's Back goes from the selected route (LLP 1115 D5).
    var canInvokeBack: Bool { back != nil }

    /// The platform's Back, completed: the authored control pressed, else
    /// the root's `navigate` with the location beneath (LLP 1115 D5).
    private func goBack() {
        switch back {
        case .press(let id)?: presenter.press(id)
        case .navigate(let location)?:
            guard let root = container, let session = presenter.session else { return }
            session.navigate(location, at: root)
        case .pop?:
            guard let key = selectedRoute?.props["navigationKey"], let session = presenter.session else { return }
            session.hostBack(key)
        case nil: break
        }
    }

    var preservesKeyboardViewport: Bool {
        !keyboardDropped && NavigationRules.freezesViewport(modalActive: presenter.modals.active, changing: changing,
                                        initiallyInteractive: navigation?.transitionCoordinator?.initiallyInteractive == true)
    }

    /// The key of the route a view is mounted under, for `layout <node>`
    /// (LLP 1035.002 D1) — an observation of UIKit's containment.
    func routeKey(containing view: UIView) -> String? {
        var responder: UIResponder? = view
        while let current = responder {
            if let route = current as? RouteController { return route.key }
            responder = current.next
        }
        return nil
    }

    func isInactiveRoute(containing view: UIView) -> Bool {
        guard let selected = container?.props["navigationKey"],
              let key = routeKey(containing: view) else { return false }
        return key != selected
    }

    /// D1: resolved at use, by HTML id, among live enabled press controls —
    /// never captured at a gesture's start (`NavigationRules.backControl`).
    private var backControl: NodeView? {
        guard let route = selectedRoute else { return nil }
        return NavigationRules.backControl(named: container?.props["navigationBack"], among: presenter.carrying("id"),
                                           id: \.id, htmlID: { $0.props["id"] }, pressable: { $0.handlers.contains("press") }, disabled: \.disabled,
                                           inActiveRoute: { $0 === route || $0.isDescendant(of: route) })
    }

    private var selectedRoute: NodeView? {
        guard let key = container?.props["navigationKey"] else { return nil }
        return routeIDs.compactMap({ presenter.views[$0] }).first(where: { $0.props["navigationKey"] == key })
    }

    /// How Back goes from the selected route, resolved at use as its
    /// control is (`NavigationRules.back`, LLP 1115 D5).
    private var back: NavigationRules.Back? {
        guard let root = container, let route = selectedRoute else { return nil }
        return NavigationRules.back(control: backControl?.id, declared: declaresBack(route), hearsNavigate: root.handlers.contains("navigate"),
                                    beneath: { [presenter] in route.props["navigationKey"].flatMap { presenter.session?.runtime.locationBeneath($0) } })
    }

    /// Whether a route holds an element named by the root's `navigationBack`.
    private func declaresBack(_ route: NodeView) -> Bool {
        guard let name = container?.props["navigationBack"] else { return false }
        return presenter.carrying("id").contains { $0.props["id"] == name && ($0 === route || $0.isDescendant(of: route)) }
    }

    /// Why Back does not go from the selected route, for the journal.
    var backRefusal: String {
        if let route = selectedRoute, declaresBack(route) { return "its navigationBack control is disabled or has no press handler" }
        return "no visit beneath the active route"
    }

    /// Where each pop recognizer's first touch went down, in its view. A
    /// pan's translation at `shouldBegin` leaves out the travel before it
    /// recognized: a real touch from x = 1 read as starting at 30, past the
    /// 20-point edge, and over a `swiperight` row the pop was refused (LLP
    /// 1080.000 §11). The edge rule judges where the finger landed.
    /// Keyed weakly: a retired stack's recognizers take their entries with them.
    private let popTouchDown = NSMapTable<UIGestureRecognizer, NSValue>.weakToStrongObjects()

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        // The first finger of each gesture replaces the last gesture's; a second finger does not.
        if gestureRecognizer.numberOfTouches == 0 { notePopTouchDown(gestureRecognizer, at: touch.location(in: gestureRecognizer.view)) }
        return true
    }
    func notePopTouchDown(_ gestureRecognizer: UIGestureRecognizer, at point: CGPoint) { popTouchDown.setObject(NSValue(cgPoint: point), forKey: gestureRecognizer) }

    /// The swipe's start: its first touch's point, else (no touch seen) its translation's origin.
    func popStart(_ pan: UIPanGestureRecognizer, in view: UIView) -> CGPoint {
        if let down = popTouchDown.object(forKey: pan)?.cgPointValue { return down }
        let location = pan.location(in: view), delta = pan.translation(in: view)
        return CGPoint(x: location.x - delta.x, y: location.y - delta.y)
    }

    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        guard let pan = gestureRecognizer as? UIPanGestureRecognizer, let view = pan.view else {
            return popMayBegin(gestureRecognizer, from: nil, in: nil, velocity: .zero)
        }
        // A pan's velocity at shouldBegin is often zero (a finger that
        // started slowly, or one sample in): read that way every back swipe
        // is refused as not horizontal. Its travel so far says the direction.
        return popShouldBegin(pan, in: view, velocity: NavigationRules.popDirection(velocity: pan.velocity(in: view), travel: pan.translation(in: view)))
    }
    /// `shouldBegin` for a pan at `velocity`: from where its finger landed.
    func popShouldBegin(_ pan: UIPanGestureRecognizer, in view: UIView, velocity: CGPoint) -> Bool {
        popMayBegin(pan, from: popStart(pan, in: view), in: view, velocity: velocity)
    }

    /// Whether a pop recognizer's swipe, starting at `start` in `view` (a
    /// pan's) with `velocity`, may pop: the showing stack's (a tab's, or a
    /// sheet's over it), by its own depth, with an enabled back control in
    /// its active route (D1), and not a pan a `swiperight` node or a canvas
    /// owns, nor more vertical than horizontal.
    func popMayBegin(_ gestureRecognizer: UIGestureRecognizer, from start: CGPoint?, in view: UIView?, velocity: CGPoint) -> Bool {
        // tvOS has no interactive pop gesture.
        #if os(tvOS)
        let owner: UINavigationController? = nil
        #else
        let owner = allNavigations.first { nav in
            if nav.interactivePopGestureRecognizer === gestureRecognizer { return true }
            if #available(iOS 26.0, *) { return nav.interactiveContentPopGestureRecognizer === gestureRecognizer }
            return false
        }
        #endif
        if let owner, owner !== navigation { return false }
        let depth = navigation?.viewControllers.count ?? 0
        let goes = back != nil
        guard NavigationRules.popMayBegin(depth: depth, changing: changing, modalActive: presenter.modals.inTransition,
                                          hasBackControl: goes,
                                          contextPreviewActive: !presenter.chrome.ids("contextTarget").isEmpty) else {
            if depth > 1, !changing, !presenter.modals.inTransition, !goes {
                presenter.session?.log("back gesture refused: \(backRefusal)")
            }
            return false
        }
        guard let start, let view else { dropKeyboard(); return popStillMayBegin }
        var overSwipeRight = false
        var hit = view.hitTest(start, with: nil)
        if CanvasInputs.owns(hit) { return false }
        while let current = hit {
            if let node = current as? NodeView, node.handlers.contains("swiperight") { overSwipeRight = true; break }
            if current === view { break }
            hit = current.superview
        }
        let begins = NavigationRules.panMayBegin(startX: start.x, overSwipeRight: overSwipeRight, velocity: velocity)
        guard begins else { return false }
        dropKeyboard()
        return popStillMayBegin
    }

    /// One viewport holds both routes, and with the keyboard up it ends at
    /// the keyboard's top: the route a back swipe revealed showed only what
    /// was above the keys (a list's top half; its bottom half once the swipe
    /// ended). The keyboard goes as the swipe is recognized, before UIKit
    /// fixes the routes' frames for the transition, and the viewport grows
    /// to the screen first.
    private func dropKeyboard() {
        guard presenter.hasKeyboardEditor || presenter.keyboardTop != nil,
              navigation?.view.window != nil else { return }
        keyboardDropped = true
        // This session's editor only: another session's in the same window
        // keeps its focus (LLP 1035.001 D5).
        droppedEditor = (FirstResponder.current as? UIView).flatMap { $0.isDescendant(of: presenter.viewport) ? $0 : nil }
        presenter.viewport.endEditing(true)
        presenter.flushKeyboardResize()
        presenter.session?.view?.fit()
        navigation?.view.layoutIfNeeded()
    }

    /// Whether a pop may still begin after dropKeyboard: the blur it caused
    /// was delivered at once and may have changed the route or its Back.
    private var popStillMayBegin: Bool {
        let may = !changing && !presenter.modals.inTransition && !awaitingKeyboardViewport
            && (navigation?.viewControllers.count ?? 0) > 1 && back != nil
        // Refused with no transition begun: nothing will reach didShow, so
        // the editor the swipe put away comes back here.
        if !may, !changing, keyboardDropped {
            keyboardDropped = false
            restoreDroppedEditor()
        }
        return may
    }

    /// The editor a back swipe put away, focused again when the swipe does
    /// not leave its route — only if it is still this session's, on screen,
    /// and nothing has taken or been given the focus since.
    private func restoreDroppedEditor() {
        defer { droppedEditor = nil }
        guard let editor = droppedEditor, editor.window != nil, editor.isDescendant(of: presenter.viewport),
              FirstResponder.current == nil || FirstResponder.current === presenter.session?.view,
              !pendingSync, presenter.pendingFocusNode == nil else { return }
        _ = editor.becomeFirstResponder()
    }

    func navigationController(_ navigationController: UINavigationController, willShow viewController: UIViewController, animated: Bool) {
        guard navigationController === navigation else { return }
        // A presented owner's appearance also calls willShow, using its
        // enclosing presentation coordinator. Only a transition to this route
        // belongs to navigation and has a matching didShow completion.
        // @ref LLP 1038 D6/D11; LLP 1035.001 D2 — cancellation calls
        // willShow again for the source, although the coordinator's .to is
        // still the pop destination. Keep that contact's source until didShow.
        if interactiveTransition, interactiveSource != nil { return }
        let transition = navigationController.transitionCoordinator
        changing = animated && transition?.viewController(forKey: .to) === viewController
        interactiveTransition = changing && transition?.initiallyInteractive == true
        interactiveSource = nil
        if interactiveTransition,
           let source = navigationController.transitionCoordinator?.viewController(forKey: .from) as? RouteController {
            interactiveSource = (source.node, source.key)
            interactiveDepth = navigationController.viewControllers.count + 1
        }
        // The route coming into view was out of the window while it was
        // covered, so no scroll asked for its text; paint what shows of it
        // as the transition starts, not when the reader next scrolls.
        transition?.animate(alongsideTransition: { [weak self] _ in self?.presenter.paintVisibleText() })
        // That paints what shows as it starts. A swipe reveals the route a
        // strip at a time, and painting stopped there until the swipe ended
        // (a back swipe showed the list's top half, then its bottom half).
        // Paint each frame's newly revealed text until the transition is over.
        if changing { startRevealing() }
    }

    private func startRevealing() {
        guard !FrameClock.shared.wants(self) else { return }
        FrameClock.shared.want(self, .navigationReveal, rate: FrameClock.full(on: presenter.viewport.window?.screen)) { [weak self] _ in self?.revealTick() }
    }

    private func revealTick() {
        guard changing else { stopRevealing(); return }
        presenter.paintVisibleText()
    }

    private func stopRevealing() {
        FrameClock.shared.drop(self)
    }

    func navigationController(_ navigationController: UINavigationController, didShow viewController: UIViewController, animated: Bool) {
        guard navigationController === navigation,
              navigationController.topViewController === viewController else { return }
        changing = false
        nativeMoved = true
        keyboardDropped = false
        stopRevealing()
        defer {
            // Tree updates during UIKit's transition retain their latest
            // intent. Apply it once the native stack is available again.
            if pendingSync {
                pendingSync = false
                sync(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil))
            }
            presenter.session?.view?.fit()
            presenter.syncModal()
            // What the settled route shows has pixels (the swipe may have
            // left a band of it unpainted, or a cancelled pop the source).
            presenter.paintVisibleText()
            presenter.flushPendingFocus()
            if autofocusOwed { autofocusOwed = false; presenter.syncAccessibility() }
            // After the owed sync above: the blur's own batch arrived during
            // the swipe and left one.
            if restoresEditor { restoresEditor = false; restoreDroppedEditor() } else { droppedEditor = nil }
            recordPop(navigationController)
            #if os(iOS)
            presenter.syncScrollsToTop() // the route now on top owns the status-bar tap
            #endif
            // Settled on a stack's root: once UIKit has finished the
            // transition (its own bar restoration included), a root whose
            // arrival no projection has handled yet reconciles the bar with
            // its tablist (§3.7).
            DispatchQueue.main.async { [weak self, weak navigationController] in
                if let navigationController { self?.settleTablist(navigationController) }
            }
            // At rest: a large title's insets are sampled now (§9.10).
            coversChanged()
            #if os(iOS)
            presenter.resolveStatusBar(settled: true)
            #endif
        }
        let source = interactiveSource
        interactiveSource = nil
        let sourceKey = source.flatMap { presenter.views[$0.node.id] === $0.node ? $0.key : nil }
        // Replaced in place while the finger was down: the source's node is gone and the app's
        // stack is as deep as it was when the swipe began.
        let sourceReplaced = source != nil && sourceKey == nil
            && selectedRouteCount == interactiveDepth
        // UIKit's own back button (a shown bar, LLP 1075.003 §3.7) pops with
        // no gesture: the route the root names has left the native stack,
        // which Exact never does itself. It is a completed pop from that route.
        let rootKey = container?.props["navigationKey"] ?? ""
        // A modal route is the root of its own presented stack, which no bar
        // pops: one not presented yet (a sheet over a sheet that waits for the
        // one under it, both pushed at once) is not this stack's popped route.
        let rootPresents = routeIDs.lazy.compactMap { self.presenter.views[$0] }
            .first { $0.props["navigationKey"] == rootKey }
            .map { ["modal", "fullscreen"].contains($0.props["navigationPresentation"] ?? "") } ?? false
        let poppedByBar = source == nil && !rootPresents && !navigationController.viewControllers.contains { ($0 as? RouteController)?.key == rootKey }
        let dispatches = (viewController as? RouteController).map {
            NavigationRules.dispatchesBack(shownKey: $0.key, rootKey: rootKey,
                                           sourceKey: poppedByBar ? rootKey : sourceKey, sourceReplaced: sourceReplaced,
                                           modalActive: presenter.modals.inTransition)
        } ?? false
        let cancelled = interactiveTransition && (viewController as? RouteController)?.node === source?.node
        // A cancelled swipe returns to the route it began on: so does the
        // editor it put away, if it is still this session's and on screen.
        restoresEditor = cancelled
        lastTransition = dispatches ? "completed" : (cancelled ? "cancelled" : "idle")
        interactiveTransition = false
        guard dispatches else { return }
        goBack()
    }

    /// A stack Exact retired: its handle goes.
    func retireStack(_ nav: UINavigationController) {
        guard let stack = stacks.removeValue(forKey: ObjectIdentifier(nav)), stack.hatched else { return }
        _ = presenter.session?.natives.navigationHatch(nav, built: false, showsBar: stack.showsBar, label: stack.label)
    }

    func reset(clearFocus: Bool = true) {
        unanimated = false
        presenter.modals.reset()
        for nav in presentedNavigations { retireNavigation(nav, preserving: false) }
        for c in controllers.values { end(c) }
        retireTabs()
        if let primaryNavigation { retireStack(primaryNavigation) }
        if !covers.isEmpty {
            let cleared = covers.keys.filter { presenter.views[$0] != nil }
            covers = [:]
            if !cleared.isEmpty { presenter.onCovers?(cleared.map { ($0, nil) }) }
        }
        primaryNavigation?.delegate = nil
        primaryNavigation?.willMove(toParent: nil)
        primaryNavigation?.view.removeFromSuperview()
        primaryNavigation?.removeFromParent()
        primaryNavigation = nil
        container = nil
        logicalChildren = [:]
        refitForBars()
        controllers.removeAll()
        routeIDs = []
        changing = false
        pendingSync = false
        interactiveSource = nil
        interactiveTransition = false
        keyboardDropped = false
        droppedEditor = nil
        autofocusOwed = false
        lastTransition = "idle"
        if clearFocus { presenter.cancelPendingFocus() }
    }
}
#if os(tvOS)
extension NavigationHost {
    /// Whether the Siri Remote's Menu goes back: a route to pop and a Back control.
    var menuGoesBack: Bool { (navigation?.viewControllers.count ?? 0) > 1 && canInvokeBack }
    func menuBack() {
        guard menuGoesBack, !changing else { return }
        goBack()
    }
}
#endif
#endif

// @ref LLP 1008 §9 — Contract routes projected into UIKit's navigation
// controller. UIKit owns recognition, arbitration, progress and cancellation.
// Only a completed pop invokes the Contract back control.
#if os(iOS)
import UIKit

private final class RouteController: UIViewController {
    let node: NodeView
    var key: String { node.props["navigationKey"] ?? "" }
    init(_ node: NodeView) {
        self.node = node
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { nil }
    override func loadView() {
        view = UIView()
        // The sheet supplies its surface behind transparent authored corners.
        // Dimming belongs outside that surface, to UIKit's presentation.
        view.backgroundColor = node.props["navigationPresentation"] == "modal"
            ? .secondarySystemGroupedBackground : node.color("background_color", .white)
        view.addSubview(node)
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
        snapshot.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(snapshot)
    }
}

final class NavigationHost: NSObject, UINavigationControllerDelegate, UIGestureRecognizerDelegate {
    unowned let presenter: Presenter
    private var primaryNavigation: UINavigationController?
    private var presentedNavigations: [UINavigationController] = []
    private var modalNavigation: UINavigationController? { presentedNavigations.last }
    private var navigation: UINavigationController? { modalNavigation ?? primaryNavigation }
    private var syncing = false
    private var mounting = false
    private weak var container: NodeView?
    private var routeIDs: [UInt32] = []
    private var controllers: [UInt32: RouteController] = [:]
    private var changing = false
    private var pendingSync = false
    private var interactiveSource: (node: NodeView, key: String)?
    /// The root key last journaled as matching no route, so a refusal is one
    /// line, not one per batch (LLP 1035.001 D6).
    private var refusedKey: String?

    init(presenter: Presenter) { self.presenter = presenter }

    /// Logical child-list edits leave declared, retained routes inside their
    /// controllers; newly added or no-longer-declared nodes use normal mounting.
    func ownsContainment(of node: NodeView, under parent: NodeView) -> Bool {
        parent === container && parent.props["navigationBack"] != nil &&
            node.props["navigationKey"] != nil && controllers[node.id]?.node === node
    }

    func prepare(_ batch: Batch) {
        guard let top = navigation?.topViewController as? RouteController else { return }
        if batch.ops.contains(where: { $0.op == .destroy && $0.id == top.node.id }) {
            top.freeze()
        }
    }

    /// Resolve the declared route stack once for both structural installation
    /// and later presentation. UIKit containment never becomes route state.
    private func projection(_ batch: Batch) -> (NodeView, [NodeView], Int, [RouteController])? {
        guard let root = presenter.root.subviews.first as? NodeView,
              root.props["navigationBack"] != nil else {
            // Retiring native ownership must not remove surviving content.
            // Restore the logical children before detaching the controller so
            // an editor remains in the same window through the handoff.
            if let container {
                for (index, node) in routeIDs.compactMap({ presenter.views[$0] }).enumerated() {
                    container.container.insertSubview(node, at: index)
                }
            }
            reset(clearFocus: container != nil)
            return nil
        }
        if container !== root {
            // A fresh owner after unmount must preserve focus requested while
            // offscreen. Replacing an existing root still retires its command.
            reset(clearFocus: container != nil)
            container = root
            routeIDs = root.container.subviews.compactMap { ($0 as? NodeView)?.id }
        }
        for op in batch.ops where op.op == .children && op.id == root.id {
            routeIDs = op.ids
        }
        let routes = routeIDs.compactMap { presenter.views[$0] }.filter { $0.props["navigationKey"] != nil }
        // D1: the stack is the prefix through the route the root names; a
        // key that names none leaves the stack alone, and says so once.
        let rootKey = root.props["navigationKey"] ?? ""
        guard let range = NavigationRules.stack(routeKeys: routes.map { $0.props["navigationKey"] ?? "" }, selected: rootKey) else {
            if refusedKey != rootKey {
                refusedKey = rootKey
                presenter.session?.log("navigationKey \"\(rootKey)\" matches no route; the stack is unchanged")
            }
            return nil
        }
        refusedKey = nil
        let selected = range.upperBound - 1
        let wanted = routes[range].map { node -> RouteController in
            let c = controllers[node.id] ?? RouteController(node)
            controllers[node.id] = c
            c.mount()
            return c
        }
        return (root, routes, selected, wanted)
    }

    private func installPrimary(root: NodeView, wanted: [RouteController]) {
        guard primaryNavigation == nil else { return }
        // Find the containing controller before installing our child.
        var responder: UIResponder? = root
        while responder != nil && !(responder is UIViewController) { responder = responder?.next }
        guard let parent = responder as? UIViewController else { return }
        let nav = UINavigationController()
        nav.setNavigationBarHidden(true, animated: false)
        nav.delegate = self
        parent.addChild(nav)
        root.addSubview(nav.view)
        nav.view.frame = root.bounds
        nav.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        nav.didMove(toParent: parent)
        primaryNavigation = nav
        nav.setViewControllers(wanted, animated: false)
        // A hidden navigation bar needs its availability check here. The
        // recognizer, competing scroll views and transition stay UIKit's.
        nav.interactivePopGestureRecognizer?.delegate = self
        if #available(iOS 26.0, *) { nav.interactiveContentPopGestureRecognizer?.delegate = self }
    }

    /// Initial containment is ready before child frames. It does not present
    /// a sheet or flush focus commands from the partially applied batch.
    func installInitialOwner(_ batch: Batch) {
        guard primaryNavigation == nil, !syncing, presenter.session?.view?.window != nil else { return }
        syncing = true
        defer { syncing = false }
        guard let (root, routes, selected, wanted) = projection(batch) else { return }
        let parts = NavigationRules.segments(presentations: routes[...selected].map { $0.props["navigationPresentation"] })
        installPrimary(root: root, wanted: Array(wanted[parts[0]]))
        primaryNavigation?.view.layoutIfNeeded()
    }

    func sync(_ batch: Batch) {
        // Installing or moving a controller can synchronously cause layout.
        // That layout must not start another containment handoff inside this one.
        guard !syncing, presenter.session?.view?.window != nil else { return }
        syncing = true
        defer { syncing = false; presenter.flushPendingFocus() }
        guard let (root, routes, selected, wanted) = projection(batch) else { return }
        let parts = NavigationRules.segments(presentations: routes[...selected].map { $0.props["navigationPresentation"] })
        installPrimary(root: root, wanted: Array(wanted[parts[0]]))
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
        for index in 0...common where index < owners.count {
            let nav = owners[index], stack = Array(wanted[parts[index]])
            let same = nav.viewControllers.count == stack.count && zip(nav.viewControllers, stack).allSatisfy { $0 === $1 }
            if !same {
                // @ref LLP 1038 D6 — replacements (tabs/open) swap immediately.
                let pushOrPop = NavigationRules.isPushOrPop(from: nav.viewControllers.map(ObjectIdentifier.init), to: stack.map(ObjectIdentifier.init))
                nav.setViewControllers(stack, animated: pushOrPop && index == owners.count - 1 && mounted.count == boundaries.count && !ExactEnv.agentFreezes && nav.view.window != nil)
            }
            nav.view.layoutIfNeeded()
        }
        if mounted.count > common {
            pendingSync = true
            presenter.modals.closeTop()
            return
        }
        if boundaries.count > mounted.count, let background = navigation, let owner {
            let source = background.topViewController as? RouteController
            let part = parts[mounted.count + 1], route = wanted[part.lowerBound].node
            guard presenter.modals.canPresent(from: owner, route: route) else { return }
            let nav = UINavigationController()
            nav.setNavigationBarHidden(true, animated: false)
            nav.delegate = self
            owner.addChild(nav)
            root.addSubview(nav.view)
            nav.view.frame = root.bounds
            nav.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            nav.didMove(toParent: owner)
            presentedNavigations.append(nav)
            nav.setViewControllers(Array(wanted[part]), animated: false)
            nav.interactivePopGestureRecognizer?.delegate = self
            if #available(iOS 26.0, *) { nav.interactiveContentPopGestureRecognizer?.delegate = self }
            nav.view.layoutIfNeeded()
            pendingSync = boundaries.count > mounted.count + 1
            let preceding = part.lowerBound > 0 ? wanted[part.lowerBound - 1].node : nil
            presenter.modals.present(route, navigation: nav, from: background, node: source?.node,
                                     preceding: preceding, owner: owner)
        }
        if let nav = navigation {
            nav.view.frame = root.bounds
            root.bringSubviewToFront(nav.view)
            nav.view.layoutIfNeeded()
        }
        controllers = controllers.filter { routeIDs.contains($0.key) }
        presenter.modals.updatePermissions()
    }

    var owner: UIViewController? { presenter.modals.owner ?? primaryNavigation?.parent }

    func willMount() { mounting = true }

    func mounted() {
        mounting = false
        sync(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil))
    }

    func unmounted() {
        mounting = false
        let root = container
        let children = routeIDs.compactMap { presenter.views[$0] }
        reset(clearFocus: false)
        // Release controller containment while preserving the actual nodes.
        // Offscreen batches now reconcile their ordinary logical child list.
        if let root, presenter.views[root.id] === root {
            for (index, node) in children.enumerated() where presenter.views[node.id] === node {
                root.container.insertSubview(node, at: index)
            }
        }
    }

    func modalDidDismiss() {
        guard pendingSync else { return }
        pendingSync = false
        sync(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil))
    }

    /// Retire only the named owner; a late completion cannot remove its
    /// successor or the retained owner beneath it.
    func retireNavigation(_ controller: UIViewController, preserving: Bool) {
        guard let nav = controller as? UINavigationController,
              presentedNavigations.contains(where: { $0 === nav }) else { return }
        presentedNavigations.removeAll { $0 === nav }
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
        nav.view.frame = frame
        nav.view.accessibilityElementsHidden = true
        return nav
    }

    /// Focus waits through controller installation as well as UIKit's push/pop.
    /// `clock settle` observes the asynchronous part under platform timing.
    var defersFocus: Bool { changing || syncing || mounting }
    var inTransition: Bool { defersFocus || pendingSync }
    /// How the last transition ended — `completed` (the Back control was
    /// pressed), `cancelled` (an interactive pop returned), `idle` (a
    /// programmatic change, or nothing yet) — for `state.navigation`.
    private var lastTransition = "idle"
    private var interactiveTransition = false

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
        guard let nav = navigation, nav.parent !== parent else { mount(); return }
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
        if let control = backControl { presenter.press(control.id) }
    }

    var canInvokeBack: Bool { backControl != nil }

    var preservesKeyboardViewport: Bool {
        NavigationRules.freezesViewport(modalActive: presenter.modals.active, changing: changing,
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
        guard let key = container?.props["navigationKey"],
              let route = routeIDs.compactMap({ presenter.views[$0] }).first(where: { $0.props["navigationKey"] == key }) else { return nil }
        return NavigationRules.backControl(named: container?.props["navigationBack"], among: presenter.carrying("id"),
                                           id: \.id, htmlID: { $0.props["id"] }, pressable: { $0.handlers.contains("press") }, disabled: \.disabled,
                                           inActiveRoute: { $0 === route || $0.isDescendant(of: route) })
    }

    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        let depth = navigation?.viewControllers.count ?? 0
        let control = backControl
        guard NavigationRules.popMayBegin(depth: depth, changing: changing, modalActive: presenter.modals.inTransition,
                                          hasBackControl: control != nil,
                                          contextPreviewActive: !presenter.chrome.ids("contextTarget").isEmpty) else {
            if depth > 1, !changing, !presenter.modals.inTransition, control == nil {
                presenter.session?.log("back gesture refused: no enabled navigationBack control in the active route")
            }
            return false
        }
        if let pan = gestureRecognizer as? UIPanGestureRecognizer, let view = pan.view {
            let location = pan.location(in: view), delta = pan.translation(in: view)
            let start = CGPoint(x: location.x - delta.x, y: location.y - delta.y)
            var overSwipeRight = false
            var hit = view.hitTest(start, with: nil)
            if CanvasInput.owns(hit) { return false }
            while let current = hit {
                if let node = current as? NodeView, node.handlers.contains("swiperight") { overSwipeRight = true; break }
                if current === view { break }
                hit = current.superview
            }
            return NavigationRules.panMayBegin(startX: start.x, overSwipeRight: overSwipeRight, velocity: pan.velocity(in: pan.view))
        }
        return true
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
        }
    }

    func navigationController(_ navigationController: UINavigationController, didShow viewController: UIViewController, animated: Bool) {
        guard navigationController === navigation,
              navigationController.topViewController === viewController else { return }
        changing = false
        defer {
            // Tree updates during UIKit's transition retain their latest
            // intent. Apply it once the native stack is available again.
            if pendingSync {
                pendingSync = false
                sync(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil))
            }
            presenter.session?.view?.fit()
            presenter.flushPendingFocus()
        }
        let source = interactiveSource
        interactiveSource = nil
        let sourceKey = source.flatMap { presenter.views[$0.node.id] === $0.node ? $0.key : nil }
        let dispatches = (viewController as? RouteController).map {
            NavigationRules.dispatchesBack(shownKey: $0.key, rootKey: container?.props["navigationKey"] ?? "",
                                           sourceKey: sourceKey, modalActive: presenter.modals.inTransition)
        } ?? false
        let cancelled = interactiveTransition && (viewController as? RouteController)?.node === source?.node
        lastTransition = dispatches ? "completed" : (cancelled ? "cancelled" : "idle")
        interactiveTransition = false
        guard dispatches, let control = backControl else { return }
        presenter.press(control.id)
    }

    func reset(clearFocus: Bool = true) {
        presenter.modals.reset()
        for nav in presentedNavigations { retireNavigation(nav, preserving: false) }
        primaryNavigation?.delegate = nil
        primaryNavigation?.willMove(toParent: nil)
        primaryNavigation?.view.removeFromSuperview()
        primaryNavigation?.removeFromParent()
        primaryNavigation = nil
        container = nil
        controllers.removeAll()
        routeIDs = []
        changing = false
        pendingSync = false
        interactiveSource = nil
        interactiveTransition = false
        lastTransition = "idle"
        if clearFocus { presenter.cancelPendingFocus() }
    }
}
#endif

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
    private var navigation: UINavigationController?
    private weak var container: NodeView?
    private var routeIDs: [UInt32] = []
    private var controllers: [UInt32: RouteController] = [:]
    private var changing = false
    /// The root key last journaled as matching no route, so a refusal is one
    /// line, not one per batch (LLP 1035.001 D6).
    private var refusedKey: String?

    init(presenter: Presenter) { self.presenter = presenter }

    func prepare(_ batch: Batch) {
        guard let top = navigation?.topViewController as? RouteController else { return }
        if batch.ops.contains(where: { $0["op"] as? String == "destroy" && $0["id"] as? Int == Int(top.node.id) }) {
            top.freeze()
        }
    }

    func sync(_ batch: Batch) {
        guard let root = presenter.root.subviews.first as? NodeView,
              root.props["navigationBack"] != nil else {
            reset()
            return
        }
        if container !== root {
            reset()
            container = root
            routeIDs = root.container.subviews.compactMap { ($0 as? NodeView)?.id }
        }
        for op in batch.ops where op["op"] as? String == "children" && op["id"] as? Int == Int(root.id) {
            routeIDs = (op["ids"] as? [Int] ?? []).map { UInt32($0) }
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
            return
        }
        refusedKey = nil
        let selected = range.upperBound - 1
        let wanted = routes[range].map { node -> RouteController in
            let c = controllers[node.id] ?? RouteController(node)
            controllers[node.id] = c
            c.mount()
            return c
        }
        let modal = routes[selected].props["navigationPresentation"] == "modal"
        if navigation == nil {
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
            navigation = nav
            nav.setViewControllers(wanted, animated: false)
            // A hidden navigation bar needs its availability check here. The
            // recognizer, competing scroll views and transition stay UIKit's.
            nav.interactivePopGestureRecognizer?.delegate = self
            if #available(iOS 26.0, *) { nav.interactiveContentPopGestureRecognizer?.delegate = self }
        }
        guard let nav = navigation else { return }
        nav.view.frame = root.bounds
        root.bringSubviewToFront(nav.view)
        // Background data may settle during an interactive swipe. Remounting
        // nodes is safe; rewriting UIKit's in-flight stack would cancel it.
        guard !changing else { return }
        if modal && !presenter.modals.active, let source = nav.topViewController as? RouteController,
           source.node !== routes[selected] {
            presenter.modals.prepareBackground(source, node: source.node)
        }
        if !modal { presenter.modals.releaseBackground() }
        // The source controller stays live in the presenting surface. Only the
        // modal route belongs to the navigation controller moved into the sheet.
        let stack = modal ? [wanted[selected]] : wanted
        let same = nav.viewControllers.count == stack.count && zip(nav.viewControllers, stack).allSatisfy { $0 === $1 }
        if !same {
            nav.setViewControllers(stack, animated: !modal && !presenter.modals.active && !ExactEnv.agentFreezes && nav.view.window != nil)
        }
        nav.view.layoutIfNeeded()
        controllers = controllers.filter { routeIDs.contains($0.key) }
        presenter.modals.sync(modal ? routes[selected] : nil)
    }

    var owner: UIViewController? { navigation?.parent }

    /// A push or pop UIKit is animating: what `clock settle` waits for under
    /// platform timing (LLP 1035.003 D5), bounded.
    var inTransition: Bool { changing }
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
                                         "phase": changing ? "in-progress" : lastTransition]
        return ["route": container?.props["navigationKey"] ?? NSNull(), "stack": stack, "transition": transition]
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

    func invokeBack() {
        if let control = backControl { presenter.press(control.id) }
    }

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
        NavigationRules.backControl(named: container?.props["navigationBack"], among: Array(presenter.views.values),
                                    id: \.id, htmlID: { $0.props["id"] }, pressable: { $0.handlers.contains("press") }, disabled: \.disabled)
    }

    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        let depth = navigation?.viewControllers.count ?? 0
        let control = backControl
        guard NavigationRules.popMayBegin(depth: depth, changing: changing, modalActive: presenter.modals.active,
                                          hasBackControl: control != nil,
                                          contextPreviewActive: presenter.views.values.contains(where: { $0.props["contextTarget"] != nil })) else {
            if depth > 1, !changing, !presenter.modals.active, control == nil {
                presenter.session?.log("back gesture refused: no enabled navigationBack control in the active route")
            }
            return false
        }
        if let pan = gestureRecognizer as? UIPanGestureRecognizer, let view = pan.view {
            let location = pan.location(in: view), delta = pan.translation(in: view)
            let start = CGPoint(x: location.x - delta.x, y: location.y - delta.y)
            var overSwipeRight = false
            var hit = view.hitTest(start, with: nil)
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
        changing = animated && !presenter.modals.active
        interactiveTransition = navigationController.transitionCoordinator?.initiallyInteractive == true
    }

    func navigationController(_ navigationController: UINavigationController, didShow viewController: UIViewController, animated: Bool) {
        changing = false
        defer { presenter.session?.view?.fit() }
        // D2: once, on a key change only — a cancelled swipe shows the key
        // the root still names; a programmatic Back already moved it.
        let dispatches = (viewController as? RouteController).map {
            NavigationRules.dispatchesBack(shownKey: $0.key, rootKey: container?.props["navigationKey"] ?? "", modalActive: presenter.modals.active)
        } ?? false
        lastTransition = dispatches ? "completed" : (interactiveTransition ? "cancelled" : "idle")
        interactiveTransition = false
        guard dispatches, let control = backControl else { return }
        presenter.press(control.id)
    }

    func reset() {
        navigation?.willMove(toParent: nil)
        navigation?.view.removeFromSuperview()
        navigation?.removeFromParent()
        navigation = nil
        container = nil
        controllers.removeAll()
        routeIDs = []
        changing = false
    }
}
#endif

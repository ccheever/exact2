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
        guard let selected = routes.firstIndex(where: { $0.props["navigationKey"] == root.props["navigationKey"] }) else { return }
        let wanted = routes[...selected].map { node -> RouteController in
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
            nav.setViewControllers(stack, animated: !modal && !presenter.modals.active && !ExactEnv.agentMode && nav.view.window != nil)
        }
        nav.view.layoutIfNeeded()
        controllers = controllers.filter { routeIDs.contains($0.key) }
        presenter.modals.sync(modal ? routes[selected] : nil)
    }

    var owner: UIViewController? { navigation?.parent }

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
        !presenter.modals.active && changing && navigation?.transitionCoordinator?.initiallyInteractive == true
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

    private var backControl: NodeView? {
        guard let target = container?.props["navigationBack"] else { return nil }
        return presenter.views.values.first {
            $0.props["id"] == target && $0.handlers.contains("press") && !$0.disabled
        }
    }

    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        guard (navigation?.viewControllers.count ?? 0) > 1, !changing, !presenter.modals.active, backControl != nil,
              !presenter.views.values.contains(where: { $0.props["contextTarget"] != nil }) else { return false }
        if let pan = gestureRecognizer as? UIPanGestureRecognizer, let view = pan.view {
            let location = pan.location(in: view), delta = pan.translation(in: view)
            let start = CGPoint(x: location.x - delta.x, y: location.y - delta.y)
            if start.x >= 20 {
                var hit = view.hitTest(start, with: nil)
                while let current = hit {
                    if let node = current as? NodeView, node.handlers.contains("swiperight") { return false }
                    if current === view { break }
                    hit = current.superview
                }
            }

            let velocity = pan.velocity(in: pan.view)
            return velocity.x > abs(velocity.y)
        }
        return true
    }

    func navigationController(_ navigationController: UINavigationController, willShow viewController: UIViewController, animated: Bool) {
        changing = animated && !presenter.modals.active
    }

    func navigationController(_ navigationController: UINavigationController, didShow viewController: UIViewController, animated: Bool) {
        changing = false
        defer { presenter.session?.view?.fit() }
        guard !presenter.modals.active, let shown = viewController as? RouteController, let root = container,
              shown.key != root.props["navigationKey"],
              let control = backControl else { return }
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

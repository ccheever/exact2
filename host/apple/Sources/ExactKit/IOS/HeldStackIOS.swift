// @ref LLP 1075.003 §3.7 — a tab whose stack shows no navigation bar and has
// pushed nothing holds its one route in a plain container, as a hand-built
// UIKit app shows such a tab: on iOS 26 and 27 a `UINavigationController`
// whose view is in a window lays out a full-screen SwiftUI host for its
// bottom toolbar and search (`_UIFloatingBarContainerView`), toolbar or not,
// about 0.9 MB in euxy. A tab the runner has not built yet has one from the
// start. The navigation controller is built (`realize`) when
// the stack first pushes, when its route first shows a bar, or when a hatch
// asks for the tab's controller (`ExactTab.controller`), and the route moves
// into it within the window, before a push animates. It is never taken
// down again.
#if os(iOS) || os(tvOS)
import UIKit

/// A tab's stack while it holds one route with no bar, or none yet.
final class HeldStack: UIViewController {
    private(set) var route: RouteController?
    /// The navigation controller built in its place, for a hatch that kept
    /// this container's handle (`ExactTab`).
    weak var built: UINavigationController?

    override func loadView() { view = UIView() }

    /// Show `next` in place of the route held now (a root replaced, as a
    /// navigation controller's `setViewControllers` replaces it, unanimated).
    func hold(_ next: RouteController?) {
        guard next !== route else { return }
        if let old = release() { old.view.removeFromSuperview() }
        guard let next else { return }
        route = next
        addChild(next)
        next.view.frame = view.bounds
        next.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(next.view)
        next.didMove(toParent: self)
        for edge in [NSDirectionalRectEdge.top, .bottom] { setContentScrollView(next.contentScrollView(for: edge), for: edge) }
    }

    /// The route leaves this container, its view where it is.
    @discardableResult
    func release() -> RouteController? {
        guard let held = route else { return nil }
        held.willMove(toParent: nil)
        held.removeFromParent()
        route = nil
        for edge in [NSDirectionalRectEdge.top, .bottom] { setContentScrollView(nil, for: edge) }
        return held
    }

    // The tab bar follows its tab's content scroll view, which a navigation
    // controller takes from its top: here, the route's.
    override func contentScrollView(for edge: NSDirectionalRectEdge) -> UIScrollView? {
        route?.contentScrollView(for: edge) ?? super.contentScrollView(for: edge)
    }
}

extension UIViewController {
    /// The routes of a stack Exact built, its root first: a navigation
    /// controller's, or the one a held stack shows.
    var stackRoutes: [UIViewController] {
        if let nav = self as? UINavigationController { return nav.viewControllers }
        return ((self as? HeldStack)?.route).map { [$0] } ?? []
    }
}

extension NavigationHost {
    /// Whether a tab's stack, its routes up to its first presentation, needs
    /// a navigation controller: it pushes, or its root shows a bar. A tab
    /// the runner has not built yet (no routes) gets one, as before: it is
    /// out of the window until it is selected, and costs nothing there.
    func needsNavigation(_ routes: [RouteController]) -> Bool {
        routes.count != 1 || bar(of: routes[0].node, pushed: false) != .hidden
    }

    /// A tab's stack: its navigation controller, or its held stack.
    func tabStack(_ panel: UInt32) -> UIViewController? { tabNavigations[panel] ?? heldTabs[panel] }

    /// The stacks under the root, in tab order (or the one).
    var primaryStacks: [UIViewController] {
        tabPanels.isEmpty ? [primaryNavigation].compactMap { $0 } : tabPanels.compactMap(tabStack)
    }

    /// The selected tab's stack, while it is held.
    var primaryHeld: HeldStack? { tabPanels.indices.contains(routerTab) ? heldTabs[tabPanels[routerTab]] : nil }

    /// The routes of each stack whose routes may show the bar: a navigation
    /// controller's unless its hatch said otherwise, and a held stack's,
    /// which no hatch has met; the primary stacks first, then the sheets'.
    var barStackRoutes: [[UIViewController]] {
        (primaryStacks + presentedNavigations).filter { stack in
            (stack as? UINavigationController).map { stacks[ObjectIdentifier($0)]?.showsBar == true } ?? true
        }.map(\.stackRoutes)
    }

    /// A tab's first stack: held while it needs no navigation controller.
    func makeTabStack(_ panel: UInt32, _ routes: [RouteController]) -> UIViewController {
        guard needsNavigation(routes) else {
            let held = HeldStack()
            heldTabs[panel] = held
            show(routes, in: held)
            return held
        }
        let nav = makeNavigation(first: routes.first?.node)
        prepareRoutes(routes, in: nav)
        nav.setViewControllers(routes, animated: false)
        recordOwned(nav)
        watchPops(nav)
        tabNavigations[panel] = nav
        return nav
    }

    /// A held stack's route (one, or none yet), projected and then shown.
    func show(_ routes: [RouteController], in held: HeldStack) {
        prepareRoutes(routes, in: held)
        held.hold(routes.first)
    }

    /// The navigation controller a hatch asks for (`ExactTab.controller`):
    /// a held tab's is built now.
    func navigation(forHatch controller: UIViewController) -> UINavigationController? {
        if let nav = controller as? UINavigationController ?? (controller as? HeldStack)?.built { return nav }
        guard let panel = heldTabs.first(where: { $0.value === controller })?.key else { return nil }
        return realize(panel, why: "a hatch asked for it")
    }

    /// Build a held tab's navigation controller, holding the route the tab
    /// showed, in the tab's place in the tab controller. The route's node
    /// stays in the window throughout, where it was, while its controller's
    /// view changes containers: UIKit lays a navigation controller's top out
    /// in a wrapper it makes outside the window, and a field leaving the
    /// window loses the focus. A push that follows starts from it, unmoved.
    @discardableResult
    func realize(_ panel: UInt32, why: String) -> UINavigationController? {
        if let nav = tabNavigations[panel] { return nav }
        guard let held = heldTabs.removeValue(forKey: panel) else { return nil }
        let route = held.release()
        var parked: (node: NodeView, frame: CGRect)?
        if let route, route.node.window != nil, let park = tabOwner?.view {
            parked = (route.node, route.node.frame)
            let frame = route.node.convert(route.node.bounds, to: park)
            park.addSubview(route.node)
            route.node.frame = frame
        }
        let nav = makeNavigation(first: route?.node)
        held.built = nav
        watchPops(nav)
        // The item, with whatever a hatch set on it, is the tab's still.
        nav.tabBarItem = held.tabBarItem
        tabNavigations[panel] = nav
        let index = tabPanels.firstIndex(of: panel) ?? -1
        if let tabs = tabController, var all = tabs.viewControllers, let at = all.firstIndex(where: { $0 === held }) {
            let selected = tabs.selectedIndex
            all[at] = nav
            tabs.setViewControllers(all, animated: false)
            if tabs.selectedIndex != selected { tabs.selectedIndex = selected }
            tabs.view.layoutIfNeeded()
        }
        let routes = route.map { [$0] } ?? []
        prepareRoutes(routes, in: nav)
        nav.setViewControllers(routes, animated: false)
        recordOwned(nav)
        nav.view.layoutIfNeeded()
        if let (node, frame) = parked, let route {
            route.mount()
            node.frame = frame
        }
        // Navigation follows it only now: UIKit showing its root as it
        // appears is not a pop from the route the root names.
        if index == routerTab { primaryNavigation = nav }
        if let (tabs, _) = tabItemsSource { tabItemsSource = (tabs, tabPanels.compactMap(tabStack)) }
        coversChanged()
        presenter.session?.log("navigation: tab \(index) builds its navigation controller (\(why))")
        return nav
    }
}
#endif

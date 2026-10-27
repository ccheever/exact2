// @ref LLP 1075.003 §3.7 — a tab whose stack shows no navigation bar and has
// pushed nothing holds its one route in a plain container, as a hand-built
// UIKit app shows such a tab: on iOS 26 and 27 a `UINavigationController`
// whose view is in a window lays out a full-screen SwiftUI host for its
// bottom toolbar and search (`_UIFloatingBarContainerView`), toolbar or not,
// about 0.9 MB in euxy. A tab the runner has not built yet has one from the
// start. The navigation controller is built (`realize`) when the stack first
// pushes or its route first shows a bar, once no transition is running, or
// when a hatch asks for the tab's controller (`ExactTab.controller`); the
// route moves into it within the window, before a push animates. It is never
// taken down again.
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
        tabPanels.isEmpty ? [primaryNavigation, split?.detail, split?.inspector].compactMap { $0 } : tabPanels.compactMap(tabStack)
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
    /// a held tab's is made now. While a transition runs it is only made and
    /// handed back; it takes the route and the tab's place once the
    /// transition is over (`syncTabs`), as a held tab that pushes does.
    func navigation(forHatch controller: UIViewController) -> UINavigationController? {
        if let nav = controller as? UINavigationController ?? (controller as? HeldStack)?.built { return nav }
        guard let (panel, held) = heldTabs.first(where: { $0.value === controller }) else { return nil }
        guard realizeWaits else { return realize(panel, why: "a hatch asked for it") }
        syncAfterTransition()
        return made(for: held)
    }

    /// A held tab's navigation controller, made once: recorded on the held
    /// stack before the `navigation` hatch runs, which may ask for it again.
    private func made(for held: HeldStack) -> UINavigationController {
        if let nav = held.built { return nav }
        let nav = makeNavigation(first: held.route?.node) { held.built = $0 }
        watchPops(nav)
        // The item, with whatever a hatch set on it, is the tab's still.
        nav.tabBarItem = held.tabBarItem
        return nav
    }

    /// A transition whose controllers a tab's replacement would cut into: a
    /// sheet presenting or dismissing (interactively too), a push or pop on
    /// any stack, the tab controller's own.
    var realizeWaits: Bool {
        presenter.modals.inTransition || tabController?.transitionCoordinator != nil
            || (Array(tabNavigations.values) + presentedNavigations).contains { $0.transitionCoordinator != nil }
    }

    /// A held tab that waited (`realizeWaits`) is built by a sync once the
    /// transition is over, whatever batch comes or does not: at the running
    /// coordinator's completion, or after 0.25 seconds, whichever is first.
    /// A declined completion retries at once only when no transition remains;
    /// otherwise the timer retries. A sync that finds a transition still
    /// running (a cancelled interactive dismissal, the next one) arms it
    /// again (`syncTabs`).
    func syncAfterTransition() {
        guard !syncAfterTransitionArmed else { return }
        syncAfterTransitionArmed = true
        var done = false
        let resync = { [weak self] in
            guard !done else { return }
            done = true
            DispatchQueue.main.async {
                guard let self, self.syncAfterTransitionArmed else { return }
                self.syncAfterTransitionArmed = false
                self.sync(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil))
            }
        }
        let coordinator = tabController?.transitionCoordinator
            ?? (Array(tabNavigations.values) + presentedNavigations).lazy.compactMap(\.transitionCoordinator).first
        if coordinator?.animate(alongsideTransition: nil, completion: { _ in resync() }) == false, !realizeWaits { resync() }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { resync() }
    }

    /// Build a held tab's navigation controller, holding the route the tab
    /// showed, in the tab's place in the tab controller. The controller has
    /// its root before it is laid out in the window (`installPrimary`).
    /// While UIKit moves the route's view into it, through a wrapper it
    /// makes outside the window, the route's node waits in the window where
    /// it is: a field in it keeps the focus, and VoiceOver's and the remote's
    /// focus are given back to what had them. A large title the bar now shows
    /// opens as it would have had the content been there (`restAtTop`).
    @discardableResult
    func realize(_ panel: UInt32, why: String) -> UINavigationController? {
        if let nav = tabNavigations[panel] { return nav }
        guard let held = heldTabs[panel] else { return nil }
        let nav = made(for: held)
        heldTabs[panel] = nil
        tabNavigations[panel] = nav
        let route = held.release()
        let index = tabPanels.firstIndex(of: panel) ?? -1
        // A scroller at its start stays there under a large title (`restAtTop`).
        let scroller = route.flatMap { contentScroller(of: $0) }.flatMap(shownScroll)
        let atStart = scroller.map { $0.contentOffset.y <= 0.5 - $0.adjustedContentInset.top } ?? false
        let kept = route.flatMap { KeptFocus($0.node) }
        let routes = route.map { [$0] } ?? []
        prepareRoutes(routes, in: nav)
        nav.setViewControllers(routes, animated: false)
        recordOwned(nav)
        if let tabs = tabController, var all = tabs.viewControllers, let at = all.firstIndex(where: { $0 === held }) {
            let selected = tabs.selectedIndex
            all[at] = nav
            tabs.setViewControllers(all, animated: false)
            if tabs.selectedIndex != selected { tabs.selectedIndex = selected }
            tabs.view.layoutIfNeeded()
        }
        nav.view.layoutIfNeeded()
        if let route, let kept {
            route.mount()
            kept.restore(in: route)
            nav.view.layoutIfNeeded()
            // The remote's item, asked for in `restore`, through that layout.
            if route.focusReturn != nil { route.updateFocusIfNeeded(); route.focusReturn = nil }
            #if !os(tvOS)
            if atStart, let sv = contentScroller(of: route).flatMap(shownScroll) { restAtTop(sv, in: route) }
            #endif
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

/// A route's node waiting in its window while its controller's view changes
/// containers, and the focus it held: the first responder, VoiceOver's
/// element and the remote's item, given back where the move took them.
struct KeptFocus {
    /// VoiceOver's focused element and the announcement that moves it, for tests.
    nonisolated(unsafe) static var voiceOverFocus: () -> Any? = { UIAccessibility.focusedElement(using: .notificationVoiceOver) }
    nonisolated(unsafe) static var announce: (Any?) -> Void = { UIAccessibility.post(notification: .layoutChanged, argument: $0) }
    let node: NodeView, frame: CGRect
    weak var responder: UIResponder?
    weak var voiceOver: NSObject?
    weak var remote: UIView?

    /// Nil for a node out of any window, which has no focus to keep.
    init?(_ node: NodeView) {
        guard let window = node.window else { return nil }
        self.node = node
        frame = node.frame
        let inside = { (view: UIView?) in view.map { $0.isDescendant(of: node) } == true }
        responder = (FirstResponder.current as? UIView).flatMap { inside($0) ? $0 : nil }
        let spoken = Self.voiceOverFocus() as? NSObject
        voiceOver = inside(spoken as? UIView ?? (spoken as? UIAccessibilityElement)?.accessibilityContainer as? UIView) ? spoken : nil
        let focused = UIFocusSystem.focusSystem(for: window)?.focusedItem as? UIView
        remote = inside(focused) ? focused : nil
        let at = node.convert(node.bounds, to: window)
        window.addSubview(node)
        node.frame = at
    }

    /// The node is in its route again: the focus it held is its own again,
    /// where what held it is still in the route and shown (`exposed`); else
    /// UIKit chooses (VoiceOver told the screen changed, with no element).
    /// The remote's item is asked for here and taken in the layout after.
    func restore(in route: RouteController) {
        node.frame = frame
        if let view = responder as? UIView, !view.isFirstResponder, Self.exposed(view, in: route), view.canBecomeFirstResponder { _ = view.becomeFirstResponder() }
        if let voiceOver {
            let view = voiceOver as? UIView ?? (voiceOver as? UIAccessibilityElement)?.accessibilityContainer as? UIView
            Self.announce(view.map { Self.exposed($0, in: route) } == true ? voiceOver : nil)
        }
        if let remote, let window = remote.window, UIFocusSystem.focusSystem(for: window)?.focusedItem !== remote,
           Self.exposed(remote, in: route), remote.canBecomeFocused {
            route.focusReturn = remote
            route.setNeedsFocusUpdate()
        }
    }

    /// Whether `view` is in the route's view and shown: neither it nor an
    /// ancestor up to the route's view is hidden, transparent or hidden
    /// from accessibility.
    static func exposed(_ view: UIView, in route: RouteController) -> Bool {
        guard route.isViewLoaded, view.isDescendant(of: route.view) else { return false }
        var at: UIView? = view
        while let v = at, v !== route.view {
            if v.isHidden || v.alpha < 0.01 || v.accessibilityElementsHidden { return false }
            at = v.superview
        }
        return true
    }
}
#endif

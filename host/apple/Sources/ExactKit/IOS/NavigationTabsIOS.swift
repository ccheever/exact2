// @ref LLP 1075.003 §3.7 — Exact's tab container on iOS: a
// `UITabBarController` with one navigation controller a tab, each tab's
// stack kept as the router keeps it (LLP 1038 §7's "one navigation owner per
// tab"), so a scroll offset, a draft or a pushed screen survives a switch.
// The authored `tablist` supplies the items — a symbol over a label, its
// `-fill` symbol when selected — and takes no room while the bar shows; a
// tab the bar selects presses its authored tab, so selection, history and
// reselect-pops-to-root stay the router's (LLP 1038 D4, D12). Under the
// agent the authored tablist paints and the bar stays hidden. A tab not yet
// selected has no routes: the runner builds a tab when it is first selected,
// as UIKit loads a tab's view (LLP 1075.003 §3.7). Which tabs a root has is
// NavigationTabs.swift's; what a tab shows, TabFace.swift's.
#if os(iOS) || os(tvOS)
import UIKit

/// The tab delegate Exact keeps: a tab the bar would select presses its
/// authored tab instead (the router decides), then the app's delegate hears
/// the rest; what Exact does not implement goes straight to the app's.
final class TabDelegateProxy: NSObject, UITabBarControllerDelegate {
    weak var host: NavigationHost?
    weak var app: UITabBarControllerDelegate?

    func tabBarController(_ tabs: UITabBarController, shouldSelect controller: UIViewController) -> Bool {
        if app?.tabBarController?(tabs, shouldSelect: controller) == false { return false }
        host?.selectTab(controller)
        return false
    }

    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || (app?.responds(to: selector) ?? false)
    }

    override func forwardingTarget(for selector: Selector!) -> Any? {
        app?.responds(to: selector) == true ? app : nil
    }
}

extension NavigationHost {
    /// Every stack Exact built: the tabs' in tab order (or the one), then
    /// the sheets'.
    var allNavigations: [UINavigationController] {
        let tabs = tabPanels.compactMap { tabNavigations[$0] }
        return (tabs.isEmpty ? [primaryNavigation].compactMap { $0 } : tabs) + presentedNavigations
    }

    /// Whether Exact's tab bar shows: not under the agent's own chrome,
    /// which sees the authored tablist (LLP 1021 D4's one presentation).
    var tabBarShows: Bool { tabController != nil && tabOwner === tabController && !ExactEnv.authoredChrome && !tablistAppearanceNone }

    /// What a tab other than the selected one holds: its stack up to its
    /// first presentation. A sheet is presented only over the selected tab;
    /// another tab's stays the router's until that tab is selected again.
    func base(_ stack: [RouteController]) -> [RouteController] {
        Array(stack[NavigationRules.segments(presentations: stack.map { $0.node.props["navigationPresentation"] })[0]])
    }

    /// Build the stacks, a stack a panel, and what holds them, before any
    /// child is laid out: the app's container when its module, already
    /// connected, returns one; else Exact's tab controller.
    func installTabs(_ p: Projection, _ tabs: NavigationTabs, first: [RouteController], in parent: UIViewController) {
        var navs: [UINavigationController] = []
        for (index, panel) in tabs.panels.enumerated() {
            let stack = index == p.at ? first : base(p.wanted[index])
            let nav = makeNavigation(first: stack.first?.node)
            prepareRoutes(stack, in: nav)
            nav.setViewControllers(stack, animated: false)
            recordOwned(nav)
            watchPops(nav)
            tabNavigations[panel.id] = nav
            navs.append(nav)
        }
        tabPanels = tabs.panels.map(\.id)
        primaryNavigation = navs[p.at]
        routerTab = p.at
        syncItems(tabs, navs: navs)
        let owned = askTabContainer(tabs, navs: navs, selected: p.at)
        tabContainerAsked = presenter.session?.natives.hatchesConnected == true
        let holder: UIViewController = owned ?? {
            let container = UITabBarController()
            tabProxy.host = self
            container.delegate = tabProxy
            container.setViewControllers(navs, animated: false)
            container.selectedIndex = p.at
            tablistAppearanceNone = tabs.tablist.wrote("appearance") && tabs.tablist.style["appearance"]?.string == "none"
            if ExactEnv.authoredChrome || tablistAppearanceNone { hideTabBar(container) }
            tabController = container
            return container
        }()
        mount(holder, in: parent, at: p.root)
        tabOwner = holder
        tint(tabs.tablist)
        // Kept under the agent too, where the authored tablist shows in the
        // bar's place: a real touch refuses it (LLP 1080.000 D7).
        adoptedTablist = tabs.tablist.id
        if owned == nil, presenter.session?.natives.hatchesConnected == true, let container = tabController {
            presenter.session?.natives.tabsHatch(container, event: 0)
            tabsHatched = true
        }
        presenter.session?.log("navigation: \(navs.count) tabs in \(owned.map { "the app's \(type(of: $0))" } ?? "a UITabBarController")\(ExactEnv.authoredChrome ? " (the authored tablist shows)" : "")")
    }

    private func mount(_ holder: UIViewController, in parent: UIViewController, at root: NodeView) {
        parent.addChild(holder)
        // Sized before it enters the window, where UIKit lays it out.
        holder.view.frame = root.bounds
        holder.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        root.addSubview(holder.view)
        holder.view.setPaintForeground(aboveAuthored: false)
        holder.didMove(toParent: parent)
    }

    /// The module's `tabContainer`, when it is connected: the app's own
    /// container for these stacks, or nil.
    private func askTabContainer(_ tabs: NavigationTabs, navs: [UINavigationController], selected: Int) -> UIViewController? {
        guard let natives = presenter.session?.natives, natives.hatchesConnected else { return nil }
        let names = tabs.panels.map { $0.props["id"] ?? "" }
        return natives.tabContainerHatch(names: names, nodes: tabs.tabs.map(\.id), selected: selected, controllers: navs) as? UIViewController
    }

    /// At a cold launch the module connects after first pixel: Exact's tab
    /// controller hears `tabs`, then `tabContainer` may take its stacks into
    /// the app's own container; the content moves once (LLP 1075.003 Q3 (c)).
    /// A sheet presented over the tabs keeps the controller it was presented
    /// from: the app's container takes their place once nothing is presented
    /// (each batch asks again until then).
    func replayTabs() {
        guard let container = tabController, let natives = presenter.session?.natives, natives.hatchesConnected else { return }
        if !tabsHatched {
            tabsHatched = true
            natives.tabsHatch(container, event: 0)
        }
        guard !tabContainerAsked, presenter.modals.routes.isEmpty, !presenter.modals.inTransition, presentedNavigations.isEmpty,
              let root = self.container, let tabs = NavigationTabs.of(root, presenter), let parent = container.parent else { return }
        tabContainerAsked = true
        // Offer the stacks in place: a container the hook returns takes them from Exact's, as UIKit moves any
        // child. Emptying Exact's first would lose its bar's selection: a tab bar controller emptied and filled
        // again keeps its selected index but shows no tab selected (iOS 26 and 27).
        let navs = tabPanels.compactMap { tabNavigations[$0] }
        guard let owned = askTabContainer(tabs, navs: navs, selected: routerTab) else {
            if container.viewControllers?.count != navs.count {
                presenter.session?.log("hatch: tabContainer took the tabs' stacks but returned no container; Exact's has them back")
                container.setViewControllers(navs, animated: false)
                container.selectedIndex = max(0, routerTab)
            }
            return
        }
        natives.tabsHatch(container, event: 1)
        container.delegate = nil
        tablistRoot = nil
        tablistHidBar = false
        container.willMove(toParent: nil)
        container.view.removeFromSuperview()
        container.removeFromParent()
        tabController = nil
        mount(owned, in: parent, at: root)
        tabOwner = owned
        presenter.session?.log("hatch: the app's \(type(of: owned)) holds the tabs; the content moved after the first frame")
    }

    private func hideTabBar(_ container: UITabBarController) {
        if #available(iOS 18.0, tvOS 18.0, *) { container.setTabBarHidden(true, animated: false) } else { container.tabBar.isHidden = true }
    }

    /// Before installing: a root that gains, loses or changes its tabs gets
    /// its containers again (the router's stacks are untouched).
    func reshape(_ p: Projection) {
        let panels = p.tabs?.panels.map(\.id) ?? []
        guard primaryNavigation != nil, panels != tabPanels else { return }
        presenter.session?.log("navigation: the tabs changed; the containers are built again")
        if tabOwner != nil { retireTabs() } else if let nav = primaryNavigation {
            retireStack(nav)
            nav.delegate = nil
            // Its routes go into the new containers: they leave this one first.
            nav.setViewControllers([], animated: false)
            nav.willMove(toParent: nil)
            nav.view.removeFromSuperview()
            nav.removeFromParent()
        }
        primaryNavigation = nil
    }

    /// Each batch: every other tab's stack, the selected tab, its items.
    func syncTabs(_ p: Projection) {
        guard tabOwner != nil, let tabs = p.tabs else { return }
        let authored = tabs.tablist.wrote("appearance") && tabs.tablist.style["appearance"]?.string == "none"
        if authored != tablistAppearanceNone {
            tablistAppearanceNone = authored
            presenter.session?.view?.setNeedsLayout()
            tablistRoot = nil
            tablistHidBar = false
            if let container = tabController, tabOwner === container, !ExactEnv.authoredChrome {
                let hidden = authored || tabs.tablist.style["display"]?.string == "none"
                if #available(iOS 18.0, tvOS 18.0, *) { container.setTabBarHidden(hidden, animated: false) }
                else { container.tabBar.isHidden = hidden }
                tablistHidBar = hidden && !authored
            }
        }
        let navs = tabs.panels.compactMap { tabNavigations[$0.id] }
        guard navs.count == tabs.panels.count else { return }
        // A tab's stack is empty until the runner builds the tab, when it is
        // first selected (LLP 1075.003 §3.7): its first routes arrive here,
        // unanimated, and its bar shows as its first route says, as a stack
        // built with them would (`makeNavigation`).
        let fill = { (nav: UINavigationController, stack: [RouteController]) in
            guard nav.viewControllers.isEmpty, let first = stack.first else { return }
            nav.setNavigationBarHidden(!(self.barShows(nav) && self.bar(of: first.node, pushed: false) != .hidden), animated: false)
            self.prepareRoutes(stack, in: nav)
            nav.setViewControllers(stack, animated: false)
            self.recordOwned(nav)
        }
        for (index, nav) in navs.enumerated() where index != p.at {
            let stack = base(p.wanted[index])
            fill(nav, stack)
            prepareRoutes(stack, in: nav)
            if nav.viewControllers.count != stack.count || !zip(nav.viewControllers, stack).allSatisfy({ $0 === $1 }) {
                nav.setViewControllers(stack, animated: false)
                recordOwned(nav)
            }
        }
        // A tab selected for the first time brings its routes: its bar is
        // projected before it is shown, where nothing animates in.
        fill(navs[p.at], base(p.wanted[p.at]))
        if routerTab != p.at { prepareRoutes(base(p.wanted[p.at]), in: navs[p.at]) }
        // @ref LLP 1038 D6 — a tab change swaps immediately.
        if let container = tabController, container.selectedIndex != p.at { container.selectedIndex = p.at }
        if routerTab != p.at {
            routerTab = p.at
            if tabController == nil { presenter.session?.natives.tabsHatch(nil, event: 2, index: p.at) }
        }
        primaryNavigation = navs[p.at]
        syncItems(tabs, navs: navs)
    }

    /// The items, from the authored tabs: each tab's own item, written in
    /// place when its face changes, so what else the app set on it (a badge)
    /// and a handle to it stay good.
    private func syncItems(_ tabs: NavigationTabs, navs: [UINavigationController]) {
        let faces = tabs.tabs.map(TabFace.init)
        // A tab's test id is its item's identifier, which XCUITest and
        // Maestro address it by (LLP 1116 D2); without one, its symbol or title.
        let ids = zip(tabs.tabs, faces).map { tab, face in tab.props["testId"] ?? face.base ?? face.title }
        let signature = zip(faces, ids).map { face, id in "\(id)|\(face.base ?? "")|\(face.title)|\(face.disabled)|\(face.accessibilityDisabled)|\(face.badge ?? "")" }
        for (index, (nav, face)) in zip(navs, faces).enumerated() where !tabItems.indices.contains(index) || tabItems[index] != signature[index] {
            let item: UITabBarItem = nav.tabBarItem
            let image = face.base.flatMap { UIImage(systemName: $0) }
            item.title = face.title
            item.image = image
            item.selectedImage = face.base.flatMap { UIImage(systemName: $0 + ".fill") } ?? image
            item.accessibilityIdentifier = ids[index]
            item.isEnabled = !face.disabled
            if face.disabled || face.accessibilityDisabled { item.accessibilityTraits.insert(.notEnabled) }
            else { item.accessibilityTraits.remove(.notEnabled) }
            // An authored badge, or one it just lost; a badge a hatch set on
            // a tab that never authored one is left alone.
            let had = tabItems.indices.contains(index) && !tabItems[index].hasSuffix("|")
            if face.badge != nil || had { item.badgeValue = face.badge }
        }
        tabItems = signature
        tint(tabs.tablist)
    }

    /// The tablist's `accent-color` tints the bar's selected item, as it
    /// tints a control (recipes F20, shop F28); `auto` keeps the system's.
    /// The row is inherited, as in CSS, and reaches the host only on the
    /// node that sets it: the nearest ancestor that does is read. Resolved
    /// per appearance, so dark mode follows.
    func tint(_ list: NodeView) {
        guard let bar = tabController?.tabBar else { return }
        var view: UIView? = list
        while let at = view, (at as? NodeView)?.channels("accent_color", dark: false) == nil { view = at.superview }
        let source = view as? NodeView
        // An accent that is the app's tint (`AccentColor`), where the bar
        // inherits the same tint, is left for the bar to inherit, as a
        // hand-built app leaves its bar's. Read from a view, the tint behind
        // an alert or a menu is already dimmed (`tintAdjustmentMode`), and
        // the bar would dim it a second time.
        if case .object(let o)? = source?.style["accent_color"], o["sys"]?.string == "@tint", let source, inheritsSameTint(bar, as: source) {
            guard tabTint != [] else { return }
            tabTint = []
            bar.tintColor = nil
            return
        }
        let wanted = [source?.channels("accent_color", dark: false), source?.channels("accent_color", dark: true)]
        guard wanted != tabTint else { return }
        tabTint = wanted
        // Never `.tintColor` in its place: that is the bar's own tint, this
        // colour, and a tablist whose accent went before the next sync (a
        // style that drops it) resolved it until the stack ran out. The
        // bar's container has the tint the bar would inherit.
        bar.tintColor = source.map { source in
            UIColor { [weak source, weak bar] traits in
                source?.channels("accent_color", dark: traits.userInterfaceStyle == .dark).map { TextEngine.color($0) }
                    ?? bar?.superview?.tintColor ?? .systemBlue
            }
        }
    }

    /// Whether the bar inherits the tint `node` does: no ancestor of the
    /// node sets a colour of its own (`applyAccentTint`), or the bar is
    /// inside the nearest that does.
    private func inheritsSameTint(_ bar: UIView, as node: NodeView) -> Bool {
        var view: UIView? = node
        while let at = view {
            if let n = at as? NodeView, n.style["accent_color"]?.namesTint != true, n.channels("accent_color", dark: false) != nil {
                return bar.isDescendant(of: n)
            }
            view = at.superview
        }
        return true
    }

    /// The bar would select `controller`: press its authored tab.
    func selectTab(_ controller: UIViewController) {
        guard let root = container, let tabs = NavigationTabs.of(root, presenter),
              let panel = tabNavigations.first(where: { $0.value === controller })?.key,
              let index = tabs.panels.firstIndex(where: { $0.id == panel }) else { return }
        let screen = reselectedRoot(index), id = tabs.tabs[index].id
        _ = act(id, 0)
        // After the tab's own press, which is queued: a batch applied under
        // a running scroll would cut it short.
        if let screen { presenter.elements.later(id, "scroll to top") { [weak self, weak screen] in if let screen { self?.scrollToTop(screen) } } }
    }

    /// The screen a tap on the bar's item for tab `index` scrolls to its
    /// top: the selected tab's, tapped again at its stack's root with
    /// nothing presented over it, as every tab bar app does (LLP 1115.000
    /// D11.3). Above the root the tap pops to it, which is the router's
    /// (LLP 1038 D12), and another tab's tap selects it: nil for both. Read
    /// before the tab's press, scrolled after it (`scrollToTop`).
    func reselectedRoot(_ index: Int) -> RouteController? {
        guard index == routerTab, tabPanels.indices.contains(index), let stack = tabNavigations[tabPanels[index]],
              stack.viewControllers.count == 1, presentedNavigations.isEmpty else { return nil }
        return stack.topViewController as? RouteController
    }

    /// A route's content scroller (`contentScroller`, the status bar's) to
    /// its top, as the status bar's tap takes it: UIKit's animation, to where
    /// a large title is open, which UIKit opens as the offset arrives.
    /// UIKit's tab bar does not do this itself: a selected tab tapped again
    /// scrolls nothing in UIKit alone (iOS 27, a real touch).
    func scrollToTop(_ c: RouteController) {
        guard let node = contentScroller(of: c), let sv = shownScroll(node) else { return }
        guard sv.contentOffset.y > 0.5 - sv.adjustedContentInset.top else { return }
        guard scrollsToTopAnimated else { return restAtTop(sv, in: c) }
        let inset = max(sv.adjustedContentInset.top, node === c.collapseScroll ? node.scrollOrigin : 0)
        sv.setContentOffset(CGPoint(x: sv.contentOffset.x, y: -inset), animated: true)
    }

    /// Where `scrollToTop` ends, in one turn, under the agent's frozen clock,
    /// where Exact starts no UIKit animation: the content at its top and a
    /// large title expanded. Measured in UIKit alone on iOS 27, an offset written
    /// at the top under a collapsed title leaves it collapsed, and one written
    /// above the top opens the title and leaves the content that far below
    /// the bar. So the scroller is pulled past its top and put at rest there,
    /// unanimated: no frame shows the pull. This is the one place Exact moves
    /// a large title itself.
    private func restAtTop(_ sv: UIScrollView, in c: RouteController) {
        sv.contentOffset.y = -sv.adjustedContentInset.top
        // tvOS has no large titles.
        #if !os(tvOS)
        guard c.navigationItem.largeTitleDisplayMode == .always, c.viewIfLoaded?.window != nil,
              let nav = c.navigationController, nav.topViewController === c, nav.transitionCoordinator == nil,
              !nav.isNavigationBarHidden else { return }
        UIView.performWithoutAnimation {
            // By the bar's height at a time, while the title still opens:
            // UIKit grows the inset by what the pull opened of it.
            for _ in 0..<4 {
                let top = sv.adjustedContentInset.top
                sv.contentOffset.y = -top - nav.navigationBar.frame.height
                nav.view.layoutIfNeeded()
                if sv.adjustedContentInset.top <= top + 0.5 { break }
            }
            sv.contentOffset.y = -sv.adjustedContentInset.top
        }
        #endif
    }

    /// What holds the tabs goes: its stacks are retired, the authored
    /// tablist paints again.
    func retireTabs() {
        // The tablist's hold on the bar goes with its container (§3.7).
        tablistRoot = nil
        tablistHidBar = false
        for nav in tabPanels.compactMap({ tabNavigations[$0] }) {
            retireStack(nav)
            nav.delegate = nil
            // Its routes may go into the next containers (a remount, new tabs).
            nav.setViewControllers([], animated: false)
        }
        if tabsHatched, tabController != nil { presenter.session?.natives.tabsHatch(tabController, event: 1) }
        if tabController == nil, tabOwner != nil { presenter.session?.natives.tabsHatch(nil, event: 3) }
        if let holder = tabOwner {
            (holder as? UITabBarController)?.delegate = nil
            holder.willMove(toParent: nil)
            holder.view.removeFromSuperview()
            holder.removeFromParent()
        }
        tabController = nil
        tabOwner = nil
        tabsHatched = false
        tabContainerAsked = false
        routerTab = -1
        tabNavigations = [:]
        tabPanels = []
        tabItems = []
        tabTint = nil
        adoptedTablist = nil
        tablistAppearanceNone = false
        measuredTablist = nil
    }

    /// After a batch: the projection, unless the batch only moved a list's
    /// rows and nothing is owed; then what presentations inherit and the
    /// authored tablist's covers.
    func batchApplied(_ batch: Batch, rowsOnly: Bool) {
        if !rowsOnly || syncOwed { sync(batch) }
        presenter.modals.inheritFromAncestors()
        followAuthoredTablist()
    }

    /// The authored tablist moved or changed size since its covers were
    /// measured: measure again. Its own cover's change (the bar's `whole`
    /// given back, an edge's padding) moves it in a batch that projects
    /// nothing, after the sync that measured it, so the covers settle only
    /// when they are measured where it now is.
    func followAuthoredTablist() {
        guard tablistAppearanceNone, !syncing, let view = presenter.session?.view,
              let list = adoptedTablist.flatMap({ presenter.views[$0] }) else { return }
        if list.convert(list.bounds, to: view) != measuredTablist { reportCovers() }
    }

    /// Whether a tab container takes this tablist's place.
    func adopts(tablist: NodeView) -> Bool {
        adoptedTablist == tablist.id && tabOwner != nil && !ExactEnv.authoredChrome &&
            !(tablist.wrote("appearance") && tablist.style["appearance"]?.string == "none")
    }
}
#endif

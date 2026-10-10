// @ref LLP 1075.003 §3.7 — Exact's tab container on iOS: a
// `UITabBarController` with one navigation controller a tab, each tab's
// stack kept as the router keeps it (LLP 1038 §7's "one navigation owner per
// tab"), so a scroll offset, a draft or a pushed screen survives a switch.
// The authored `tablist` supplies the items — a symbol over a label, its
// `-fill` symbol when selected — and takes no room while the bar shows; a
// tab the bar selects presses its authored tab, so selection, history and
// reselect-pops-to-root stay the router's (LLP 1038 D4, D12). Under the
// agent the authored tablist paints and the bar stays hidden. Which tabs a
// root has is NavigationTabs.swift's.
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

/// A tab bar item an authored tab supplies: its symbol, if it has one, and
/// its label (a text-only tab is a title-only item).
private struct TabFace: Equatable {
    let base: String?, title: String, disabled: Bool
    /// A filled box holding a text among the tab's children: the item's
    /// badge (LLP 1075.003 §9.9), as the web paints it on the tab.
    let badge: String?
    init(_ tab: NodeView) {
        var symbol: String?, label = tab.props["accessibilityLabel"] ?? "", badges: [String] = []
        if tab.isNativeButton, let face = tab.face {
            // A native button's children are its face (LLP 1069.011.000 D1).
            symbol = face.symbol
            if let title = face.title, !title.isEmpty { label = title }
        }
        for case let child as NodeView in tab.container.subviews {
            if child.kind == "image", let name = child.props["symbolName"], !name.isEmpty { symbol = name }
            else if child.isParagraph, !child.accessibleText.isEmpty { label = child.accessibleText }
            else if let text = Self.badgeText(child) { badges.append(text) }
        }
        // One badge box, or none: two are not a badge.
        self.badge = badges.count == 1 ? badges[0] : nil
        base = symbol.map { $0.hasSuffix(".fill") ? String($0.dropLast(5)) : $0 }
        title = label
        disabled = tab.disabled
    }
    /// A shown box with a visible fill whose only child (views and flat
    /// leaves alike) is one shown, non-empty text: that text. A pill around
    /// a symbol and a label is not, nor a transparent box or text.
    private static func badgeText(_ box: NodeView) -> String? {
        let shown = { (v: NodeView) in !v.isHidden && v.style["display"]?.string != "none" && v.alpha > 0 }
        guard !box.isParagraph, box.kind != "image", shown(box),
              let fill = box.channels("background_color"), fill[3] > 0,
              box.presenter?.flats.holdsLeaves(box.id) != true else { return nil }
        let children = box.container.subviews.compactMap { $0 as? NodeView }
        guard children.count == 1, let text = children.first, text.isParagraph, shown(text),
              !text.accessibleText.isEmpty else { return nil }
        return text.accessibleText
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
    var tabBarShows: Bool { tabController != nil && tabOwner === tabController && !ExactEnv.authoredChrome }

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
            if ExactEnv.authoredChrome { hideTabBar(container) }
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
        let navs = tabPanels.compactMap { tabNavigations[$0] }
        container.setViewControllers([], animated: false)
        guard let owned = askTabContainer(tabs, navs: navs, selected: routerTab) else {
            container.setViewControllers(navs, animated: false)
            container.selectedIndex = max(0, routerTab)
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
        let navs = tabs.panels.compactMap { tabNavigations[$0.id] }
        guard navs.count == tabs.panels.count else { return }
        for (index, nav) in navs.enumerated() where index != p.at {
            let stack = base(p.wanted[index])
            prepareRoutes(stack, in: nav)
            if nav.viewControllers.count != stack.count || !zip(nav.viewControllers, stack).allSatisfy({ $0 === $1 }) {
                nav.setViewControllers(stack, animated: false)
                recordOwned(nav)
            }
        }
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
        let signature = faces.map { "\($0.base ?? "")|\($0.title)|\($0.disabled)|\($0.badge ?? "")" }
        for (index, (nav, face)) in zip(navs, faces).enumerated() where !tabItems.indices.contains(index) || tabItems[index] != signature[index] {
            let item: UITabBarItem = nav.tabBarItem
            let image = face.base.flatMap { UIImage(systemName: $0) }
            item.title = face.title
            item.image = image
            item.selectedImage = face.base.flatMap { UIImage(systemName: $0 + ".fill") } ?? image
            item.accessibilityIdentifier = face.base ?? face.title
            item.isEnabled = !face.disabled
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
        let wanted = [source?.channels("accent_color", dark: false), source?.channels("accent_color", dark: true)]
        guard wanted != tabTint else { return }
        tabTint = wanted
        bar.tintColor = source.map { source in
            UIColor { [weak source] traits in
                source?.channels("accent_color", dark: traits.userInterfaceStyle == .dark).map { TextEngine.color($0) } ?? .tintColor
            }
        }
    }

    /// The bar would select `controller`: press its authored tab.
    func selectTab(_ controller: UIViewController) {
        guard let root = container, let tabs = NavigationTabs.of(root, presenter),
              let panel = tabNavigations.first(where: { $0.value === controller })?.key,
              let index = tabs.panels.firstIndex(where: { $0.id == panel }) else { return }
        _ = act(tabs.tabs[index].id, 0)
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
    }

    /// Whether a tab container takes this tablist's place.
    func adopts(tablist: NodeView) -> Bool { adoptedTablist == tablist.id && tabOwner != nil && !ExactEnv.authoredChrome }
}
#endif

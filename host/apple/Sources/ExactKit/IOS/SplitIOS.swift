// @ref LLP 1127 — a list and its detail on iOS are UIKit's split view.
// Where a stack's root route holds a list whose rows push (the runner's
// rule, `StackSplit`), the stack is the two columns of a
// `UISplitViewController`: the root route in the list's navigation
// controller, the routes above it in the detail's. How many columns show,
// their widths and where the bars stand are UIKit's. Collapsed, UIKit nests
// the detail's controller in the list's and takes it back as it expands
// (LLP 1127 §1, item 2), so a pop in either is the router's Back. Expanded,
// a sheet that says `navigationInspector` is the split's inspector column
// (D14).
#if os(iOS) || os(tvOS)
import UIKit

/// UIKit's split, which says when a layout changed whether the list shows
/// beside the detail: a split laid out wide from the start sends no
/// expansion to its delegate.
private final class SplitController: UISplitViewController {
    var laidOut: () -> Void = {}
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        laidOut()
    }
}

final class SplitIOS: NSObject, UISplitViewControllerDelegate {
    let controller: UISplitViewController = SplitController(style: .doubleColumn)
    /// The list's column: the stack's root route.
    let list: UINavigationController
    /// The detail's column: the routes above the root.
    let detail: UINavigationController
    /// The detail while the stack is at its root: the platform's empty page,
    /// saying "No Selection" (LLP 1127 D7, Q2).
    let empty = UIViewController()
    /// UIKit collapsed or expanded the columns, or showed or hid one.
    var changed: () -> Void = {}
    /// The stack last shown, root first: expanded again, the columns show it.
    private var stack: [RouteController] = []
    /// `showsList` when `changed` was last called.
    private var told: Bool?
    /// The inspector's column (iOS 26), made when a route first goes there.
    private(set) var inspector: UINavigationController?
    /// The person closed the inspector's column; the router has yet to hear it.
    var inspectorClosed: () -> Void = {}
    /// The list whose rows made the stack a split, which keeps it one while
    /// it has no row to act on (a search that matches nothing).
    var listNode: UInt32?

    init(list: UINavigationController, detail: UINavigationController) {
        self.list = list
        self.detail = detail
        super.init()
        #if os(iOS)
        empty.view.backgroundColor = .systemBackground
        // English until the host has a strings table for its own words (LLP 1127 D7)
        var unavailable = UIContentUnavailableConfiguration.empty()
        unavailable.text = "No Selection"
        empty.contentUnavailableConfiguration = unavailable
        #endif
        controller.delegate = self
        #if os(iOS)
        // The list's column is a sidebar, in UIKit's style for one, as SwiftUI's split sets it.
        controller.primaryBackgroundStyle = .sidebar
        #endif
        (controller as? SplitController)?.laidOut = { [weak self] in
            guard let self, told != showsList else { return }
            told = showsList
            DispatchQueue.main.async { self.changed() }
        }
        controller.setViewController(list, for: .primary)
        controller.setViewController(detail, for: .secondary)
    }

    /// Whether the list shows beside the detail, live (`BlockShown.beside`).
    var showsList: Bool { !controller.isCollapsed && controller.displayMode != .secondaryOnly }
    /// Whether the list shows over the detail: then only the list is live (LLP 1127 D8).
    var overlaysDetail: Bool { !controller.isCollapsed && [.oneOverSecondary, .twoOverSecondary].contains(controller.displayMode) }
    /// Whether UIKit nests the detail's controller in the list's now.
    var nested: Bool { controller.isCollapsed && list.viewControllers.contains { $0 === detail } }
    /// Whether UIKit is moving the detail's controller between the columns:
    /// from asking which column to put on top as it collapses, or which
    /// display mode to expand to, until it has collapsed or expanded. Either
    /// end ends the move: an iPhone Duo unfolding after a Back while folded
    /// has UIKit ask about a collapse within the expansion and end with the
    /// expansion alone. Meanwhile the detail's controller enters or leaves
    /// the list's, which is no Back, and the split can still read collapsed
    /// or expanded as before.
    private(set) var moving = false
    /// Whether the person sees the detail's routes: collapsed, only while
    /// nested, as a pop of the detail's controller leaves its routes in it.
    private var showsDetail: Bool { (controller.isCollapsed ? nested : true) && detail.viewControllers.contains { $0 is RouteController } }
    /// The controller whose top the person sees.
    var front: UINavigationController { showsDetail ? detail : list }
    /// The routes UIKit shows, root first.
    var shown: [RouteController] {
        list.viewControllers.prefix(1).compactMap { $0 as? RouteController } + (showsDetail ? detail.viewControllers.compactMap { $0 as? RouteController } : [])
    }
    /// Whether a stack is one of the columns'.
    func holds(_ nav: UINavigationController) -> Bool { nav === list || nav === detail }

    /// The stack, root first, in the two columns: what UIKit shows of it is
    /// UIKit's. A push or pop within the detail is animated as a stack's is;
    /// a new detail replaces the last at once, as a hand-built split's does.
    func show(_ stack: [RouteController], animated: Bool) {
        guard let root = stack.first else { return }
        let moved = self.stack.map(ObjectIdentifier.init) != stack.map(ObjectIdentifier.init)
        self.stack = stack
        // UIKit is moving the detail's controller between the columns: the
        // stack shows once it has expanded or collapsed. Shown now, the
        // columns' stacks would be set from the split's state before the
        // move, and UIKit, finding them changed, pushes the detail twice and
        // asserts.
        if moving { return }
        let rest = Array(stack.dropFirst())
        let current = detail.viewControllers.compactMap { $0 as? RouteController }
        let within = !current.isEmpty && !rest.isEmpty && NavigationRules.isPushOrPop(from: current.map(ObjectIdentifier.init), to: rest.map(ObjectIdentifier.init))
        if controller.isCollapsed {
            if list.viewControllers.first !== root { list.setViewControllers([root], animated: false) }
            if rest.isEmpty {
                // Nested, UIKit pops the detail's controller; out of sight, it empties.
                if nested { controller.show(.primary) } else if !same([empty]) { detail.setViewControllers([empty], animated: false) }
                return
            }
            if !same(rest) { detail.setViewControllers(rest, animated: animated && within && nested) }
            if !nested { controller.show(.secondary) }
            return
        }
        if list.viewControllers.count != 1 || list.viewControllers.first !== root { list.setViewControllers([root], animated: false) }
        let next: [UIViewController] = rest.isEmpty ? [empty] : rest
        if !same(next) { detail.setViewControllers(next, animated: animated && within) }
        // A selection while UIKit shows the list over the detail shows the
        // detail (LLP 1127 D6); any other change (a search typed in the list) leaves the list.
        if moved, !rest.isEmpty, controller.displayMode == .oneOverSecondary { controller.show(.secondary) }
    }

    /// The route the inspector's column shows, while the split is expanded.
    var inspecting: RouteController? {
        guard !controller.isCollapsed, let inspector else { return nil }
        return inspector.viewControllers.first as? RouteController
    }

    /// The routes shown beside the stack's top, live: the list beside its
    /// detail, and the detail beside its inspector.
    var beside: [RouteController] {
        let list = showsList ? list.viewControllers.prefix(1).compactMap { $0 as? RouteController } : []
        let detail = inspecting != nil ? (self.detail.topViewController as? RouteController).map { [$0] } ?? [] : []
        return list + detail
    }

    /// The routes shown under a pane over them, not live: the detail under
    /// the list that overlays it.
    var under: [RouteController] {
        overlaysDetail ? (detail.topViewController as? RouteController).map { [$0] } ?? [] : []
    }

    /// Whether this split can show an inspector's column (UIKit's, iOS 26).
    var inspects: Bool {
        #if os(iOS)
        if #available(iOS 26.0, *) { return true }
        #endif
        return false
    }

    /// The sheet route that says `navigationInspector` in the inspector's
    /// column, or none there (LLP 1127 D14). Collapsed, the column empties at
    /// once, for the route to go to its sheet; closed while expanded, the
    /// route stays until the column's transition is over.
    func inspect(_ route: RouteController?, in made: UINavigationController?) {
        // tvOS has no inspector's column.
        #if os(iOS)
        guard #available(iOS 26.0, *) else { return }
        if let route, let nav = made {
            if inspector !== nav { inspector = nav; controller.setViewController(nav, for: .inspector) }
            if nav.viewControllers.first !== route { nav.setViewControllers([route], animated: false) }
            if !controller.isShowing(.inspector) {
                controller.show(.inspector)
                // Its content is laid out where it ends, at once: in UIKit's
                // animated pass it grew from no size, flying in from the
                // column's corner, where a hand-built inspector's stays put.
                UIView.performWithoutAnimation { controller.view.layoutIfNeeded() }
            }
            return
        }
        guard let nav = inspector, !nav.viewControllers.isEmpty else { return }
        if controller.isCollapsed || !controller.isShowing(.inspector) { return nav.setViewControllers([], animated: false) }
        closing = true
        controller.hide(.inspector)
        let empty = { [weak self, weak nav] in
            guard let self, let nav, closing else { return }
            closing = false
            nav.setViewControllers([], animated: false)
        }
        if let coordinator = controller.transitionCoordinator { coordinator.animate(alongsideTransition: nil) { _ in empty() } } else { empty() }
        #endif
    }

    /// Exact is closing the inspector's column: UIKit's report of it is not the person's.
    private var closing = false

    private func same(_ next: [UIViewController]) -> Bool {
        detail.viewControllers.count == next.count && zip(detail.viewControllers, next).allSatisfy { $0 === $1 }
    }

    // Collapsed, the detail is on top while it holds a route (LLP 1127 D3's answer).
    func splitViewController(_ svc: UISplitViewController,
                             topColumnForCollapsingToProposedTopColumn proposedTopColumn: UISplitViewController.Column) -> UISplitViewController.Column {
        moving = true
        return detail.viewControllers.contains { $0 is RouteController } ? .secondary : .primary
    }
    func splitViewController(_ svc: UISplitViewController,
                             displayModeForExpandingToProposedDisplayMode proposedDisplayMode: UISplitViewController.DisplayMode) -> UISplitViewController.DisplayMode {
        moving = true
        return proposedDisplayMode
    }

    func splitViewController(_ svc: UISplitViewController, didHide column: UISplitViewController.Column) {
        #if os(iOS)
        guard #available(iOS 26.0, *), column == .inspector, !closing, !svc.isCollapsed else { return }
        DispatchQueue.main.async { self.inspectorClosed() }
        #endif
    }

    func splitViewControllerDidCollapse(_ svc: UISplitViewController) {
        moving = false
        show(stack, animated: false)
        DispatchQueue.main.async { self.changed() }
    }
    func splitViewControllerDidExpand(_ svc: UISplitViewController) {
        moving = false
        // Collapsed, a pop of the detail's controller left its routes in it.
        show(stack, animated: false)
        DispatchQueue.main.async { self.changed() }
    }
    func splitViewController(_ svc: UISplitViewController, willChangeTo displayMode: UISplitViewController.DisplayMode) {
        DispatchQueue.main.async { self.changed() }
    }
}

extension Presenter {
    /// Whether a grouped list shows as UIKit's sidebar: in a split's list column beside its detail.
    package func showsAsSidebar(_ list: UIView) -> Bool { navigation.showsAsSidebar(list) }
    /// The row of a sidebar whose detail shows, which stays selected.
    package func sidebarSelection(_ list: UIView) -> UInt32? { navigation.sidebarSelection(list) }
    /// Whether a view is in a split's column whose ground is UIKit's: the sidebar or the inspector.
    package func onColumnGround(_ view: UIView) -> Bool { navigation.onColumnGround(view) }
}

extension NavigationHost {
    /// Whether the stack whose first route is `root` shows as UIKit's split:
    /// its list's rows push, by the runner's rule, under the platform's chrome.
    /// An installed split keeps its list while that list has no row to act on.
    func wantsSplit(_ root: RouteController?) -> Bool {
        // tvOS takes it once a hand-built split is measured with the focus engine (LLP 1127 §4).
        #if os(tvOS)
        return false
        #else
        guard !ExactEnv.authoredChrome, let root, let session = presenter.session else { return false }
        return session.runtime.stackSplit(root.node.id, retaining: split?.listNode) != nil
        #endif
    }

    /// The split's two columns, each a navigation controller of its own, as
    /// the primary owner.
    func installSplit(_ p: Projection, first: [RouteController], in parent: UIViewController) {
        let list = makeNavigation(first: first.first?.node)
        let detail = makeNavigation(first: nil)
        let split = SplitIOS(list: list, detail: detail)
        split.listNode = first.first.flatMap { presenter.session?.runtime.stackSplit($0.node.id, retaining: nil)?.list }
        split.changed = { [weak self] in self?.splitChanged() }
        split.inspectorClosed = { [weak self] in self?.inspectorClosed() }
        self.split = split
        primaryNavigation = list
        prepareRoutes(Array(first.prefix(1)), in: list)
        prepareRoutes(Array(first.dropFirst()), in: detail, firstPushed: true)
        detail.setNavigationBarHidden(false, animated: false)
        split.show(first, animated: false)
        for nav in [list, detail] { recordOwned(nav); watchPops(nav) }
        parent.addChild(split.controller)
        split.controller.view.frame = ownerFrame(split.controller, in: p.root)
        split.controller.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        p.root.addSubview(split.controller.view)
        split.controller.view.setPaintForeground(aboveAuthored: false)
        fitOwner()
        split.controller.didMove(toParent: parent)
    }

    /// The stack in the split's columns, its routes projected into each.
    func syncSplit(_ split: SplitIOS, _ stack: [RouteController], animated: Bool) {
        prepareRoutes(Array(stack.prefix(1)), in: split.list)
        prepareRoutes(Array(stack.dropFirst()), in: split.detail, firstPushed: true)
        let shown = split.shown
        split.show(stack, animated: animated)
        for nav in [split.list, split.detail] { recordOwned(nav) }
        // A sidebar's selected row follows the detail.
        if split.shown != shown, split.showsList, let root = stack.first,
           let list = presenter.session?.runtime.stackSplit(root.node.id, retaining: nil)?.list {
            presenter.groupedLists?.sync(changed: [list])
        }
    }

    /// UIKit changed what the split shows: the bars' insets, which columns'
    /// routes are live, and the list's appearance follow.
    func splitChanged() {
        // Beside the detail, the list and its route are on UIKit's sidebar.
        if let root = split?.list.viewControllers.first as? RouteController { repaintGrounds(root) }
        coversChanged()
        presenter.owners.publish()
        presenter.groupedLists?.sync(changed: nil)
        // An inspector moves between its column and its sheet as the split expands and collapses.
        if selectedRoute?.props["navigationInspector"] == "true" { sync(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil)) }
    }

    /// The sheet an expanded split shows as its inspector (`navigationInspector`,
    /// LLP 1127 D14): the stack's one sheet, of one route.
    func inspectorRoute(_ parts: [Range<Int>], _ wanted: [RouteController]) -> RouteController? {
        guard let split, split.inspects, !split.controller.isCollapsed, parts.count == 2, parts[1].count == 1 else { return nil }
        let route = wanted[parts[1].lowerBound]
        return route.node.props["navigationInspector"] == "true" ? route : nil
    }

    /// The person closed the inspector's column: Back from its route, as a
    /// swipe down that closes its sheet is.
    func inspectorClosed() {
        guard let route = split?.inspector?.viewControllers.first as? RouteController, route.key == selectedKey else { return }
        goBack()
    }

    /// Whether `view` is in the split's list column while it shows beside the detail.
    func showsAsSidebar(_ view: UIView) -> Bool {
        guard let split, split.showsList, let list = split.list.viewControllers.first?.viewIfLoaded else { return false }
        return view.isDescendant(of: list)
    }

    /// Whether `view` is in a split's column whose ground is UIKit's: the
    /// list beside its detail (the sidebar), or the inspector.
    func onColumnGround(_ view: UIView) -> Bool {
        if showsAsSidebar(view) { return true }
        guard let route = split?.inspecting?.viewIfLoaded else { return false }
        return view.isDescendant(of: route)
    }

    /// A route's grounds, its grouped lists' and its boxes on the page's
    /// ground follow the column it is in.
    func repaintGrounds(_ route: RouteController) {
        route.paintBackdrop()
        route.node.applyBoxLayer()
        for box in presenter.views.values where box !== route.node && box.paintsPageGround && box.isDescendant(of: route.node) { box.applyBoxLayer() }
        let lists = presenter.carrying("listStyle").filter { $0.kind == "list" && $0.isDescendant(of: route.node) }
        for list in lists { list.applyBoxLayer() }
        if !lists.isEmpty { presenter.groupedLists?.sync(changed: Set(lists.map(\.id))) }
    }

    /// The row of the split's list whose detail shows: the detail's highest
    /// route that is a row's (`StackSplit.rows`), a page beneath what it pushed.
    func sidebarSelection(_ view: UIView) -> UInt32? {
        guard showsAsSidebar(view), let split, let root = split.list.viewControllers.first as? RouteController,
              let rule = presenter.session?.runtime.stackSplit(root.node.id, retaining: nil) else { return nil }
        return split.detail.viewControllers.reversed().lazy.compactMap { ($0 as? RouteController).flatMap { rule.rows[$0.node.id] } }.first
    }

    /// The split goes. `releasing`, its routes leave its columns first, for
    /// the containers that take them next (`reshape`).
    func retireSplit(releasing: Bool = false) {
        guard let split else { return }
        for nav in [split.list, split.detail] + [split.inspector].compactMap({ $0 }) {
            retireStack(nav)
            if releasing { nav.delegate = nil; nav.setViewControllers([], animated: false) }
        }
        split.detail.delegate = nil
        split.controller.willMove(toParent: nil)
        split.controller.view.removeFromSuperview()
        split.controller.removeFromParent()
        self.split = nil
    }
}
#endif

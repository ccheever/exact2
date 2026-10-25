// @ref LLP 1075.003 §3.5 — what Exact's containers cover of each route, told
// to the kernel as covers, and the development check of what Exact owns on
// the objects it handed a hatch.
#if os(iOS) || os(tvOS)
import UIKit

extension NavigationHost {
    // MARK: The content area (LLP 1075.003 §3.5)

    /// Whether the root's routes have a native primary container (a stack
    /// or the tabs), so the session's view takes the whole of its own
    /// (ExactViewIOS `fit`): the document and the window the viewport units
    /// resolve against. Its bars, whether shown or not, and a
    /// presentation's containers never decide it (LLP 1123 D1); its routes
    /// clear what covers them (`reportCovers`). Not under the agent's own
    /// chrome, where no native container shows.
    var wantsWholeView: Bool { primaryOwner != nil && !ExactEnv.authoredChrome }

    /// What covers a route of its controller's safe area (`safe`): the
    /// edges past the document's insets (`env()`, which the page authored
    /// against) for a route in the document, and every edge of its own for a
    /// presented one, whose sheet the document's insets do not clear (LLP
    /// 1123 D2). The top is a collapsing title's scroller's when it has one;
    /// the sides and the bottom only while the containers take the whole
    /// view (`whole`) or the page covers it (`covered`: `viewport-fit="cover"`,
    /// whose `env()` is the window's safe area and not the tab bar's, which a
    /// child of a `UITabBarController` reads in its own), and the bottom only
    /// as far as the route reaches it.
    static func routeEdges(safe: UIEdgeInsets, document: UIEdgeInsets, presented: Bool, whole: Bool, covered: Bool = false,
                           collapses: Bool, reached: CGFloat) -> HostCover.Edges {
        let env = presented ? .zero : document
        let edges = whole || covered
        return .init(top: collapses ? 0 : max(0, safe.top - env.top),
                     right: edges ? max(0, safe.right - env.right) : 0,
                     bottom: edges ? max(0, min(safe.bottom, reached) - env.bottom) : 0,
                     left: edges ? max(0, safe.left - env.left) : 0)
    }

    /// What Exact's containers cover of each route — its controller's safe
    /// area past the insets the kernel already has as `env()`: a shown bar's
    /// top, and every edge while the containers take the whole view for a
    /// page that did not ask to cover it — and the header a bar replaces,
    /// reported when they change, after the batch being applied.
    func reportCovers() {
        var changes: [(UInt32, HostCover?)] = []
        var wanted: [UInt32: HostCover] = [:]
        #if os(iOS)
        if #available(iOS 27.1, *) {
            for id in presenter.arrangements.hiddenPanes { wanted[id] = .whole }
        }
        #endif
        for c in controllers.values where presenter.views[c.node.id] === c.node {
            // A lifted header takes no room, on a stack yet or not (`coverLifted`).
            for node in (c.lifted.map({ [$0] }) ?? c.barNodes) + [c.scoped].compactMap({ $0 }) { wanted[node.id] = .whole }
            // Include the departing route until UIKit finishes its transition
            let nav = c.navigationController
            let held = nav.map {
                $0.viewControllers.contains(c) || $0.transitionCoordinator?.viewController(forKey: .from) === c
            } ?? (c.parent is HeldStack)
            guard held else { continue }
            // A presented route's controller takes its sheet's whole view, and
            // the document's insets (`env()`) are not the sheet's: its route is
            // covered for its own safe area whatever the document's fit (LLP
            // 1123 D2; its own clearance and credit are stage 2's)
            let presented = nav.map { nav in presentedNavigations.contains { $0 === nav } } ?? false
            let covered = wantsWholeView && presenter.viewportFit == "cover"
            let whole = presented || (wantsWholeView && !covered)
            let shows = nav.map(barShows) ?? !ExactEnv.authoredChrome
            guard shows || whole || covered else { continue }
            let edges: HostCover.Edges?
            // A collapsing title's scroller goes under the bar, which insets it;
            // its expanded inset is where CSS scrollTop 0 rests.
            // While the bar is not as the route wants it (mid-transition to or
            // from a route without one, §9.10), its last cover stands.
            // An active search's bar is UIKit's: the content follows it, as
            // UIKit's own does, but a title's insets are not sampled.
            let search = searching(c)
            let settled = search || (nav?.isNavigationBarHidden ?? true) != routeShowsBar(c, in: nav)
            // A route showing no bar has no title: its inset is the status bar's, none in landscape (LLP 1130 D3).
            let barless = !routeShowsBar(c, in: nav)
            if settled, !search, let node = c.collapseScroll, let sv = c.collapseScrollView, sv === node.scrollView, sv.adjustedContentInset.top > 0 || barless {
                let inset = sv.adjustedContentInset.top
                if c.insetsBarless != barless { c.insetsBarless = barless; node.scrollCollapsed = 0 }
                // tvOS has no large titles: every title is inline.
                #if os(tvOS)
                let inline = true
                #else
                let inline = c.navigationItem.largeTitleDisplayMode != .always || barless
                #endif
                if inline {
                    // An inline title does not collapse: its inset is the one it has.
                    node.scrollOrigin = inset; node.scrollCollapsed = inset
                } else if nav?.transitionCoordinator == nil {
                    // A large title's widest and narrowest, at rest only.
                    if inset > node.scrollOrigin { node.scrollOrigin = inset }
                    if node.scrollCollapsed == 0 || inset < node.scrollCollapsed { node.scrollCollapsed = inset }
                }
            }
            if settled, c.viewIfLoaded?.window != nil {
                let safe = c.view.safeAreaInsets
                // Only what the route reaches of the bottom bars is covered:
                // none once the keyboard ends it above them (`ownerFrame`).
                let reached = c.node.convert(c.node.bounds, to: c.view).maxY - (c.view.bounds.height - safe.bottom)
                edges = Self.routeEdges(safe: safe, document: presenter.insets, presented: presented, whole: whole, covered: covered,
                                        collapses: c.collapseScroll != nil || !c.ridingBars.isEmpty, reached: reached)
                c.bottomClearance = edges?.bottom ?? 0
            } else if case .edges(let e)? = covers[c.node.id] {
                edges = e
            } else { edges = nil }
            // A scroller UIKit insets lays its content out between those
            // insets (LLP 1116.000 D1.2): content that fills its height rests
            // under the bar and above the home indicator, and scrolling
            // content still passes under both. A large title's inset is its
            // widest, at rest (CSS scrollTop 0).
            // A transparent bar insets nothing of its scroller: the content
            // starts under it (`collapse`), so no `.inset` is reported there.
            let topPeers = arrangementScrollers(in: c, top: true), bottomPeers = arrangementScrollers(in: c, top: false)
            var insetNodes = [c.collapseScroll, c.edgeScroll].compactMap { $0 }
            for peer in topPeers + bottomPeers where !insetNodes.contains(where: { $0 === peer }) { insetNodes.append(peer) }
            for node in insetNodes where wanted[node.id] != .whole && node.scrollView != nil && !(node === c.collapseScroll && Self.transparentBar(c.node)) {
                if settled, c.viewIfLoaded?.window != nil, let safe = node.scrollView?.safeAreaInsets {
                    // The bottom edge's scroller takes the route's bottom
                    // clearance, which the route then pads nothing of
                    // (`scroller_insets_bottom`) and still moves an
                    // absolutely positioned footer past. Its own safe area
                    // holds none while that padding ends it above the bars:
                    // read alone, it would leave it there for good.
                    let peerTop = topPeers.contains { $0 === node }, peerBottom = bottomPeers.contains { $0 === node }
                    let reach = node === c.edgeScroll || peerBottom ? c.bottomClearance : 0
                    let ownsTop = node === c.collapseScroll || peerTop
                    #if os(iOS)
                    if peerTop, let scroll = node.scrollView {
                        if c.navigationItem.largeTitleDisplayMode == .always { node.scrollOrigin = max(node.scrollOrigin, scroll.adjustedContentInset.top) }
                        else { node.scrollOrigin = scroll.adjustedContentInset.top }
                        if node !== c.collapseScroll { node.scrollCollapsed = node.scrollOrigin }
                    }
                    #endif
                    // A band's part of the safe area is a UI inset over control bars, not layout (LLP 1131 D2).
                    let top = max(0, max(safe.top, ownsTop ? node.scrollOrigin : 0) - node.pageBand.top), bottom = max(0, max(safe.bottom, reach) - node.pageBand.bottom)
                    if top > 0 || bottom > 0 { wanted[node.id] = .inset(top: top, bottom: bottom) }
                } else if case .inset? = covers[node.id] { wanted[node.id] = covers[node.id] }
            }
            guard let edges else { continue }
            if edges != .init(top: 0, right: 0, bottom: 0, left: 0) { wanted[c.node.id] = .edges(edges) }
        }
        // The tablist whose place the tab bar takes takes no room.
        if let list = adoptedTablist.flatMap({ presenter.views[$0] }) {
            measuredTablist = presenter.session?.view.map { list.convert(list.bounds, to: $0) }
            if adopts(tablist: list) { wanted[list.id] = .whole }
            else if tablistAppearanceNone, list.style["display"]?.string != "none",
                    wantsWholeView, presenter.viewportFit != "cover",
                    let view = presenter.session?.view, view.window != nil {
                // Authored tabs are page content outside the routes. Protect
                // only the edges they reach; leave the viewport and bar
                // backgrounds full-size. A sheet leaves them where they are
                // (LLP 1123 D1).
                let rect = list.convert(list.bounds, to: view)
                let safe = view.bounds.inset(by: view.safeAreaInsets)
                let edges = HostCover.Edges(
                    top: max(0, min(rect.height, safe.minY - rect.minY)),
                    right: max(0, min(rect.width, rect.maxX - safe.maxX)),
                    bottom: max(0, min(rect.height, rect.maxY - safe.maxY)),
                    left: max(0, min(rect.width, safe.minX - rect.minX)))
                if edges != .init(top: 0, right: 0, bottom: 0, left: 0) { wanted[list.id] = .edges(edges) }
            }
        }
        for (id, cover) in wanted where covers[id] != cover { changes.append((id, cover)) }
        for id in covers.keys where wanted[id] == nil && presenter.views[id] != nil { changes.append((id, nil)) }
        covers = wanted
        guard !changes.isEmpty else { return }
        // A scroller's least content height is the port its cover leaves
        // (`scrollFloor`); the kernel sends no new content size when only that changed.
        for (id, _) in changes { presenter.views[id]?.fitScroll() }
        // Under Q3 (c) a cold launch's first frame runs no app code: a hatch
        // that changes a bar the plan already showed moves the content once.
        if replayingHatches, changes.contains(where: { presenter.views[$0.0]?.props["navigationKey"] != nil }) {
            presenter.session?.log("hatch: the content area moved after the first frame, by a hatch run at launch")
        }
        // Never inside a batch: the stack is installed partway through the
        // first one, whose remaining frames would overwrite the covered ones.
        presenter.afterBatch { [weak self] in self?.deliverCovers() }
    }

    /// What the kernel is told is what is wanted when it is told. A report
    /// queued behind a batch ran after later ones that did not wait, and its
    /// covers, measured under the bar's first frame before the status bar's
    /// inset (116 points with a large title of 168), won: team-kanban's
    /// persona switcher lay under the large title, drawn over by it and
    /// untappable (LLP 1116.006 D1).
    func deliverCovers() {
        var changes: [(UInt32, HostCover?)] = []
        for (id, cover) in covers where delivered[id] != cover { changes.append((id, cover)) }
        for id in delivered.keys where covers[id] == nil && presenter.views[id] != nil { changes.append((id, nil)) }
        delivered = covers
        guard !changes.isEmpty else { return }
        presenter.onCovers?(changes)
    }

    // MARK: The development check (LLP 1075.003 §3.5)

    /// Whether this build checks what Exact owns: every build but a
    /// production bake.
    static let checksOwnership = !ExactEnv.productionBake

    /// Record the controllers Exact set on a stack.
    func recordOwned(_ nav: UINavigationController) {
        stacks[ObjectIdentifier(nav)]?.written = nav.viewControllers.map(ObjectIdentifier.init)
    }

    /// UIKit completed a transition (`didShow`): a pop the user made (UIKit's
    /// back button, the edge swipe) leaves a prefix of what Exact wrote, and
    /// is recorded; any other change is not Exact's and stays to be reported.
    func recordPop(_ nav: UINavigationController) {
        guard let stack = stacks[ObjectIdentifier(nav)] else { return }
        let now = nav.viewControllers.map(ObjectIdentifier.init)
        if now.count <= stack.written.count, Array(stack.written.prefix(now.count)) == now { stack.written = now }
    }

    /// Before each batch: compare what Exact owns on each hatched object with
    /// what Exact last wrote, and journal a difference once, by name. It is
    /// detection, not enforcement; a change made and undone between two
    /// batches is not seen.
    func checkOwned() {
        guard NavigationHost.checksOwnership, !changing, !syncing, presenter.session?.natives.hatchesConnected == true else { return }
        func say(_ what: String, _ property: String) {
            let line = "\(what): \(property) changed outside Exact, which owns it"
            if ownedReported.insert(line).inserted { presenter.session?.log(line) }
        }
        for nav in allNavigations {
            guard let stack = stacks[ObjectIdentifier(nav)], stack.hatched else { continue }
            if nav.delegate !== stack.proxy { say("navigation \(stack.label)", "delegate") }
            if nav.transitionCoordinator == nil, !searching(nav.topViewController as? RouteController), nav.isNavigationBarHidden == topShowsBar(nav) { say("navigation \(stack.label)", "navigation bar visibility") }
            if nav.viewControllers.map(ObjectIdentifier.init) != stack.written { say("navigation \(stack.label)", "viewControllers") }
            #if !os(tvOS)
            if let pop = nav.interactivePopGestureRecognizer, pop.delegate !== self { say("navigation \(stack.label)", "the pop gesture's delegate") }
            if #available(iOS 26.0, tvOS 26.0, *), let pop = nav.interactiveContentPopGestureRecognizer, pop.delegate !== self {
                say("navigation \(stack.label)", "the content pop gesture's delegate")
            }
            #endif
        }
        for c in controllers.values where c.hatched && presenter.views[c.node.id] === c.node {
            if c.navigationController != nil, c.isViewLoaded, c.node.superview !== c.contentMount { say("route \(c.key)", "view") }
            guard let node = c.ownedScroll, let scroll = node.scrollView else { continue }
            for property in NavigationHost.ownedChanges(scroll, of: node, insetByUIKit: c.collapseScroll === node || c.edgeScroll === node) { say("route \(c.key)", property) }
        }
    }

    /// What Exact sets on a node's scroll view and nothing else may (LLP
    /// 1075.003 §3.5) and differs from what Exact writes: no inset (a
    /// refresh control insets it while it spins), no automatic adjustment
    /// but a route's collapse or bottom-edge scroller's, the backend's Exact-owned delegate, the node's keyboard dismissal (authored, or
    /// the platform's for what it scrolls, LLP 1115.000 D9.1). The offset
    /// and the content size are Exact's too but are not checked: they move
    /// with the user and with layout, so no last write predicts them.
    static func ownedChanges(_ s: UIScrollView, of node: NodeView, insetByUIKit: Bool) -> [String] {
        var out: [String] = []
        // A native backend owns layout/footer insets, composed with the
        // refresh control and keyboard; zero is only the plain-scroll baseline.
        if s === node.scroll {
            #if os(tvOS)
            if s.contentInset != node.bandInset { out.append("contentInset") }
            #else
            if s.contentInset != node.bandInset, s.refreshControl?.isRefreshing != true { out.append("contentInset") } // a band's is the host's
            #endif
        }
        if s.contentInsetAdjustmentBehavior != (insetByUIKit ? .automatic : .never) { out.append("contentInsetAdjustmentBehavior") }
        if s !== node.scrollView || !node.ownsScrollDelegate { out.append("delegate") }
        if s.keyboardDismissMode != node.keyboardDismissal { out.append("keyboardDismissMode") }
        return out
    }
}
#endif

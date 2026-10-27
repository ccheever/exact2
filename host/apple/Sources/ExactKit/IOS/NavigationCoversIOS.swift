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

    /// What a route clears of its controller's safe area (`safe`): every edge
    /// of its own, whatever the document's fit, as each content owner clears
    /// the chrome its author did not write (LLP 1123 D3; `env()` is the
    /// session root's, D1); the sides and the bottom only while the
    /// containers take the whole view or a presentation holds the route
    /// (`edges`), and the bottom only as far as the route reaches it.
    static func routeEdges(safe: UIEdgeInsets, edges: Bool, reached: CGFloat) -> HostCover.Edges {
        .init(top: safe.top,
              right: edges ? safe.right : 0,
              bottom: edges ? max(0, min(safe.bottom, reached)) : 0,
              left: edges ? safe.left : 0)
    }

    /// Whether `sv` is pulled past where its title's inset rests (`origin`),
    /// as to refresh. UIKit grows the scroller's safe area with the pull, and
    /// with the refresh control's room while it shows, and gives it back as
    /// the list returns: neither is the bar's inset, where CSS scrollTop 0
    /// rests, and a hand-built list lays out nothing for it. Reported, each
    /// frame of a pull laid the page out again.
    static func stretched(_ sv: UIScrollView, origin: CGFloat) -> Bool {
        origin > 0 && sv.contentOffset.y < -origin - 0.5
    }

    /// What Exact's containers cover of each route — its controller's safe
    /// area: a shown bar's top, and every edge while the containers take the
    /// whole view — and the header a bar replaces, reported when they change,
    /// after the batch being applied.
    func reportCovers() {
        var changes: [(UInt32, HostCover?)] = []
        var wanted: [UInt32: HostCover] = [:]
        var owners: [UInt32: OwnerValues] = [:]
        for c in controllers.values where presenter.views[c.node.id] === c.node {
            // A lifted header takes no room, on a stack yet or not (`coverLifted`).
            for node in (c.lifted.map({ [$0] }) ?? c.barNodes) + [c.scoped].compactMap({ $0 }) { wanted[node.id] = .whole }
            // Include the departing route until UIKit finishes its transition
            let nav = c.navigationController
            let held = nav.map {
                $0.viewControllers.contains(c) || $0.transitionCoordinator?.viewController(forKey: .from) === c
            } ?? (c.parent is HeldStack)
            guard held else { continue }
            // A presented route's controller takes its sheet's whole view (LLP 1123 D2), as the
            // containers take the session's: the route is covered on every edge
            let whole = wantsWholeView || nav.map { nav in presentedNavigations.contains { $0 === nav } } ?? false
            let shows = nav.map(barShows) ?? !ExactEnv.authoredChrome
            guard shows || whole else { continue }
            var edges: HostCover.Edges?
            // What the route clears, its values' content root (`ownerValues`)
            var clearance: HostCover.Edges?
            // A collapsing title's scroller goes under the bar, which insets it;
            // its expanded inset is where CSS scrollTop 0 rests.
            // While the bar is not as the route wants it (mid-transition to or
            // from a route without one, §9.10), its last cover stands.
            // An active search's bar is UIKit's: the content follows it, as
            // UIKit's own does, but a title's insets are not sampled.
            c.safeArea.sync(c)
            let search = searching(c)
            let settled = search || (nav?.isNavigationBarHidden ?? true) != routeShowsBar(c, in: nav)
            // A route showing no bar has no title: its inset is the status bar's, none in landscape (LLP 1130 D3).
            let barless = !routeShowsBar(c, in: nav)
            if !c.ignoresSafeArea("top"), settled, !search, let node = c.collapseScroll, let sv = c.collapseScrollView, sv === node.scrollView, sv.adjustedContentInset.top > 0 || barless {
                let inset = sv.adjustedContentInset.top
                if c.insetsBarless != barless { c.insetsBarless = barless; node.scrollCollapsed = 0 }
                // tvOS has no large titles: every title is inline.
                #if os(tvOS)
                let inline = true
                #else
                let inline = c.navigationItem.largeTitleDisplayMode != .always || barless
                #endif
                if Self.stretched(sv, origin: node.scrollOrigin) {
                    // Pulled past its top, as to refresh: no title's inset.
                } else if inline {
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
                // The route's end with its page unscrolled: scrolling moves the route, not the bars.
                // A route the keyboard shortens can end above the bars, with no bottom cover.
                let end = c.node.convert(c.node.bounds, to: c.contentMount).maxY + (c.contentViewport?.frame.minY ?? 0)
                let reached = end - (c.view.bounds.maxY - safe.bottom)
                let own = Self.routeEdges(safe: safe, edges: whole, reached: reached)
                clearance = own
                // A band riding the bar goes under it, with the scroller below the band: the route is covered nothing
                // at the top (LLP 1131). A collapsing title's scroller, the route's first child in flow, goes under
                // the bar too, and the route keeps its top cover: the kernel pads nothing of it while UIKit insets
                // that scroller (`scroller_insets_top`), and places an absolutely positioned child past it, as a
                // view pinned to the safe-area guide stays below the bar, expanded or collapsed.
                edges = c.ridingBars.isEmpty ? own : HostCover.Edges(top: 0, right: own.right, bottom: own.bottom, left: own.left)
                c.bottomClearance = own.bottom
            } else if case .edges(let e)? = covers[c.node.id] {
                edges = e
            } else { edges = nil }
            // An edge the route turns off (`-exact-safe-area`, LLP 1123 D4) has no
            // clearance: the content root reaches it, and its values say the inset there.
            let cleared = { (e: HostCover.Edges) in
                HostCover.Edges(top: c.ignoresSafeArea("top") ? 0 : e.top, right: c.ignoresSafeArea("right") ? 0 : e.right,
                                bottom: c.ignoresSafeArea("bottom") ? 0 : e.bottom, left: c.ignoresSafeArea("left") ? 0 : e.left)
            }
            edges = edges.map(cleared)
            clearance = clearance.map(cleared)
            if c.ignoresSafeArea("bottom") { c.bottomClearance = 0 }
            // A scroller UIKit insets lays its content out between those
            // insets (LLP 1116.000 D1.2): content that fills its height rests
            // under the bar and above the home indicator, and scrolling
            // content still passes under both. A large title's inset is its
            // widest, at rest (CSS scrollTop 0).
            for node in [c.collapseScroll, c.edgeScroll].compactMap({ $0 }) where node.scrollView != nil {
                if settled, c.viewIfLoaded?.window != nil, let safe = node.scrollView?.safeAreaInsets {
                    // The bottom edge's scroller takes the route's bottom
                    // clearance, which the route then pads nothing of
                    // (`scroller_insets_bottom`) and still moves an
                    // absolutely positioned footer past. Its own safe area
                    // holds none while that padding ends it above the bars:
                    // read alone, it would leave it there for good.
                    let reach = node === c.edgeScroll ? c.bottomClearance : 0
                    // The title's collapsing scroller takes the route's top
                    // clearance the same way (`scroller_insets_top`), and is
                    // reported inset by it until UIKit insets it from under
                    // the bar, or the route would go on padding it below the
                    // bar. A route that pads its top has no scroller under
                    // the bar: one UIKit insets there before that padding
                    // first lands is reported inset by nothing, or the route
                    // would never pad it.
                    let rise = node === c.collapseScroll && c.ridingBars.isEmpty ? edges?.top ?? 0 : 0
                    let padded = c.collapseScroll == nil && c.ridingBars.isEmpty && (edges?.top ?? 0) > 0.5
                    // A band's part of the safe area is a UI inset over control bars, not layout (LLP 1131 D2).
                    let title = node === c.collapseScroll ? node.scrollOrigin : 0
                    let stretched = title > 0 && node.scrollView.map { Self.stretched($0, origin: title) } == true
                    let inset = max(0, max(stretched ? min(safe.top, title) : safe.top, title) - node.pageBand.top)
                    let top = c.ignoresSafeArea("top") || padded ? 0 : inset > 0.5 ? inset : rise
                    let bottom = c.ignoresSafeArea("bottom") ? 0 : max(0, max(safe.bottom, reach) - node.pageBand.bottom)
                    if top > 0 || bottom > 0 { wanted[node.id] = .inset(top: top, bottom: bottom) }
                } else if case .inset? = covers[node.id] { wanted[node.id] = covers[node.id] }
            }
            if settled, let values = ownerValues(of: c, clearance: clearance) { owners[c.node.id] = values }
            else if let held = ownerValues[c.node.id] { owners[c.node.id] = held }
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
        let ownersChanged = owners != ownerValues
        ownerValues = owners
        guard !changes.isEmpty || ownersChanged else { return }
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
        // A callback the runtime waits on can make no runtime call: deliver once it returns
        if Owner.shared.isServingMain {
            DispatchQueue.main.async { [weak self] in self?.deliverCovers() }
            return
        }
        var changes: [(UInt32, HostCover?)] = []
        for (id, cover) in covers where delivered[id] != cover { changes.append((id, cover)) }
        for id in delivered.keys where covers[id] == nil && presenter.views[id] != nil { changes.append((id, nil)) }
        delivered = covers
        var owners: [(UInt32, OwnerValues?)] = []
        for (id, values) in ownerValues where deliveredOwners[id] != values { owners.append((id, values)) }
        for id in deliveredOwners.keys where ownerValues[id] == nil && presenter.views[id] != nil { owners.append((id, nil)) }
        deliveredOwners = ownerValues
        guard !changes.isEmpty || !owners.isEmpty else { return }
        presenter.onCovers?(changes, owners)
    }

    /// Whether the kernel pads `c`'s top by its cover (`reportCovers`): a
    /// route whose title's scroller goes under the bar keeps its top cover,
    /// which the kernel pads nothing of while UIKit insets that scroller.
    func padsTop(_ c: RouteController) -> Bool {
        guard case .edges(let e)? = covers[c.node.id] else { return false }
        return e.top > 0.5 && c.collapseScroll == nil
    }

    /// A route's values as a content owner (LLP 1123 D9): what UIKit reports
    /// for a view at its content root, the route's box less what it clears
    /// (`clearance`). That is its cover, and the top too where a band rides
    /// the bar, which the cover leaves out. A route whose title's scroller,
    /// or a band riding the bar, goes under the bar pads nothing at the top
    /// while its content root still starts at the bar's bottom, as a view
    /// pinned to the safe-area guide does (D3): its top inset stays 0 as the
    /// title collapses, and `cqh` leaves the bar out. Its bottom is the
    /// cover's, at a tab bar's top though a scroller reaches under the bar.
    /// The regions are its controller's, measured from there (D10); the
    /// segments are the parts of the document's that the content root holds:
    /// the window's division moved into the route, read once the document has
    /// it (UIKit's regions trail a hinge update, `ExactView.fit`), and an
    /// agent's `prefer segments` the same way.
    func ownerValues(of c: RouteController, clearance: HostCover.Edges?) -> OwnerValues? {
        guard let view = c.viewIfLoaded, view.window != nil else { return nil }
        let e = clearance ?? .init(top: 0, right: 0, bottom: 0, left: 0)
        let content = c.node.convert(c.node.bounds, to: view).inset(by: UIEdgeInsets(top: e.top, left: e.left, bottom: e.bottom, right: e.right))
        guard content.width > 0, content.height > 0 else { return nil }
        let safe = view.bounds.inset(by: view.safeAreaInsets)
        let inset = [safe.minY - content.minY, content.maxX - safe.maxX, content.maxY - safe.maxY, safe.minX - content.minX].map { max(0, $0) }
        let vp = presenter.viewport, document = presenter.fold
        let parts = (document.cols * document.rows > 1 ? document.rects : []).map {
            vp.convert($0.offsetBy(dx: vp.pageOrigin.x, dy: vp.pageOrigin.y), to: view).intersection(content)
        }
        let fold = ViewportFold.grid(parts.filter { !$0.isNull && $0.width > 0 && $0.height > 0 }.map { $0.offsetBy(dx: -content.minX, dy: -content.minY) })
        return OwnerValues(safe: inset, regions: LayoutRegionsIOS.insets(of: view, viewport: content), fold: fold, size: content.size)
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
            for property in NavigationHost.ownedChanges(scroll, of: node, insetByUIKit: c.collapseScroll === node || c.edgeScroll === node, safeArea: c.safeArea.insets(for: scroll)) { say("route \(c.key)", property) }
        }
    }

    /// What Exact sets on a node's scroll view and nothing else may (LLP
    /// 1075.003 §3.5) and differs from what Exact writes: no inset (a
    /// refresh control insets it while it spins), no automatic adjustment
    /// but a route's collapse or bottom-edge scroller's, the backend's Exact-owned delegate, the node's keyboard dismissal (authored, or
    /// the platform's for what it scrolls, LLP 1115.000 D9.1). The offset
    /// and the content size are Exact's too but are not checked: they move
    /// with the user and with layout, so no last write predicts them. A
    /// route's opted-out edges add their correction (`safeArea`) to the inset.
    static func ownedChanges(_ s: UIScrollView, of node: NodeView, insetByUIKit: Bool, safeArea: UIEdgeInsets = .zero) -> [String] {
        var out: [String] = []
        // A native backend owns layout/footer insets, composed with the
        // refresh control and keyboard; zero is only the plain-scroll baseline.
        if s === node.scroll {
            let band = node.bandInset
            let expected = UIEdgeInsets(top: band.top + safeArea.top, left: band.left + safeArea.left,
                                        bottom: band.bottom + safeArea.bottom, right: band.right + safeArea.right)
            #if os(tvOS)
            if s.contentInset != expected { out.append("contentInset") }
            #else
            if s.contentInset != expected, s.refreshControl?.isRefreshing != true { out.append("contentInset") } // a band's is the host's
            #endif
        }
        if s.contentInsetAdjustmentBehavior != (insetByUIKit ? .automatic : .never) { out.append("contentInsetAdjustmentBehavior") }
        if s !== node.scrollView || !node.ownsScrollDelegate { out.append("delegate") }
        if s.keyboardDismissMode != node.keyboardDismissal { out.append("keyboardDismissMode") }
        return out
    }
}
#endif

// @ref LLP 1075.003 §3.5, §3.7 — Exact's navigation bar on iOS, one per
// stack and fixed (titles that do not collapse): the header-shaped route's
// projection, the content area the bar leaves (`HostCover`), the delegate
// slot Exact keeps and forwards, the authored elements a hook acts on, and
// the development check of what Exact owns. When the hooks run is
// NavigationIOS.swift's; the module side is ExactNativeModule.swift.
#if os(iOS)
import UIKit

/// A route whose first child is a `header` holding exactly one heading
/// (LLP 1035.001 D9's header-shaped route). The bar shows the heading as
/// its title — large for a level-1 heading, inline for any other — and the
/// header's buttons as its items, those before the heading leading and
/// those after it trailing, each pressing its authored button. The stack's
/// Back control is UIKit's back button, never an item. iOS does not paint
/// the header; the web and the agent do (LLP 1021 D4's one presentation).
struct HeaderShape: Equatable {
    struct Item: Equatable {
        let id: UInt32, title: String, symbol: String?, label: String?, disabled: Bool
        /// A face drawn from the button's one filled box (an avatar or a
        /// badge: a shape holding a text or a symbol), when the button has
        /// one: the item's image, in the box's own colours and corners.
        let badge: BadgeFace?
        /// The popover a button opens (`popovertarget`): the item's menu,
        /// its rows read when it opens, as LLP 1021 D3's pull-down.
        let menu: String?
        /// A native button's prominent style: a prominent bar item (iOS 26),
        /// as LLP 1069.011.000 D2 maps `NSToolbarItem`.
        let prominent: Bool
        init(_ button: NodeView) {
            var symbol: String?, text = ""
            func walk(_ node: NodeView) {
                for case let child as NodeView in node.container.subviews {
                    if child.kind == "image", let name = child.props["symbolName"], !name.isEmpty { symbol = symbol ?? name }
                    else if child.isParagraph, text.isEmpty { text = child.accessibleText }
                    else { walk(child) }
                }
            }
            // A native button's children are its face, not views (LLP 1069.011.000 D1).
            let face = button.isNativeButton ? button.face : nil
            if let face {
                symbol = face.symbol
                text = face.title ?? ""
            } else {
                walk(button)
            }
            id = button.id
            badge = button.isNativeButton ? nil : BadgeFace(button)
            menu = button.props["popovertargetaction"] == "hide" ? nil : button.props["popovertarget"]
            self.symbol = symbol
            title = text
            label = button.props["accessibilityLabel"] ?? face?.label
            disabled = button.disabled
            prominent = ["filled", "bordered-prominent", "prominent-glass", "prominent-clear-glass"].contains(face?.style ?? "")
        }
        /// Everything a bar item is made from.
        var source: String { "\(id):\(title):\(symbol ?? ""):\(label ?? ""):\(disabled):\(prominent):\(badge?.source ?? ""):\(menu ?? "")" }
    }
    let header: NodeView
    let title: String
    let level: Int
    let leading: [Item], trailing: [Item]

    /// `back` names the stack's Back control, left to UIKit's back button
    /// when there is one (`backIsUIKits`): the root of a presented stack
    /// has none, so there it is an item like the others.
    init?(route: NodeView, back: String?, backIsUIKits: Bool = true) {
        guard let header = route.container.subviews.lazy.compactMap({ $0 as? NodeView }).first,
              header.props["semanticTag"] == "header" else { return nil }
        var headings: [NodeView] = [], before: [Item] = [], after: [Item] = []
        func walk(_ node: NodeView) {
            for case let child as NodeView in node.container.subviews {
                if child.isParagraph, child.props["accessibilityHeadingLevel"] != nil {
                    headings.append(child)
                } else if child.handlers.contains("press") || (child.isButton && child.props["popovertarget"] != nil && child.props["popovertargetaction"] != "hide") {
                    guard !backIsUIKits || back == nil || child.props["id"] != back else { continue }
                    if headings.isEmpty { before.append(Item(child)) } else { after.append(Item(child)) }
                } else {
                    walk(child)
                }
            }
        }
        walk(header)
        guard headings.count == 1 else { return nil }
        self.header = header
        title = headings[0].accessibleText
        level = Int(headings[0].props["accessibilityHeadingLevel"] ?? "") ?? 2
        leading = before
        trailing = after
    }

    static func == (a: HeaderShape, b: HeaderShape) -> Bool {
        a.header === b.header && a.title == b.title && a.level == b.level && a.leading == b.leading && a.trailing == b.trailing
    }
}

/// A bar item's face drawn from a button whose one child is a filled box
/// holding a text or a symbol (an avatar, a badge): CSS's colours and
/// corners at the bar's image size, light and dark, as UIKit draws a
/// raster item image (`.alwaysOriginal`).
struct BadgeFace: Equatable {
    static let size: CGFloat = 36
    let text: String, symbol: String?
    let light: [[Double]], dark: [[Double]]
    let corners: [CGSize]
    init?(_ button: NodeView) {
        let kids = button.container.subviews.compactMap { $0 as? NodeView }
        guard kids.count == 1, let box = kids.first, box.channels("background_color") != nil else { return nil }
        var text = "", symbol: String?, ink: NodeView?
        func walk(_ node: NodeView) {
            for case let child as NodeView in node.container.subviews {
                if child.kind == "image", let name = child.props["symbolName"], !name.isEmpty { symbol = symbol ?? name; ink = ink ?? child }
                else if child.isParagraph, text.isEmpty { text = child.accessibleText; ink = ink ?? child }
                else { walk(child) }
            }
        }
        walk(box)
        guard !text.isEmpty || symbol != nil else { return nil }
        let key = symbol == nil ? "text_color" : "tint_color"
        func colours(_ dark: Bool) -> [[Double]] {
            [box.channels("background_color", dark: dark) ?? [0, 0, 0, 0], ink?.channels(key, dark: dark) ?? (dark ? [1, 1, 1, 1] : [0, 0, 0, 1])]
        }
        self.text = text; self.symbol = symbol
        light = colours(false); dark = colours(true)
        corners = box.cornerSizes(in: CGRect(x: 0, y: 0, width: Self.size, height: Self.size))
    }
    var source: String { "\(text)|\(symbol ?? "")|\(light)|\(dark)|\(corners)" }

    var image: UIImage {
        let light = draw(self.light).withRenderingMode(.alwaysOriginal)
        light.imageAsset?.register(draw(self.dark).withRenderingMode(.alwaysOriginal), with: UITraitCollection(userInterfaceStyle: .dark))
        return light
    }
    private func draw(_ c: [[Double]]) -> UIImage {
        let rect = CGRect(x: 0, y: 0, width: Self.size, height: Self.size)
        return UIGraphicsImageRenderer(size: rect.size).image { _ in
            TextEngine.color(c[0]).setFill()
            UIBezierPath(cgPath: BorderPaint.roundedRect(rect, corners, shape: nil)).fill()
            let ink = TextEngine.color(c[1])
            if let symbol, let glyph = UIImage(systemName: symbol, withConfiguration: UIImage.SymbolConfiguration(pointSize: Self.size * 0.42))?.withTintColor(ink, renderingMode: .alwaysOriginal) {
                glyph.draw(at: CGPoint(x: (rect.width - glyph.size.width) / 2, y: (rect.height - glyph.size.height) / 2))
            } else {
                let attrs: [NSAttributedString.Key: Any] = [.font: UIFont.systemFont(ofSize: Self.size * 0.42, weight: .medium), .foregroundColor: ink]
                let s = (text as NSString).size(withAttributes: attrs)
                (text as NSString).draw(at: CGPoint(x: (rect.width - s.width) / 2, y: (rect.height - s.height) / 2), withAttributes: attrs)
            }
        }
    }
}

/// The delegate Exact gives each navigation controller it builds: Exact's
/// own handling first (pop reconciliation, LLP 1035.001 D1, D2), then the
/// app's delegate, which receives straight anything Exact does not
/// implement — a custom transition's animator (LLP 1075.003 §3.5).
final class NavigationDelegateProxy: NSObject, UINavigationControllerDelegate {
    weak var host: NavigationHost?
    weak var app: UINavigationControllerDelegate?

    func navigationController(_ nav: UINavigationController, willShow controller: UIViewController, animated: Bool) {
        host?.navigationController(nav, willShow: controller, animated: animated)
        app?.navigationController?(nav, willShow: controller, animated: animated)
    }

    func navigationController(_ nav: UINavigationController, didShow controller: UIViewController, animated: Bool) {
        host?.navigationController(nav, didShow: controller, animated: animated)
        app?.navigationController?(nav, didShow: controller, animated: animated)
    }

    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || (app?.responds(to: selector) ?? false)
    }

    override func forwardingTarget(for selector: Selector!) -> Any? {
        app?.responds(to: selector) == true ? app : nil
    }
}

/// What a projected bar item presses: its authored button, as a tap would.
final class BarPress: NSObject {
    let id: UInt32
    weak var host: NavigationHost?
    init(_ id: UInt32, _ host: NavigationHost) { self.id = id; self.host = host }
    @objc func press() { _ = host?.act(id, 0) }
}

/// One stack Exact built: its delegate, whether it shows the bar (LLP 1037
/// F1: for its whole life), and what the development check last saw.
final class NavigationStack {
    let proxy = NavigationDelegateProxy()
    var showsBar: Bool
    let order: Int
    let label: String
    var hooked = false
    var written: [ObjectIdentifier] = []
    init(showsBar: Bool, order: Int) {
        self.showsBar = showsBar
        self.order = order
        label = "#\(order)"
    }
}

extension NavigationHost {
    /// Whether a stack's bar shows: the stack's choice, except under the
    /// agent, where the authored header paints (LLP 1075.003 §3.4).
    func barShows(_ nav: UINavigationController) -> Bool {
        !ExactEnv.agentMode && stacks[ObjectIdentifier(nav)]?.showsBar == true
    }

    /// A navigation controller for a stack whose first route is `first`:
    /// Exact's delegate, the bar's visibility from the plan, then the
    /// `navigation` hook (LLP 1075.003 Q3 (c)). Large titles are on, and each
    /// route's item says whether its title is large, so a level-1 heading
    /// pushed over an inline one is large; `prefersLargeTitles` is the app's
    /// from the hook on (§3.5).
    func makeNavigation(first: NodeView?) -> UINavigationController {
        let nav = UINavigationController()
        let shape = first.flatMap { HeaderShape(route: $0, back: container?.props["navigationBack"]) }
        stackCount += 1
        let stack = NavigationStack(showsBar: shape != nil, order: stackCount)
        stack.proxy.host = self
        stacks[ObjectIdentifier(nav)] = stack
        nav.delegate = stack.proxy
        nav.navigationBar.prefersLargeTitles = true
        if presenter.session?.natives.hooksConnected == true {
            stack.showsBar = presenter.session?.natives.navigationHook(nav, built: true, showsBar: stack.showsBar, label: stack.label) ?? stack.showsBar
            stack.hooked = true
        }
        nav.setNavigationBarHidden(!barShows(nav), animated: false)
        return nav
    }

    /// The `navigation` hook for a stack built before the module connected
    /// (a cold launch's), before any of its routes' hooks run. A changed
    /// `showsBar` moves the content once, journaled (LLP 1075.003 Q3 (c)).
    func hookNavigation(_ nav: UINavigationController) {
        guard let natives = presenter.session?.natives, natives.hooksConnected,
              let stack = stacks[ObjectIdentifier(nav)], !stack.hooked else { return }
        let before = barShows(nav)
        stack.showsBar = natives.navigationHook(nav, built: true, showsBar: stack.showsBar, label: stack.label)
        stack.hooked = true
        guard barShows(nav) != before else { return }
        nav.setNavigationBarHidden(!barShows(nav), animated: false)
        for case let c as RouteController in nav.viewControllers { c.projectedSource = nil }
        presenter.session?.log("hook navigation \(stack.label): showsBar changed after the first frame; the content moves once")
    }

    /// The navigation controller holding `controller`, among Exact's.
    func stackController(of controller: UIViewController) -> UINavigationController? {
        allNavigations.first { $0.viewControllers.contains(controller) }
    }

    /// Project each route of a stack into its navigation item when its
    /// authored source changed, then run the route hook — before UIKit lays
    /// the stack out (LLP 1075.003 §3.2, §3.5 "projected defaults").
    func prepareRoutes(_ routes: [RouteController], in nav: UINavigationController) {
        hookNavigation(nav)
        projectBack(routes, in: nav)
        for (index, c) in routes.enumerated() {
            let shows = barShows(nav)
            let back = container?.props["navigationBack"]
            let shape = shows ? HeaderShape(route: c.node, back: back, backIsUIKits: index > 0) : nil
            let canGoBack = index > 0 && canInvokeBack
            let scroll = contentScroll(of: c)
            let dataset = c.node.props["dataset"]
            let source = "\(shape.map { "\($0.header.id)|\($0.title)|\($0.level)|\($0.leading.map(\.source))|\($0.trailing.map(\.source))" } ?? "-")|\(canGoBack)"
            // The hook runs again after anything Exact wrote to the item (the
            // Back control the route above gives it, too) and when the route
            // moves to another stack (a root whose tabs changed).
            let signature = "\(source)|\(c.backSource ?? "")|\(ObjectIdentifier(nav))|\(dataset ?? "")|\(scroll.map { "\(ObjectIdentifier($0))" } ?? "-")"
            if c.projectedSource != source {
                c.projectedSource = source
                project(shape, into: c, canGoBack: canGoBack, shows: shows)
            }
            collapse(c, shape: shape, scroll: scroll)
            guard c.projected != signature || !c.hooked else { continue }
            c.projected = signature
            guard presenter.session?.natives.hooksConnected == true else { continue }
            presenter.session?.natives.routeHook(c.hooked ? .changed : .built, controller: c, navigation: nav, scroll: scroll,
                                                 key: c.key, dataset: dataset)
            c.hooked = true
            if let scroll { c.ownedScroll = element(named: c.node.props["navigationScroll"] ?? "", in: c.node).flatMap { $0.scroll === scroll ? $0 : nil } }
        }
    }

    /// Write the defaults an authored header gives a navigation item, and
    /// lift the header out of the route's layout while the bar shows it.
    private func project(_ shape: HeaderShape?, into c: RouteController, canGoBack: Bool, shows: Bool) {
        let item = c.navigationItem
        if let lifted = c.lifted, lifted !== shape?.header {
            lifted.isHidden = false
            c.lifted = nil
        }
        guard shows else { return }
        item.title = shape?.title
        item.largeTitleDisplayMode = shape?.level == 1 ? .always : .never
        item.hidesBackButton = !canGoBack
        item.leftItemsSupplementBackButton = true
        c.barPresses = []
        item.leftBarButtonItems = shape?.leading.map { barItem($0, c) }
        item.rightBarButtonItems = shape?.trailing.reversed().map { barItem($0, c) }
        if let header = shape?.header {
            header.isHidden = true
            c.lifted = header
        }
    }

    /// UIKit's back button stands for each route's authored Back control,
    /// shaped as it is (LLP 1075.003, James's review): a symbol alone is the
    /// chevron alone (`.minimal`); text is the button's title. UIKit reads
    /// both from the item beneath, which is where they are written, on change.
    private func projectBack(_ routes: [RouteController], in nav: UINavigationController) {
        guard barShows(nav), routes.count > 1 else { return }
        let name = container?.props["navigationBack"]
        for index in 1..<routes.count {
            let control = name.flatMap { element(named: $0, in: routes[index].node) }
            let shape = control.map { HeaderShape.Item($0) }
            let source = shape.map { "\($0.symbol ?? "")|\($0.title)" } ?? "-"
            let below = routes[index - 1]
            guard below.backSource != source else { continue }
            below.backSource = source
            let item = below.navigationItem
            switch shape {
            case let s? where s.title.isEmpty: item.backButtonDisplayMode = .minimal; item.backButtonTitle = nil
            case let s?: item.backButtonDisplayMode = .default; item.backButtonTitle = s.title
            case nil: item.backButtonDisplayMode = .default; item.backButtonTitle = nil
            }
        }
    }

    /// Stage 3 (LLP 1075.003 §3.7): a large title collapses with its route's
    /// content scroll view when that scroller comes right after the header
    /// the bar replaces. The scroller then goes under the bar, UIKit insets it
    /// and follows its offset, and CSS `scrollTop` is measured from the
    /// expanded title's inset (`scrollOrigin`), so an authored offset lands
    /// where the browser's does while the bar's height changes.
    private func collapse(_ c: RouteController, shape: HeaderShape?, scroll: UIScrollView?) {
        let kids = c.node.container.subviews.compactMap { $0 as? NodeView }
        let node = kids.firstIndex { $0 === shape?.header }.flatMap { kids.indices.contains($0 + 1) ? kids[$0 + 1] : nil }
        let target = shape?.level == 1 && node?.scroll != nil && node?.scroll === scroll ? node : nil
        guard c.collapseScroll !== target else { return }
        if let old = c.collapseScroll, let sv = old.scroll {
            let top = sv.contentOffset.y + old.scrollTopInset(sv)
            sv.contentInsetAdjustmentBehavior = .never
            old.scrollOrigin = 0
            sv.contentOffset.y = top
        }
        c.collapseScroll = target
        if let sv = target?.scroll { sv.contentInsetAdjustmentBehavior = .always }
        c.setContentScrollView(target?.scroll, for: .top)
        presenter.session?.log("navigation: route \(c.key) \(target == nil ? "keeps its title still" : "collapses its title with its scroller")")
    }

    private func barItem(_ i: HeaderShape.Item, _ c: RouteController) -> UIBarButtonItem {
        let press = BarPress(i.id, self)
        c.barPresses.append(press)
        let action = #selector(BarPress.press)
        let image = i.badge?.image ?? i.symbol.flatMap { UIImage(systemName: $0) }
        let item: UIBarButtonItem
        if let name = i.menu {
            // A pull-down: UIKit opens it on tap; the rows are the popover's,
            // read as it opens, after the button's own press (both fire, as
            // LLP 1021 D1 has it on the web).
            let id = i.id
            let deferred = UIDeferredMenuElement.uncached { [weak self] completion in
                guard let self else { return completion([]) }
                if self.presenter.views[id]?.handlers.contains("press") == true { self.presenter.press(id) }
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) { [weak self] in
                    guard let self, let pop = self.presenter.carrying("popover").first(where: { $0.props["id"] == name }) else { return completion([]) }
                    completion(self.presenter.menus.items(of: pop))
                }
            }
            item = image.map { UIBarButtonItem(image: $0, menu: UIMenu(children: [deferred])) }
                ?? UIBarButtonItem(title: i.title, menu: UIMenu(children: [deferred]))
        } else {
            item = image.map { UIBarButtonItem(image: $0, style: .plain, target: press, action: action) }
                ?? UIBarButtonItem(title: i.title, style: .plain, target: press, action: action)
        }
        item.accessibilityLabel = i.label ?? (i.title.isEmpty ? nil : i.title)
        item.isEnabled = !i.disabled
        if #available(iOS 26.0, *) {
            if i.prominent { item.style = .prominent }
            // A drawn face is its own shape: no glass capsule around it.
            if i.badge != nil { item.hidesSharedBackground = true }
        }
        return item
    }

    /// The scroll view a route names with `navigationScroll`, resolved
    /// inside it by HTML id as its Back control is (E7: nothing searches).
    func contentScroll(of c: RouteController) -> UIScrollView? {
        guard let name = c.node.props["navigationScroll"], !name.isEmpty else { return nil }
        return element(named: name, in: c.node)?.scroll
    }

    private func element(named name: String, in route: NodeView) -> NodeView? {
        presenter.carrying("id").first { $0.props["id"] == name && ($0 === route || $0.isDescendant(of: route)) }
    }

    /// The live node a route (by key) holds under an HTML id: a hook's
    /// `route.element(id)` (LLP 1075.003 §3.4).
    func resolve(route key: String, id: String) -> NodeView? {
        guard let route = controllers.values.first(where: { $0.key == key && presenter.views[$0.node.id] === $0.node })?.node else { return nil }
        return element(named: id, in: route)
    }

    /// A hook's act on an authored element, as the DOM's: `click()` presses
    /// it as a tap does, `focus()` and `blur()` follow the focus rules. Each
    /// waits until the batch being applied is done. False when refused.
    func act(_ id: UInt32, _ action: UInt32) -> Bool {
        guard let node = presenter.views[id] else { return false }
        switch action {
        case 0:
            guard node.handlers.contains("press"), !node.disabled else { return false }
            // Still the node it was when asked: a reload restarts node ids.
            presenter.afterBatch { [weak presenter = self.presenter, weak node] in
                if let presenter, let node, presenter.views[id] === node { presenter.press(id) }
            }
        case 1: presenter.afterBatch { [weak presenter = self.presenter, weak node] in if let node { presenter?.focusNode(node) } }
        case 2: presenter.afterBatch { [weak node] in if let node { _ = (node.textArea ?? node.field ?? node).resignFirstResponder() } }
        default: return false
        }
        return true
    }

    /// The app's delegate for a controller whose own slot Exact keeps.
    /// UIKit reads which methods a delegate answers when it is set, so the
    /// proxy is set again.
    func setAppDelegate(_ controller: AnyObject, _ delegate: AnyObject?) {
        if let tabs = controller as? UITabBarController, tabs === tabController {
            tabProxy.app = delegate as? UITabBarControllerDelegate
            tabs.delegate = nil
            tabs.delegate = tabProxy
            presenter.session?.log("hook tabs: delegate \(delegate.map { "\(type(of: $0))" } ?? "cleared")")
            return
        }
        guard let nav = controller as? UINavigationController, let stack = stacks[ObjectIdentifier(nav)] else { return }
        stack.proxy.app = delegate as? UINavigationControllerDelegate
        nav.delegate = nil
        nav.delegate = stack.proxy
        presenter.session?.log("hook navigation \(stack.label): delegate \(delegate.map { "\(type(of: $0))" } ?? "cleared")")
    }

    // MARK: The content area (LLP 1075.003 §3.5)

    /// Whether a native container shows its bars, so the session's view
    /// takes the whole of its own (ExactViewIOS `fit`).
    var wantsWholeView: Bool {
        (tabOwner != nil && !ExactEnv.agentMode) || allNavigations.contains(where: barShows)
    }

    /// What Exact's containers cover of each route — its controller's safe
    /// area past the insets the kernel already has as `env()`: a shown bar's
    /// top, and every edge while the containers take the whole view for a
    /// page that did not ask to cover it — and the header a bar replaces,
    /// reported when they change, after the batch being applied.
    func reportCovers() {
        var changes: [(UInt32, HostCover?)] = []
        var wanted: [UInt32: HostCover] = [:]
        let whole = wantsWholeView && presenter.viewportFit != "cover"
        for c in controllers.values where presenter.views[c.node.id] === c.node {
            guard let nav = c.navigationController, nav.viewControllers.contains(c) else { continue }
            let shows = barShows(nav)
            guard shows || whole else { continue }
            if let header = c.lifted { wanted[header.id] = .whole }
            let edges: HostCover.Edges
            // A collapsing title's scroller goes under the bar, which insets it;
            // its expanded inset is where CSS scrollTop 0 rests.
            if let node = c.collapseScroll, let sv = node.scroll, sv.adjustedContentInset.top > 0 {
                let inset = sv.adjustedContentInset.top
                if inset > node.scrollOrigin { node.scrollOrigin = inset }
                if node.scrollCollapsed == 0 || inset < node.scrollCollapsed { node.scrollCollapsed = inset }
            }
            if c.viewIfLoaded?.window != nil {
                let safe = c.view.safeAreaInsets, env = presenter.insets
                let top = c.collapseScroll == nil ? max(0, safe.top - env.top) : 0
                edges = .init(top: top, right: whole ? max(0, safe.right - env.right) : 0,
                              bottom: whole ? max(0, safe.bottom - env.bottom) : 0, left: whole ? max(0, safe.left - env.left) : 0)
            } else if case .edges(let e)? = covers[c.node.id] {
                edges = e
            } else { continue }
            if edges != .init(top: 0, right: 0, bottom: 0, left: 0) { wanted[c.node.id] = .edges(edges) }
        }
        // The tablist whose place the tab bar takes takes no room.
        if let list = adoptedTablist, tabOwner != nil, !ExactEnv.agentMode { wanted[list] = .whole }
        for (id, cover) in wanted where covers[id] != cover { changes.append((id, cover)) }
        for id in covers.keys where wanted[id] == nil && presenter.views[id] != nil { changes.append((id, nil)) }
        covers = wanted
        guard !changes.isEmpty else { return }
        // Under Q3 (c) a cold launch's first frame runs no app code: a hook
        // that changes a bar the plan already showed moves the content once.
        if replayingHooks, changes.contains(where: { presenter.views[$0.0]?.props["navigationKey"] != nil }) {
            presenter.session?.log("hook: the content area moved after the first frame, by a hook run at launch")
        }
        // Never inside a batch: the stack is installed partway through the
        // first one, whose remaining frames would overwrite the covered ones.
        presenter.afterBatch { [weak presenter = self.presenter] in presenter?.onCovers?(changes) }
    }

    // MARK: The development check (LLP 1075.003 §3.5)

    /// Whether this build checks what Exact owns: every build but a
    /// production bake.
    static let checksOwnership: Bool = {
        let trust = ((GpuModule.bakedCompatibility["inputs"] as? [String: Any])?["trust"] as? String) ?? "development"
        return trust != "production"
    }()

    /// Record the controllers Exact set on a stack.
    func recordOwned(_ nav: UINavigationController) {
        guard NavigationHost.checksOwnership else { return }
        stacks[ObjectIdentifier(nav)]?.written = nav.viewControllers.map(ObjectIdentifier.init)
    }

    /// UIKit completed a transition (`didShow`): a pop the user made (UIKit's
    /// back button, the edge swipe) leaves a prefix of what Exact wrote, and
    /// is recorded; any other change is not Exact's and stays to be reported.
    func recordPop(_ nav: UINavigationController) {
        guard NavigationHost.checksOwnership, let stack = stacks[ObjectIdentifier(nav)] else { return }
        let now = nav.viewControllers.map(ObjectIdentifier.init)
        if now.count <= stack.written.count, Array(stack.written.prefix(now.count)) == now { stack.written = now }
    }

    /// Before each batch: compare what Exact owns on each hooked object with
    /// what Exact last wrote, and journal a difference once, by name. It is
    /// detection, not enforcement; a change made and undone between two
    /// batches is not seen.
    func checkOwned() {
        guard NavigationHost.checksOwnership, !changing, !syncing, presenter.session?.natives.hooksConnected == true else { return }
        func say(_ what: String, _ property: String) {
            let line = "\(what): \(property) changed outside Exact, which owns it"
            if ownedReported.insert(line).inserted { presenter.session?.log(line) }
        }
        for nav in allNavigations {
            guard let stack = stacks[ObjectIdentifier(nav)], stack.hooked else { continue }
            if nav.delegate !== stack.proxy { say("navigation \(stack.label)", "delegate") }
            if nav.isNavigationBarHidden == barShows(nav) { say("navigation \(stack.label)", "navigation bar visibility") }
            if nav.viewControllers.map(ObjectIdentifier.init) != stack.written { say("navigation \(stack.label)", "viewControllers") }
            if let pop = nav.interactivePopGestureRecognizer, pop.delegate !== self { say("navigation \(stack.label)", "the pop gesture's delegate") }
            if #available(iOS 26.0, *), let pop = nav.interactiveContentPopGestureRecognizer, pop.delegate !== self {
                say("navigation \(stack.label)", "the content pop gesture's delegate")
            }
        }
        for c in controllers.values where c.hooked && presenter.views[c.node.id] === c.node {
            if c.navigationController != nil, c.isViewLoaded, c.node.superview !== c.view { say("route \(c.key)", "view") }
            guard let node = c.ownedScroll, let scroll = node.scroll else { continue }
            for property in NavigationHost.ownedChanges(scroll, of: node, collapsing: c.collapseScroll === node) { say("route \(c.key)", property) }
        }
    }

    /// What Exact sets on a node's scroll view and nothing else may (LLP
    /// 1075.003 §3.5) and differs from what Exact writes: no inset (a
    /// refresh control insets it while it spins), no automatic adjustment,
    /// the node as its delegate, the authored keyboard dismissal. The offset
    /// and the content size are Exact's too but are not checked: they move
    /// with the user and with layout, so no last write predicts them.
    static func ownedChanges(_ s: UIScrollView, of node: NodeView, collapsing: Bool) -> [String] {
        var out: [String] = []
        if s.contentInset != .zero, s.refreshControl?.isRefreshing != true { out.append("contentInset") }
        if s.contentInsetAdjustmentBehavior != (collapsing ? .always : .never) { out.append("contentInsetAdjustmentBehavior") }
        if s.delegate !== node { out.append("delegate") }
        let dismiss: UIScrollView.KeyboardDismissMode = switch node.props["keyboardDismissMode"] {
        case "interactive": .interactive
        case "on-drag": .onDrag
        default: .none
        }
        if s.keyboardDismissMode != dismiss { out.append("keyboardDismissMode") }
        return out
    }
}
#endif

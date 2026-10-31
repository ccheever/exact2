// @ref LLP 1008 §9 — Contract routes projected into UIKit's navigation
// controller. UIKit owns recognition, arbitration, progress and cancellation.
// Only a completed pop invokes the Contract back control. The bar a stack
// shows, what a route projects into it and when the app module's hatches run
// are LLP 1075.003's (NavigationBarIOS.swift, NativeHatches.swift).
#if os(iOS) || os(tvOS)
import UIKit

final class RouteController: UIViewController, UIScrollViewDelegate {
    let node: NodeView
    var key: String { node.props["navigationKey"] ?? "" }
    fileprivate var usesPageViewport = false
    private(set) var contentViewport: Viewport?
    private let keyboardInset = KeyboardInset()
    var contentMount: UIView { contentViewport ?? view }
    private var viewportKeyboardDismissal: UIScrollView.KeyboardDismissMode { node.props["keyboardDismissMode"] == nil ? .interactive : node.keyboardDismissal }
    /// What Exact last projected into the navigation item, and that plus
    /// what the route hatch last saw; whether the hatch has run; the header
    /// lifted into the bar; the content scroll view a hatch was handed.
    var projectedSource: BarSource?, projected: HatchSource?, backSource: String?
    /// `projectedSource` with every item's enabling left out: an item that is only enabled or disabled changes
    /// in place, on the item kept for it (`NavigationHost.enable`, `barItemsMade`).
    var projectedFaces: BarSource?
    var hatched = false
    /// Whether a route of its stack is beneath it, to go back to: written
    /// as the stack's routes are prepared, before UIKit shows any of them
    /// (`NavigationRules.bar`).
    var pushed = false
    weak var lifted: NodeView?
    var barNodes: [NodeView] = []
    /// The filter tablist the search bar shows as its scope bar (LLP 1134).
    weak var scoped: NodeView?
    #if EXACT_INSPECT
    /// The header the bar shows the title of, and the node each bar item
    /// stands for (inspect mode selects them, InspectIOS.swift).
    weak var shownHeader: NodeView?
    var itemNodes: [ObjectIdentifier: UInt32] = [:]
    #endif
    weak var host: NavigationHost?
    weak var ownedScroll: NodeView?
    /// The scroller a large title collapses with (LLP 1075.003 Stage 3).
    weak var collapseScroll: NodeView?
    /// The physical scroller bound to UIKit; a node can replace its backend.
    var collapseScrollView: UIScrollView?
    /// A scroller drawn through control bars at the top or the foot (LLP 1131 D2), the content scroll view there.
    weak var bandedTop: UIScrollView?, bandedBottom: UIScrollView?
    /// Its insets were last taken with no bar shown (LLP 1130 D3), so a title's are measured afresh.
    var insetsBarless = false
    var ridingBars: [NodeView] = []
    func rideBars() {
        let offset = node.convert(CGPoint(x: 0, y: view.safeAreaInsets.top), from: view).y
        for bar in ridingBars where abs(bar.controlRide - offset) > 0.001 {
            bar.controlRide = offset
            bar.applyTransform()
        }
    }
    /// The scroller that is the route's last in-flow child, which reaches
    /// the screen's bottom edge (LLP 1116 D2).
    weak var edgeScroll: NodeView?
    /// The bottom cover the route has with no such scroller (`reportCovers`).
    var bottomClearance: CGFloat = 0
    /// The targets of the bar items projected from its header.
    var barPresses: [BarPress] = []
    /// The bar items last made, by node: one whose kind is unchanged is
    /// updated in place, so UIKit's glass animates the change (`barItemMade`).
    var barItemsMade: [UInt32: (item: HeaderShape.Item, bar: UIBarButtonItem, press: BarPress, menuFrom: UInt64?)] = [:]
    /// Whether the bar has been projected: the items of a later projection animate where it shows.
    var barProjected = false
    /// The space before each group, by the item that follows it.
    var barSpaces: [UInt32: UIBarButtonItem] = [:]
    /// The bar items of its header's readouts, by node, kept across
    /// projections so a readout's view changes in place (§9.15).
    var readouts: [UInt32: UIBarButtonItem] = [:]
    /// The header's search field as UIKit's search controller (§9.6).
    var search: HeaderSearch?
    /// The title Exact drew from the heading's group, and the subtitle it
    /// wrote; whether the root's tablist was hidden when it last looked (§9.10).
    let titleInk = NavigationTitleInk()
    var titleView: HeaderTitleView?, subtitle: String?, tablistHidden: Bool?
    /// The route appeared, first or again after a cancelled swipe: a zoom
    /// push's landing hears it (ZoomPushIOS).
    var appeared: (() -> Void)?
    init(_ node: NodeView) {
        self.node = node
        super.init(nibName: nil, bundle: nil)
        #if os(iOS)
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (c: RouteController, _: UITraitCollection) in c.reopenTitle() }
        #endif
    }
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        appeared?()
    }
    required init?(coder: NSCoder) { nil }
    // The bar's height changed (a push to an inline title, a rotation, a
    // hatch): the route's content area follows (LLP 1075.003 §3.5).
    override func viewSafeAreaInsetsDidChange() {
        super.viewSafeAreaInsetsDidChange()
        rideBars()
        host?.coversChanged()
    }
    // Back on a screen whose list pushed the one leaving: the row that was
    // tapped fades with the transition (LLP 1115.000 D11.3).
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        host?.presenter.groupedLists?.appearing(node, alongside: animated ? transitionCoordinator : nil)
        host?.presenter.rowCells.appearing(node, alongside: animated ? transitionCoordinator : nil)
    }
    override func viewIsAppearing(_ animated: Bool) {
        super.viewIsAppearing(animated)
        // Its block, at the size UIKit gives it, before its first frame (LLP 1123 D2)
        host?.presenter.owners.publish()
        // UIKit has attached and sized the real destination. Finish its
        // content and focus here, before the transition's first frame.
        host?.presenter.afterBatch { [weak self] in
            guard let host = self?.host else { return }
            host.coversChanged()
            if let pending = host.presenter.pendingFocusNode, host.isArriving(pending) {
                host.presenter.flushPendingFocus()
            }
            host.presenter.syncAccessibility()
            host.presenter.paintVisibleText()
        }
    }

    // Its route is laid out in this view's bounds (LLP 1123 D2).
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        fitContentViewport()
        host?.presenter.owners.publish()
        #if os(iOS)
        search?.labelScopes()
        #endif
    }

    override func loadView() {
        view = PresentationMount()
        paintBackdrop()
        view.addSubview(node)
        host?.presenter.session?.natives.routeLoaded(node)
    }

    /// The backdrop's colours last written, light and dark.
    private var backdropKey: [[Double]?]?
    /// The controller's view paints what shows behind the route: a dynamic
    /// colour resolved for the controller's current appearance, re-resolved
    /// by UIKit when it changes (a route loaded before its window has a trait
    /// collection would otherwise keep the light colour in dark mode), and
    /// written again when a batch changes it. A pushed route shows its
    /// backdrop (`backdrop`); a route under no background shows the system
    /// background, not white. A sheet is the route's own background where it
    /// has one, as a grouped sheet's surface is the grouped background (LLP
    /// 1116.021 D7, amending LLP 1115 D2), so no white band shows where the
    /// route stops above a keyboard's bar; else clear, over the surface its
    /// presentation paints (`ModalController.paintSurface`: UIKit's glass at
    /// a partial detent, LLP 1141.001 D3). What is behind the sheet is not,
    /// and dimming belongs to UIKit's presentation.
    func paintBackdrop() {
        guard isViewLoaded else { return }
        let modal = node.props["navigationPresentation"] == "modal"
        let key = modal
            ? [ownBackdrop(dark: false), ownBackdrop(dark: true)]
            : [backdrop(dark: false), backdrop(dark: true)]
        guard view.backgroundColor == nil || key != backdropKey else { return }
        backdropKey = key
        #if os(tvOS)
        // tvOS has no system backgrounds; white stands in, as the viewport's.
        let system = UIColor.white
        #else
        // Where nothing is painted, the platform's surface (LLP 1115 D2).
        let system = UIColor.platformBackground(in: host?.presenter.root ?? view)
        #endif
        view.backgroundColor = UIColor { [weak self] traits in
            let dark = traits.userInterfaceStyle == .dark
            let colour = modal ? self?.ownBackdrop(dark: dark) : self?.backdrop(dark: dark)
            return colour.map { TextEngine.color($0) } ?? (modal ? .clear : system)
        }
    }

    /// The route's own background, if it paints one.
    private func ownBackdrop(dark: Bool) -> [Double]? {
        if node.groundIsSheets { return nil }
        // As written, never greyed: the view keeps the colour its provider gave (TintDimmingIOS.swift)
        return node.undimmedChannels("background_color", dark: dark).flatMap { $0.count < 4 || $0[3] > 0 ? $0 : nil }
    }

    /// What shows behind the route where it paints nothing: its own
    /// background, else the nearest one its logical ancestors paint — its
    /// tabpanel's, then the boxes up to the root — as the web shows them
    /// through a transparent route (LLP 1116 D2: a root's
    /// `-exact-grouped-background` under every screen).
    func backdrop(dark: Bool) -> [Double]? {
        let painted = { (n: NodeView) in n.undimmedChannels("background_color", dark: dark).flatMap { $0.count < 4 || $0[3] > 0 ? $0 : nil } }
        if let own = ownBackdrop(dark: dark) { return own }
        guard let host, let root = host.container else { return nil }
        var next: UIView? = host.logicalParent(of: node)
        while let v = next {
            if let n = v as? NodeView, let colour = painted(n) { return colour }
            if v === root { break }
            next = v.superview
        }
        return nil
    }
    func mount() {
        loadViewIfNeeded()
        if usesPageViewport, contentViewport == nil {
            let scroll = Viewport(frame: view.bounds)
            scroll.documentNode = node
            scroll.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            scroll.contentInsetAdjustmentBehavior = .never
            scroll.keyboardDismissMode = viewportKeyboardDismissal
            #if os(iOS)
            scroll.scrollsToTop = false
            #endif
            scroll.delegate = self
            view.addSubview(scroll)
            contentViewport = scroll
        } else if !usesPageViewport, let scroll = contentViewport {
            contentViewport = nil
            keyboardInset.apply(0, to: scroll)
            view.addSubview(node)
            scroll.removeFromSuperview()
        }
        if node.superview !== contentMount { contentMount.addSubview(node) }
        host?.presenter.regrid(node) // from the controller's view, a grid origin of its own
    }
    /// The route's room for the keys, and under `resizes-visual` its
    /// viewport's frame, extent and inset.
    func fitContentViewport(in block: CGRect? = nil) {
        #if os(iOS)
        // Under a search in the bar the route keeps its layout (`block(in:)`)
        // and takes the keys as safe area, as SwiftUI's keyboard region is:
        // UIKit insets its edge scroller so the last rows reach above them.
        // A visual viewport takes them as its inset instead, without reflow.
        if let presenter = host?.presenter {
            var keys: CGFloat = 0
            if contentViewport == nil, presenter.editsInBar(of: view), let top = presenter.keyboardTop, let window = view.window {
                let edge = view.convert(CGPoint(x: 0, y: top), from: window).y
                keys = max(0, view.bounds.maxY - edge - (view.safeAreaInsets.bottom - additionalSafeAreaInsets.bottom))
            }
            if additionalSafeAreaInsets.bottom != keys { additionalSafeAreaInsets.bottom = keys }
        }
        #endif
        guard let scroll = contentViewport, let presenter = host?.presenter else { return }
        let oldSize = scroll.bounds.size, oldContent = scroll.contentSize, oldInset = scroll.contentInset
        let allocation = block ?? presenter.owners.block(in: view)
        let frame = ExactEnv.authoredChrome ? allocation : view.bounds
        if scroll.frame != frame { scroll.frame = frame }
        scroll.keyboardDismissMode = viewportKeyboardDismissal
        let extent = CGSize(width: max(node.bounds.width, node.style["overflow_x"]?.string == "visible" || node.style["overflow_x"] == nil ? node.content.width : 0),
                            height: max(node.bounds.height, node.style["overflow_y"]?.string == "visible" || node.style["overflow_y"] == nil ? node.content.height : 0))
        let size = CGSize(width: max(allocation.width, node.frame.minX + extent.width), height: max(allocation.height, node.frame.minY + extent.height))
        if scroll.contentSize != size { scroll.contentSize = size }
        // Keep drawing under the keyboard while the content clears its occupied area
        var overlap = max(0, frame.maxY - allocation.maxY)
        if presenter.keyboardPolicy == "resizes-visual", presenter.ownsKeyboard(in: view),
           let top = presenter.keyboardTop, let window = view.window {
            let keys = view.convert(CGPoint(x: 0, y: top), from: window).y
            overlap = min(allocation.height, max(0, allocation.maxY - keys))
        }
        keyboardInset.apply(overlap, to: scroll)
        if oldSize != scroll.bounds.size || oldContent != size || oldInset != scroll.contentInset {
            let offset = PixelGrid.offset(scroll.contentOffset, in: scroll)
            if offset != scroll.contentOffset { scroll.contentOffset = offset }
        }
    }

    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        guard scrollView === contentViewport, let presenter = host?.presenter else { return }
        presenter.owners.publish()
        presenter.stickies.scrolled(node.id)
        presenter.transformGeometry.changed()
        presenter.videoVisibility?.changed()
        presenter.scrollPump.scrolled(nil)
        FrameSampler.scrolled(scrollView, in: presenter.session, person: false)
        presenter.session?.frameHealth.trackScrolled(scrollView)
        node.repaintThrough()
    }

    func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
        host?.presenter.elementScrolls.userScrolled(scrollView)
        if let session = host?.presenter.session { session.frameHealth.beginScroll(scrollView, session: session) }
    }
    func scrollViewDidEndDragging(_ scrollView: UIScrollView, willDecelerate decelerate: Bool) {
        if !decelerate { host?.presenter.session?.frameHealth.endScroll(scrollView) }
    }
    func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) { host?.presenter.session?.frameHealth.endScroll(scrollView) }

    func scrollViewDidEndScrollingAnimation(_ scrollView: UIScrollView) {
        host?.presenter.elementScrolls.revealEnded(scrollView)
    }

    // A button can remove a route before UIKit starts its pop. Preserve its
    // outgoing pixels for that transition; an interactive pop uses live views.
    func freeze() {
        guard isViewLoaded, let snapshot = view.snapshotView(afterScreenUpdates: false) else { return }
        snapshot.frame = view.bounds
        // Pinned to the top, never stretched: the viewport can grow under it
        // as the pop starts (a keyboard going away, see NavigationHost.sync).
        snapshot.autoresizingMask = [.flexibleWidth, .flexibleBottomMargin]
        view.addSubview(snapshot)
        snapshot.setPaintForeground()
    }

    /// Where the remote's focus goes back to after the route moved into a
    /// navigation controller (HeldStackIOS `KeptFocus`).
    weak var focusReturn: UIView?
    override var preferredFocusEnvironments: [any UIFocusEnvironment] {
        focusReturn.map { [$0] } ?? super.preferredFocusEnvironments
    }

    /// The stack it is in: its navigation controller, or a tab's held stack.
    var stack: UIViewController? { navigationController ?? (parent as? HeldStack) }

    // A held stack's tab bar follows the route's scroll views as a navigation controller's follows its top's.
    override func setContentScrollView(_ scrollView: UIScrollView?, for edge: NSDirectionalRectEdge) {
        super.setContentScrollView(scrollView, for: edge)
        (parent as? HeldStack)?.setContentScrollView(scrollView, for: edge)
        if edge == .top { followTopEdge(scrollView) }
    }

    private var topEdgeWatch: NSKeyValueObservation?
    /// iOS 26 draws the bar's edge effect on the outermost scroll view under
    /// it, which for a route whose scroller (a list) lies in its page is the
    /// page's viewport, not that scroller (measured on iOS 27: UIKit alone
    /// does the same with a table view in a scroll view). So the viewport's
    /// top effect shows only while that scroller is scrolled from its top,
    /// as UIKit's does over a list that is the controller's own: no effect
    /// over a list at rest. Its style stays `automatic`: the rows blurred and
    /// faded under the whole bar to a line at its foot, as a UIKit or SwiftUI
    /// list or scroll view draws them there (iOS 27, measured), where `soft`
    /// left them legible under the title.
    private func followTopEdge(_ scrollView: UIScrollView?) {
        topEdgeWatch = nil
        guard #available(iOS 26.0, tvOS 26.0, *), let page = contentViewport else { return }
        guard let scrollView, scrollView !== page, node.style["scroll_edge_effect"] == nil else {
            if page.topEdgeEffect.isHidden, node.style["scroll_edge_effect"]?.string != "none" { page.topEdgeEffect.isHidden = false }
            return
        }
        let follow = { [weak page] (sv: UIScrollView) in
            let rest = sv.contentOffset.y <= 0.5 - sv.adjustedContentInset.top
            if let page, page.topEdgeEffect.isHidden != rest { page.topEdgeEffect.isHidden = rest }
        }
        follow(scrollView)
        topEdgeWatch = scrollView.observe(\.contentOffset) { sv, _ in follow(sv) }
    }
}

final class NavigationHost: NSObject, UINavigationControllerDelegate, UIGestureRecognizerDelegate {
    unowned let presenter: Presenter
    /// The selected tab's stack. An app's tab container can show a stack
    /// without appearance callbacks, so restart its reveal here.
    var primaryNavigation: UINavigationController? {
        didSet {
            guard primaryNavigation !== oldValue else { return }
            if changing { startRevealing() }
        }
    }
    var primaryRoutes: [RouteController] { primaryStacks.flatMap(\.stackRoutes).compactMap { $0 as? RouteController } }
    private(set) var presentedNavigations: [UINavigationController] = []
    /// The theme's title ink each bar was last given (`titleInk`), held no
    /// longer than its bar.
    let titleInks = NSMapTable<UINavigationBar, UIColor>.weakToStrongObjects()
    var modalNavigation: UINavigationController? { presentedNavigations.last }
    private var navigation: UINavigationController? { modalNavigation ?? primaryNavigation }
    /// The route on top of the stack in front: the one the person sees.
    var activeRoute: RouteController? { (navigation?.topViewController ?? primaryHeld?.route) as? RouteController }
    private(set) var syncing = false
    private var mounting = false
    private(set) weak var container: NodeView?
    private var routeIDs: [UInt32] = []
    private(set) var controllers: [UInt32: RouteController] = [:]
    /// Each route owner's logical children (the root's, each tabpanel's),
    /// while UIKit containment holds the route views (LLP 1075.003 §3.7).
    var logicalChildren: [UInt32: [UInt32]] = [:]
    /// Exact's tab container and its stacks by panel id; the tablist whose
    /// place its bar takes; the tab delegate Exact keeps and forwards.
    var tabController: UITabBarController?
    /// What holds the tabs: Exact's tab controller, or a container the app's
    /// `tabContainer` hatch returned (LLP 1075.003 §3.6).
    var tabOwner: UIViewController?
    var tabsHatched = false, tabContainerAsked = false, routerTab = -1
    var tabNavigations: [UInt32: UINavigationController] = [:]
    /// The tabs whose stacks have no navigation controller yet (HeldStackIOS).
    var heldTabs: [UInt32: HeldStack] = [:]
    /// A sync is armed for when the transition a held tab waits on ends.
    var syncAfterTransitionArmed = false
    var tabPanels: [UInt32] = []
    /// How many routes the selected stack declared at the last projection.
    private var selectedRouteCount = 0
    var adoptedTablist: UInt32?
    var tablistAppearanceNone = false
    /// Where the authored tablist was when its covers were last measured.
    var measuredTablist: CGRect?
    /// The root whose tablist last decided the bar, and whether Exact hid the
    /// bar for a root's hidden tablist (LLP 1075.003 §3.7, amended).
    weak var tablistRoot: RouteController?
    var tablistHidBar = false
    var tabItems: [String] = []
    /// The tabs and stacks the items were last made from, made again when
    /// the size class (a path glyph's size) changes.
    var tabItemsSource: (NavigationTabs, [UIViewController])?
    /// The bar's tint as last written: the tablist's accent, light and dark.
    var tabTint: [[Double]?]?
    var tabTintOwn = false
    let tabProxy = TabDelegateProxy()
    /// A context menu's commit pushes without the stack's animation: UIKit
    /// animates it (`.pop`, LLP 1021 §5.1). The selected route's key tells
    /// whether its press navigated.
    var unanimated = false
    /// Whether a reselected tab's scroll to the top (`scrollToTop`) is
    /// UIKit's animated one: not under the agent's frozen clock, where Exact
    /// starts no UIKit animation, nor in a test outside an app's scene, where
    /// UIKit never moves an animated offset.
    var scrollsToTopAnimated = !ExactEnv.agentFreezes
    var activeKey: String? { container?.props["navigationKey"] }
    /// While a push or pop runs: paints what each frame newly reveals.
    /// LLP 1075.003: each stack Exact built, by controller; what each shown
    /// bar covers of its route; the ownership changes already journaled;
    /// whether the hatches are being replayed for a cold launch's objects.
    var stacks: [ObjectIdentifier: NavigationStack] = [:]
    var stackCount = 0
    var covers: [UInt32: HostCover] = [:]
    /// The covers the kernel was last told (`deliverCovers`).
    var delivered: [UInt32: HostCover] = [:]
    /// Whether this boot's first batch looked for the tablist the boot
    /// covered (`adoptBootCover`); every boot covers its own.
    private(set) var bootCoverSeen = false
    var ownedReported: Set<String> = []
    var replayingHatches = false
    private var coversPending = false
    /// Whether the session's view last took the whole of its own for a bar.
    var tookWholeView = false
    private var pendingSync = false
    /// The press whose commit a deferred sync still owes (PressHandoffIOS).
    private weak var pendingPress: PressHandoff?
    /// A sync was deferred (a transition ran, or there was no window yet)
    /// and is still owed, or UIKit moved a stack itself (its back button, a
    /// swipe) since the last one: the next batch syncs, whatever it holds.
    var syncOwed: Bool {
        pendingSync || changing || windowless || nativeMoved || !settled
            || ([primaryNavigation].compactMap { $0 } + presentedNavigations).contains { $0.transitionCoordinator != nil }
    }
    /// No window yet; UIKit moved a stack since the last sync; the last
    /// sync ran to its end (one that stopped early, waiting for the first
    /// draw, a transition or a presentation it could not make yet, is owed).
    private var windowless = false, nativeMoved = false, settled = false
    /// Syncs asked for, for tests.
    private(set) var syncCalls = 0
    /// The root key last journaled as matching no route, so a refusal is one
    /// line, not one per batch (LLP 1035.001 D6).
    private var refusedKey: String?
    /// The keys of the stack last shown, through its selected route, and the
    /// route selected in the root's place while its key matches none (D1).
    private var shownKeys: [String] = [], fallbackKey: String?
    /// The key of the selected route: the root's, or while the root's
    /// matches no route, the route left selected of the stack last shown.
    var selectedKey: String? {
        guard let key = container?.props["navigationKey"] else { return nil }
        return key == refusedKey ? fallbackKey ?? key : key
    }

    init(presenter: Presenter) { self.presenter = presenter }

    /// The node a route is a child of in the tree, where UIKit holds its
    /// view elsewhere: the root or, with tabs, its tabpanel.
    func logicalParent(of route: NodeView) -> NodeView? {
        guard let owner = logicalChildren.first(where: { $0.value.contains(route.id) })?.key else { return nil }
        return owner == container?.id ? container : presenter.views[owner]
    }

    /// Logical child-list edits leave declared, retained routes inside their
    /// controllers; newly added or no-longer-declared nodes use normal mounting.
    /// A route's logical parent is the root or, with tabs, its tabpanel.
    func ownsContainment(of node: NodeView, under parent: NodeView) -> Bool {
        (parent === container || (parent !== container && logicalChildren[parent.id] != nil)) &&
            container?.props["navigationBack"] != nil &&
            node.props["navigationKey"] != nil && controllers[node.id]?.node === node
    }

    /// One batch's projection: the root, each stack's routes — one a tab
    /// (LLP 1075.003 §3.7), or the root's own — which stack holds the route
    /// the root names and where, and each stack's wanted controllers (the
    /// selected stack's prefix through that route; another tab's whole).
    struct Projection {
        let root: NodeView
        let tabs: NavigationTabs?
        let stacks: [[NodeView]]
        let at: Int, selected: Int
        let wanted: [[RouteController]]
        var routes: [NodeView] { stacks[at] }
        var chosen: [RouteController] { wanted[at] }
    }

    /// Put each route node back under its logical parent, before native
    /// ownership retires, so surviving content stays in the window.
    private func restoreLogicalChildren() {
        for (owner, ids) in logicalChildren {
            guard let parent = owner == container?.id ? container : presenter.views[owner] else { continue }
            for (index, node) in ids.compactMap({ presenter.views[$0] }).enumerated() where presenter.views[node.id] === node {
                parent.container.insertSubview(node, at: index)
                presenter.regrid(node)
            }
        }
    }

    func prepare(_ batch: Batch) {
        checkOwned()
        presenter.elements.checkOwned()
        guard let top = navigation?.topViewController as? RouteController else { return }
        if batch.ops.contains(where: { $0.op == .destroy && $0.id == top.node.id }) {
            top.freeze()
        }
    }

    /// Resolve the declared route stacks once for both structural installation
    /// and later presentation. UIKit containment never becomes route state.
    private func projection(_ batch: Batch) -> Projection? {
        guard let root = presenter.root.subviews.first as? NodeView,
              root.props["navigationBack"] != nil else {
            // Retiring native ownership must not remove surviving content.
            // Restore the logical children before detaching the controller so
            // an editor remains in the same window through the handoff.
            if container != nil { restoreLogicalChildren() }
            reset(clearFocus: container != nil)
            return nil
        }
        if container !== root {
            // A fresh owner after unmount must preserve focus requested while
            // offscreen. Replacing an existing root still retires its command.
            reset(clearFocus: container != nil)
            container = root
            logicalChildren = [root.id: root.container.subviews.compactMap { ($0 as? NodeView)?.id }]
        }
        for op in batch.ops where op.op == .children && logicalChildren[op.id] != nil {
            logicalChildren[op.id] = op.ids
        }
        // With tabs, each panel holds a stack; a panel seen for the first
        // time still holds its rows, and its children ops say the rest.
        let tabs = NavigationTabs.of(root, presenter)
        for panel in tabs?.panels ?? [] where logicalChildren[panel.id] == nil {
            logicalChildren[panel.id] = panel.container.subviews.compactMap { ($0 as? NodeView)?.id }
        }
        let owners = tabs?.panels ?? [root]
        let stacks = owners.map { owner in
            (logicalChildren[owner.id] ?? []).compactMap { presenter.views[$0] }.filter { $0.props["navigationKey"] != nil }
        }
        routeIDs = stacks.flatMap { $0.map(\.id) }
        // Every route left the tree (the root shows something else now): the
        // containers go with them, or their views — a removed route's frozen
        // snapshot among them — stay over the new content (mail F21).
        if routeIDs.isEmpty, primaryOwner != nil || !presentedNavigations.isEmpty {
            presenter.session?.log("navigation: the root has no routes now; its containers are retired")
            reset(clearFocus: false)
            return nil
        }
        // D1: the stack is the prefix through the route the root names; a
        // key that names none leaves the stack alone, less the routes that
        // left the tree (a closed sheet's), and says so once.
        let rootKey = root.props["navigationKey"] ?? ""
        let keys = { (routes: [NodeView]) in routes.map { $0.props["navigationKey"] ?? "" } }
        var selecting = rootKey
        if !stacks.contains(where: { keys($0).contains(rootKey) }) {
            if refusedKey != rootKey {
                refusedKey = rootKey
                presenter.session?.log("navigationKey \"\(rootKey)\" matches no route among the root's children or those of the tabpanels its tablist names; the stack is unchanged")
            }
            guard let survivor = NavigationRules.survivor(of: shownKeys, declared: stacks.map(keys)) else { fallbackKey = nil; return nil }
            selecting = survivor
        } else {
            refusedKey = nil
        }
        fallbackKey = selecting == rootKey ? nil : selecting
        guard let at = stacks.firstIndex(where: { NavigationRules.stack(routeKeys: keys($0), selected: selecting) != nil }),
              let range = NavigationRules.stack(routeKeys: keys(stacks[at]), selected: selecting) else { return nil }
        shownKeys = Array(keys(stacks[at])[range])
        selectedRouteCount = stacks[at].count
        let wanted = stacks.enumerated().map { index, routes in
            var presented = false
            return (index == at ? Array(routes[range]) : routes).map { node -> RouteController in
                presented = presented || ["modal", "fullscreen"].contains(node.props["navigationPresentation"] ?? "")
                let c = controllers[node.id] ?? RouteController(node)
                // A separate visual viewport accommodates the keyboard without reflowing the route
                c.usesPageViewport = !presented && !ExactEnv.authoredChrome && presenter.keyboardPolicy == "resizes-visual"
                c.host = self
                controllers[node.id] = c
                c.mount()
                return c
            }
        }
        return Projection(root: root, tabs: tabs, stacks: stacks, at: at, selected: range.upperBound - 1, wanted: wanted)
    }

    /// The native container fills the page's area. It owns the screen, so
    /// the document is in its mount, where no scroll view holds it (`hostDocument`).
    func fitOwner() {
        presenter.hostDocument(fixed: wantsWholeView)
        guard let owner = primaryOwner, let view = owner.viewIfLoaded, let root = view.superview else { return }
        let frame = ownerFrame(owner, in: root)
        if view.frame != frame { view.frame = frame }
    }

    private func installPrimary(_ p: Projection, first: [RouteController]) {
        guard primaryOwner == nil else { return }
        // Find the containing controller before installing our child.
        guard let parent = ViewPresentation.controller(for: p.root) else { return }
        if let tabs = p.tabs {
            installTabs(p, tabs, first: first, in: parent)
            return
        }
        // The stack holds its first routes before its view enters the window,
        // as the tabs' stacks do (`installTabs`): UIKit shows a root's large
        // title collapsed when the stack was laid out in the window before it
        // had that root.
        let nav = makeNavigation(first: first.first?.node)
        primaryNavigation = nav
        prepareRoutes(first, in: nav)
        nav.setViewControllers(first, animated: false)
        recordOwned(nav)
        watchPops(nav)
        parent.addChild(nav)
        nav.view.frame = ownerFrame(nav, in: p.root)
        nav.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        p.root.addSubview(nav.view)
        nav.view.setPaintForeground(aboveAuthored: false)
        fitOwner()
        nav.didMove(toParent: parent)
    }

    /// A hidden navigation bar needs its availability check here. The
    /// recognizer, competing scroll views and transition stay UIKit's.
    /// UIKit gives both recognizers its own delegate when the controller's
    /// view loads: a tab's stack, built before its tab controller shows it,
    /// is loaded first, or the swipe never asks Exact.
    func watchPops(_ nav: UINavigationController) {
        nav.loadViewIfNeeded()
        if nav.view.accessibilityIdentifier == nil {
            nav.view.accessibilityIdentifier = "exact-navigation-\(UUID().uuidString)"
        }
        // tvOS has no interactive pop gesture.
        #if !os(tvOS)
        nav.interactivePopGestureRecognizer?.delegate = self
        if #available(iOS 26.0, tvOS 26.0, *) { nav.interactiveContentPopGestureRecognizer?.delegate = self }
        #endif
    }

    /// Initial containment is ready before child frames. It does not present
    /// a sheet or flush focus commands from the partially applied batch.
    func installInitialOwner(_ batch: Batch) {
        guard primaryOwner == nil, !syncing, presenter.session?.view?.window != nil else { giveBackBootCover(); return }
        syncing = true
        defer { syncing = false }
        guard let p = projection(batch) else { giveBackBootCover(); return }
        // After the projection, whose first `reset` clears the covers.
        _ = adoptBootCover()
        let parts = NavigationRules.segments(presentations: p.routes[...p.selected].map { $0.props["navigationPresentation"] })
        installPrimary(p, first: Array(p.chosen[parts[0]]))
        primaryOwner?.view.layoutIfNeeded()
        // The content area the bar leaves reaches layout in this turn, before
        // the first frame (LLP 1075.003 Q3 (c)).
        reportCovers()
        refitForBars()
    }

    /// The root's tablist the runtime covered before the first layout,
    /// for the tab bar it expects to take its place (host/apple/src/closed.rs
    /// `cover_root_tablist`), becomes one of the presenter's covers, which
    /// `reportCovers` keeps while the bar shows and gives back otherwise.
    /// Once, at the first batch's install.
    private func adoptBootCover() -> Bool {
        guard !bootCoverSeen else { return false }
        bootCoverSeen = true
        guard let root = presenter.root.subviews.first as? NodeView,
              let tabs = presenter.session?.runtime.rootTabs(root.id), tabs.covered, covers[tabs.tablist] == nil else { return false }
        covers[tabs.tablist] = .whole
        delivered[tabs.tablist] = .whole
        return true
    }

    /// No containers at the first batch: the tablist the boot covered is
    /// laid out again (`adoptBootCover`).
    private func giveBackBootCover() {
        if adoptBootCover() { reportCovers() }
    }

    /// The runtime restarts (`Presenter.reset`): its next boot covers the
    /// tablist again, for its first batch to adopt or give back.
    func rebooting() { bootCoverSeen = false }

    /// A native primary container coming or going changes the view the
    /// session takes (ExactViewIOS `fit`, LLP 1123 D1), whatever its bars
    /// show: lay it out again this turn.
    func refitForBars() {
        guard wantsWholeView != tookWholeView else { return }
        tookWholeView = wantsWholeView
        // Now, not at the layout: under `cover` or the agent's own size the
        // view's area does not change, and `fit` refits the owner only when it does.
        presenter.hostDocument(fixed: wantsWholeView)
        presenter.session?.view?.setNeedsLayout()
    }

    func sync(_ batch: Batch) {
        // Installing or moving a controller can synchronously cause layout.
        // That layout must not start another containment handoff inside this one.
        syncCalls += 1
        guard !syncing else { return }
        let window = presenter.session?.view?.window
        windowless = presenter.session != nil && window == nil
        // With no session (a presenter on its own) nothing is projected or owed.
        if presenter.session == nil { settled = true }
        guard window != nil else { return }
        nativeMoved = false; settled = false
        syncing = true
        defer { syncing = false; presenter.flushPendingFocus(); presenter.elementScrolls.flush() }
        guard let p = projection(batch) else { settled = true; return }
        let root = p.root, wanted = p.chosen
        // A presentation whose route a logical ancestor's `display: none`
        // hides is not presented, nor is any above it: its sheet is out of
        // the document's tree, which no longer hides it (LLP 1123 D2)
        let segments = NavigationRules.segments(presentations: p.routes[...p.selected].map { $0.props["navigationPresentation"] })
        let parts = Array(segments.enumerated().prefix { $0.offset == 0 || !logicallyHidden(wanted[$0.element.lowerBound].node) }.map(\.element))
        reshape(p)
        installPrimary(p, first: Array(wanted[parts[0]]))
        // The other tabs' stacks, the selected tab and its items (LLP 1075.003 §3.7).
        if p.tabs != nil { syncTabs(p) }
        // Initial content draws before a presentation covers it.
        if parts.count > 1, let session = presenter.session, session.firstDrawMs == nil { return }
        // The selected tab held while its navigation controller waits for
        // a transition in the way (`syncTabs`, HeldStackIOS) shows nothing
        // new until it is built.
        let heldWaits = primaryHeld != nil && needsNavigation(Array(wanted[parts[0]]))
        guard !changing, !presenter.modals.inTransition, !heldWaits else {
            pendingSync = true
            owePress()
            reportCovers()
            return
        }
        pendingSync = false
        // The press a deferred sync is owed to: what this sync presents or
        // pushes is that press's work, and it is owed again if this defers too.
        let owed = pendingPress.flatMap { $0 === presenter.pressHandoff ? $0 : nil }
        pendingPress = nil
        owed?.syncing = true
        defer {
            owed?.syncing = false
            owed?.pending = false
            if pendingSync { owePress(owed) }
            owed?.settle()
        }
        let boundaries = parts.dropFirst().map { wanted[$0.lowerBound].node }
        let mounted = presenter.modals.routes
        let common = zip(boundaries, mounted).prefix {
            $0.0 === $0.1.node && $0.0.props["navigationPresentation"] == $0.1.kind
        }.count
        // The selected tab while it is held (HeldStackIOS): `syncTabs` built its
        // navigation controller if this batch pushes onto it or shows its bar.
        if let held = primaryHeld {
            show(Array(wanted[parts[0]]), in: held)
            held.view.layoutIfNeeded()
        }
        // Mount the destination under a departing presentation before starting
        // dismissal, so its editor can accept the batch's focus handoff.
        let owners: [UINavigationController?] = (primaryHeld != nil ? [nil] : [primaryNavigation].compactMap { $0 }) + presentedNavigations
        for index in 0...common where index < owners.count {
            guard let nav = owners[index] else { continue }
            let stack = Array(wanted[parts[index]])
            prepareRoutes(stack, in: nav)
            let same = nav.viewControllers.count == stack.count && zip(nav.viewControllers, stack).allSatisfy { $0 === $1 }
            if !same {
                // @ref LLP 1038 D6 — a tab change swaps immediately. A replacement within one stack
                // whose new top was not on it (a finished screen giving way to its result) arrives
                // as UIKit's own push, rather than cutting.
                let pushOrPop = NavigationRules.isPushOrPop(from: nav.viewControllers.map(ObjectIdentifier.init), to: stack.map(ObjectIdentifier.init))
                let arrives = stack.count > 1 && nav.viewControllers.first === stack.first
                    && !nav.viewControllers.contains { $0 === stack.last }
                #if os(iOS)
                // A route pushed from a source zooms from it (ZoomPushIOS).
                if (pushOrPop || arrives), stack.count > 1, let top = stack.last as? RouteController, !nav.viewControllers.contains(where: { $0 === top }) {
                    prepareZoomPush(top, beneath: stack[stack.count - 2] as? RouteController)
                }
                #endif
                nav.setViewControllers(stack, animated: (pushOrPop || arrives) && index == owners.count - 1 && mounted.count == boundaries.count && !ExactEnv.agentFreezes && !unanimated && nav.view.window != nil)
                recordOwned(nav)
                // A press that pushed may hand its control's focus to the arriving route's first autofocus (PressHandoffIOS).
                if pushOrPop || arrives { presenter.pressShows((stack.last as? RouteController)?.node) }
            }
            nav.view.layoutIfNeeded()
        }
        unanimated = false
        if mounted.count > common {
            pendingSync = true
            presenter.modals.closeTop()
            return
        }
        if boundaries.count > mounted.count, let background = modalNavigation ?? primaryOwner, let owner {
            let part = parts[mounted.count + 1], route = wanted[part.lowerBound].node
            guard presenter.modals.canPresent(from: owner, route: route) else { return }
            // The sheet mounts its own navigation (`ModalHost.present`)
            let nav = makeNavigation(first: route)
            nav.view.frame = root.bounds
            presentedNavigations.append(nav)
            prepareRoutes(Array(wanted[part]), in: nav)
            nav.setViewControllers(Array(wanted[part]), animated: false)
            recordOwned(nav)
            presenter.pressShows(wanted[part].last?.node) // a route pushed in the sheet as it opens
            watchPops(nav)
            nav.view.layoutIfNeeded()
            pendingSync = boundaries.count > mounted.count + 1
            let preceding = part.lowerBound > 0 ? wanted[part.lowerBound - 1].node : nil
            presenter.modals.present(route, navigation: nav, from: background,
                                     preceding: preceding, owner: owner)
        }
        // A presented navigation fills its sheet; the primary owner the root,
        // which no presentation resizes (LLP 1123 D1)
        if let top = primaryOwner {
            fitOwner()
            placeOwner()
            top.view.layoutIfNeeded()
        }
        for (id, c) in controllers where !routeIDs.contains(id) { end(c) }
        controllers = controllers.filter { routeIDs.contains($0.key) }
        presenter.modals.updatePermissions()
        if p.tabs != nil { replayTabs() }
        reportCovers()
        refitForBars()
        // A route that left every stack gives its block back (LLP 1123 D2)
        presenter.owners.publish()
        settled = true
    }

    /// The container standing in for the routes paints where they are in
    /// the root's children, as CSS paints siblings in tree order (LLP
    /// 1083.000): above the children before them, under those after them —
    /// an authored tablist, a toast. On top of every child it hid a sibling
    /// tablist and a root overlay, and took their taps (shop F21, recipes F23).
    func placeOwner() {
        guard let root = container, let owner = primaryOwner?.view, owner.superview === root else { return }
        let ids = logicalChildren[root.id] ?? []
        let panels = tabPanels.compactMap { presenter.views[$0] }
        let region = ids.firstIndex { id in
            routeIDs.contains(id) || presenter.views[id].map { child in panels.contains { $0 === child || $0.isDescendant(of: child) } } == true
        }
        let after = Set(region.map { ids[($0 + 1)...] } ?? [])
        guard let next = root.subviews.first(where: { ($0 as? NodeView).map { after.contains($0.id) } == true }) else {
            if root.subviews.last !== owner { root.bringSubviewToFront(owner) }
            return
        }
        let at = root.subviews.firstIndex { $0 === next }!
        if at == 0 || root.subviews[at - 1] !== owner { root.insertSubview(owner, belowSubview: next) }
    }

    /// A route's controller leaves for good: its header paints again and the
    /// route hatch hears `routeEnded` (LLP 1075.003 §3.2).
    private func end(_ c: RouteController) {
        if let header = c.lifted { header.isHidden = false; c.lifted = nil }
        c.scoped?.isHidden = false
        c.scoped = nil
        for node in c.barNodes { node.isHidden = false }
        c.barNodes = []
        guard c.hatched else { return }
        c.hatched = false
        presenter.session?.natives.routeHatch(.ended, controller: c, navigation: nil, scroll: nil, key: c.key, dataset: c.node.props["dataset"])
    }

    /// At a cold launch the module loads a turn after first pixel (LLP
    /// 1075.003 Q3 (c)): each hatch runs once for every object already live,
    /// stacks first, in the order they were built.
    func replayHatches() {
        guard presenter.session?.natives.hatchesConnected == true else { return }
        replayingHatches = true
        defer { replayingHatches = false }
        for nav in allNavigations { hatchNavigation(nav) }
        replayTabs()
        // (Held stacks too, whose routes' hatches have not run either; a tab
        // container the app took built every tab's navigation controller.)
        for stack in primaryStacks + presentedNavigations {
            prepareRoutes(stack.stackRoutes.compactMap { $0 as? RouteController }, in: stack)
            stack.view.layoutIfNeeded()
        }
        reportCovers()
        refitForBars()
    }

    /// UIKit changed a route's safe area: lay out its content in this call.
    func coversChanged() {
        // sync reports the final containment below; intermediate safe areas
        // belong to the controller the route is leaving.
        guard !syncing, !coversPending else { return }
        coversPending = true
        presenter.afterBatch { [weak self] in
            guard let self else { return }
            coversPending = false
            reportCovers()
        }
    }

    /// What presents a sheet: the last sheet, else the primary owner's
    /// parent — with tabs the tab controller's, never a tab's own stack.
    var owner: UIViewController? { presenter.modals.owner ?? primaryOwner?.parent }
    /// The controller Exact installed under the root: the tab container, or
    /// the one stack.
    var primaryOwner: UIViewController? { tabOwner ?? primaryNavigation }

    func willMount() { mounting = true }

    func mounted() {
        mounting = false
        sync(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil))
    }

    func unmounted() {
        mounting = false
        let root = container, logical = logicalChildren
        reset(clearFocus: false)
        // Release controller containment while preserving the actual nodes.
        // Offscreen batches now reconcile their ordinary logical child list.
        guard let root, presenter.views[root.id] === root else { return }
        for (owner, ids) in logical {
            guard let parent = owner == root.id ? root : presenter.views[owner] else { continue }
            for (index, node) in ids.compactMap({ presenter.views[$0] }).enumerated() where presenter.views[node.id] === node {
                parent.container.insertSubview(node, at: index)
                presenter.regrid(node)
            }
        }
    }

    /// A sync deferred behind a running transition is owed to the press
    /// whose work it is (`owed`, else the live press while its work runs):
    /// that press's handoff waits for it. A newer press owns it instead.
    private func owePress(_ owed: PressHandoff? = nil) {
        guard let press = owed ?? presenter.pressHandoff.flatMap({ $0.working ? $0 : nil }) else { return }
        if pendingPress !== press { pendingPress?.pending = false; pendingPress?.settle() }
        press.pending = true
        pendingPress = press
    }

    func modalDidDismiss() {
        guard pendingSync else { return }
        pendingSync = false
        sync(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil))
        presenter.syncModal()
    }

    /// Retire only the named owner; a late completion cannot remove its
    /// successor or the retained owner beneath it.
    func retireNavigation(_ controller: UIViewController, preserving: Bool) {
        guard let nav = controller as? UINavigationController,
              presentedNavigations.contains(where: { $0 === nav }) else { return }
        presentedNavigations.removeAll { $0 === nav }
        // A presentation gone (UIKit's own dismissal included): the next
        // batch syncs, whatever it holds, so a route still declared returns.
        nativeMoved = true
        retireStack(nav)
        nav.delegate = nil
        if !preserving {
            nav.willMove(toParent: nil)
            nav.view.removeFromSuperview()
            nav.removeFromParent()
        }
    }

    /// The removed editor stays in its outgoing presentation while UIKit
    /// dismisses it.
    func preserveModalContent(in parent: UIViewController) -> UIViewController? {
        guard let nav = presentedNavigations.first(where: { $0.parent === parent }) else { return nil }
        let frame = nav.view.convert(nav.view.bounds, to: parent.view)
        parent.view.insertSubview(nav.view, at: 0)
        nav.view.setPaintForeground(false)
        nav.view.frame = frame
        nav.view.accessibilityElementsHidden = true
        return nav
    }

    /// Focus waits through installation and push/pop unless it targets the arriving route.
    /// `clock settle` observes the asynchronous part under platform timing.
    var defersFocus: Bool { changing || syncing || mounting }
    var inTransition: Bool { defersFocus || pendingSync }

    /// An explicit focus request may reach the arriving screen once UIKit mounts it.
    func isArriving(_ node: NodeView) -> Bool {
        guard node.headerField == nil, !syncing, !mounting, let navigation, let transition = pushOrPop(on: navigation),
              let route = transition.viewController(forKey: transition.isCancelled ? .from : .to) as? RouteController else { return false }
        return presenter.authored(node, inside: route.node)
    }

    /// How the last transition ended — `completed` (the Back control was
    /// pressed), `cancelled` (an interactive pop returned), `idle` (a
    /// programmatic change, or nothing yet) — for `state.navigation`.
    private var lastTransition = "idle"
    /// Whether UIKit is pushing or popping the stack navigation follows: from
    /// the transition's first frame through `didShow`, until the completion
    /// `willShow` registers
    var changing: Bool { navigation.map { nativePopPending(on: $0) || pushOrPop(on: $0) != nil } ?? false }

    /// UIKit removes a controller before resigning its search field and calling willShow.
    /// A shorter prefix of our last write belongs to that unfinished native pop.
    private func nativePopPending(on nav: UINavigationController) -> Bool {
        guard let written = stacks[ObjectIdentifier(nav)]?.written else { return false }
        let current = nav.viewControllers.map(ObjectIdentifier.init)
        return current.count < written.count && current.elementsEqual(written.prefix(current.count))
    }
    var hasInteractiveTransition: Bool { allNavigations.contains { $0.transitionCoordinator?.isInteractive == true } }

    /// A push or pop, or any stack's controller transition, including a sheet presentation.
    var transitioning: Bool {
        // A rotation's or resize's coordinator moves no controller: not one.
        changing || ([primaryNavigation].compactMap { $0 } + presentedNavigations).contains { $0.transitionCoordinator?.viewController(forKey: .from) != nil }
    }
    /// For `state.navigation` (LLP 1035.002 D2): the route the root names,
    /// UIKit's stack by key, and the transition's phase — observations.
    func observation() -> [String: Any] {
        // Scope Back to its container: Duo places the control outside the navigation bar
        let nativeBack: String? = {
            guard let nav = navigation, nav.viewIfLoaded?.window != nil, !nav.isNavigationBarHidden,
                  nav.viewControllers.count > 1, canInvokeBack else { return nil }
            let owner: UIViewController? = nav === primaryNavigation ? primaryOwner : nav
            return owner?.view.accessibilityIdentifier
        }()
        let stack = (navigation?.viewControllers ?? primaryHeld?.stackRoutes ?? []).compactMap { ($0 as? RouteController)?.key }
        let transition: [String: Any] = ["interactive": navigation?.transitionCoordinator?.isInteractive ?? false,
                                         "phase": changing || pendingSync || presenter.modals.inTransition ? "in-progress" : lastTransition]
        let root = container ?? presenter.root.subviews.first as? NodeView
        return ["route": root?.props["navigationKey"] ?? NSNull(), "stack": stack, "transition": transition,
                "nativeBack": nativeBack ?? NSNull()]
    }

    /// Whether a route or a logical ancestor of it (`eventParent`) is
    /// `display: none`.
    func logicallyHidden(_ route: NodeView) -> Bool {
        presenter.logicalAncestry(of: route).prefix { $0 !== presenter.root }
            .contains { ($0 as? NodeView)?.style["display"]?.string == "none" }
    }

    func invokeBack(from source: NodeView) {
        guard presenter.session?.view?.window != nil,
              presenter.views[source.id] === source, routeIDs.contains(source.id),
              source.props["navigationKey"] == selectedKey else { return }
        goBack()
    }

    /// Whether the platform's Back goes from the selected route (LLP 1115 D5).
    var canInvokeBack: Bool { back != nil }

    /// The agent's `back` from a pushed route (LLP 1116.006 D4): UIKit's
    /// Back button, ending where its pop ends (`goBack`), as tvOS's Menu
    /// does. nil when the stack in front has nothing beneath its top (a
    /// sheet's own root is the sheet's); "" when it went, else why not.
    func agentBack() -> String? {
        guard (navigation?.viewControllers.count ?? 0) > 1 else { return nil }
        guard back != nil else { return backRefusal }
        goBack()
        return ""
    }

    /// The platform's Back, completed: the authored control pressed, else
    /// the root's `navigate` with the location beneath (LLP 1115 D5).
    private func goBack() {
        switch back {
        case .press(let id)?: presenter.press(id)
        case .navigate(let location)?:
            guard let root = container, let session = presenter.session else { return }
            session.navigate(location, at: root)
        case .pop?:
            guard let key = selectedRoute?.props["navigationKey"], let session = presenter.session else { return }
            session.hostBack(key)
        case nil: break
        }
    }

    /// A history jump uses navigation; the authored Back control handles the adjacent intent.
    private func goBack(from source: String, to destination: String) {
        pendingSync = true
        guard let session = presenter.session, let root = container,
              let location = session.runtime.visitLocation(destination) else {
            presenter.session?.log("Back history refused: the destination has no router visit; the declared stack is restored")
            return
        }
        if root.handlers.contains("navigate") {
            session.navigate(location, at: root)
            if selectedKey == source { session.log("Back history: the navigation handler has not changed the selected visit; the declared stack is restored") }
        } else {
            session.hostBack(from: source, to: destination)
            if selectedKey != destination { session.log("Back history: the requested destination is not selected; the declared stack is restored") }
        }
    }

    /// The key of the route a view is mounted under, for `layout <node>`
    /// (LLP 1035.002 D1) — an observation of UIKit's containment.
    func routeKey(containing view: UIView) -> String? {
        ViewPresentation.controllers(for: view).compactMap { ($0 as? RouteController)?.key }.first
    }

    func isInactiveRoute(containing view: UIView) -> Bool {
        guard let selected = selectedKey,
              let key = routeKey(containing: view) else { return false }
        return key != selected
    }

    /// D1: resolved at use, by HTML id, among live enabled press controls —
    /// never captured at a gesture's start (`NavigationBack.control`).
    private var backControl: NodeView? {
        guard let route = selectedRoute else { return nil }
        let routes = routeIDs.compactMap { presenter.views[$0] }
        return NavigationBack.control(named: container?.props["navigationBack"], among: presenter.carrying("id"),
                                           id: \.id, htmlID: { $0.props["id"] }, pressable: { $0.handlers.contains("press") }, disabled: \.disabled,
                                           inActiveRoute: { $0 === route || $0.isDescendant(of: route) },
                                           inAnyRoute: { control in routes.contains { control === $0 || control.isDescendant(of: $0) } })
    }

    private var selectedRoute: NodeView? {
        guard let key = selectedKey else { return nil }
        return routeIDs.compactMap({ presenter.views[$0] }).first(where: { $0.props["navigationKey"] == key })
    }

    /// How Back goes from the selected route, resolved at use as its
    /// control is (`NavigationBack.way`, LLP 1115 D5).
    var back: NavigationBack.Way? {
        guard let root = container, let route = selectedRoute else { return nil }
        return NavigationBack.way(control: backControl?.id, declared: declaresBack(route), hearsNavigate: root.handlers.contains("navigate"),
                                    beneath: { [presenter] in route.props["navigationKey"].flatMap { presenter.session?.runtime.locationBeneath($0) } })
    }

    /// Whether a route holds an element named by the root's `navigationBack`.
    private func declaresBack(_ route: NodeView) -> Bool {
        guard let name = container?.props["navigationBack"] else { return false }
        return presenter.carrying("id").contains { $0.props["id"] == name && ($0 === route || $0.isDescendant(of: route)) }
    }

    /// Why Back does not go from the selected route, for the journal.
    var backRefusal: String {
        if let route = selectedRoute, declaresBack(route) { return "its navigationBack control is disabled or has no press handler" }
        return "no visit beneath the active route"
    }

    /// Where each pop recognizer's first touch went down, in its view. A
    /// pan's translation at `shouldBegin` leaves out the travel before it
    /// recognized: a real touch from x = 1 read as starting at 30, past the
    /// 20-point edge, and over a `swiperight` row the pop was refused (LLP
    /// 1080.000 §11). The edge rule judges where the finger landed.
    /// Keyed weakly: a retired stack's recognizers take their entries with them.
    private let popTouchDown = NSMapTable<UIGestureRecognizer, NSValue>.weakToStrongObjects()

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        // The first finger of each gesture replaces the last gesture's; a second finger does not.
        if gestureRecognizer.numberOfTouches == 0 { notePopTouchDown(gestureRecognizer, at: touch.location(in: gestureRecognizer.view)) }
        return true
    }
    func notePopTouchDown(_ gestureRecognizer: UIGestureRecognizer, at point: CGPoint) { popTouchDown.setObject(NSValue(cgPoint: point), forKey: gestureRecognizer) }

    /// The swipe's start: its first touch's point, else (no touch seen) its translation's origin.
    func popStart(_ pan: UIPanGestureRecognizer, in view: UIView) -> CGPoint {
        if let down = popTouchDown.object(forKey: pan)?.cgPointValue { return down }
        let location = pan.location(in: view), delta = pan.translation(in: view)
        return CGPoint(x: location.x - delta.x, y: location.y - delta.y)
    }

    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        guard let pan = gestureRecognizer as? UIPanGestureRecognizer, let view = pan.view else {
            return popMayBegin(gestureRecognizer, from: nil, in: nil, velocity: .zero)
        }
        // A pan's velocity at shouldBegin is often zero (a finger that
        // started slowly, or one sample in): read that way every back swipe
        // is refused as not horizontal. Its travel so far says the direction.
        return popShouldBegin(pan, in: view, velocity: NavigationRules.popDirection(velocity: pan.velocity(in: view), travel: pan.translation(in: view)))
    }
    /// `shouldBegin` for a pan at `velocity`: from where its finger landed.
    func popShouldBegin(_ pan: UIPanGestureRecognizer, in view: UIView, velocity: CGPoint) -> Bool {
        popMayBegin(pan, from: popStart(pan, in: view), in: view, velocity: velocity)
    }

    /// Whether a pop recognizer's swipe, starting at `start` in `view` (a
    /// pan's) with `velocity`, may pop: the showing stack's (a tab's, or a
    /// sheet's over it), by its own depth, with an enabled back control in
    /// its active route (D1), and not a pan a `swiperight` node or a canvas
    /// owns, nor more vertical than horizontal.
    func popMayBegin(_ gestureRecognizer: UIGestureRecognizer, from start: CGPoint?, in view: UIView?, velocity: CGPoint) -> Bool {
        // tvOS has no interactive pop gesture.
        #if os(tvOS)
        let owner: UINavigationController? = nil
        #else
        let owner = allNavigations.first { nav in
            if nav.interactivePopGestureRecognizer === gestureRecognizer { return true }
            if #available(iOS 26.0, *) { return nav.interactiveContentPopGestureRecognizer === gestureRecognizer }
            return false
        }
        #endif
        if let owner, owner !== navigation { return false }
        let depth = navigation?.viewControllers.count ?? 0
        let goes = back != nil
        guard NavigationRules.popMayBegin(depth: depth, changing: changing, modalActive: presenter.modals.inTransition,
                                          hasBackControl: goes,
                                          contextPreviewActive: !presenter.chrome.ids("contextTarget").isEmpty) else {
            if depth > 1, !changing, !presenter.modals.inTransition, !goes {
                presenter.session?.log("back gesture refused: \(backRefusal)")
            }
            return false
        }
        guard let start, let view else { return true }
        var overSwipeRight = false
        var hit = view.hitTest(start, with: nil)
        if CanvasInputs.owns(hit) { return false }
        while let current = hit {
            if let node = current as? NodeView, node.handlers.contains("swiperight") { overSwipeRight = true; break }
            if current === view { break }
            hit = current.superview
        }
        return NavigationRules.panMayBegin(startX: start.x, overSwipeRight: overSwipeRight, velocity: velocity)
    }

    /// UIKit's push or pop on `nav`, while it runs: its coordinator presents
    /// nothing (a sheet's has a presentation style; a rotation's is not the
    /// stack's) and moves the stack's top controller in or out. The top is
    /// the destination until a cancelled swipe's rollback ends, then the
    /// source again in its `didShow`.
    private func pushOrPop(on nav: UINavigationController) -> UIViewControllerTransitionCoordinator? {
        guard let transition = nav.transitionCoordinator, transition.presentationStyle == .none else { return nil }
        let top = nav.topViewController
        return transition.viewController(forKey: .to) === top || transition.viewController(forKey: .from) === top ? transition : nil
    }

    func navigationController(_ navigationController: UINavigationController, willShow viewController: UIViewController, animated: Bool) {
        guard navigationController === navigation else { return }
        HostLog.note(presenter.session, {
            let by = navigationController.transitionCoordinator?.isInteractive == true ? "the edge swipe" : nativePopPending(on: navigationController) ? "UIKit's Back" : "the app"
            return "navigation: willShow \((viewController as? RouteController)?.key ?? "?") (\(animated ? "animated" : "unanimated"), by \(by))"
        }())
        // The route coming into view was out of the window while it was
        // covered, so no scroll asked for its text; paint what shows of it
        // as the transition starts, not when the reader next scrolls.
        navigationController.transitionCoordinator?.animate(alongsideTransition: { [weak self] _ in self?.presenter.paintVisibleText() })
        // @ref LLP 1038 D6/D11; LLP 1035.001 D2 — a cancelled swipe calls
        // willShow again for its source, in the transition already begun.
        guard let transition = pushOrPop(on: navigationController), transition.viewController(forKey: .to) === viewController else { return }
        // Arranged panes keep their layout while their route slides
        #if os(iOS)
        if #available(iOS 27.1, *) { presenter.arrangements.holdStill(during: transition) }
        #endif
        // As the swipe begins: an update may remove its source's node while
        // the finger is down
        let swipe: Swipe? = transition.initiallyInteractive
            ? (transition.viewController(forKey: .from) as? RouteController).map { ($0.node, $0.key, navigationController.viewControllers.count + 1) }
            : nil
        transition.animate(alongsideTransition: nil) { [weak self, weak navigationController] context in
            guard let self, let navigationController else { return }
            transitionEnded(navigationController, swipe: swipe, cancelled: context.isCancelled)
        }
        // That paints what shows as it starts. A swipe reveals the route a
        // strip at a time, and painting stopped there until the swipe ended
        // (a back swipe showed the list's top half, then its bottom half).
        // Paint each frame's newly revealed text until the transition is over.
        startRevealing()
    }

    private func startRevealing() {
        guard !FrameClock.shared.wants(self) else { return }
        FrameClock.shared.want(self, .navigationReveal, rate: FrameClock.full(on: presenter.viewport.window?.screen)) { [weak self] _ in self?.revealTick() }
    }

    private func revealTick() {
        guard changing else { stopRevealing(); return }
        presenter.paintVisibleText()
    }

    private func stopRevealing() {
        FrameClock.shared.drop(self)
    }

    func navigationController(_ navigationController: UINavigationController, didShow viewController: UIViewController, animated: Bool) {
        guard navigationController === navigation, navigationController.topViewController === viewController else {
            recordPop(navigationController)
            return
        }
        HostLog.note(presenter.session, "navigation: didShow \((viewController as? RouteController)?.key ?? "?")")
        // An animated push or pop ends in its coordinator's completion
        guard pushOrPop(on: navigationController) == nil else { return }
        transitionEnded(navigationController, swipe: nil, cancelled: false)
    }

    /// An interactive pop's source and the stack's depth before it began.
    typealias Swipe = (node: NodeView, key: String, depth: Int)

    /// A push or pop of the stack navigation follows is over: the deferred
    /// work runs and a Back UIKit performed reaches the router. One on a
    /// stack navigation stopped following (another tab was selected, or
    /// the stacks were reset) is not navigation's.
    private func transitionEnded(_ navigationController: UINavigationController,
                                 swipe: Swipe?, cancelled: Bool) {
        // Finish the departed search field while its authored handlers still exist
        if !cancelled, navigationController === navigation,
           let written = stacks[ObjectIdentifier(navigationController)]?.written {
            let departed = Set(written).subtracting(navigationController.viewControllers.map(ObjectIdentifier.init))
            for route in controllers.values where departed.contains(ObjectIdentifier(route)) {
                if route.search?.endEditing(completion: { [weak self, weak navigationController] in
                    guard let self, let navigationController else { return }
                    completeTransition(navigationController, swipe: swipe, cancelled: cancelled)
                }) == true { return }
            }
        }
        completeTransition(navigationController, swipe: swipe, cancelled: cancelled)
    }

    private func completeTransition(_ navigationController: UINavigationController,
                                    swipe: Swipe?, cancelled: Bool) {
        // Identify the visit UIKit removed before recording its shorter stack
        let departed = stacks[ObjectIdentifier(navigationController)]?.written.last.flatMap { id in
            controllers.values.first { ObjectIdentifier($0) == id && !navigationController.viewControllers.contains($0) }
        }
        let written = Set(stacks[ObjectIdentifier(navigationController)]?.written ?? [])
        let poppedKeys = controllers.values.filter { written.contains(ObjectIdentifier($0)) && !navigationController.viewControllers.contains($0) }.map(\.key)
        let source = swipe.map { (node: $0.node, key: $0.key) } ?? departed.map { (node: $0.node, key: $0.key) }
        recordPop(navigationController)
        guard navigationController === navigation, let viewController = navigationController.topViewController else { return }
        nativeMoved = true
        stopRevealing()
        // The route a pop took away keeps its block until it leaves the stack (LLP 1123 D2)
        defer { presenter.owners.publish() }
        defer {
            // Tree updates during UIKit's transition retain their latest
            // intent. Apply it once the native stack is available again.
            if pendingSync {
                pendingSync = false
                sync(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil))
            }
            presenter.session?.view?.fit()
            presenter.syncModal()
            // What the settled route shows has pixels (the swipe may have
            // left a band of it unpainted, or a cancelled pop the source).
            presenter.paintVisibleText()
            presenter.flushPendingFocus()
            presenter.elementScrolls.flush()
            #if os(iOS)
            presenter.syncScrollsToTop() // the route now on top owns the status-bar tap
            #endif
            // Settled on a stack's root: once UIKit has finished the
            // transition (its own bar restoration included), a root whose
            // arrival no projection has handled yet reconciles the bar with
            // its tablist (§3.7).
            DispatchQueue.main.async { [weak self, weak navigationController] in
                if let navigationController { self?.settleTablist(navigationController) }
            }
            // At rest: a large title's insets are sampled now (§9.10).
            coversChanged()
            #if os(iOS)
            presenter.resolveStatusBar(settled: true)
            #endif
        }
        let sourceKey = source.flatMap { presenter.views[$0.node.id] === $0.node ? $0.key : nil }
        // Replaced in place while the finger was down: the source's node is gone and the app's
        // stack is as deep as it was when the swipe began.
        let sourceReplaced = swipe != nil && sourceKey == nil
            && selectedRouteCount == swipe?.depth
        let rootKey = selectedKey ?? ""
        let uikitTop = (viewController as? RouteController)?.key ?? ""
        let sourceRule = (viewController as? RouteController).map {
            NavigationRules.dispatchesBack(shownKey: $0.key, rootKey: rootKey,
                                           sourceKey: sourceKey, sourceReplaced: sourceReplaced,
                                           modalActive: presenter.modals.inTransition)
        } ?? false
        let behindUIKit = !presenter.modals.inTransition
            && NavigationRules.appIsBehind(poppedKeys: poppedKeys, uikitTop: uikitTop, shownKeys: shownKeys, rootKey: rootKey)
        // A cancelled pop performed no Back, whatever the app did to its source meanwhile
        let dispatches = !cancelled && (sourceRule || behindUIKit)
        if dispatches, !sourceRule {
            presenter.session?.log("navigation: UIKit popped to \(uikitTop); the app's stack follows it from \(rootKey)")
        }
        lastTransition = dispatches ? "completed" : (cancelled ? "cancelled" : "idle")
        guard dispatches else { return }
        if swipe == nil, let destination = (viewController as? RouteController)?.key,
           let from = shownKeys.firstIndex(of: rootKey),
           let to = shownKeys.firstIndex(of: destination), from - to > 1 {
            goBack(from: rootKey, to: destination)
            return
        }
        goBack()
    }

    /// A stack Exact retired: its handle goes.
    func retireStack(_ nav: UINavigationController) {
        guard let stack = stacks.removeValue(forKey: ObjectIdentifier(nav)), stack.hatched else { return }
        _ = presenter.session?.natives.navigationHatch(nav, built: false, showsBar: stack.showsBar, label: stack.label)
    }

    func reset(clearFocus: Bool = true) {
        syncAfterTransitionArmed = false
        unanimated = false
        presenter.modals.reset()
        for nav in presentedNavigations { retireNavigation(nav, preserving: false) }
        for c in controllers.values { end(c) }
        retireTabs()
        if let primaryNavigation { retireStack(primaryNavigation) }
        if !covers.isEmpty || !delivered.isEmpty {
            let cleared = Set(covers.keys).union(delivered.keys).filter { presenter.views[$0] != nil }
            covers = [:]
            delivered = [:]
            if !cleared.isEmpty { presenter.onCovers?(cleared.map { ($0, nil) }) }
        }
        primaryNavigation?.delegate = nil
        primaryNavigation?.willMove(toParent: nil)
        primaryNavigation?.view.removeFromSuperview()
        primaryNavigation?.removeFromParent()
        primaryNavigation = nil
        container = nil
        logicalChildren = [:]
        shownKeys = []; fallbackKey = nil
        refitForBars()
        for controller in controllers.values { presenter.pressHandoff?.drop(controller.node) } // reset: no route of a press stays
        controllers.removeAll()
        routeIDs = []
        pendingSync = false
        pendingPress?.pending = false; pendingPress?.settle(); pendingPress = nil
        lastTransition = "idle"
        if clearFocus { presenter.cancelPendingFocus() }
        // The routes no stack holds now give their blocks back (LLP 1123 D2)
        presenter.owners.publish()
    }
}
#if os(tvOS)
extension NavigationHost {
    /// Whether the Siri Remote's Menu goes back: a route to pop and a Back control.
    var menuGoesBack: Bool { (navigation?.viewControllers.count ?? 0) > 1 && canInvokeBack }
    func menuBack() {
        guard menuGoesBack, !changing else { return }
        goBack()
    }
}
#endif
#endif

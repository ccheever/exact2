// @ref LLP 1008 §9 — UIKit owns modal presentation and its keyboard guide.
// The session's viewport moves into that container; layout remains the kernel's.
#if os(iOS) || os(tvOS)
import UIKit

private final class ModalController: UIViewController, UIGestureRecognizerDelegate {
    weak var host: ModalHost?
    let routeID: UInt32
    /// The route a `fit-content` detent measures (LLP 1075.003 §9.11).
    private weak var route: NodeView?
    var retiringRoot: NodeView?
    var retiringNavigation: UIViewController?
    /// The keyboard probe and a freeze's pixels: what this controller adds
    /// to its view beside the viewport (LLP 1080.001 D3).
    private(set) var probe: UIView?
    private(set) var pixels: UIView?
    init(host: ModalHost, route: NodeView, fullscreen: Bool, detent: String?) {
        self.host = host
        routeID = route.id
        self.route = route
        super.init(nibName: nil, bundle: nil)
        #if os(tvOS)
        // tvOS has no sheets; every modal covers the screen.
        modalPresentationStyle = .overFullScreen
        #else
        modalPresentationStyle = fullscreen ? .overFullScreen : .pageSheet
        if !fullscreen { updateDetent(detent) }
        #endif
    }
    private var detentValue: String?
    func updateDetent(_ value: String?) {
        #if !os(tvOS)
        guard modalPresentationStyle == .pageSheet,
              let sheet = sheetPresentationController,
              detentValue != value || sheet.detents.isEmpty else { return }
        detentValue = value
        let content = { [weak self] () -> CGFloat? in
            guard let route = self?.route else { return nil }
            // A scroller's extent counts its padding, and a container's
            // bottom cover (the sheet's safe area, which UIKit adds below the
            // detent) is padding; a route that does not scroll sends its
            // children's extent, which leaves it out (LLP 1075.003 §9.11).
            guard route.scroll != nil, case .edges(let e)? = self?.host?.presenter.navigation.covers[route.id] else { return route.content.height }
            return route.content.height - e.bottom
        }
        let configure = {
            // Authored points and `fit-content` exclude the bottom safe
            // area, which UIKit adds.
            // One resting height leaves overscroll and dismissal with UIKit;
            // several (`"300 large"`, LLP 1075.003 from James's review) make
            // the sheet resizable among them, from the first, with its grabber.
            let detents = ModalHost.detents(value, content: content)
            sheet.detents = detents.isEmpty ? [.large()] : detents
            sheet.selectedDetentIdentifier = sheet.detents.first?.identifier
            sheet.prefersGrabberVisible = detents.count > 1
            sheet.prefersScrollingExpandsWhenScrolledToEdge = false
        }
        if viewIfLoaded?.window != nil { sheet.animateChanges(configure) }
        else { configure() }
        #endif
    }
    /// The route's content extent changed: a `fit-content` detent resolves
    /// again, animated as UIKit animates a detent change, in the same sheet.
    func contentChanged() {
        #if !os(tvOS)
        guard modalPresentationStyle == .pageSheet, let sheet = sheetPresentationController,
              (detentValue ?? "").split(separator: " ").contains("fit-content") else { return }
        if viewIfLoaded?.window != nil { sheet.animateChanges { sheet.invalidateDetents() } }
        else { sheet.invalidateDetents() }
        #endif
    }
    required init?(coder: NSCoder) { nil }
    #if os(iOS)
    // LLP 1105 D6: UIKit asks a presented controller that covers the screen;
    // it answers the presenter's one resolved style, as the root does.
    override var preferredStatusBarStyle: UIStatusBarStyle { host?.presenter.statusBar.style ?? .default }
    #endif
    private var backdropTap: UITapGestureRecognizer?
    /// Each appearance, the first and one after a cancelled interactive dismissal.
    var appeared: (() -> Void)?
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        appeared?()
        #if os(iOS)
        // Back after a cancelled drag or zoom dismissal, which no completion
        // resolves: what landed during the gesture shows now (LLP 1105 D5).
        // The finished coordinator may still be installed: only a newly
        // started push or pop holds the style.
        let resolve = { [weak self] in self?.host?.presenter.resolveStatusBar(settled: true) }
        if !(transitionCoordinator?.animate(alongsideTransition: nil, completion: { _ in resolve() }) ?? false) { resolve() }
        #endif
        guard backdropTap == nil, let container = presentationController?.containerView else { return }
        let tap = UITapGestureRecognizer(target: self, action: #selector(tappedBackdrop))
        tap.delegate = self
        container.addGestureRecognizer(tap)
        backdropTap = tap
    }
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        if CanvasInputs.owns(touch.view) { return false }
        guard let container = presentationController?.containerView,
              let presented = presentationController?.presentedView,
              host?.canDismissByBackdrop(routeID) == true else { return false }
        return !presented.convert(presented.bounds, to: container).contains(touch.location(in: container))
    }
    @objc private func tappedBackdrop() { host?.dismissByBackdrop(routeID) }
    override func loadView() {
        view = UIView()
        #if os(tvOS)
        // tvOS has neither grouped backgrounds nor a keyboard layout guide.
        view.backgroundColor = .white
        #else
        view.backgroundColor = .secondarySystemGroupedBackground
        let probe = UIView()
        probe.isHidden = true
        probe.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(probe)
        self.probe = probe
        NSLayoutConstraint.activate([
            probe.topAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor),
            probe.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            probe.widthAnchor.constraint(equalToConstant: 0),
            probe.heightAnchor.constraint(equalToConstant: 0),
        ])
        #endif
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        host?.fit()
    }
    func freeze() {
        guard isViewLoaded, let pixels = view.snapshotView(afterScreenUpdates: false) else { return }
        pixels.frame = view.bounds
        pixels.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(pixels)
        self.pixels = pixels
    }
}

// Each boundary owns its presenting geometry and native controller. The
// viewport belongs to the top boundary; covered owners remain mounted.
private final class Presentation {
    let controller: ModalController
    let route: NodeView
    let kind: String
    let navigation: UIViewController
    weak var home: UIView?
    weak var owner: UIViewController?
    let background: UIViewController
    let backgroundNode: NodeView?
    weak var backgroundHome: UIView?
    let backgroundHomeFrame: CGRect
    let backgroundInteraction: Bool
    let backgroundAccessibility: Bool
    var geometry: [UInt32: (node: NodeView, ops: [BatchOp.Kind: BatchOp])] = [:]
    var presenting = true
    var animated = false
    /// Samples where a zoom lands while the route's nodes are still mapped:
    /// run as a Close destroys the route (`prepare`), so the zoom back starts
    /// from the photo as it is then, zoomed or panned.
    var sampleLanding: (() -> Void)?
    var alreadyDismissed = false

    init(host: ModalHost, route: NodeView, navigation: UIViewController,
         background: UIViewController, node: NodeView?, home: UIView, owner: UIViewController) {
        self.route = route
        kind = route.props["navigationPresentation"] ?? "modal"
        controller = ModalController(host: host, route: route, fullscreen: kind == "fullscreen", detent: route.props["navigationDetent"])
        self.navigation = navigation
        self.background = background
        backgroundNode = node
        backgroundHome = background.view.superview
        backgroundHomeFrame = background.view.frame
        backgroundInteraction = background.view.isUserInteractionEnabled
        backgroundAccessibility = background.view.accessibilityElementsHidden
        self.home = home
        self.owner = owner
    }
}

/// A zoom's last alignment answer (`ModalHost.present`), and whether the
/// zoom now running is the person's interactive dismissal, which lands
/// unaligned.
final class ZoomLanding {
    var rect: CGRect?
    private(set) var interactive = false
    /// `interactiveDismissShouldBegin`'s answer, passed through: an accepted
    /// dismissal lands unaligned until the route appears again.
    func dismissal(begins: Bool) -> Bool {
        if begins { interactive = true }
        return begins
    }
    /// The route appeared: first, or again after the finger cancelled.
    func appeared() { interactive = false }
    /// What `alignmentRectProvider` answers for a resolved rect: none while an
    /// interactive dismissal runs. UIKit follows the finger with the whole
    /// route and, given an alignment rect, re-bases the content onto it at
    /// lift-off: a visible jump and a held frame (a plain UIKit fixture does
    /// the same). Unaligned, the release carries the drag on in one motion,
    /// the route cross-fading into its source.
    func answer(_ resolved: CGRect?) -> CGRect? {
        if let resolved { rect = resolved }
        return interactive ? nil : rect
    }
}

final class ModalHost: NSObject, UIAdaptivePresentationControllerDelegate {
    unowned let presenter: Presenter
    private var layers: [Presentation] = []
    // Native dismissal outlives logical removal and reload. Retire from the
    // top down, without letting an old completion touch a new owner.
    private var retiring: [Presentation] = []
    private var dismissing: Presentation?
    private var closing = false
    /// The fullscreen controller hidden until its zoom starts (`present`).
    private weak var zoomGuard: UIViewController?
    private var refusedRoute: UInt32?
    var active: Bool { !layers.isEmpty || !retiring.isEmpty || closing }
    var inTransition: Bool {
        defersFocus || isDismissing || layers.contains { $0.controller.isBeingPresented || $0.controller.isBeingDismissed }
    }
    var isDismissing: Bool { !retiring.isEmpty }
    /// LLP 1080.001 D3: every presentation's controller view, live and
    /// retiring, with what the presenter put in it beside the viewport; and
    /// whether a node's geometry is held while a modal owns the viewport.
    var inspectionRoots: [(view: UIView, label: String, owned: [UIView])] {
        var seen = Set<ObjectIdentifier>()
        return (layers.map { ($0, false) } + retiring.map { ($0, true) }).compactMap { layer, retiring in
            guard let view = layer.controller.viewIfLoaded, seen.insert(ObjectIdentifier(view)).inserted else { return nil }
            let owned = [layer.controller.probe, layer.controller.pixels, layer.background.viewIfLoaded].compactMap { $0 }
            return (view, "modal #\(layer.route.id)\(retiring ? " (retiring)" : "")", owned)
        }
    }
    /// The removed routes and navigations a dismissal keeps on screen until
    /// UIKit ends it: unmapped node views on purpose.
    var inspectionRetiring: [UIView] {
        (layers + retiring).flatMap { [$0.controller.retiringRoot, $0.controller.retiringNavigation?.viewIfLoaded].compactMap { $0 } }
    }
    func holdsGeometry(_ id: UInt32) -> Bool { (layers + retiring).contains { $0.geometry[id] != nil } }
    var defersFocus: Bool { closing || layers.contains { $0.presenting } }
    var closedby: String? { layers.last?.route.props["closedby"] }
    var presentation: String? { layers.last?.kind }
    var coordinateView: UIView? { layers.last?.controller.viewIfLoaded }
    /// Whether the topmost presentation is a sheet at its content's height
    /// (LLP 1075.003 §9.11): the viewport is that sheet, and the page's
    /// height units the screen's, which the sheet cannot move.
    var fitsContent: Bool {
        #if os(iOS)
        guard let layer = layers.last, layer.kind != "fullscreen" else { return false }
        return (layer.route.props["navigationDetent"] ?? "").split(separator: " ").contains("fit-content")
        #else
        return false
        #endif
    }
    var owner: UIViewController? { layers.last?.controller }
    var routes: [(node: NodeView, kind: String)] { layers.map { ($0.route, $0.kind) } }
    #if os(iOS)
    /// The topmost presented route the status bar sits over (LLP 1105 D2): a
    /// full-screen one always; a sheet only at its large detent on a
    /// compact-width screen. Nil when the bar is over the screen behind.
    /// The topmost covering presentation's navigation view (its routes and
    /// their pushes, not the app's root nodes that ride along with the
    /// viewport); nil when the bar is over the primary screen. Every
    /// presentation's navigation view, which the primary screen's scope
    /// leaves out; and every presentation's whole view, where a node may sit.
    var statusBarScope: (covering: UIView?, presented: [UIView], views: [UIView]) {
        let covering = layers.last { layer in
            let c = layer.controller
            if c.modalPresentationStyle == .overFullScreen { return true }
            guard let sheet = c.sheetPresentationController, c.traitCollection.horizontalSizeClass == .compact else { return false }
            return (sheet.selectedDetentIdentifier ?? sheet.detents.first?.identifier) == .large
        }
        return (covering?.navigation.viewIfLoaded, layers.compactMap { $0.navigation.viewIfLoaded }, layers.compactMap { $0.controller.viewIfLoaded })
    }
    /// Every presented controller re-reads the style with the root (D6):
    /// UIKit asks the topmost one that covers, which a sheet over it may hide.
    func statusBarChanged() { for layer in layers { layer.controller.setNeedsStatusBarAppearanceUpdate() } }
    #endif

    init(presenter: Presenter) { self.presenter = presenter }

    #if os(iOS)
    /// A `navigationDetent`'s words as UIKit detents, in order; an unknown
    /// word is skipped. `content` is the route's laid-out content height,
    /// nil or zero before it is measured.
    static func detents(_ value: String?, content: @escaping () -> CGFloat?) -> [UISheetPresentationController.Detent] {
        (value ?? "").split(separator: " ").enumerated().compactMap { index, word in
            switch word {
            case "large": return .large()
            case "medium": return .medium()
            case "fit-content":
                // @ref LLP 1075.003 §9.11 — the route's content, which its
                // box (the sheet) does not change; before it is measured,
                // the sheet's maximum.
                return .custom(identifier: .init("fit-content-\(index)")) { context in
                    guard let height = content(), height > 0 else { return context.maximumDetentValue }
                    return min(height.rounded(.up), context.maximumDetentValue)
                }
            default:
                guard let height = Double(word), height.isFinite, height > 0 else { return nil }
                return .custom(identifier: .init("authored-\(index)")) { context in min(CGFloat(height), context.maximumDetentValue) }
            }
        }
    }
    #endif

    func canDismissByBackdrop(_ id: UInt32) -> Bool {
        guard let layer = layers.last, layer.route.id == id, layer.route.props["closedby"] == "any",
              !inTransition else { return false }
        return !refusesDismissal(of: layer.route)
    }
    func dismissByBackdrop(_ id: UInt32) {
        guard canDismissByBackdrop(id), let route = layers.last?.route else { return }
        presenter.navigation.invokeBack(from: route)
    }

    func prepare(_ batch: Batch) {
        for layer in layers where batch.ops.contains(where: {
            $0.op == .destroy && $0.id == layer.route.id
        }) {
            let controller = layer.controller
            layer.sampleLanding?()
            controller.freeze()
            controller.retiringRoot = layer.route
            controller.retiringNavigation = presenter.navigation.preserveModalContent(in: controller)
        }
    }

    func retainsRemovedView(_ node: NodeView) -> Bool {
        (layers + retiring).contains { layer in
            guard let root = layer.controller.retiringRoot else { return false }
            return node === root || node.isDescendant(of: root)
        }
    }

    private func background(for node: NodeView) -> Presentation? {
        layers.first { layer in
            guard let source = layer.backgroundNode else { return false }
            return node === source || node.isDescendant(of: source)
        }
    }

    func defersGeometry(for node: NodeView) -> Bool { background(for: node) != nil }

    func deferGeometry(_ op: BatchOp, for node: NodeView) -> Bool {
        guard let layer = background(for: node) else { return false }
        let kind = op.op
        var saved = layer.geometry[node.id] ?? (node, [:])
        saved.ops[kind] = op
        layer.geometry[node.id] = saved
        return true
    }

    private func releaseBackground(_ layer: Presentation) {
        let background = layer.background
        layer.backgroundHome?.addSubview(background.view)
        background.view.frame = layer.backgroundHomeFrame
        background.view.isUserInteractionEnabled = layer.backgroundInteraction
        background.view.accessibilityElementsHidden = layer.backgroundAccessibility
        // Frames precede content extents, as in a normal batch. A retired
        // identity cannot replay geometry into its replacement.
        let geometry = layer.geometry
        layer.geometry = [:]
        for (id, kind) in NavigationRules.replayOrder(deferred: geometry.mapValues { Set($0.ops.keys.map(\.rawValue)) }) {
            if let saved = geometry[id], presenter.views[id] === saved.node,
               let op = saved.ops[BatchOp.Kind(rawValue: kind) ?? .unknown] { presenter.applyGeometry(op) }
        }
        for saved in geometry.values where presenter.views[saved.node.id] === saved.node {
            saved.node.restoreScrollPosition()
            saved.node.applyPendingScroll()
        }
    }

    func updatePermissions() {
        for layer in layers {
            layer.controller.isModalInPresentation = refusesDismissal(of: layer.route)
            layer.controller.updateDetent(layer.route.props["navigationDetent"])
        }
    }

    /// A route's content extent changed (a `content` op): its sheet's
    /// `fit-content` detent follows.
    func contentChanged(_ node: NodeView) {
        for layer in layers where layer.route === node { layer.controller.contentChanged() }
    }

    /// The image a zoom's end shows: the node itself, or the first image
    /// among its authored descendants (each node's children are in its
    /// `container`), depth first.
    static func zoomImage(_ node: NodeView) -> NodeView? {
        if node.kind == "image" { return node }
        for child in node.container.subviews.compactMap({ $0 as? NodeView }) {
            if let image = zoomImage(child) { return image }
        }
        return nil
    }

    /// Where a zoom lands, in the zoomed controller's view: the target's box,
    /// or for an image the image as drawn (its `object-fit` in its content
    /// box, clipped there), sized from its own pixels or, before they land,
    /// `fallbackNatural`.
    static func zoomAlignment(target: NodeView, fallbackNatural: CGSize?, in zoomed: UIView) -> CGRect {
        let box = target.convert(target.bounds, to: zoomed)
        guard let image = zoomImage(target),
              let natural = image.raster?.image.naturalSize ?? fallbackNatural, natural.width > 0, natural.height > 0 else { return box }
        let content = image.contentBox()
        let drawn = RasterGeometry.rect(natural: natural, content: content, fit: image.style["object_fit"]?.string ?? "fill")
        return image.convert(drawn.intersection(content), to: zoomed)
    }

    func canPresent(from parent: UIViewController, route: NodeView) -> Bool {
        guard parent.presentedViewController == nil else {
            if refusedRoute != route.id {
                refusedRoute = route.id
                presenter.session?.log("modal route #\(route.id) refused: the owning controller already presents")
            }
            return false
        }
        refusedRoute = nil
        return true
    }

    func present(_ route: NodeView, navigation: UIViewController, from background: UIViewController,
                 node: NodeView?, preceding: NodeView?, owner: UIViewController) {
        guard let home = presenter.viewport.superview else { return }
        let layer = Presentation(host: self, route: route, navigation: navigation,
                                 background: background, node: node, home: home, owner: owner)
        let frame = background.view.convert(background.view.bounds, to: home)
        background.view.isUserInteractionEnabled = false
        background.view.accessibilityElementsHidden = true
        home.addSubview(background.view)
        background.view.frame = frame
        let controller = layer.controller
        if #available(iOS 18.0, tvOS 18.0, *), layer.kind == "fullscreen", route.props["navigationSource"] != nil {
            let options = UIViewController.Transition.ZoomOptions()
            let landing = ZoomLanding()
            options.interactiveDismissShouldBegin = { [weak self, weak route] context in
                guard let self, let route, context.willBegin else { return false }
                return landing.dismissal(begins: !self.refusesDismissal(of: route))
            }
            controller.appeared = landing.appeared
            // Where the source lands: the presented route's own element with
            // the source's id, when it has one; for an image, the image as it
            // is drawn (`object-fit`), so the zoom morphs photo into photo
            // rather than the whole route into the thumbnail with two photos
            // cross-fading (LLP 1035.001, the zoom's alignment).
            // The last answer, for the zoom back after a Close: by then the
            // route's nodes have left the presenter's map (`release`) though
            // UIKit still shows them. `prepare` samples it just before.
            let resolve: (UIView, NodeView?) -> CGRect? = { [weak self, weak route] zoomed, sourceView in
                guard let self, let route, let name = route.props["navigationSource"],
                      let target = presenter.views.values.filter({
                          $0.props["id"] == name && $0.window != nil && $0.isDescendant(of: route)
                      }).min(by: { $0.id < $1.id }) else { return nil }
                // Before the viewer's own pixels land, the source's image (the
                // view UIKit zooms from) is the same photo.
                let source = sourceView.flatMap(Self.zoomImage)
                return Self.zoomAlignment(target: target, fallbackNatural: source?.raster?.image.naturalSize, in: zoomed)
            }
            options.alignmentRectProvider = { context in
                landing.answer(context.zoomedViewController.viewIfLoaded.flatMap { resolve($0, context.sourceView as? NodeView) })
            }
            layer.sampleLanding = { [weak controller] in
                if let zoomed = controller?.viewIfLoaded, let rect = resolve(zoomed, nil) { landing.rect = rect }
            }
            controller.preferredTransition = .zoom(options: options, sourceViewProvider: { [weak self, weak route, weak preceding] _ in
                guard let self, let route, let preceding,
                      presenter.views[preceding.id] === preceding,
                      let name = route.props["navigationSource"] else { return nil }
                return presenter.views.values.filter {
                    $0.props["id"] == name && $0.window != nil &&
                        ($0 === preceding || $0.isDescendant(of: preceding))
                }.min(by: { $0.id < $1.id })
            })
            // Hidden until UIKit's zoom has it: on a device a frame could be
            // drawn with the presented view in place at full size before the
            // zoom took it over (one frame of the whole destination, then the
            // presenting screen again, then the zoom).
            controller.view.alpha = 0
            zoomGuard = controller
        }
        layers.append(layer)
        controller.isModalInPresentation = refusesDismissal(of: route)
        controller.loadViewIfNeeded()
        presenter.navigation.move(to: controller) { controller.view.addSubview(presenter.viewport) }
        controller.presentationController?.delegate = self
        let guarded = zoomGuard === controller
        zoomGuard = nil
        owner.present(controller, animated: !ExactEnv.agentFreezes) { [weak self, weak layer, weak controller] in
            if guarded { UIView.performWithoutAnimation { controller?.view.alpha = 1 } }
            guard let self, let layer else { return }
            layer.presenting = false
            #if os(iOS)
            // Up: what the bar sits over now (LLP 1105 D5).
            presenter.resolveStatusBar()
            #endif
            DispatchQueue.main.async { [weak self, weak layer] in
                guard let self, let layer else { return }
                if layers.contains(where: { $0 === layer }) {
                    fit()
                    presenter.navigation.modalDidDismiss()
                }
                // The transition's transforms are gone: a transform drag in
                // the presented route measures again, or it stays refused
                // until something else lays out (LLP 1057.001 §4).
                presenter.transformGeometry.changed()
                presenter.syncModal()
                drainRetired()
            }
        }
        if guarded {
            // Shown as the transition starts, in the block UIKit runs with
            // its first frame; at once if there is no coordinator or it
            // refuses the block; and in any case when the presentation ends.
            let show = { UIView.performWithoutAnimation { controller.view.alpha = 1 } }
            let taken = controller.transitionCoordinator?.animate(alongsideTransition: { _ in show() }, completion: { _ in show() }) ?? false
            if !taken { show() }
        }
        fit()
    }

    func fit() {
        guard !closing else { return }
        presenter.session?.view?.fit()
        presenter.flushPendingFocus()
    }

    func closeTop(animated: Bool = !ExactEnv.agentFreezes, refit: Bool = true) {
        guard let layer = layers.last else { return }
        closing = true
        if !layer.alreadyDismissed { presenter.cancelPendingFocus() }
        let controller = layer.controller
        if let owner = layer.owner, let home = layer.home {
            if controller.retiringNavigation != nil {
                home.addSubview(presenter.viewport)
            } else {
                presenter.navigation.move(to: owner) { home.addSubview(presenter.viewport) }
            }
        }
        presenter.navigation.retireNavigation(layer.navigation, preserving: controller.retiringNavigation != nil)
        layers.removeLast()
        releaseBackground(layer)
        layer.animated = animated
        retiring.append(layer)
        if refit { presenter.session?.view?.fit() }
        closing = false
        // Post-batch focus transfers before dismissal releases the old editor.
        DispatchQueue.main.async { [weak self] in self?.drainRetired() }
    }

    private func drainRetired() {
        guard dismissing == nil, let layer = retiring.first, !layer.presenting else { return }
        dismissing = layer
        let completion = { [weak self, layer] in
            guard let self, dismissing === layer else { return }
            dismissing = nil
            retiring.removeAll { $0 === layer }
            #if os(iOS)
            // Gone: what the bar sits over now (LLP 1105 D5).
            presenter.resolveStatusBar()
            #endif
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                drainRetired()
                guard let session = presenter.session, session.state != .destroyed else { return }
                presenter.navigation.modalDidDismiss()
                fit()
                // As after a presentation: what the dismissal's transition
                // covered measures again (a drag in the route left on top).
                presenter.transformGeometry.changed()
                presenter.syncModal()
            }
        }
        if layer.alreadyDismissed || layer.controller.presentingViewController == nil { completion() }
        else { layer.controller.dismiss(animated: layer.animated, completion: completion) }
    }

    private func refusesDismissal(of route: NodeView) -> Bool {
        presenter.views[route.id] !== route || presenter.navigation.isInactiveRoute(containing: route) ||
            NavigationRules.modalRefusesDismissal(closedby: route.props["closedby"]) || !presenter.navigation.canInvokeBack
    }

    func presentationControllerShouldDismiss(_ presentationController: UIPresentationController) -> Bool {
        guard let layer = layers.last, layer.controller === presentationController.presentedViewController else { return false }
        return !refusesDismissal(of: layer.route)
    }

    func presentationControllerDidAttemptToDismiss(_ presentationController: UIPresentationController) {
        guard layers.last?.controller === presentationController.presentedViewController,
              !presenter.navigation.canInvokeBack else { return }
        presenter.session?.log("modal dismissal refused: no enabled navigationBack control in the active route")
    }

    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
        guard let layer = layers.last, layer.controller === presentationController.presentedViewController else { return }
        closing = true
        presenter.cancelPendingFocus()
        layer.presenting = false
        layer.alreadyDismissed = true
        // UIKit still uses this hierarchy on its callback stack. Remove its
        // owner before dispatch, so an action may safely keep the same route.
        DispatchQueue.main.async { [weak self, layer] in
            guard let self, layers.last === layer else { return }
            closeTop(animated: false, refit: false)
            presenter.navigation.invokeBack(from: layer.route)
            presenter.navigation.modalDidDismiss()
            fit()
        }
    }

    func unmounted() { reset() }

    func reset() {
        // Restore the viewport immediately, then dismiss native owners from
        // the top down. New projection waits until all retired owners finish.
        for layer in layers.reversed() {
            layer.controller.freeze()
            layer.controller.retiringRoot = layer.route
            layer.controller.retiringNavigation = presenter.navigation.preserveModalContent(in: layer.controller)
            closeTop(animated: false, refit: false)
        }
    }
}
#endif

#if os(iOS)
// A sheet dragged between detents moves what the status bar sits over
// with no batch (LLP 1105 D5).
extension ModalHost: UISheetPresentationControllerDelegate {
    func sheetPresentationControllerDidChangeSelectedDetentIdentifier(_ sheetPresentationController: UISheetPresentationController) {
        presenter.resolveStatusBar()
    }
}
#endif

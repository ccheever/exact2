// @ref LLP 1008 §9 — UIKit owns modal presentation and its keyboard guide.
// The session's viewport moves into that container; layout remains the kernel's.
#if os(iOS)
import UIKit

private final class ModalController: UIViewController, UIGestureRecognizerDelegate {
    weak var host: ModalHost?
    let routeID: UInt32
    var retiringRoot: NodeView?
    var retiringNavigation: UIViewController?
    /// The keyboard probe and a freeze's pixels: what this controller adds
    /// to its view beside the viewport (LLP 1080.001 D3).
    private(set) var probe: UIView?
    private(set) var pixels: UIView?
    init(host: ModalHost, routeID: UInt32, fullscreen: Bool, detent: String?) {
        self.host = host
        self.routeID = routeID
        super.init(nibName: nil, bundle: nil)
        modalPresentationStyle = fullscreen ? .overFullScreen : .pageSheet
        if !fullscreen { updateDetent(detent) }
    }
    private var detentValue: String?
    func updateDetent(_ value: String?) {
        guard modalPresentationStyle == .pageSheet,
              let sheet = sheetPresentationController,
              detentValue != value || sheet.detents.isEmpty else { return }
        detentValue = value
        let configure = {
            // Authored points exclude the bottom safe area, which UIKit adds.
            // One resting height leaves overscroll and dismissal with UIKit;
            // several (`"300 large"`, LLP 1075.003 from James's review) make
            // the sheet resizable among them, from the first, with its grabber.
            let detents = (value ?? "").split(separator: " ").enumerated().compactMap { index, word -> UISheetPresentationController.Detent? in
                switch word {
                case "large": return .large()
                case "medium": return .medium()
                default:
                    guard let height = Double(word), height.isFinite, height > 0 else { return nil }
                    return .custom(identifier: .init("authored-\(index)")) { context in min(CGFloat(height), context.maximumDetentValue) }
                }
            }
            sheet.detents = detents.isEmpty ? [.large()] : detents
            sheet.selectedDetentIdentifier = sheet.detents.first?.identifier
            sheet.prefersGrabberVisible = detents.count > 1
            sheet.prefersScrollingExpandsWhenScrolledToEdge = false
        }
        if viewIfLoaded?.window != nil { sheet.animateChanges(configure) }
        else { configure() }
    }
    required init?(coder: NSCoder) { nil }
    private var backdropTap: UITapGestureRecognizer?
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        guard backdropTap == nil, let container = presentationController?.containerView else { return }
        let tap = UITapGestureRecognizer(target: self, action: #selector(tappedBackdrop))
        tap.delegate = self
        container.addGestureRecognizer(tap)
        backdropTap = tap
    }
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        if CanvasInput.owns(touch.view) { return false }
        guard let container = presentationController?.containerView,
              let presented = presentationController?.presentedView,
              host?.canDismissByBackdrop(routeID) == true else { return false }
        return !presented.convert(presented.bounds, to: container).contains(touch.location(in: container))
    }
    @objc private func tappedBackdrop() { host?.dismissByBackdrop(routeID) }
    override func loadView() {
        view = UIView()
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
    var alreadyDismissed = false

    init(host: ModalHost, route: NodeView, navigation: UIViewController,
         background: UIViewController, node: NodeView?, home: UIView, owner: UIViewController) {
        self.route = route
        kind = route.props["navigationPresentation"] ?? "modal"
        controller = ModalController(host: host, routeID: route.id, fullscreen: kind == "fullscreen", detent: route.props["navigationDetent"])
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

final class ModalHost: NSObject, UIAdaptivePresentationControllerDelegate {
    unowned let presenter: Presenter
    private var layers: [Presentation] = []
    // Native dismissal outlives logical removal and reload. Retire from the
    // top down, without letting an old completion touch a new owner.
    private var retiring: [Presentation] = []
    private var dismissing: Presentation?
    private var closing = false
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
    var owner: UIViewController? { layers.last?.controller }
    var routes: [(node: NodeView, kind: String)] { layers.map { ($0.route, $0.kind) } }

    init(presenter: Presenter) { self.presenter = presenter }

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
        if #available(iOS 18.0, *), layer.kind == "fullscreen", route.props["navigationSource"] != nil {
            let options = UIViewController.Transition.ZoomOptions()
            options.interactiveDismissShouldBegin = { [weak self, weak route] context in
                guard let self, let route, context.willBegin else { return false }
                return !self.refusesDismissal(of: route)
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
        }
        layers.append(layer)
        controller.isModalInPresentation = refusesDismissal(of: route)
        controller.loadViewIfNeeded()
        presenter.navigation.move(to: controller) { controller.view.addSubview(presenter.viewport) }
        controller.presentationController?.delegate = self
        owner.present(controller, animated: !ExactEnv.agentFreezes) { [weak self, weak layer] in
            guard let self, let layer else { return }
            layer.presenting = false
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

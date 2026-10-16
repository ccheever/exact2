// @ref LLP 1008 §9 — UIKit owns modal presentation and its keyboard guide.
// @ref LLP 1123 D2 — a presentation mounts only its own navigation: the
// session's viewport and the document stay in the session's view.
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
    /// to its view beside the mount (LLP 1080.001 D3).
    private(set) var probe: UIView?
    private(set) var pixels: UIView?
    /// What the presentation's navigation is mounted in.
    let mount = PresentationMount()
    init(host: ModalHost, route: NodeView, fullscreen: Bool, detent: String?, grabber: String?) {
        self.host = host
        routeID = route.id
        self.route = route
        super.init(nibName: nil, bundle: nil)
        #if os(tvOS)
        // tvOS has no sheets; every modal covers the screen.
        modalPresentationStyle = .overFullScreen
        #else
        modalPresentationStyle = fullscreen ? .overFullScreen : .pageSheet
        if !fullscreen { updateSheet(detent: detent, cornerRadius: ModalController.cornerRadius(of: route), grabber: grabber) }
        #endif
    }
    /// A sheet route's authored `border-radius`, its top-left corner's: the
    /// sheet's corner radius. Nil leaves UIKit's, which on iOS 26 and later
    /// follows the display's corners and insets a large sheet from the bottom edge.
    static func cornerRadius(of route: NodeView) -> CGFloat? {
        guard route.wrote("border_radius_top_left") else { return nil }
        return BorderPaint.radii(route.style, in: route.bounds)[0].width
    }
    private var detentValue: String?, cornerRadius: CGFloat?, grabberValue: String?
    func updateSheet(detent value: String?, cornerRadius radius: CGFloat?, grabber: String?) {
        #if !os(tvOS)
        guard modalPresentationStyle == .pageSheet,
              let sheet = sheetPresentationController,
              detentValue != value || cornerRadius != radius || grabberValue != grabber || sheet.detents.isEmpty else { return }
        // A grabber or radius alone changes only itself: the detents and the
        // one the person dragged the sheet to stay.
        let newDetents = detentValue != value || sheet.detents.isEmpty
        detentValue = value
        cornerRadius = radius
        grabberValue = grabber
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
            if newDetents {
                var refused: [Substring] = []
                let detents = ModalHost.detents(value, refused: { refused.append($0) }, content: content)
                sheet.detents = detents.isEmpty ? [.large()] : detents
                if !refused.isEmpty, let route = self.route { self.host?.presenter.session?.log(ModalHost.refusal(detent: value ?? "", words: refused, route: route.props["navigationKey"] ?? "", kept: !detents.isEmpty)) }
                sheet.selectedDetentIdentifier = sheet.detents.first?.identifier
            }
            // `navigationGrabber` says; unsaid, a sheet that resizes has
            // one, as the HIG puts it on a resizable sheet.
            sheet.prefersGrabberVisible = grabber == "visible" || grabber != "hidden" && sheet.detents.count > 1
            sheet.preferredCornerRadius = radius
            // Several: scrolling at the content's top grows the sheet, as
            // UIKit's own sheets do (LLP 1115 wave 1).
            sheet.prefersScrollingExpandsWhenScrolledToEdge = sheet.detents.count > 1
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
        mount.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(mount)
        #if os(tvOS)
        // tvOS has neither grouped backgrounds nor a keyboard layout guide.
        view.backgroundColor = .white
        #else
        // A sheet's surface where the author painted none: the platform's
        // (LLP 1115 D2), which UIKit elevates in dark mode.
        view.backgroundColor = ThemeRoles.color("background") ?? ThemeRoles.color("Canvas", or: .systemBackground)
        let probe = UIView()
        probe.isHidden = true
        probe.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(probe)
        self.probe = probe
        // The guide is the keys alone: a route ends at them, not at the home
        // indicator (`OwnerBlocks.block`)
        view.keyboardLayoutGuide.usesBottomSafeArea = false
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
        // The keyboard probe lays the sheet out as its keyboard layout guide
        // moves, where its routes end (`OwnerBlocks.block`)
        host?.presenter.owners.publish()
    }
    func freeze() {
        guard isViewLoaded, let pixels = view.snapshotView(afterScreenUpdates: false) else { return }
        pixels.frame = view.bounds
        pixels.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(pixels)
        self.pixels = pixels
    }
}

// Each boundary owns its native controller and the navigation it mounts;
// covered owners remain mounted, inert.
private final class Presentation {
    let controller: ModalController
    let route: NodeView
    let kind: String
    let navigation: UIViewController
    let background: UIViewController
    let backgroundInteraction: Bool
    let backgroundAccessibility: Bool
    var presenting = true
    var animated = false
    /// Samples where a zoom lands while the route's nodes are still mapped:
    /// run as a Close destroys the route (`prepare`), so the zoom back starts
    /// from the photo as it is then, zoomed or panned.
    var sampleLanding: (() -> Void)?
    var alreadyDismissed = false

    init(host: ModalHost, route: NodeView, navigation: UIViewController,
         background: UIViewController) {
        self.route = route
        kind = route.props["navigationPresentation"] ?? "modal"
        controller = ModalController(host: host, route: route, fullscreen: kind == "fullscreen", detent: route.props["navigationDetent"], grabber: route.props["navigationGrabber"])
        self.navigation = navigation
        self.background = background
        backgroundInteraction = background.view.isUserInteractionEnabled
        backgroundAccessibility = background.view.accessibilityElementsHidden
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
    /// retiring, with what the presenter put in it: its probes and the mount
    /// its navigation is in.
    var inspectionRoots: [(view: UIView, label: String, owned: [UIView])] {
        var seen = Set<ObjectIdentifier>()
        return (layers.map { ($0, false) } + retiring.map { ($0, true) }).compactMap { layer, retiring in
            guard let view = layer.controller.viewIfLoaded, seen.insert(ObjectIdentifier(view)).inserted else { return nil }
            let owned = [layer.controller.probe, layer.controller.pixels, layer.controller.mount].compactMap { $0 }
            return (view, "modal #\(layer.route.id)\(retiring ? " (retiring)" : "")", owned)
        }
    }
    /// The removed routes and navigations a dismissal keeps on screen until
    /// UIKit ends it: unmapped node views on purpose.
    var inspectionRetiring: [UIView] {
        (layers + retiring).flatMap { [$0.controller.retiringRoot, $0.controller.retiringNavigation?.viewIfLoaded].compactMap { $0 } }
    }
    var defersFocus: Bool { closing || layers.contains { $0.presenting } }
    var closedby: String? { layers.last?.route.props["closedby"] }
    var presentation: String? { layers.last?.kind }
    var coordinateView: UIView? { layers.last?.controller.viewIfLoaded }
    /// Whether the top presentation, modal as every one is (LLP 1123 D2),
    /// makes `view` inert: it is neither in it nor in a `dialog`'s sheet,
    /// which UIKit presents over it (HTML modal dialogs).
    func inertBehind(_ view: UIView) -> Bool {
        guard let top = coordinateView else { return false }
        return !view.isDescendant(of: top) && !presenter.menus.sheets.shows(view)
    }
    var owner: UIViewController? { layers.last?.controller }
    /// The presentations' controller views, live and retiring, bottom to
    /// top: where the session mounts beside its viewport (LLP 1123 D2).
    var mountRoots: [UIView] { (layers + retiring).compactMap { $0.controller.viewIfLoaded } }
    /// The presentations' navigations, live and retiring, bottom to top.
    var navigations: [UINavigationController] { (layers + retiring).compactMap { $0.navigation as? UINavigationController } }
    /// The top live presentation's navigation, the one nothing is dismissing:
    /// UIKit's presentation makes what is under it inert.
    var topNavigation: UINavigationController? { layers.last?.navigation as? UINavigationController }
    func mounts(_ view: UIView) -> Bool { mountRoots.contains { view.isDescendant(of: $0) } }
    var routes: [(node: NodeView, kind: String)] { layers.map { ($0.route, $0.kind) } }
    /// Each presentation's mount and the route it holds.
    var mounted: [(mount: PresentationMount, route: NodeView)] { layers.map { ($0.controller.mount, $0.route) } }
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
    /// A `navigationDetent`'s words as UIKit detents, in order; a word that
    /// names none (a bound value the compiler did not see) is skipped and
    /// handed to `refused`. `content` is the route's laid-out content height,
    /// nil or zero before it is measured.
    static func detents(_ value: String?, refused: ((Substring) -> Void)? = nil, content: @escaping () -> CGFloat?) -> [UISheetPresentationController.Detent] {
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
            case _ where word.hasSuffix("%"):
                // @ref LLP 1075.003 §9.13 — a share of the sheet's maximum,
                // as `.custom { $0.maximumDetentValue * 0.45 }` is in UIKit.
                guard let share = Double(word.dropLast()), share.isFinite, share > 0, share <= 100 else { refused?(word); return nil }
                return .custom(identifier: .init("authored-\(index)")) { context in context.maximumDetentValue * CGFloat(share) / 100 }
            default:
                guard let height = Double(word), height.isFinite, height > 0 else { refused?(word); return nil }
                return .custom(identifier: .init("authored-\(index)")) { context in min(CGFloat(height), context.maximumDetentValue) }
            }
        }
    }

    /// The journal's line for a bound `navigationDetent` with words that name
    /// no detent: the value, the route, and what the sheet uses instead.
    static func refusal(detent value: String, words: [Substring], route: String, kept: Bool) -> String {
        let named = words.map { "\"\($0)\"" }.joined(separator: ", ")
        return "navigationDetent \"\(value)\" on route \"\(route)\": \(named) is not large, medium, fit-content, a positive point height or a percentage above 0% and at most 100%; the sheet uses \(kept ? "its other detents" : "the platform's large detent")"
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
    /// The agent's `back` on the top sheet (LLP 1116.006 D4): its swipe
    /// down, refused as UIKit's is (`presentationControllerShouldDismiss`)
    /// and ending as a completed one does (`invokeBack`). nil with no sheet
    /// up; "" when it went, else why not.
    func agentDismiss() -> String? {
        guard let route = layers.last?.route else { return nil }
        if inTransition { return "the sheet is still moving: `clock settle` first" }
        if NavigationRules.modalRefusesDismissal(closedby: route.props["closedby"]) { return "its closedby is \"none\"" }
        if refusesDismissal(of: route) { return presenter.navigation.backRefusal }
        presenter.navigation.invokeBack(from: route)
        return ""
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

    private func releaseBackground(_ layer: Presentation) {
        let background = layer.background
        background.view.isUserInteractionEnabled = layer.backgroundInteraction
        background.view.accessibilityElementsHidden = layer.backgroundAccessibility
    }

    func updatePermissions() {
        for layer in layers {
            layer.controller.isModalInPresentation = refusesDismissal(of: layer.route)
            layer.controller.updateSheet(detent: layer.route.props["navigationDetent"], cornerRadius: ModalController.cornerRadius(of: layer.route), grabber: layer.route.props["navigationGrabber"])
        }
    }

    /// What each presentation's route inherits from its logical ancestors,
    /// which UIKit's hierarchy no longer carries to it, mounted in its sheet
    /// (LLP 1123 D2): an authored `aria-hidden` and `inert` above it (WAI-ARIA
    /// tree exclusion, HTML inert subtrees), set on the view the navigation
    /// is mounted in, apart from the hiding a presentation over it gives its
    /// navigation; and a `color-scheme`, the presented controller's
    /// appearance, which UIKit carries to its sheet and everything in it.
    /// After every batch, a transition running or not.
    func inheritFromAncestors() {
        for layer in layers where layer.controller.isViewLoaded { inherit(layer) }
    }

    private func inherit(_ layer: Presentation) {
        var hidden = false, inert = false, scheme = UIUserInterfaceStyle.unspecified
        var at = presenter.eventParent(of: layer.route)
        while let view = at {
            if let node = view as? NodeView {
                hidden = hidden || node.props["accessibilityElementsHidden"] == "true"
                inert = inert || node.props["inert"] == "true"
                if scheme == .unspecified { scheme = node.overrideUserInterfaceStyle }
            }
            at = presenter.eventParent(of: view)
        }
        let mount = layer.controller.mount
        if mount.accessibilityElementsHidden != (hidden || inert) { mount.accessibilityElementsHidden = hidden || inert }
        if mount.isUserInteractionEnabled == inert {
            mount.isUserInteractionEnabled = !inert
            // An inert subtree holds no focus (HTML's inert), as `endEditing`
            // on an authored inert node ends what is under it
            if inert { mount.endEditing(true) }
        }
        // The trait update is taken now, as `applyColorScheme` takes it
        if layer.controller.overrideUserInterfaceStyle != scheme {
            layer.controller.overrideUserInterfaceStyle = scheme
            layer.controller.updateTraitsIfNeeded()
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
                 preceding: NodeView?, owner: UIViewController) {
        let layer = Presentation(host: self, route: route, navigation: navigation,
                                 background: background)
        // The presenting screen stays where it is, inert (HTML modal dialogs)
        background.view.isUserInteractionEnabled = false
        background.view.accessibilityElementsHidden = true
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
        controller.addChild(navigation)
        presenter.collections.watchContacts(in: controller.mount)
        controller.mount.addSubview(navigation.view)
        #if os(iOS)
        if controller.mount.relay == nil { controller.mount.relay = MountRelay(presenter, mount: controller.mount) }
        #endif
        navigation.view.setPaintForeground(aboveAuthored: false)
        navigation.view.frame = controller.mount.bounds
        navigation.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        navigation.didMove(toParent: controller)
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
            // Its routes, if they live on, give their blocks back (LLP 1123 D2)
            presenter.owners.publish()
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
        presenter.session?.log("modal dismissal refused: \(presenter.navigation.backRefusal)")
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
        // Dismiss native owners from the top down, each keeping what it shows
        // for its dismissal. New projection waits until all retired owners finish.
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

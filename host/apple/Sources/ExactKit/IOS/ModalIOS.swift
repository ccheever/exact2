// @ref LLP 1008 §9 — UIKit owns modal presentation and its keyboard guide.
// The session's viewport moves into that container; layout remains the kernel's.
#if os(iOS)
import UIKit

private final class ModalController: UIViewController {
    unowned let host: ModalHost
    let routeID: UInt32
    init(host: ModalHost, routeID: UInt32) {
        self.host = host
        self.routeID = routeID
        super.init(nibName: nil, bundle: nil)
        modalPresentationStyle = .pageSheet
        sheetPresentationController?.detents = [.large()]
    }
    required init?(coder: NSCoder) { nil }
    override func loadView() {
        view = UIView()
        view.backgroundColor = .secondarySystemGroupedBackground
        let probe = UIView()
        probe.isHidden = true
        probe.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(probe)
        NSLayoutConstraint.activate([
            probe.topAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor),
            probe.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            probe.widthAnchor.constraint(equalToConstant: 0),
            probe.heightAnchor.constraint(equalToConstant: 0),
        ])
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        host.fit()
    }
    func freeze() {
        guard isViewLoaded, let pixels = view.snapshotView(afterScreenUpdates: false) else { return }
        pixels.frame = view.bounds
        pixels.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(pixels)
    }
}

final class ModalHost: NSObject, UIAdaptivePresentationControllerDelegate {
    unowned let presenter: Presenter
    private var controller: ModalController?
    private weak var home: UIView?
    private weak var owner: UIViewController?
    private var background: UIViewController?
    private weak var backgroundNode: NodeView?
    private var backgroundFrame = CGRect.zero
    private var backgroundInteraction = true
    private var backgroundAccessibility = false
    private var deferredGeometry: [UInt32: (node: NodeView, ops: [String: [String: Any]])] = [:]
    private var closing = false
    private var presenting = false
    private var pendingFocus: (args: [Any], selectText: Bool)?
    private var deliveringFocus = false
    /// The route last journaled as refused, so a refusal is one line.
    private var refusedRoute: UInt32?
    var active: Bool { controller != nil || closing }
    /// A presentation or dismissal UIKit is animating (LLP 1035.003 D5).
    var inTransition: Bool { presenting || closing }
    /// The focus still waiting for the sheet to finish presenting, by id,
    /// and the modal route's close policy — for `state` (LLP 1035.002 D2).
    var pendingFocusTarget: String? { pendingFocus?.args.first as? String }
    var closedby: String? { controller.flatMap { presenter.views[$0.routeID]?.props["closedby"] } }
    var coordinateView: UIView? { controller?.viewIfLoaded }

    init(presenter: Presenter) { self.presenter = presenter }

    func prepare(_ batch: Batch) {
        if let controller {
            if batch.ops.contains(where: { $0["op"] as? String == "destroy" && $0["id"] as? Int == Int(controller.routeID) }) {
                controller.freeze()
            }
        }
    }

    // Retain the source's actual views after its batch (selection has ended),
    // before navigation removes its controller. Geometry stays in the presenting
    // surface's coordinates; dynamic colors and native materials remain live.
    func prepareBackground(_ source: UIViewController, node: NodeView) {
        guard background == nil, let exactView = presenter.session?.view else { return }
        background = source
        backgroundNode = node
        backgroundFrame = source.view.convert(source.view.bounds, to: exactView)
        backgroundInteraction = source.view.isUserInteractionEnabled
        backgroundAccessibility = source.view.accessibilityElementsHidden
    }

    func defersGeometry(for node: NodeView) -> Bool {
        guard let source = backgroundNode else { return false }
        return node === source || node.isDescendant(of: source)
    }

    func deferGeometry(_ op: [String: Any], for node: NodeView) -> Bool {
        guard defersGeometry(for: node), let kind = op["op"] as? String else { return false }
        var saved = deferredGeometry[node.id] ?? (node, [:])
        saved.ops[kind] = op
        deferredGeometry[node.id] = saved
        return true
    }

    func releaseBackground() {
        if let background {
            background.willMove(toParent: nil)
            background.view.removeFromSuperview()
            background.removeFromParent()
            background.view.isUserInteractionEnabled = backgroundInteraction
            background.view.accessibilityElementsHidden = backgroundAccessibility
        }
        background = nil
        backgroundNode = nil
        let geometry = deferredGeometry
        deferredGeometry = [:]
        // Catch the views up to the kernel's latest geometry before the normal
        // viewport resize: frames before content sizes, ids ascending, as a
        // batch applies them (LLP 1035.001 D4, `NavigationRules.replayOrder`).
        for (id, kind) in NavigationRules.replayOrder(deferred: geometry.mapValues { Set($0.ops.keys) }) {
            if let saved = geometry[id], presenter.views[id] === saved.node,
               let op = saved.ops[kind] { presenter.applyGeometry(op) }
        }
        for saved in geometry.values where presenter.views[saved.node.id] === saved.node {
            saved.node.restoreScrollPosition()
            saved.node.applyPendingScroll()
        }
    }

    func sync(_ route: NodeView?) {
        guard !closing else { return }
        guard let route else {
            if controller != nil { finish(animated: !ExactEnv.agentFreezes) }
            return
        }
        if let controller {
            controller.isModalInPresentation = NavigationRules.modalRefusesDismissal(closedby: route.props["closedby"])
            return
        }
        guard let exactView = presenter.session?.view,
              let parent = presenter.navigation.owner else { return }
        guard parent.presentedViewController == nil else {
            // D6: a refused intent is a journal line, not silence.
            if refusedRoute != route.id {
                refusedRoute = route.id
                presenter.session?.log("modal route #\(route.id) refused: the owning controller already presents")
            }
            return
        }
        refusedRoute = nil
        home = presenter.viewport.superview
        owner = parent
        if let background {
            parent.addChild(background)
            background.view.frame = backgroundFrame
            background.view.isUserInteractionEnabled = false
            background.view.accessibilityElementsHidden = true
            exactView.addSubview(background.view)
            background.didMove(toParent: parent)
        }
        let sheet = ModalController(host: self, routeID: route.id)
        controller = sheet
        sheet.isModalInPresentation = NavigationRules.modalRefusesDismissal(closedby: route.props["closedby"])
        sheet.loadViewIfNeeded()
        presenter.navigation.move(to: sheet) { sheet.view.addSubview(presenter.viewport) }
        sheet.presentationController?.delegate = self
        presenting = true
        parent.present(sheet, animated: !ExactEnv.agentFreezes) { [weak self] in
            self?.presenting = false
            self?.fit()
        }
        fit()
    }

    func fit() {
        guard !closing else { return }
        presenter.session?.view?.fit()
        flushFocus()
    }

    func deferFocus(_ args: [Any], selectText: Bool = false) -> Bool {
        guard presenting, !deliveringFocus else { return false }
        pendingFocus = (args, selectText)
        flushFocus()
        return true
    }

    private func flushFocus() {
        guard let pending = pendingFocus, let name = pending.args.first as? String,
              let node = presenter.views.values.first(where: { $0.props["id"] == name }),
              node.window != nil, node.bounds.width > 0, node.bounds.height > 0 else { return }
        pendingFocus = nil
        deliveringFocus = true
        presenter.focusElement(pending.args, selectText: pending.selectText)
        deliveringFocus = false
    }

    private func restoreViewport() {
        if let owner, let home {
            presenter.navigation.move(to: owner) { home.addSubview(presenter.viewport) }
        }
        releaseBackground()
    }

    private func finish(animated: Bool, refit: Bool = true) {
        guard let sheet = controller else { return }
        closing = true
        pendingFocus = nil
        presenting = false
        restoreViewport()
        controller = nil
        if refit { presenter.session?.view?.fit() }
        sheet.dismiss(animated: animated)
        closing = false
        home = nil
        owner = nil
    }

    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
        guard controller === presentationController.presentedViewController else { return }
        closing = true
        pendingFocus = nil
        presenting = false
        restoreViewport()
        controller = nil
        presenter.navigation.invokeBack()
        presenter.session?.view?.fit()
        closing = false
        home = nil
        owner = nil
    }

    func reset() {
        // A reload has replaced the runtime, but its initial batch is not
        // mounted yet. ExactView.rebooted() refits after that batch is applied.
        if controller != nil { finish(animated: false, refit: false) }
        releaseBackground()
    }
}
#endif

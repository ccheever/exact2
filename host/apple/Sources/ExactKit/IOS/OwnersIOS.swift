// @ref LLP 1123 D2 — what of a window is the session's: its viewport, and
// each presentation's controller view, where a presented route is mounted.
// Whether a view is mounted there, where a finger reaches it, and ending
// the session's editing ask these, never whether the viewport holds a view.
#if os(iOS) || os(tvOS)
import UIKit

extension Presenter {
    /// Whether `view` is mounted in this session: in its viewport, or in a
    /// presentation beside it.
    func mounts(_ view: UIView) -> Bool {
        view.isDescendant(of: viewport) || modals.mounts(view)
    }

    /// Where a finger reaches `view`, in viewport points (the viewport's
    /// coordinates less its scrolling, as the agent's boxes are): the
    /// viewport for the document; for a presentation's content, its
    /// presentation's view within the window.
    func reach(of view: UIView) -> CGRect {
        let port = CGRect(origin: .zero, size: viewport.bounds.size)
        guard !view.isDescendant(of: viewport), let window = viewport.window,
              let mount = modals.mountRoots.last(where: { view.isDescendant(of: $0) }) else { return port }
        let shown = mount.convert(mount.bounds, to: nil).intersection(window.bounds)
        return viewportPoints(viewport.convert(shown, from: nil))
    }

    /// Ends editing wherever the session is mounted; whether any editor resigned.
    @discardableResult
    package func endEditing() -> Bool {
        modals.mountRoots.reduce(viewport.endEditing(true)) { $1.endEditing(true) || $0 }
    }
}
#endif

#if os(iOS) || os(tvOS)
/// Each route's root laid out in its controller's view (LLP 1123 D2): the
/// route is a content owner, and its view's bounds are its containing block
/// (LLP 1115.002 D8.2), as a pane's are on the Mac (BlockHostMac.swift).
/// The runner is told where each route a window shows is: a presented
/// route as a top-layer box, as a dialog is (`openTopLayerFrames`), and
/// any other by a placement, its block in the routes' owner's local
/// coordinates, sent with the block when either changes, so that it keeps
/// its place among its siblings.
final class OwnerBlocks {
    private unowned let presenter: Presenter
    /// What the runtime was last told, by route: its block, and its
    /// placement unless it is presented.
    private(set) var told: [UInt32: (block: HostBlock, placement: BlockPlacement?)] = [:]
    private var resetting = false
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Every route a navigation holds, the primary stacks' first and each
    /// presentation's after, bottom to top; in each stack in its order, the
    /// route a pop is taking away last until the transition ends. Each with
    /// its controller's view, its block in that view's coordinates, and
    /// whether a window shows that view: a route under a pushed one, or in a
    /// tab not selected, is out of the window.
    private var routes: [(route: RouteController, view: UIView, block: CGRect, shown: Bool, presented: Bool)] {
        let navigation = presenter.navigation
        let primary = navigation.allNavigations.filter { nav in !navigation.presentedNavigations.contains { $0 === nav } }
        return primary.flatMap { routes(of: $0, presented: false) } + presenter.modals.navigations.flatMap { routes(of: $0, presented: true) }
    }

    private func routes(of nav: UINavigationController, presented: Bool) -> [(route: RouteController, view: UIView, block: CGRect, shown: Bool, presented: Bool)] {
        var controllers = nav.viewControllers
        if let leaving = nav.transitionCoordinator?.viewController(forKey: .from),
           !controllers.contains(where: { $0 === leaving }) { controllers.append(leaving) }
        return controllers.compactMap { controller in
            guard let route = controller as? RouteController, let view = route.viewIfLoaded,
                  presenter.views[route.node.id] === route.node else { return nil }
            let block = self.block(in: view)
            return (route, view, block, view.window != nil && !block.isEmpty, presented)
        }
    }

    /// A route's block: its controller's view, ending where the document
    /// ends when the view is in the document. A keyboard that resizes the
    /// document ends it at the keys (`interactive-widget`'s
    /// `resizes-content`), and the routes follow (LLP 1123 C29), while the
    /// tab container keeps the screen's height so that its bar stays under
    /// the keys (`NavigationHost.ownerFrame`). The document's end is the
    /// routes' owner's, in its own untransformed space.
    private func block(in view: UIView) -> CGRect {
        guard let document = presenter.navigation.container, view.isDescendant(of: document) else { return view.bounds }
        let end = view.convert(CGPoint(x: 0, y: document.bounds.height), from: document).y
        var block = view.bounds
        block.size.height = max(0, min(block.height, end - block.minY))
        return block
    }

    /// Where each presented route a window shows is, in viewport points,
    /// bottom to top: the top-layer boxes the runner tests before the page.
    /// And the index of the first of the top presentation's, under which
    /// UIKit's presentation makes the rest inert: the runner keeps it and
    /// what is above it live to its point reads, and the rest inert. None
    /// while nothing is presented.
    var presented: (frames: [(UInt32, CGRect)], modal: Int?) {
        var frames: [(UInt32, CGRect)] = [], modal: Int?
        let top = presenter.modals.topNavigation
        for nav in presenter.modals.navigations {
            if nav === top { modal = frames.count }
            frames += routes(of: nav, presented: true).filter(\.shown)
                .map { ($0.route.node.id, presenter.viewportPoints($0.view.convert($0.block, to: presenter.viewport))) }
        }
        return (frames, modal)
    }

    /// Gives each route a window shows its block, and an ordinary one its
    /// placement. A route out of the window keeps what it had; a route no
    /// navigation holds gives its block back. Waits for a batch being
    /// applied, as `fit` does, and for a slice in flight (`whenIdle`), and
    /// samples what to tell when it tells it, so a report that waited tells
    /// what is current, and the ones behind it what changed since: none.
    func publish() {
        guard !resetting else { return }
        if presenter.applying {
            presenter.afterBatch { [weak self] in self?.publish() }
            return
        }
        presenter.session?.whenIdle { [weak self] in
            guard let self else { return }
            if presenter.applying { publish() } else { tell() }
        }
    }

    /// What changed since the runtime was last told, told now.
    private func tell() {
        guard !resetting, let session = presenter.session, session.state != .destroyed else { return }
        // A callback the runtime waits on can make no runtime call: tell
        // once it returns, as `ExactView.fit` resizes then
        if Owner.shared.isServingMain {
            DispatchQueue.main.async { [weak self] in self?.publish() }
            return
        }
        var now: [UInt32: (block: HostBlock, placement: BlockPlacement?)] = [:]
        for (route, view, block, shown, presented) in routes {
            let id = route.node.id
            guard shown else {
                if let held = told[id] { now[id] = held }
                continue
            }
            now[id] = (.size(block.size), presented ? nil : placement(of: route, block: block, in: view))
        }
        var blocks: [(UInt32, HostBlock?)] = [], placements: [(UInt32, BlockPlacement?)] = []
        for (id, entry) in now {
            if told[id]?.block != entry.block { blocks.append((id, entry.block)) }
            if told[id]?.placement != entry.placement { placements.append((id, entry.placement)) }
        }
        // A block taken back forgets its placement with it
        blocks += told.keys.filter { now[$0] == nil && presenter.views[$0] != nil }.map { ($0, nil) }
        told = now
        guard !blocks.isEmpty || !placements.isEmpty else { return }
        session.apply(session.runtime.blocks(blocks, placements: placements))
    }

    /// Where an ordinary route's block is shown, in the local coordinates
    /// of the routes' owner (`Scrolled::place`), which holds the native
    /// container the route is in, as it holds the route in the document. A
    /// tab's panel does not hold its route in UIKit, so the panel's own
    /// transform and place move nothing UIKit shows, and conversion to the
    /// owner crosses only UIKit's containers, which neither turn nor skew
    /// what they hold: the rectangle is exact, where one converted into a
    /// turned panel would be its bounding box. The conversion leaves out
    /// the owner's transform and every ancestor's, which the runner applies
    /// itself. None while the owner is not in the window.
    private func placement(of route: RouteController, block: CGRect, in view: UIView) -> BlockPlacement? {
        guard let owner = presenter.navigation.container, owner.window != nil, owner.window === view.window else { return nil }
        return BlockPlacement(rect: view.convert(block, to: owner), within: owner.id)
    }

    /// The runtime is being replaced: the routes `teardown` takes down were
    /// the old runtime's, which is not told, and the new one knows no block.
    func reset(_ teardown: () -> Void) {
        resetting = true
        teardown()
        resetting = false
        told = [:]
    }
}

extension Presenter {
    /// Open top-layer views in viewport points, back to front: the presented
    /// routes a window shows (LLP 1123 D2), then the menus and popovers above
    /// them; a native confirmation has no frame of its own. And the index of
    /// the top presentation's first route (`OwnerBlocks.presented`).
    package var openTopLayer: (frames: [(UInt32, CGRect)], modal: Int?) {
        let routes = owners.presented
        return (routes.frames + menus.presented.map { ($0.id, $0.topLayerFrame(in: viewport)) } + (menus.nativeConfirmation.map { [($0.id, CGRect.zero)] } ?? []), routes.modal)
    }
    package var openTopLayerFrames: [(UInt32, CGRect)] { openTopLayer.frames }

    /// A rectangle in the viewport's coordinates, less its scrolling: where
    /// a person sees it in the viewport, as the runner reads a top-layer box.
    func viewportPoints(_ rect: CGRect) -> CGRect {
        rect.offsetBy(dx: -viewport.contentOffset.x, dy: -viewport.contentOffset.y)
    }
}
#endif

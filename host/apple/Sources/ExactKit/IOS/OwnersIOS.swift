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
        let mount = port(of: view)
        guard mount !== viewport, let window = viewport.window else { return CGRect(origin: .zero, size: viewport.bounds.size) }
        let shown = mount.convert(mount.bounds, to: nil).intersection(window.bounds)
        return viewportPoints(viewport.convert(shown, from: nil))
    }

    /// A native module's own text input (a composer's UITextView) focused
    /// where this session is mounted: the keyboard is this session's
    /// business as for its own fields, so the page resizes above it.
    var hasNativeKeyboardEditor: Bool {
        guard let responder = FirstResponder.current as? UIView, responder is UIKeyInput else { return false }
        return mounts(responder)
    }

    /// Ends editing wherever the session is mounted; whether any editor resigned.
    @discardableResult
    package func endEditing() -> Bool {
        modals.mountRoots.reduce(viewport.endEditing(true)) { $1.endEditing(true) || $0 }
    }

    /// What a touch reaches after `view`, as UIKit gives a touch to the
    /// views it falls in: the superview, but past a presented route, out of
    /// its sheet's mount, its logical parent (`MountRelay`, the press's
    /// `pressResponder`). The walks that say what a touch would reach
    /// (an agent's tap, VoiceOver's activation, a text's menu source) take
    /// it, so they reach what a finger does.
    func touchParent(of view: UIView) -> UIView? {
        if let node = view as? NodeView, navigation.controllers[node.id]?.node === node, modals.mounts(node) {
            return eventParent(of: node)
        }
        return view.superview
    }

    /// What a touch's listeners (a pointer observer, a long press, a double
    /// tap) are reached through after `view`: `touchParent`, and out of a
    /// dialog's sheet or a lifted popover to its authored parent, as their
    /// `MountRelay`s reach them. A press keeps to `touchParent`.
    func listenerParent(of view: UIView) -> UIView? {
        (view as? NodeView).flatMap { menus.sheets.authoredParent(of: $0) } ?? menus.logicalParent(of: view) ?? touchParent(of: view)
    }

    /// The view a press on `target`, reached from `start` (`touchParent`),
    /// must land in: the target, or, when the walk left a sheet's mount for a
    /// logical ancestor in another mount, whose own box is not under the
    /// finger (LLP 1123 D2), the content UIKit hit-tested the touch to,
    /// `start`, as a node judges its own press.
    func pressBox(of target: UIView, from start: UIView) -> UIView {
        var at: UIView? = start, crossed = false
        while let cur = at, cur !== target {
            let up = touchParent(of: cur)
            crossed = crossed || up !== cur.superview
            at = up
        }
        return at != nil && crossed ? start : target
    }

    /// The port `view` is mounted in: its presentation's mount, or the
    /// viewport.
    func port(of view: UIView) -> UIView { modals.mountRoots.last { view.isDescendant(of: $0) } ?? viewport }

    /// Reveals the field being edited as the keys move, unless it is in a
    /// presentation, whose editor waits for the keys to settle
    /// (`keyboardSettled`).
    func revealEditorAsTheKeysMove() {
        if editor.map({ !modals.mounts($0) }) ?? true { reveal(editor) }
    }

    #if !os(tvOS)
    /// The keys stopped moving: an editor in a presentation is revealed in
    /// its route once the route's blocks are told and laid out
    /// (`OwnerBlocks.revealOnceTold`), behind a slice in flight as they are.
    /// UIKit lays a medium sheet out under the keys before it grows the
    /// sheet for them, the route a point tall for a frame, and a reveal into
    /// that left the route scrolled, taking the next detent drag.
    @objc func keyboardSettled(_ n: Notification) {
        guard keyboardTop != nil else { return }
        owners.revealOnceTold()
    }
    #endif

    /// The editor being edited.
    private var editor: NodeView? { editing ?? views.values.first { $0.field?.isFirstResponder == true || $0.textArea?.isFirstResponder == true } }

    /// Reveals the editor of a presentation's route while the keys are up.
    func revealSheetEditor() {
        guard keyboardTop != nil, let editor,
              modals.mounts(editor) else { return }
        if ExactEnv.agentFreezes { reveal(editor) } else { UIView.animate(withDuration: 0.25) { self.reveal(editor) } }
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
    private var resetting = false, revealOwed = false
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
        guard let document = presenter.navigation.container, view.isDescendant(of: document) else {
            // A route of the top presentation, the one that holds the focused
            // editor, ends at the docked keyboard: the top of its sheet's
            // keyboard layout guide, which UIKit lays out with the sheet as
            // it grows it for the keys and as they leave, follows no
            // undocked keyboard, and is empty without keys (it leaves out
            // the bottom safe area, `ModalController.loadView`). The route's
            // own scroller then reveals the field, and the sheet's bar stays
            // (LLP 1123 C26).
            var block = view.bounds
            // tvOS has no keyboard layout guide.
            #if !os(tvOS)
            if let sheet = presenter.modals.coordinateView, view.isDescendant(of: sheet),
               case let keys = sheet.keyboardLayoutGuide.layoutFrame, keys.height > 0 {
                let end = view.convert(CGPoint(x: 0, y: keys.minY), from: sheet).y
                block.size.height = max(0, min(block.height, end - block.minY))
            }
            #endif
            return block
        }
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
                else if !block.isEmpty {
                    // A retained controller may rejoin below the top after
                    // leaving the stack. Its loaded bounds still own its
                    // content; no placement is claimed while it is offscreen.
                    now[id] = (.size(block.size), nil)
                }
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
        if !blocks.isEmpty || !placements.isEmpty { session.apply(session.runtime.blocks(blocks, placements: placements)) }
        // A sheet's editor whose keys settled shows in the blocks just told
        if revealOwed {
            revealOwed = false
            presenter.revealSheetEditor()
        }
    }

    /// Reveals a sheet's editor once the routes' current blocks are told,
    /// in the order they are (`keyboardSettled`).
    func revealOnceTold() {
        revealOwed = true
        publish()
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
        revealOwed = false
    }
}

extension Presenter {
    /// Open top-layer views in viewport points, back to front: the presented
    /// routes a window shows (LLP 1123 D2), then the menus and popovers above
    /// them; a native confirmation has no frame of its own. And the index of
    /// the top presentation's first route, or the confirmation above it.
    package var openTopLayer: (frames: [(UInt32, CGRect)], modal: Int?) {
        let routes = owners.presented
        let frames = routes.frames + menus.presented.map { ($0.id, $0.topLayerFrame(in: viewport)) }
        if let confirmation = menus.nativeConfirmation {
            return (frames + [(confirmation.id, CGRect.zero)], frames.count)
        }
        return (frames, routes.modal)
    }
    package var openTopLayerFrames: [(UInt32, CGRect)] { openTopLayer.frames }

    /// A rectangle in the viewport's coordinates, less its scrolling: where
    /// a person sees it in the viewport, as the runner reads a top-layer box.
    func viewportPoints(_ rect: CGRect) -> CGRect {
        rect.offsetBy(dx: -viewport.contentOffset.x, dy: -viewport.contentOffset.y)
    }
}
#endif

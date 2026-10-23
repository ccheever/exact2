// @ref LLP 1123 D2 — session ownership and presentation crossings.
#if os(iOS) || os(tvOS)
import UIKit

/// Every node a press traversed, with the inertness rule from that side of
/// the first presentation crossing. Held identities survive no pool reuse.
struct PressPath {
    private var nodes: [(held: HeldNode, crossed: Bool, epoch: UInt64, inWindow: Bool)] = []
    private var openings: [() -> Bool] = []
    private let reachedTarget: Bool
    let bubbled: Bool
    var presentationsOpen: Bool { openings.allSatisfy { $0() } }

    init(from source: UIView, to target: UIView, presenter: Presenter) {
        var at: UIView? = source, crossed = false
        while let view = at {
            if let node = view as? NodeView { nodes.append((HeldNode(node), crossed, node.pressEpoch, node.window != nil)) }
            if view === target { break }
            if let open = presenter.menus.pressLifetime(of: view) { openings.append(open) }
            let parent = presenter.listenerParent(of: view)
            crossed = crossed || parent !== view.superview
            at = parent
        }
        reachedTarget = at === target
        bubbled = crossed
    }

    func isLive(in presenter: Presenter) -> Bool {
        reachedTarget && presentationsOpen && nodes.allSatisfy { entry in
            guard let node = entry.held.live(in: presenter) else { return false }
            // A node in a window when captured must still be in one.
            return node.superview != nil && (node.window != nil || !entry.inWindow) && node.pressEpoch == entry.epoch
                && !node.disabled && !(entry.crossed ? node.authoredInert : node.inert)
        }
    }
}

extension Presenter {
    /// Whether `view` is mounted in this session's document or a presentation beside it.
    func mounts(_ view: UIView) -> Bool {
        view.isDescendant(of: root) || view.isDescendant(of: viewport) || surface(containing: view).map { $0.kind != .document } == true
    }

    /// Where a finger reaches `view`, in viewport points (the viewport's
    /// coordinates less its scrolling, as the agent's boxes are): the
    /// viewport for the document; for a presentation's content, its
    /// presentation's view within the window; over a dialog's sheet, the
    /// popover's layer there.
    func reach(of view: UIView) -> CGRect {
        let mount = port(of: view)
        guard mount !== viewport, let window = viewport.window else { return CGRect(origin: .zero, size: viewportSize) }
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
        presentations(includingRetiring: true).reduce(root.endEditing(true) || viewport.endEditing(true)) { $1.endEditing(true) || $0 }
    }

    /// The next view reached by a touch or its listeners: the superview,
    /// except that a presented route, a dialog's sheet and a lifted popover
    /// bubble to their authored parent outside the presentation. The agent
    /// and accessibility activation take the same steps as a real touch.
    func listenerParent(of view: UIView) -> UIView? {
        if let node = view as? NodeView, let parent = menus.sheets.authoredParent(of: node) { return parent }
        if let parent = menus.logicalParent(of: view) { return parent }
        if let node = view as? NodeView, navigation.controllers[node.id]?.node === node, modals.mounts(node) {
            return eventParent(of: node)
        }
        return view.superview
    }

    func authored(_ view: UIView, inside ancestor: UIView) -> Bool {
        sequence(first: view, next: listenerParent).contains { $0 === ancestor }
    }

    /// The view a press on `target`, reached from `start` (`listenerParent`),
    /// must land in: the target, or, when the walk left a presentation for a
    /// logical ancestor in another mount, whose own box is not under the
    /// finger (LLP 1123 D2), the content UIKit hit-tested the touch to,
    /// `start`, as a node judges its own press.
    func pressBox(of target: UIView, from start: UIView) -> UIView {
        var at: UIView? = start, crossed = false
        while let cur = at, cur !== target {
            let up = listenerParent(of: cur)
            crossed = crossed || up !== cur.superview
            at = up
        }
        return at != nil && crossed ? start : target
    }

    /// The openings a press crosses before reaching `target`, or through
    /// the whole listener ancestry when no target is given. Capture before
    /// callbacks: closing and reopening the same view ends the old press.
    func pressLifetimes(from start: UIView, to target: UIView? = nil) -> [() -> Bool] {
        var lifetimes: [() -> Bool] = [], at: UIView? = start
        while let view = at, view !== target {
            if let open = menus.pressLifetime(of: view) { lifetimes.append(open) }
            at = listenerParent(of: view)
        }
        return lifetimes
    }

    /// The port `view` is mounted in: its presentation's mount, or the
    /// viewport.
    func port(of view: UIView) -> UIView {
        guard let surface = surface(containing: view), surface.kind != .document else { return viewport }
        return surface.view
    }

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
    var editor: NodeView? { editing ?? views.values.first { $0.field?.isFirstResponder == true || $0.textArea?.isFirstResponder == true } }

    /// Reveals the editor of a presentation's route while the keys are up.
    func revealSheetEditor() {
        guard keyboardTop != nil, let editor,
              modals.mounts(editor) else { return }
        if ExactEnv.agentFreezes { reveal(editor) } else { UIView.animate(withDuration: 0.25) { self.reveal(editor) } }
    }
}
#endif

#if os(iOS) || os(tvOS)
/// The view whose UIKit placement orders a top-layer box (LLP 1123 D2).
struct TopLayerBox {
    let view: UIView
    /// None: the top route presentation, which shows no box yet (its routes
    /// are mounting, or laid out empty), still begins the modal boundary.
    let frame: TopLayerFrame?
    let modal: UIViewController?
}

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
        presenter.navigation.primaryStacks.flatMap { routes(of: $0, presented: false) } + presenter.modals.navigations.flatMap { routes(of: $0, presented: true) }
    }

    /// A stack's routes: a navigation controller's, or a tab's held one.
    private func routes(of stack: UIViewController, presented: Bool) -> [(route: RouteController, view: UIView, block: CGRect, shown: Bool, presented: Bool)] {
        var controllers = stack.stackRoutes
        if let leaving = (stack as? UINavigationController)?.transitionCoordinator?.viewController(forKey: .from),
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

    /// Each presented route a window shows; the top live modal stack's
    /// first box begins its boundary. Retiring routes remain visible but
    /// claim no modal boundary (LLP 1123 D2).
    var presented: [TopLayerBox] {
        let top = presenter.modals.topNavigation
        var boxes = presenter.modals.navigations.flatMap { nav in
            routes(of: nav, presented: true).filter(\.shown).map { route in
                TopLayerBox(view: route.view,
                            frame: (route.route.node.id, presenter.viewportPoints(route.view.convert(route.block, to: presenter.viewport)), nil),
                            modal: nav === top ? nav : nil)
            }
        }
        if let top, !boxes.contains(where: { $0.modal === top }) { boxes.append(TopLayerBox(view: top.view, frame: nil, modal: top)) }
        return boxes
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
    /// Open top-layer boxes in UIKit's paint order, back to front (LLP
    /// 1123 D2). A popover's current host decides which sheet it is above:
    /// one opened earlier or outside the dialog stays beneath its card.
    /// The last modal boundary leaves only it and what is above it live.
    package var openTopLayer: (frames: [TopLayerFrame], modal: Int?) {
        let boxes = owners.presented + menus.sheets.presented + menus.presented.map {
            TopLayerBox(view: $0, frame: ($0.id, $0.topLayerFrame(in: viewport), nil), modal: nil)
        }
        func path(_ view: UIView) -> [Int] {
            var path: [Int] = [], at = view
            while let parent = at.superview {
                let siblings = NodeView.hitOrder(parent.subviews).reversed()
                path.append(siblings.enumerated().first { $0.element === at }?.offset ?? 0)
                at = parent
            }
            return Array(path.reversed())
        }
        // A presentation not in the window yet is above what presents it.
        func place(_ box: TopLayerBox) -> [Int] {
            guard box.view.window == nil, let presenting = box.modal?.presentingViewController?.viewIfLoaded else { return path(box.view) }
            return path(presenting) + [Int.max]
        }
        let ordered = boxes.enumerated().map { (box: $0.element, path: place($0.element), index: $0.offset) }.sorted {
            $0.path == $1.path ? $0.index < $1.index : $0.path.lexicographicallyPrecedes($1.path)
        }.map(\.box)
        let frames = ordered.compactMap(\.frame)
        let topModal = ordered.last { $0.modal != nil }?.modal
        let modal = topModal.flatMap { controller in
            ordered.firstIndex { $0.modal === controller }.map { at in ordered[..<at].filter { $0.frame != nil }.count }
        }
        if let confirmation = menus.nativeConfirmation {
            return (frames + [(confirmation.id, CGRect.zero, nil)], menus.openModally(confirmation) == true ? frames.count : modal)
        }
        return (frames, modal)
    }
    package var openTopLayerFrames: [(UInt32, CGRect)] { openTopLayer.frames.map { ($0.0, $0.1) } }

    /// A rectangle in the viewport's coordinates, less its scrolling: where
    /// a person sees it in the viewport, as the runner reads a top-layer box.
    func viewportPoints(_ rect: CGRect) -> CGRect {
        rect.offsetBy(dx: -viewport.pageOrigin.x, dy: -viewport.pageOrigin.y)
    }
}
#endif

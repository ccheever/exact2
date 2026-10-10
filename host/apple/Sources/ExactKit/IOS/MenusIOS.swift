// The native menu arm (exact2 LLP 1021 D3): a popover whose rows are
// buttons presents as the platform's pull-down — a UIMenu off the invoker,
// built from the rows' data (text → title, aria-checked → the system
// checkmark, disabled → dimmed; a row without a press handler separates
// sections) — and a selection dispatches the row's press by view id into
// the runner: the same journal entry a painted tap makes, so the runner
// cannot tell the presentations apart. The popover subtree itself never
// paints here (the kernel-painted top layer is the web's; D2's is owed).
// Items are rebuilt every time the menu opens (UIDeferredMenuElement
// .uncached), after the invoker's own press has gone to the runner — both
// fire, D1 — so a menu that refreshes its rows on that press shows the
// refreshed rows.

#if os(iOS) || os(tvOS)
import UIKit

final class MenuHost {
    private weak var presenter: Presenter?
    private var touchObserver: PopoverTouch?
    private var overlays: [UInt32: UIButton] = [:]
    /// The target and press shape each overlay's menu was built for.
    private var menuShapes: [UInt32: String] = [:]
    private var confirmation: Confirmation?
    /// Under the agent, the popovers open in their painted presentation
    /// (LLP 1021 D4), by `id`: each in the top layer while open, as macOS's
    /// and the web's are, out of its parent and back at its index after.
    private var agentOpen: [String: Lifted] = [:]
    /// How many popovers the agent has opened: each one's place in the stack.
    private var opened = 0
    private final class Lifted {
        let source: UInt32
        weak var popover: NodeView?
        weak var parent: UIView?
        var index: Int
        let center: CGPoint
        /// Its place in HTML's popover stack: when it opened, and the open
        /// popover it is nested in (its opener or its parent inside that
        /// one), which hiding hides it with (LLP 1021 §5, "Submenus").
        let order: Int
        let ancestor: String?
        let layer = TopLayer(frame: .zero)
        init(source: UInt32, popover: NodeView, order: Int, ancestor: String?) {
            self.source = source; self.popover = popover; self.order = order; self.ancestor = ancestor
            parent = popover.superview
            index = parent?.subviews.firstIndex(of: popover) ?? 0
            center = popover.center
        }
    }
    /// The top layer takes no touch itself, only what it holds.
    private final class TopLayer: UIView {
        override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
            guard !isHidden, isUserInteractionEnabled, bounds.contains(point) else { return nil }
            for child in NodeView.hitOrder(subviews) {
                if let hit = child.hitTest(convert(point, to: child), with: event) { return hit }
            }
            return nil
        }
    }

    /// An alertdialog-shaped popover has one or more actions and at most one
    /// hide-only cancel: a chooser ("Open in…") is one action per choice.
    /// Retain this owner until native dismissal completes; ids alone are not
    /// enough because a restart can reuse them for unrelated controls.
    #if os(tvOS)
    // tvOS has no popovers; the sheet keeps the adaptive delegate only.
    private typealias ConfirmationDelegate = UIAdaptivePresentationControllerDelegate
    #else
    private typealias ConfirmationDelegate = UIPopoverPresentationControllerDelegate
    #endif
    private final class Confirmation: NSObject, ConfirmationDelegate {
        /// How it shows (LLP 1115 D6): `alert`, the platform's centred
        /// alert, for a confirmation (a message and a cancel, at most three
        /// buttons); `sheet`, UIKit's unanchored action sheet on a compact
        /// screen (its Cancel drawn; iOS 27 places it mid-screen), for a chooser with a cancel; `popover`, the sheet as a
        /// popover at its invoker, where a tap outside is its only way out
        /// (no cancel) or the screen is regular width.
        enum Style: String { case alert, sheet, popover }
        weak var host: MenuHost?
        weak var source: NodeView?
        /// Opened by an invoker, not by `showModal` from an action.
        let sourced: Bool
        weak var popover: NodeView?
        let route: String?
        let style: Style
        let alert: UIAlertController
        var finishing = false
        /// An action as presented: the node, the title the sheet shows for
        /// it, and whether it could be chosen.
        final class Presented {
            weak var node: NodeView?
            let title: String, enabled: Bool
            init(_ node: NodeView, title: String, enabled: Bool) { self.node = node; self.title = title; self.enabled = enabled }
        }
        let actions: [Presented]
        init(host: MenuHost, source: NodeView?, popover: NodeView, actions: [Presented], style: Style, title: String?, message: String?) {
            self.host = host; self.source = source; self.popover = popover; self.actions = actions; self.style = style
            sourced = source != nil
            route = host.presenter?.navigation.routeKey(containing: source ?? popover)
            alert = UIAlertController(title: title, message: message, preferredStyle: style == .alert ? .alert : .actionSheet)
        }
        func owns(_ node: NodeView) -> Bool { actions.contains { $0.node === node } }
        func adaptivePresentationStyle(for controller: UIPresentationController) -> UIModalPresentationStyle { .none }
        #if !os(tvOS)
        func popoverPresentationControllerDidDismissPopover(_ controller: UIPopoverPresentationController) {
            host?.cancelled(self)
        }
        #endif
        func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
            host?.cancelled(self)
        }
    }

    init(presenter: Presenter) { self.presenter = presenter }
    #if os(iOS)
    /// The context menus (LLP 1021 §5.1, ContextMenusIOS.swift).
    private(set) lazy var context = ContextMenuHost(presenter: presenter!, menus: self)
    #endif
    /// The focus a native menu sets aside while it shows (MenuFocusIOS.swift).
    private(set) lazy var focus = MenuFocus(presenter: presenter!)

    private func isDialog(_ node: NodeView) -> Bool { node.props["semanticTag"] == "dialog" }
    private func isConfirmation(_ node: NodeView) -> Bool {
        isDialog(node) || node.props["accessibilityRole"] == "alertdialog"
    }
    private func target(of source: NodeView) -> String? {
        if source.props["commandfor"] != nil { return source.props["commandfor"] }
        return source.props["popovertarget"]
    }
    private func closes(_ source: NodeView, _ target: NodeView) -> Bool {
        if isDialog(target) {
            return source.props["commandfor"] == target.props["id"] && source.props["command"] == "close"
        }
        return self.target(of: source) == target.props["id"] && (source.props["command"] == "hide-popover" || source.props["popovertargetaction"] == "hide")
    }
    private func opens(_ source: NodeView, _ target: NodeView) -> Bool {
        self.target(of: source) == target.props["id"] && (isDialog(target)
            ? source.props["command"] == "show-modal" : (source.props["commandfor"] == nil ? source.props["popovertargetaction"] != "hide" : ["show-popover", "toggle-popover"].contains(source.props["command"] ?? "")))
    }

    /// After a batch: hide every popover, and lay a transparent button
    /// whose primary action is the system menu over every invoker of one.
    func sync() {
        // Ordinary menus retain LLP 1021 D4's existing agent presentation.
        // Confirmations use the same UIKit owner under either input carrier.
        guard let presenter else { return }
        let surface = presenter.modals.coordinateView ?? presenter.viewport
        if touchObserver?.view !== surface {
            touchObserver?.view?.removeGestureRecognizer(touchObserver!)
            let observer = PopoverTouch(host: self)
            surface.addGestureRecognizer(observer); touchObserver = observer
        }
        if let owner = confirmation, !valid(owner) { resetConfirmation() }
        var popovers: [String: NodeView] = [:]
        for (name, entry) in agentOpen {
            guard let pop = entry.popover, presenter.views[pop.id] === pop, pop.props["id"] == name,
                  pop.props["popover"] != nil else { drop(name); continue }
        }
        var lifting: [Lifted] = []
        for v in presenter.carrying("popover") + presenter.carrying("tag:dialog").filter({ $0.props["popover"] == nil }) {
            // The agent's painted presentation: in the top layer while open,
            // hidden while closed, as on the web and macOS, so a closed
            // popover covers nothing (`agentTap`).
            if !isConfirmation(v), ExactEnv.agentMode || v.props["id"].map({ agentOpen[$0] != nil }) == true {
                if v.props["popover"] != nil {
                    if let name = v.props["id"], let entry = agentOpen[name], entry.popover === v { lifting.append(entry) }
                    else if !v.isHidden { v.isHidden = true }
                }
                continue
            }
            // UIKit's setter is not free, even to the same value: every batch.
            if !v.isHidden { v.isHidden = true }
            if let name = v.props["id"] { popovers[name] = v }
        }
        // In the order they opened, so a submenu stays above its menu.
        for entry in lifting.sorted(by: { $0.order < $1.order }) { lift(entry) }
        var live = Set<UInt32>()
        for v in presenter.carrying("popovertarget") + presenter.carrying("commandfor").filter({ $0.props["popovertarget"] == nil }) {
            guard let target = target(of: v), let pop = popovers[target],
                  // A row that only hides its popover (a menu item closing
                  // itself, the spec's way) is not an invoker.
                  opens(v, pop)
            else { continue }
            if v.isNativeButton { continue } // Its own primary action invokes after activation (D13).
            live.insert(v.id)
            if isConfirmation(pop) {
                let button = overlays[v.id] ?? UIButton(type: .custom)
                button.menu = nil
                button.showsMenuAsPrimaryAction = false
                button.autoresizingMask = [.flexibleWidth, .flexibleHeight]
                button.accessibilityLabel = v.props["accessibilityLabel"] ?? title(of: v)
                button.accessibilityIdentifier = v.props["testId"] ?? v.props["id"]
                button.isEnabled = !v.disabled
                let actionId = UIAction.Identifier("exact.confirmation")
                button.removeAction(identifiedBy: actionId, for: .touchUpInside)
                button.addAction(UIAction(identifier: actionId) { [weak self, weak v, weak pop] _ in
                    guard let self, let v, let pop else { return }
                    _ = self.openConfirmation(from: v, popover: pop)
                }, for: .touchUpInside)
                v.addSubview(button); button.frame = v.bounds; overlays[v.id] = button
                continue
            }
            let button = overlays[v.id] ?? {
                #if os(iOS)
                let b = MenuButton(type: .custom)
                b.focus = focus
                #else
                let b = UIButton(type: .custom)
                #endif
                b.showsMenuAsPrimaryAction = true
                b.autoresizingMask = [.flexibleWidth, .flexibleHeight]
                overlays[v.id] = b
                return b
            }()
            if button.superview !== v { v.addSubview(button) }
            button.frame = v.bounds
            button.accessibilityLabel = v.props["accessibilityLabel"] ?? title(of: v)
            button.accessibilityIdentifier = v.props["testId"] ?? v.props["id"]
            button.isEnabled = !v.disabled
            // One menu per invoker and press shape, kept across batches: it
            // reads its rows each time it opens, so a batch (a poll's answer,
            // every second) has nothing to change in it, and replacing it
            // would reload a menu that is open — UIKit shows its "Loading…"
            // row again each time.
            let hasPress = v.handlers.contains("press")
            // The popover's `aria-label` titles the menu ("Open location in").
            let heading = pop.props["accessibilityLabel"] ?? ""
            let shape = "\(target)|\(hasPress)|\(heading)"
            if menuShapes[v.id] != shape || button.menu == nil {
                menuShapes[v.id] = shape
                let invokerId = v.id
                button.menu = UIMenu(title: heading, children: [
                    UIDeferredMenuElement.uncached { [weak self] completion in
                        // The popover by name now: the node a batch made
                        // when the menu was built may since be another.
                        let rows = { self?.popover(named: target).map { self?.items(of: $0) ?? [] } ?? [] }
                        guard hasPress else { completion(rows()); return }
                        self?.presenter?.press(invokerId)
                        // The runner applies the press on its own thread; the
                        // items are read after it has (80 ms is invisible under
                        // the menu's own presentation).
                        DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) { completion(rows()) }
                    }
                ])
            }
        }
        for (id, b) in overlays where !live.contains(id) {
            b.removeFromSuperview()
            overlays[id] = nil
            menuShapes[id] = nil
        }
        #if os(iOS)
        context.sync()
        #endif
    }

    /// A custom hide-only control inside a lifted content popover has the
    /// same default activation as a native invoker, with or without a handler.
    func closesPresentedContent(_ source: NodeView) -> Bool {
        guard source.isButton, let name = target(of: source),
              let pop = agentOpen[name]?.popover else { return false }
        return !isConfirmation(pop) && closes(source, pop) && eligible(source)
    }
    /// D13: activation finished; now read the surviving control's live target.
    func invoke(_ source: NodeView) {
        guard source.isNativeButton || closesPresentedContent(source), eligible(source), let name = target(of: source), !name.isEmpty,
              let pop = popover(named: name), opens(source, pop) || closes(source, pop) else { return }
        if isConfirmation(pop) {
            if closes(source, pop), let owner = confirmation, owner.popover === pop { finish(owner, chosen: nil) }
            else if opens(source, pop) { _ = openConfirmation(from: source, popover: pop, dispatchPress: false) }
        } else {
            let toggle = source.props["command"] == "toggle-popover" ||
                (source.props["commandfor"] == nil && (source.props["popovertargetaction"] ?? "toggle") == "toggle")
            if closes(source, pop) || toggle && agentOpen[name] != nil { drop(name) }
            else if agentOpen[name] == nil { agentShow(pop, named: name, from: source.id) }
        }
    }

    /// A live popover by its id.
    private func popover(named name: String) -> NodeView? {
        presenter?.views.values.first { $0.props["id"] == name && ($0.props["popover"] != nil || isDialog($0)) }
    }

    private func live(_ node: NodeView) -> Bool { presenter?.views[node.id] === node }
    func eligible(_ node: NodeView, inertBoundary: UIView? = nil) -> Bool {
        guard live(node), !node.disabled, presenter?.navigation.isInactiveRoute(containing: node) == false else { return false }
        var ancestor: UIView? = node
        var checksInert = true
        while let view = ancestor {
            // A modal dialog escapes inertness above itself. Its issued command
            // also survives the invoker subsequently becoming inert.
            if view === inertBoundary { checksInert = false }
            if let n = view as? NodeView, n.disabled || (checksInert && n.props["inert"] == "true") { return false }
            if view.isHidden {
                guard let pop = view as? NodeView, isConfirmation(pop),
                      pop.props["popover"] != nil || isDialog(pop) else { return false }
            }
            ancestor = view.superview
        }
        return true
    }
    private func valid(_ owner: Confirmation) -> Bool {
        guard let pop = owner.popover else { return false }
        guard owner.sourced else { return showable(pop) && presenter?.navigation.routeKey(containing: pop) == owner.route
            && owner.actions.allSatisfy { presented(owner, $0) } }
        guard let source = owner.source else { return false }
        let modal = isDialog(pop)
        return eligible(source, inertBoundary: modal ? source : nil) && live(pop) && source.window != nil
            && opens(source, pop) && isConfirmation(pop)
            && presenter?.navigation.routeKey(containing: source) == owner.route
            && owner.actions.allSatisfy { presented(owner, $0) }
    }
    /// A confirmation `showModal` may present: live, in a window, on the
    /// active route, not under a disabled or (for a popover) inert ancestor.
    private func showable(_ pop: NodeView) -> Bool {
        live(pop) && pop.window != nil && isConfirmation(pop) && eligible(pop, inertBoundary: isDialog(pop) ? pop : nil)
    }
    /// One of the owner's actions still shows what the sheet shows: live, in
    /// its popover, closing it, its title and its enablement unchanged. A
    /// row that now says something else (a reused row given another
    /// provider) ends the sheet rather than dispatching under an old title.
    private func presented(_ owner: Confirmation, _ entry: Confirmation.Presented) -> Bool {
        guard let action = entry.node, let pop = owner.popover, live(action) else { return false }
        return closes(action, pop) && action.isDescendant(of: pop) && title(of: action) == entry.title
            && choosable(action, in: pop) == entry.enabled
    }
    private func choosable(_ action: NodeView, in pop: NodeView) -> Bool {
        eligible(action, inertBoundary: isDialog(pop) ? pop.superview : nil)
    }
    /// The chosen action may be dispatched: the sheet is still the one
    /// presented and this action was, and is, choosable.
    private func valid(_ owner: Confirmation, chosen action: NodeView) -> Bool {
        guard let pop = owner.popover, let entry = owner.actions.first(where: { $0.node === action }) else { return false }
        return entry.enabled && presented(owner, entry) && choosable(action, in: pop)
    }
    private func cancelled(_ owner: Confirmation) {
        // UIKit also delivers this after a selected action's handler. That
        // notification must not cancel the action awaiting the next turn.
        guard confirmation === owner, !owner.finishing else { return }
        owner.finishing = true
        confirmation = nil
    }
    /// `chosen` is the selected action, nil for the cancel.
    private func finish(_ owner: Confirmation, chosen: NodeView?) {
        guard confirmation === owner, !owner.finishing else { return }
        owner.finishing = true
        owner.alert.dismiss(animated: !ExactEnv.agentFreezes) { [weak self, owner] in
            // A native action arrives after the alert leaves its window;
            // dismiss can therefore complete synchronously inside UIKit's
            // selection callback, before its dismissal delegate is called.
            // Keep the owner until that stack unwinds before app code can
            // destroy the presenting editor or dismiss its parent sheet.
            DispatchQueue.main.async { [weak self, owner] in
                guard let self, self.confirmation === owner else { return }
                let action = chosen.flatMap { self.valid(owner) && self.valid(owner, chosen: $0) ? $0 : nil }
                self.confirmation = nil
                if let action { self.presenter?.press(action.id) }
            }
        }
    }
    private func resetConfirmation() {
        guard let owner = confirmation else { return }
        confirmation = nil
        owner.finishing = true
        // Only this session's alert; never dismiss through a containing app's
        // root controller, which might now present something unrelated.
        owner.alert.dismiss(animated: false) { _ = owner }
    }
    func reset() {
        resetConfirmation()
        overlays.values.forEach { $0.removeFromSuperview() }
        overlays.removeAll()
        for name in Array(agentOpen.keys) { drop(name) }
        #if os(iOS)
        context.reset()
        #endif
    }

    /// Whether `node` is an open popover in the top layer (the agent's).
    func lifted(_ node: NodeView) -> Bool {
        #if os(iOS)
        if context.lifts(node) { return true }
        #endif
        return agentOpen.values.contains { $0.popover === node }
    }
    /// A children op on `parent` while a child of it is in the top layer:
    /// the child stays there, its index kept for its return; one the op
    /// drops closes.
    /// A frame op on `node`: a context menu's lifted row takes it as its home.
    func framed(_ node: NodeView) {
        #if os(iOS)
        context.framed(node)
        #endif
    }
    func children(_ parent: UIView, _ wanted: [NodeView]) {
        #if os(iOS)
        context.children(parent, wanted)
        #endif
        for (name, entry) in agentOpen where entry.parent === parent {
            if let i = wanted.firstIndex(where: { $0 === entry.popover }) { entry.index = i } else { drop(name) }
        }
    }
    /// Into the top layer, above everything of the page's (its routes and
    /// containers are under the viewport's root) and so in the agent's
    /// `screenshot`, anchored to its opener by its `position-area` (as
    /// macOS's; below it by default). Over a modal,
    /// the modal's view.
    private func lift(_ entry: Lifted) {
        guard let presenter, let pop = entry.popover else { return }
        let host: UIView = presenter.modals.coordinateView ?? presenter.viewport
        if entry.layer.superview !== host || host.subviews.last !== entry.layer { host.addSubview(entry.layer) }
        entry.layer.setPaintForeground()
        if entry.layer.frame != host.bounds { entry.layer.frame = host.bounds }
        if pop.superview !== entry.layer { entry.layer.addSubview(pop) }
        if pop.isHidden { pop.isHidden = false }
        guard let source = presenter.views[entry.source], source.window != nil else { return }
        let anchorView: UIView = source.isNativeButton ? presenter.controls.controls[source.id] ?? source : source
        let anchor = anchorView.convert(anchorView.bounds, to: entry.layer), size = pop.bounds.size
        let at = PositionArea.origin(PositionArea.of(pop), anchor: anchor, size: size,
                                    margins: PositionArea.margins(of: pop), in: entry.layer.bounds)
        let center = CGPoint(x: at.x + size.width / 2, y: at.y + size.height / 2)
        if pop.center != center { pop.center = center }
    }
    /// Out of the top layer, hidden, back where it was; the popovers nested
    /// in it first, as hiding a popover hides those above it in the stack.
    private func drop(_ name: String) {
        for (nested, entry) in agentOpen where entry.ancestor == name { drop(nested) }
        guard let entry = agentOpen.removeValue(forKey: name) else { return }
        entry.popover?.endEditing(true)
        entry.layer.removeFromSuperview()
        guard let pop = entry.popover else { return }
        pop.isHidden = true
        if presenter?.views[pop.id] === pop, let parent = entry.parent {
            parent.insertSubview(pop, at: min(entry.index, parent.subviews.count))
            pop.center = entry.center
        } else {
            pop.removeFromSuperview()
        }
    }

    /// Whether `node`, a popover, is open: its confirmation presented, or,
    /// under the agent, its painted presentation shown.
    func isOpen(_ node: NodeView) -> Bool {
        if confirmation?.popover === node { return true }
        return node.props["id"].map { agentOpen[$0] != nil } == true
    }

    /// Production and agent touches share HTML's light-dismiss exclusions.
    func lightDismiss(_ touched: UIView) {
        var kept = Set<String>()
        var inside = agentOpen.first { _, entry in
            entry.popover.map { touched === $0 || touched.isDescendant(of: $0) } ?? false
        }?.key
        while let name = inside, !kept.contains(name) { kept.insert(name); inside = agentOpen[name]?.ancestor }
        let node = sequence(first: touched as UIView?, next: { $0?.superview }).compactMap { $0 as? NodeView }.first
        for (name, entry) in agentOpen.sorted(by: { $0.value.order > $1.value.order }) {
            guard agentOpen[name] != nil, entry.popover?.props["popover"] != "manual", !kept.contains(name) else { continue }
            if let source = presenter?.views[entry.source], touched === source || touched.isDescendant(of: source) { continue }
            if node.map({ target(of: $0) == name }) == true { continue }
            drop(name)
        }
    }

    /// Under the agent, what a tap on `node` does to the painted popovers
    /// (LLP 1021 D4, as the web's and macOS's). Outside an open one, and not
    /// its opener, it dismisses it before the tap is delivered, so the tap
    /// still presses what it lands on. What the tapped node itself asks —
    /// an opener toggling its popover, a hide-only button closing it — is
    /// done after the tap is delivered (on the next turn), so the tap lands
    /// where the node is; an opened popover is in the top layer below its
    /// opener with its `autofocus` field focused.
    func agentTap(_ node: NodeView) {
        guard ExactEnv.agentMode else { return }
        lightDismiss(node)
        guard !node.isNativeButton, let presenter else { return }
        var byName: [String: NodeView] = [:]
        for pop in presenter.carrying("popover") where !isConfirmation(pop) {
            if let name = pop.props["id"] { byName[name] = pop }
        }
        guard let name = target(of: node), let pop = byName[name], closes(node, pop) || opens(node, pop) else { return }
        let hides = closes(node, pop), source = node.id
        DispatchQueue.main.async { [weak self, weak presenter, weak pop] in
            guard let self, let presenter, let pop, presenter.views[pop.id] === pop else { return }
            if self.agentOpen[name] != nil { self.drop(name); return }
            guard !hides else { return }
            self.agentShow(pop, named: name, from: source)
        }
    }
    private func agentShow(_ pop: NodeView, named name: String, from source: UInt32) {
        // Nested in the latest open popover holding its opener or itself.
        let opener = presenter?.views[source]
        let ancestor = agentOpen.filter { _, open in
            guard let outer = open.popover else { return false }
            return opener.map { $0.isDescendant(of: outer) } == true || pop.isDescendant(of: outer)
        }.max { $0.value.order < $1.value.order }?.key
        opened += 1
        let entry = Lifted(source: source, popover: pop, order: opened, ancestor: ancestor)
        agentOpen[name] = entry
        lift(entry)
        if let field = Self.autofocus(in: pop) { presenter?.focusNode(field) }
    }
    /// Under the agent, a context menu (LLP 1021 §5.1) is its popover painted
    /// in the top layer, anchored to `node`, as an invoker's (D4): its
    /// preview row and menu rows are nodes the agent reads and taps. Called
    /// after `node`'s own `contextmenu` has been delivered; any other open
    /// popover closes, as a tap outside it would.
    /// Whether it opened one.
    @discardableResult
    func agentContext(_ node: NodeView) -> Bool {
        // Its own action may have unmounted or disabled it, or renamed its popover.
        guard ExactEnv.agentMode, let presenter, let name = node.props["contextPopover"], eligible(node) else { return false }
        for other in Array(agentOpen.keys) { drop(other) }
        guard let pop = presenter.carrying("popover").first(where: { $0.props["id"] == name && !isConfirmation($0) }) else {
            presenter.session?.log("context menu \(name) refused: no popover has that id")
            return false
        }
        agentShow(pop, named: name, from: node.id)
        return true
    }
    /// Whether a tap on `node` goes through the agent's painted popovers —
    /// one is open (the tap dismisses it first), or `node` is a painted
    /// popover, is in one, or opens or closes one. Native invokers' content
    /// popovers use production touches; custom invokers retain the agent carrier
    /// (LLP 1080.000 D7, stage 3).
    func agentPainted(_ node: NodeView) -> Bool {
        guard ExactEnv.agentMode, let presenter else { return false }
        if node.isNativeButton { return false } // Native invokers and outside buttons take real UIKit touches.
        if !agentOpen.isEmpty {
            return agentOpen.values.contains { presenter.views[$0.source]?.isNativeButton != true }
        }
        let painted = (presenter.carrying("popover") + presenter.carrying("tag:dialog").filter { $0.props["popover"] == nil }).filter { !isConfirmation($0) }
        return painted.contains { pop in node === pop || node.isDescendant(of: pop) || (pop.props["id"] != nil && target(of: node) == pop.props["id"]) }
    }
    private static func autofocus(in view: UIView) -> NodeView? {
        for sub in view.subviews {
            if let node = sub as? NodeView, let value = node.props["autofocus"], value != "false" { return node }
            if let found = autofocus(in: sub) { return found }
        }
        return nil
    }
    func unmounted() { resetConfirmation() }
    var inTransition: Bool {
        guard let owner = confirmation else { return false }
        return owner.finishing || owner.alert.isBeingPresented || owner.alert.isBeingDismissed
    }
    func observation() -> [String: Any]? {
        #if os(iOS)
        if let open = context.observation() { return open }
        #endif
        if confirmation == nil, let open = agentOpen.values.max(by: { $0.order < $1.order }), let pop = open.popover {
            return ["kind": "popover", "source": Int(open.source), "popover": Int(pop.id), "phase": "open"]
        }
        guard let owner = confirmation else { return nil }
        return ["kind": "confirmation", "source": owner.source.map { Int($0.id) as Any } ?? NSNull(),
                "popover": owner.popover.map { Int($0.id) as Any } ?? NSNull(), "phase": inTransition ? "transition" : "open",
                "actionStyle": owner.alert.actions.first?.style == .destructive ? "destructive" : "default",
                "presentation": owner.style.rawValue,
                "actions": owner.actions.count]
    }
    /// LLP 1080.001 D3: the views this host adds — a node's overlay button,
    /// an open popover's top layer — and the popovers it hides or lifts.
    func inspectionOwns(_ view: UIView) -> Bool {
        overlays.values.contains { $0 === view } || agentOpen.values.contains { $0.layer === view }
    }
    func hides(_ node: NodeView) -> Bool {
        presenter?.carrying("popover").contains { $0 === node } == true || presenter?.carrying("tag:dialog").contains { $0 === node } == true
    }
    func projects(_ node: NodeView) -> Bool { agentOpen.values.contains { $0.popover === node } }

    func ownsConfirmationNode(_ node: NodeView) -> Bool {
        var ancestor: UIView? = node
        while let view = ancestor {
            if let n = view as? NodeView, isConfirmation(n), n.props["popover"] != nil || isDialog(n) { return true }
            ancestor = view.superview
        }
        return false
    }
    /// Public UIKit has no UIAlertAction view/rectangle. Activation is named
    /// honestly, and only works while this original action is presented.
    func activate(_ node: NodeView) -> Bool? {
        if node.isNativeButton && !ownsConfirmationNode(node) { return nil }
        if ownsConfirmationNode(node) {
            guard let owner = confirmation, !inTransition, valid(owner) else { return false }
            if owner.owns(node) {
                guard valid(owner, chosen: node) else { return false }
                finish(owner, chosen: node); return true
            }
            if let pop = owner.popover, node.isDescendant(of: pop), !node.disabled,
               closes(node, pop) {
                finish(owner, chosen: nil); return true
            }
            return false
        }
        guard let name = target(of: node), let pop = presenter?.views.values.first(where: {
            $0.props["id"] == name && isConfirmation($0) && ($0.props["popover"] != nil || isDialog($0))
        }) else { return nil }
        guard opens(node, pop) else { return false }
        return openConfirmation(from: node, popover: pop)
    }
    /// A sheet action's handler: the action at `index` as presented, nil
    /// for the cancel. Its own entry, never a successor's at that index.
    private func select(_ owner: Confirmation, _ index: Int?) {
        guard let index else { finish(owner, chosen: nil); return }
        guard owner.actions.indices.contains(index), let node = owner.actions[index].node else {
            finish(owner, chosen: nil); return
        }
        finish(owner, chosen: node)
    }
    /// `showModal(id)` from an action (LLP 1115 D6): the confirmation by its
    /// `id`, a `dialog` or an alertdialog popover, presented with no invoker
    /// (a context menu's item asks before it deletes). A menu that is still
    /// leaving holds UIKit's presentation; this waits out up to two seconds.
    func showModal(_ name: String, attempt: Int = 0) {
        guard let pop = popover(named: name) else {
            presenter?.session?.log("showModal \(name) refused: no dialog or popover has that id"); return
        }
        guard isConfirmation(pop) else {
            presenter?.session?.log("showModal \(name) refused: iOS presents a dialog or a role=alertdialog popover, as its alert"); return
        }
        if confirmation?.popover === pop { return } // HTML: showModal on an open modal dialog does nothing
        if confirmation == nil, showable(pop), let controller = controller(for: pop), busy(controller), attempt < 20 {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in self?.showModal(name, attempt: attempt + 1) }
            return
        }
        if !openConfirmation(from: nil, popover: pop), confirmation?.popover !== pop {
            presenter?.session?.log("showModal \(name): nothing presented")
        }
    }
    /// `close(id)` from an action: the confirmation by its `id`, as its cancel.
    func closeModal(_ name: String) {
        guard let owner = confirmation, let pop = owner.popover, pop.props["id"] == name else { return }
        finish(owner, chosen: nil)
    }
    private func controller(for view: UIView) -> UIViewController? {
        var responder: UIResponder? = view
        while responder != nil && !(responder is UIViewController) { responder = responder?.next }
        return responder as? UIViewController
    }
    private func busy(_ controller: UIViewController) -> Bool {
        controller.presentedViewController != nil || controller.isBeingDismissed || controller.isBeingPresented
    }
    private func openConfirmation(from source: NodeView?, popover pop: NodeView, dispatchPress: Bool = true) -> Bool {
        func openable() -> Bool {
            guard confirmation == nil else { return false }
            guard let source else { return showable(pop) }
            return eligible(source) && live(pop) && source.window != nil
        }
        guard openable() else { return false }
        if isDialog(pop) && pop.props["closedby"] != "any" {
            presenter?.session?.log("dialog refused: native confirmation currently requires closedby=any")
            return false
        }
        // Session.press applies its batch synchronously. Both invoker actions
        // fire, as on the web; read the updated rows only if these identities
        // survived that action (no elapsed-time guess or successor id).
        if dispatchPress, let source, source.handlers.contains("press") { presenter?.press(source.id) }
        guard openable() else { return false }
        let children = pop.container.subviews.compactMap { $0 as? NodeView }
        let actions = children.filter { $0.isButton && $0.handlers.contains("press") }
        let cancels = children.filter { $0.isButton && !$0.handlers.contains("press") && closes($0, pop) }
        let boundary = isDialog(pop) ? pop.superview : nil
        // Refused shapes are said, never silently dropped: the tap opens nothing.
        func refuse(_ why: String) -> Bool {
            presenter?.session?.log("confirmation \(pop.props["id"] ?? "?") refused: \(why)")
            return false
        }
        guard !actions.isEmpty else { return refuse("no action (a button with press)") }
        guard cancels.count <= 1 else { return refuse("\(cancels.count) cancels; at most one hide-only button") }
        guard children.allSatisfy({ child in child.kind == "text" || actions.contains { $0 === child } || cancels.contains { $0 === child } }) else {
            return refuse("only text, actions and one cancel may be its rows")
        }
        guard actions.allSatisfy({ closes($0, pop) }) else { return refuse("each action must also hide it (popovertargetaction=hide)") }
        // A disabled choice shows dimmed; the others stay choosable.
        guard actions.contains(where: { eligible($0, inertBoundary: boundary) }) else { return refuse("every action is disabled") }
        guard let controller = controller(for: source ?? pop), !busy(controller) else { return false }
        let presented = actions.map { Confirmation.Presented($0, title: title(of: $0), enabled: eligible($0, inertBoundary: boundary)) }
        let texts = children.filter { $0.kind == "text" }.map(title(of:)).filter { !$0.isEmpty }
        let label = pop.props["accessibilityLabel"].flatMap { $0.isEmpty ? nil : $0 }
        let style = Self.style(texts: texts.count, actions: actions.count, cancels: cancels.count,
                               compact: controller.traitCollection.horizontalSizeClass != .regular)
        // The alert's title is its aria-label ("Remove book?"), else its
        // first line; the message is the rest. A sheet is headed by its
        // label, its text the message (LLP 1115 D6).
        var heading = label, lines = texts
        if style == .alert, heading == nil, !lines.isEmpty { heading = lines.removeFirst() }
        let owner = Confirmation(host: self, source: source, popover: pop, actions: presented, style: style,
                                 title: heading, message: lines.isEmpty ? nil : lines.joined(separator: "\n"))
        // A native action's tint is its accent (LLP 1069.011.000 D5); the
        // alert has one tint, the first action's. Unsaid, it is UIKit's
        // (LLP 1115 D4): a `color` the action only inherits is not its own.
        let lead = actions[0]
        owner.alert.view.tintColor = lead.isNativeButton
            ? lead.channels("accent_color").map { TextEngine.color($0) }
            : lead.ownUIColor("text_color")
        for (index, entry) in presented.enumerated() {
            let action = actions[index]
            let style: UIAlertAction.Style = action.props["destructive"] == "true" ? .destructive : .default
            let alertAction = UIAlertAction(title: entry.title, style: style) { [weak self, weak owner] _ in
                if let owner { self?.select(owner, index) }
            }
            alertAction.isEnabled = entry.enabled
            if action.props["accessibilityChecked"] == "true" { alertAction.accessibilityTraits.insert(.selected) }
            owner.alert.addAction(alertAction)
        }
        if let cancel = cancels.first {
            owner.alert.addAction(UIAlertAction(title: title(of: cancel), style: .cancel) { [weak self, weak owner] _ in
                if let owner { self?.select(owner, nil) }
            })
        }
        #if !os(tvOS)
        if style == .popover {
            guard let presentation = owner.alert.popoverPresentationController else { return false }
            presentation.delegate = owner
            if let source { anchor(presentation, at: source, area: PositionArea.of(pop)) }
            else {
                // No invoker: arrowless, over the middle of its screen.
                presentation.sourceView = controller.view
                presentation.sourceRect = CGRect(x: controller.view.bounds.midX, y: controller.view.bounds.midY, width: 0, height: 0)
                presentation.permittedArrowDirections = []
            }
        }
        #endif
        confirmation = owner
        controller.present(owner.alert, animated: !ExactEnv.agentFreezes)
        return true
    }
    /// LLP 1115 D6, by the HIG: a confirmation — explanatory text, a
    /// cancel, at most three buttons — is an alert, centred, its Cancel
    /// kept (React Native's `Alert.alert`, SwiftUI's `.alert`). Anything
    /// else is an action sheet: UIKit's unanchored one on a compact screen
    /// when it has a cancel to show (UIKit draws no cancel in a popover), else a
    /// popover, which a tap outside dismisses.
    private static func style(texts: Int, actions: Int, cancels: Int, compact: Bool) -> Confirmation.Style {
        if texts > 0, cancels == 1, actions + cancels <= 3 { return .alert }
        #if os(tvOS)
        return .sheet
        #else
        return cancels > 0 && compact ? .sheet : .popover
        #endif
    }
    #if !os(tvOS)
    private func anchor(_ presentation: UIPopoverPresentationController, at source: NodeView, area: String) {
        let anchor: UIView = source.isNativeButton ? presenter?.controls.controls[source.id] ?? source : source
        presentation.sourceView = anchor
        // A labelled row anchors at its text; an icon control uses its box.
        let labels = source.container.subviews.compactMap { $0 as? NodeView }.filter { $0.kind == "text" }
        let labelBox = labels.reduce(CGRect.null) { $0.union($1.convert($1.bounds, to: source)) }
        presentation.sourceRect = source.isNativeButton ? anchor.bounds : (labelBox.isNull ? source.bounds : CGRect(x: labelBox.minX, y: 0, width: labelBox.width, height: source.bounds.height))
        presentation.permittedArrowDirections = []
        presentation.canOverlapSourceViewRect = true
        Self.place(presentation, area, source: anchor)
    }
    #endif

    #if !os(tvOS)
    /// LLP 1021 §5: the sheet's side of its invoker, from its popover's
    /// `position-area`. `none` keeps the arrowless placement above (D2's
    /// below-left, native Messages' prompts). UIKit places a popover by the
    /// arrow directions it permits — `.down` puts it above its source,
    /// `.up` below, `.left` to its right (`right span-bottom`) — centred on
    /// the source rect where it fits, so a centred
    /// area anchors at the whole invoker. `center` (the invoker's own cell)
    /// anchors there with no arrow and may cover it, `none`'s presentation:
    /// UIKit centres it across the invoker and picks its vertical position.
    static func place(_ presentation: UIPopoverPresentationController, _ area: String, source: UIView) {
        guard area != "none" else { return }
        if PositionArea.centred(area) { presentation.sourceRect = source.bounds }
        if area == "center" {
            presentation.permittedArrowDirections = []
            presentation.canOverlapSourceViewRect = true
        } else {
            presentation.permittedArrowDirections = area.hasPrefix("top") ? .down : area.hasPrefix("right") ? .left : .up
            presentation.canOverlapSourceViewRect = false
        }
    }
    #endif

    /// The menu grammar, extracted (LLP 1021 D3): button rows become
    /// actions, a row that opens another menu its submenu (§5,
    /// "Submenus"); any other row is a section boundary. `path` holds the
    /// popovers above `pop`, whose rows opened it.
    func items(of pop: NodeView, path: [NodeView] = []) -> [UIMenuElement] {
        var sections: [[UIMenuElement]] = [[]]
        for case let row as NodeView in pop.container.subviews {
            // A context menu's preview (§5.1) is not one of its items.
            if row.props["contextPreview"] == "true" { continue }
            // A submenu authored inside its parent is no row of it.
            if row.props["popover"] != nil { continue }
            if let children = submenu(of: row, path: path + [pop]) {
                // UIMenu has no disabled state: a disabled opener is a dimmed action.
                let element: UIMenuElement = row.props["disabled"] == "true"
                    ? UIAction(title: title(of: row), image: image(of: row), attributes: .disabled) { _ in }
                    : UIMenu(title: title(of: row), image: image(of: row), children: children)
                element.subtitle = row.isNativeButton ? row.face?.subtitle : nil
                sections[sections.count - 1].append(element)
            } else if row.handlers.contains("press") {
                let id = row.id
                let image = image(of: row)
                let action = UIAction(title: title(of: row), image: image) { [weak self] _ in
                    self?.presenter?.press(id)
                }
                action.subtitle = row.isNativeButton ? row.face?.subtitle : nil
                if row.props["accessibilityChecked"] == "true" { action.state = .on }
                if row.props["disabled"] == "true" { action.attributes.insert(.disabled) }
                if row.props["destructive"] == "true" { action.attributes.insert(.destructive) }
                sections[sections.count - 1].append(action)
            } else if !(sections.last?.isEmpty ?? true) {
                sections.append([])
            }
        }
        let filled = sections.filter { !$0.isEmpty }
        if filled.count <= 1 { return filled.first ?? [] }
        return filled.map { UIMenu(options: .displayInline, children: $0) }
    }
    /// The items of the popover `row` opens as a submenu: one its
    /// `popovertarget` names to toggle or show, not a confirmation, not on
    /// `path` (a cycle is no submenu), with at least one item.
    private func submenu(of row: NodeView, path: [NodeView]) -> [UIMenuElement]? {
        guard row.isButton, let name = row.props["popovertarget"], row.props["popovertargetaction"] != "hide",
              let sub = presenter?.carrying("popover").first(where: { $0.props["id"] == name }),
              !isConfirmation(sub), !path.contains(where: { $0 === sub }) else { return nil }
        let children = items(of: sub, path: path)
        return children.isEmpty ? nil : children
    }

    /// A row's item image: its symbol, custom or native (LLP 1069.011.000
    /// D5), else its `img` — a provider's own icon — once that has loaded.
    /// The menu reads what the hidden row already holds; opening it never
    /// fetches, and an image still loading is no image until the next open.
    private func image(of row: NodeView) -> UIImage? {
        guard row.isButton else { return nil }
        if let symbol = row.face?.symbol { return UIImage(systemName: symbol) }
        guard !row.isNativeButton, let img = Self.firstImage(in: row) else { return nil }
        if let symbol = img.image, img.imageSource?.hasPrefix("symbol:") == true { return symbol }
        return img.raster.map { Self.rowImage($0.image.image) }
    }
    /// A menu row draws an image at its own size: fit it in the row's icon
    /// box, keeping its ratio.
    static func rowImage(_ bitmap: CGImage) -> UIImage {
        let side: CGFloat = 24, natural = CGSize(width: bitmap.width, height: bitmap.height)
        let scale = min(side / max(natural.width, 1), side / max(natural.height, 1))
        let size = CGSize(width: natural.width * scale, height: natural.height * scale)
        return UIGraphicsImageRenderer(size: size).image { _ in
            UIImage(cgImage: bitmap).draw(in: CGRect(origin: .zero, size: size))
        }.withRenderingMode(.alwaysOriginal)
    }
    private static func firstImage(in view: UIView) -> NodeView? {
        for case let node as NodeView in (view as? NodeView)?.container.subviews ?? view.subviews {
            if node.kind == "image" { return node }
            if let found = firstImage(in: node) { return found }
        }
        return nil
    }

    private func title(of v: NodeView) -> String {
        if v.kind == "text" { return v.paragraphSpec().runs.map(\.text).joined() }
        // A native button's children are its face, not views: its title, else its label.
        if v.isNativeButton { return v.face?.shown ?? "" }
        // A custom button whose face fits shows it too: a symbol-only row its
        // label (LLP 1069.011.000 D5); other content keeps its text.
        if v.isButton, let face = v.face, face.fits, let shown = face.shown { return shown }
        // Its accessible name: an `aria-hidden` child (a submenu row's `›`,
        // which the platform menu draws itself) is not part of it.
        return v.container.subviews
            .compactMap { $0 as? NodeView }
            .filter { $0.props["accessibilityElementsHidden"] != "true" }
            .map(title(of:))
            .filter { !$0.isEmpty }
            .joined(separator: " ")
    }
}
/// Observes UIKit touches without consuming the underlying control's activation.
final class PopoverTouch: UIGestureRecognizer, UIGestureRecognizerDelegate {
    private weak var host: MenuHost?
    init(host: MenuHost) {
        self.host = host
        super.init(target: nil, action: nil)
        cancelsTouchesInView = false; delaysTouchesBegan = false; delaysTouchesEnded = false
        delegate = self
    }
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        if let touched = touches.first?.view { host?.lightDismiss(touched) }
        state = .failed
    }
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool { true }
}

#endif

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
    /// Logical visibility can reopen before UIKit finishes dismissing the old alert.
    private struct PendingConfirmation {
        let popover: NodeView, source: NodeView?, modal: Bool
        let incarnation: UInt64, sourceIncarnation: UInt64?
        init(_ popover: NodeView, _ source: NodeView?, modal: Bool) {
            self.popover = popover; self.source = source; self.modal = modal
            incarnation = popover.incarnation; sourceIncarnation = source?.incarnation
        }
    }
    private var pendingConfirmation: PendingConfirmation?
    /// Under the agent, the popovers open in their painted presentation
    /// (LLP 1021 D4), by `id`: each in the top layer while open, as macOS's
    /// and the web's are, out of its parent and back at its index after.
    private var agentOpen: [String: Lifted] = [:]
    var presented: [NodeView] { agentOpen.values.sorted { $0.order < $1.order }.compactMap(\.popover) }
    func logicalParent(of view: UIView) -> UIView? {
        #if os(iOS)
        if let parent = context.logicalParent(of: view) { return parent }
        #endif
        return agentOpen.values.first { $0.popover === view }?.parent
    }
    var nativeConfirmation: NodeView? { pendingConfirmation?.popover ?? (confirmation?.finishing == false ? confirmation?.popover : nil) }
    /// How many popovers the agent has opened: each one's place in the stack.
    private var opened = 0
    private final class Lifted {
        var placement = PositionArea.History()
        let source: UInt32?
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
        #if os(iOS)
        var relay: MountRelay?
        #endif
        init(source: UInt32?, popover: NodeView, order: Int, ancestor: String?) {
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
        /// A source element, whether an invoker or an explicit `source=`.
        let sourced: Bool
        let invoked: Bool
        /// Opened by `show-modal`, not as a popover: HTML's popover commands
        /// leave it open, and its dialog `close` closes only this mode
        let modal: Bool
        weak var popover: NodeView?
        let route: String?
        let style: Style
        let alert: UIAlertController
        var finishing = false
        var dismissing = false
        var chosen: NodeView?
        /// An action as presented: the node, the title the sheet shows for
        /// it, and whether it could be chosen.
        final class Presented {
            weak var node: NodeView?
            let title: String
            /// A prompt's follows its action as the fields are typed in
            /// (`followPrompt`), as a hand-built rename's Save does.
            var enabled: Bool
            init(_ node: NodeView, title: String, enabled: Bool) { self.node = node; self.title = title; self.enabled = enabled }
        }
        let actions: [Presented]
        /// The alert's buttons for `actions`, in order.
        var buttons: [UIAlertAction] = []
        /// A prompt's text fields (`PromptFieldIOS.swift`).
        var fields: [PromptField] = []
        init(host: MenuHost, source: NodeView?, popover: NodeView, modal: Bool, actions: [Presented], style: Style, title: String?, message: String?) {
            self.host = host; self.source = source; self.popover = popover; self.modal = modal; self.actions = actions; self.style = style
            sourced = source != nil; invoked = source.map { host.opens($0, popover) } ?? false
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
    /// A `dialog` that is not a confirmation, as UIKit's sheet (DialogSheetIOS.swift).
    private(set) lazy var sheets = DialogSheets(presenter: presenter!)

    private func isDialog(_ node: NodeView) -> Bool { node.props["semanticTag"] == "dialog" }
    private func isConfirmation(_ node: NodeView) -> Bool {
        isDialog(node) || node.props["accessibilityRole"] == "alertdialog"
    }
    /// The `id` of what `source` invokes, as HTML resolves it (`Invocation`).
    private func target(of source: NodeView) -> String? { presenter?.invocation(of: source)?.target.props["id"] }
    private func closes(_ source: NodeView, _ target: NodeView, modal: Bool) -> Bool {
        presenter?.invocation(of: source)?.closes(target, modally: modal) ?? false
    }
    private func opens(_ source: NodeView, _ target: NodeView) -> Bool {
        presenter?.invocation(of: source).map { $0.target === target && $0.opens } ?? false
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
        if let owner = confirmation, !owner.fields.isEmpty { followPrompt(owner) }
        if let owner = confirmation, !valid(owner) { resetConfirmation() }
        sheets.sync()
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
            // A dialog in its sheet shows there (LLP 1115.000 D11.5).
            if !v.isHidden, !sheets.owns(v) { v.isHidden = true }
            // The lowest id among several with one `id`, as `invocation(of:)` resolves it.
            if let name = v.props["id"], popovers[name] == nil { popovers[name] = v }
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
                if v.disabled || v.ariaUnavailable { button.accessibilityTraits.insert(.notEnabled) }
                else { button.accessibilityTraits.remove(.notEnabled) }
                let actionId = UIAction.Identifier("exact.confirmation")
                button.removeAction(identifiedBy: actionId, for: .touchUpInside)
                button.addAction(UIAction(identifier: actionId) { [weak self, weak v, weak pop] _ in
                    guard let self, let v, let pop else { return }
                    _ = self.openConfirmation(from: v, popover: pop, modal: self.showsModal(v))
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
            #if os(iOS)
            if let native = button as? MenuButton {
                if native.popover !== pop { native.dismissMenu() }
                native.popover = pop
            }
            (button as? MenuButton)?.toggled = { [weak presenter, weak pop] open in
                if let pop { presenter?.popoverEvents.changed(pop, open: open) }
            }
            #endif
            if button.superview !== v { v.addSubview(button) }
            button.frame = v.bounds
            button.accessibilityLabel = v.props["accessibilityLabel"] ?? title(of: v)
            button.accessibilityIdentifier = v.props["testId"] ?? v.props["id"]
            button.isEnabled = !v.disabled
            if v.disabled || v.ariaUnavailable { button.accessibilityTraits.insert(.notEnabled) }
            else { button.accessibilityTraits.remove(.notEnabled) }
            // One menu per invoker, kept across batches: it reads its rows
            // each time it opens, so a batch (a poll's answer, every second)
            // has nothing to change in it, and replacing it would reload a
            // menu that is open — UIKit shows its "Loading…" row again each time.
            // The popover's `aria-label` titles the menu ("Open location in").
            let heading = pop.props["accessibilityLabel"] ?? ""
            let shape = "\(target)|\(heading)"
            if menuShapes[v.id] != shape || button.menu == nil {
                menuShapes[v.id] = shape
                button.menu = pullDown(of: v, titled: heading)
            }
        }
        for (id, b) in overlays where !live.contains(id) {
            #if os(iOS)
            (b as? MenuButton)?.dismissMenu()
            #endif
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
        guard source.isButton, let name = target(of: source) else { return false }
        // A dialog's own close button, in its sheet (LLP 1115.000 D11.5).
        if let dialog = sheets.topDialog, dialog.props["id"] == name { return closes(source, dialog, modal: true) && eligible(source) }
        guard let pop = agentOpen[name]?.popover else { return false }
        return !isConfirmation(pop) && closes(source, pop, modal: false) && eligible(source)
    }
    /// D13: activation finished; now read the surviving control's live target.
    func invoke(_ source: NodeView) {
        // A bar item's `command="show-modal"` (`BarPress`): the page hides
        // its button while the bar shows it, and what it opens presents with
        // no invoker, as a pull-down's confirmation does. A bar item that
        // opens a popover is its pull-down (`pullDown`), never this.
        let barModal = !eligible(source) && eligible(source, inBar: true)
            && presenter?.invocation(of: source)?.action == .showModal
        guard source.isNativeButton || barModal || closesPresentedContent(source), barModal || eligible(source),
              let invocation = presenter?.invocation(of: source), let name = invocation.target.props["id"] else { return }
        let pop = invocation.target, action = invocation.action
        if barModal {
            if isConfirmation(pop), openModally(pop) == nil, !sheets.owns(pop) {
                _ = openConfirmation(from: nil, popover: pop, modal: true, dispatchPress: false)
            }
            return
        }
        if isConfirmation(pop) {
            let modally = openModally(pop), open = modally != nil
            // HTML's dialog commands act only on a modal dialog, and its
            // popover commands only on a popover.
            if let modally, modally != (action == .close || action == .showModal) { return }
            if action == .close, agentOpen[name] != nil { return }
            if action == .hide || action == .close || action == .toggle && open {
                hideConfirmation(pop)
            } else if !open {
                _ = openConfirmation(from: source, popover: pop, modal: action == .showModal, dispatchPress: false)
            }
        } else {
            let open = agentOpen[name] != nil
            if action == .hide || action == .toggle && open { drop(name) }
            else if !open { agentShow(pop, named: name, from: source.id) }
        }
    }

    func action(_ command: String, _ args: [Any]) {
        guard let presenter, let name = args.first as? String, let pop = popover(named: name), pop.props["popover"] != nil else {
            self.presenter?.session?.log("\(command) refused: no popover has that id"); return
        }
        if openModally(pop) == true { presenter.session?.log("\(command) ignored: \(name) is open as a modal dialog"); return }
        let sourceId = args.count > 1 ? args[1] as? String : nil
        let source = sourceId.flatMap { id in presenter.views.values.first { $0.props["id"] == id } }
        if sourceId != nil && source == nil { presenter.session?.log("\(command) refused: no source has that id"); return }
        if command == "hidePopover" || command == "togglePopover" && isOpen(pop) {
            if pendingConfirmation?.popover === pop {
                pendingConfirmation = nil
                presenter.popoverEvents.changed(pop, open: false)
            }
            if let owner = confirmation, owner.popover === pop { finish(owner, chosen: nil) }
            else { drop(name) }
            #if os(iOS)
            for case let button as MenuButton in overlays.values where button.popover === pop { button.dismissMenu() }
            context.close(pop)
            #endif
        } else if !isOpen(pop), !pop.inert, pop.window != nil, source.map({ eligible($0) }) ?? true {
            if isConfirmation(pop) { _ = openConfirmation(from: source, popover: pop, modal: false, dispatchPress: false) }
            else { agentShow(pop, named: name, from: source?.id) }
        }
    }
    /// A live popover by its id.
    private func popover(named name: String) -> NodeView? {
        presenter?.views.values.first { $0.props["id"] == name && ($0.props["popover"] != nil || isDialog($0)) }
    }

    private func live(_ node: NodeView) -> Bool { presenter?.views[node.id] === node }
    /// `inBar`: the node is a bar item's button, which the page hides while
    /// the bar shows it: that hiding, and only that, does not count.
    /// A pull-down's rows, read as UIKit opens it: the invoker's press applies
    /// (synchronously), then what the surviving invoker opens now, as HTML
    /// runs a button's listeners before its invocation; a confirmation or
    /// dialog the press retargeted it to presents in place of the menu. Only
    /// the invoker it was built for acts, and only while it is eligible.
    ///
    /// The popover's `aria-label` titles the rows. In the page it is an
    /// inline section's heading, as a SwiftUI `Menu`'s title is drawn there:
    /// a top-level `UIMenu` title draws a taller header and sets the rows
    /// about 10 pt lower. A bar item's pull-down is a hand-built bar item's
    /// `UIMenu(title:)`, taller header and all (LLP 1021, amended 2026-10-16).
    func pullDown(of v: NodeView, titled heading: String, inBar: Bool = false) -> UIMenu {
        let rows = rows(of: v, inBar: inBar)
        if heading.isEmpty { return UIMenu(children: [rows]) }
        return inBar ? UIMenu(title: heading, children: [rows]) : UIMenu(children: [UIMenu(title: heading, options: .displayInline, children: [rows])])
    }
    private func rows(of v: NodeView, inBar: Bool) -> UIDeferredMenuElement {
        UIDeferredMenuElement.uncached { [weak self, weak v, token = v.incarnation] completion in
            func current() -> NodeView? {
                guard let v, v.incarnation == token, self?.presenter?.views[v.id] === v, self?.eligible(v, inBar: inBar) == true else { return nil }
                return v
            }
            guard let self, let presenter = self.presenter, let invoker = current() else { return completion([]) }
            if invoker.handlers.contains("press") { presenter.press(invoker.id) }
            guard let invoker = current(), let invocation = presenter.invocation(of: invoker), invocation.opens else { return completion([]) }
            if self.isConfirmation(invocation.target) {
                completion([])
                let target = invocation.target, targetToken = target.incarnation
                DispatchQueue.main.async {
                    guard let invoker = current(), target.incarnation == targetToken, presenter.views[target.id] === target else { return }
                    _ = self.openConfirmation(from: inBar ? nil : invoker, popover: target, modal: invocation.action == .showModal, dispatchPress: false)
                }
                return
            }
            completion(self.items(of: invocation.target))
        }
    }
    func eligible(_ node: NodeView, contextMenu: Bool = false, inertBoundary: UIView? = nil, inBar: Bool = false) -> Bool {
        guard live(node), !(contextMenu ? node.formDisabled : node.disabled), presenter?.navigation.isInactiveRoute(containing: node) == false else { return false }
        var ancestor: UIView? = node
        var checksInert = true
        while let view = ancestor {
            // A modal dialog escapes inertness above itself. Its issued command
            // also survives the invoker subsequently becoming inert.
            if view === inertBoundary { checksInert = false }
            if let n = view as? NodeView, (contextMenu ? n.formDisabled : n.disabled) || (checksInert && n.props["inert"] == "true") { return false }
            if view.isHidden, !(inBar && presenter?.navigation.hiddenForBar(view) == true) {
                guard let pop = view as? NodeView, isConfirmation(pop),
                      pop.props["popover"] != nil || isDialog(pop) else { return false }
            }
            ancestor = presenter?.eventParent(of: view) ?? view.superview
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
            && (!owner.invoked || opens(source, pop)) && isConfirmation(pop)
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
        return closes(action, pop, modal: owner.modal) && action.isDescendant(of: pop) && title(of: action) == entry.title
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
    /// A prompt follows the page as its field is typed in: an action its
    /// author disables while the name is empty is dimmed, as a hand-built
    /// rename's Save is, rather than a change that withdraws it. A field the
    /// page swapped for another (an `if` over two inputs) withdraws it: the
    /// field shown is no longer the input's.
    private func followPrompt(_ owner: Confirmation) {
        guard let pop = owner.popover, !owner.finishing else { return }
        let shown = pop.container.subviews.compactMap { $0 as? NodeView }.filter(\.isPromptField)
        guard shown.count == owner.fields.count, zip(shown, owner.fields).allSatisfy({ $0 === $1.node }) else {
            resetConfirmation()
            return
        }
        for (entry, button) in zip(owner.actions, owner.buttons) {
            guard let node = entry.node, live(node) else { continue }
            entry.enabled = choosable(node, in: pop)
            button.isEnabled = entry.enabled
        }
    }
    private func cancelled(_ owner: Confirmation) {
        // UIKit also delivers this after a selected action's handler. That
        // notification must not cancel the action awaiting the next turn.
        guard confirmation === owner, !owner.finishing else { return }
        owner.finishing = true
        owner.fields.forEach { $0.end() }
        confirmation = nil
        owner.popover?.queueDialogClose()
        if let pop = owner.popover { presenter?.popoverEvents.changed(pop, open: false) }
    }
    /// `chosen` is the selected action, nil for the cancel.
    private func finish(_ owner: Confirmation, chosen: NodeView?) {
        guard confirmation === owner, !owner.finishing else { return }
        owner.finishing = true
        // A field's `change` before the action's press, as a blur's before a click.
        owner.fields.forEach { $0.end() }
        owner.chosen = chosen
        if let pop = owner.popover { presenter?.popoverEvents.changed(pop, open: false) }
        // UIKit drops a dismissal during presentation. Its completion below
        // starts it once the alert has arrived; logical visibility is already closed.
        if !owner.alert.isBeingPresented { dismissConfirmation(owner) }
    }
    private func dismissConfirmation(_ owner: Confirmation) {
        guard confirmation === owner, !owner.dismissing else { return }
        owner.dismissing = true
        owner.alert.dismiss(animated: !ExactEnv.agentFreezes) { [weak self, owner] in
            // A native action arrives after the alert leaves its window;
            // dismiss can therefore complete synchronously inside UIKit's
            // selection callback, before its dismissal delegate is called.
            // Keep the owner until that stack unwinds before app code can
            // destroy the presenting editor or dismiss its parent sheet.
            DispatchQueue.main.async { [weak self, owner] in
                guard let self, self.confirmation === owner else { return }
                let action = owner.chosen.flatMap { self.valid(owner) && self.valid(owner, chosen: $0) ? $0 : nil }
                self.confirmation = nil
                let pending = self.pendingConfirmation
                self.pendingConfirmation = nil
                if let pending {
                    let pop = pending.popover
                    if pop.incarnation != pending.incarnation || pending.source?.incarnation != pending.sourceIncarnation ||
                        !self.openConfirmation(from: pending.source, popover: pop, modal: pending.modal, dispatchPress: false, announce: false) {
                        self.presenter?.popoverEvents.changed(pop, open: false)
                    }
                }
                if let action { self.presenter?.press(action.id) }
            }
            owner.popover?.queueDialogClose()
        }
    }
    private func resetConfirmation() {
        if let pending = pendingConfirmation {
            pendingConfirmation = nil
            presenter?.popoverEvents.changed(pending.popover, open: false)
        }
        guard let owner = confirmation else { return }
        confirmation = nil
        if !owner.finishing, let pop = owner.popover { presenter?.popoverEvents.changed(pop, open: false) }
        owner.finishing = true
        owner.fields.forEach { $0.end() }
        // Only this session's alert; never dismiss through a containing app's
        // root controller, which might now present something unrelated.
        owner.alert.dismiss(animated: false) { _ = owner }
    }
    func reset() {
        resetConfirmation()
        sheets.reset()
        overlays.values.forEach {
            #if os(iOS)
            ($0 as? MenuButton)?.dismissMenu()
            #endif
            $0.removeFromSuperview()
        }
        overlays.removeAll()
        for name in Array(agentOpen.keys) { drop(name) }
        #if os(iOS)
        context.reset()
        #endif
        laidOut.removeAll()
    }

    /// The top-layer boxes given back to layout to show in Exact's views.
    /// The Apple host covers each `dialog` and popover whole while it is
    /// closed (`host/apple/src/closed.rs`), as a browser lays out neither:
    /// UIKit's alert and menu read their rows, never their frames.
    private var laidOut = Set<UInt32>()
    private var coverPending = false
    /// `box` laid out now, before it shows in Exact's views: a sheet, the
    /// top layer, a context menu's preview.
    func layOut(_ box: NodeView) {
        guard let session = presenter?.session, laidOut.insert(box.id).inserted else { return }
        session.apply(session.runtime.covers([(box.id, nil)]))
        var stack: [UIView] = [box]
        while let view = stack.popLast() {
            (view as? NodeView)?.makeWaitingField()
            stack.append(contentsOf: view.subviews)
        }
    }
    /// The `input`s `batch` makes inside a closed `dialog` or popover: the
    /// kernel lays out neither, and their UIKit fields wait for it to show
    /// (`NodeView.makeWaitingField`). Their boxes are found from the batch's
    /// children ops and the views there are.
    func waitingFields(_ batch: Batch) -> Set<UInt32> {
        guard let presenter, batch.ops.contains(where: { $0.op == .create && $0.kind == "input" }) else { return [] }
        var parents: [UInt32: UInt32] = [:], made: [UInt32: [String: String]] = [:]
        for op in batch.ops {
            if op.op == .children { for child in op.ids { parents[child] = op.id } }
            else if op.op == .create { made[op.id] = op.props }
        }
        func parent(_ id: UInt32) -> UInt32? {
            if let p = parents[id] { return p }
            return sequence(first: presenter.views[id]?.superview, next: { $0?.superview }).lazy.compactMap { $0 as? NodeView }.first?.id
        }
        var waiting = Set<UInt32>()
        for op in batch.ops where op.op == .create && op.kind == "input" {
            var at = parent(op.id)
            while let id = at {
                let props = made[id] ?? presenter.views[id]?.props ?? [:]
                if props["popover"] != nil || props["semanticTag"] == "dialog" {
                    if !laidOut.contains(id) { waiting.insert(op.id) }
                    break
                }
                at = parent(id)
            }
        }
        return waiting
    }
    /// After one closes: what shows in Exact's views no more is covered
    /// again, after this turn, unless it has stopped being a box of the top
    /// layer (the host gave it back for good).
    func coverClosed() {
        guard !laidOut.isEmpty, !coverPending else { return }
        coverPending = true
        DispatchQueue.main.async { [weak self] in self?.coverClosedNow() }
    }
    private func coverClosedNow() {
        coverPending = false
        guard let presenter, let session = presenter.session else { return }
        var covers: [(UInt32, HostCover?)] = []
        for id in laidOut {
            guard let node = presenter.views[id] else { laidOut.remove(id); continue }
            guard !showsInViews(node) else { continue }
            laidOut.remove(id)
            if node.props["popover"] != nil || isDialog(node) { covers.append((id, .whole)) }
        }
        if !covers.isEmpty { session.apply(session.runtime.covers(covers)) }
    }
    private func showsInViews(_ node: NodeView) -> Bool {
        if sheets.owns(node) || agentOpen.values.contains(where: { $0.popover === node }) { return true }
        #if os(iOS)
        return context.open?.popover === node
        #else
        return false
        #endif
    }
    /// Whether `node` is an open popover in the top layer (the agent's).
    func lifted(_ node: NodeView) -> Bool {
        #if os(iOS)
        if context.lifts(node) { return true }
        #endif
        return sheets.owns(node) || agentOpen.values.contains { $0.popover === node }
    }
    /// A children op on `parent` while a child of it is in the top layer:
    /// the child stays there, its index kept for its return; one the op
    /// drops closes.
    /// A frame op on `node`: a context menu's lifted row takes it as its home.
    func framed(_ node: NodeView) {
        #if os(iOS)
        context.framed(node)
        #endif
        sheets.framed(node)
    }
    func children(_ parent: UIView, _ wanted: [NodeView]) {
        #if os(iOS)
        context.children(parent, wanted)
        #endif
        sheets.children(parent, wanted)
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
        #if os(iOS)
        if entry.relay == nil { entry.relay = MountRelay(presenter, mount: entry.layer) }
        #endif
        let host: UIView = presenter.modals.coordinateView ?? presenter.viewport
        if entry.layer.superview !== host || host.subviews.last !== entry.layer { host.addSubview(entry.layer) }
        entry.layer.setPaintForeground()
        if entry.layer.frame != host.bounds { entry.layer.frame = host.bounds }
        if pop.superview !== entry.layer { entry.layer.addSubview(pop) }
        if pop.isHidden { pop.isHidden = false }
        guard let source = entry.source.flatMap({ presenter.views[$0] }), source.window != nil else {
            pop.center = CGPoint(x: entry.layer.bounds.midX, y: entry.layer.bounds.midY); return
        }
        let anchorView: UIView = source.isNativeButton ? presenter.controls.controls[source.id] ?? source : source
        let anchor = anchorView.convert(anchorView.bounds, to: entry.layer), size = pop.bounds.size
        let at = PositionArea.origin(PositionArea.of(pop), anchor: anchor, size: size,
                                    margins: PositionArea.margins(of: pop),
                                    fallbacks: pop.style["position_try_fallbacks"]?.string ?? "none",
                                    history: &entry.placement, in: entry.layer.bounds)
        let center = CGPoint(x: at.x + size.width / 2, y: at.y + size.height / 2)
        if pop.center != center { pop.center = center }
    }
    /// Out of the top layer, hidden, back where it was; the popovers nested
    /// in it first, as hiding a popover hides those above it in the stack.
    private func drop(_ name: String) {
        for (nested, entry) in agentOpen where entry.ancestor == name { drop(nested) }
        guard let entry = agentOpen.removeValue(forKey: name) else { return }
        coverClosed()
        entry.popover?.endEditing(true)
        entry.layer.removeFromSuperview()
        guard let pop = entry.popover else { return }
        presenter?.popoverEvents.changed(pop, open: false)
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
        if pendingConfirmation?.popover === node || confirmation?.popover === node && confirmation?.finishing == false || sheets.isOpen(node) { return true }
        #if os(iOS)
        if context.isOpen(node) || overlays.values.contains(where: { ($0 as? MenuButton).map { $0.popover === node && $0.isMenuOpen } == true }) { return true }
        #endif
        return node.props["id"].map { agentOpen[$0] != nil } == true
    }

    /// Production and agent touches share HTML's light-dismiss exclusions.
    func lightDismiss(_ touched: UIView) {
        var kept = Set<String>()
        var inside = agentOpen.first { _, entry in
            entry.popover.map { touched === $0 || touched.isDescendant(of: $0) } ?? false
        }?.key
        while let name = inside, !kept.contains(name) { kept.insert(name); inside = agentOpen[name]?.ancestor }
        let touchedNodes = sequence(first: touched as UIView?, next: { $0?.superview }).compactMap { $0 as? NodeView }
        for (name, entry) in agentOpen.sorted(by: { $0.value.order > $1.value.order }) {
            guard agentOpen[name] != nil, entry.popover?.props["popover"] != "manual", !kept.contains(name) else { continue }
            // A touch on an eligible button that invokes it now is not outside
            // it (HTML's light dismiss); the button that opened it may since
            // name something else.
            if let pop = entry.popover, touchedNodes.contains(where: { eligible($0) && presenter?.invocation(of: $0)?.target === pop }) { continue }
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
        // After the tap's handler has run: what the surviving button invokes now.
        DispatchQueue.main.async { [weak self, weak presenter, weak node, token = node.incarnation] in
            guard let self, let presenter, let node, node.incarnation == token, presenter.views[node.id] === node, self.eligible(node),
                  let invocation = presenter.invocation(of: node), !self.isConfirmation(invocation.target),
                  let name = invocation.target.props["id"] else { return }
            let open = self.agentOpen[name] != nil
            switch invocation.action {
            case .hide where open, .toggle where open: self.drop(name)
            case .show where !open, .toggle where !open: self.agentShow(invocation.target, named: name, from: node.id)
            default: break
            }
        }
    }
    private func agentShow(_ pop: NodeView, named name: String, from source: UInt32?) {
        layOut(pop)
        guard live(pop) else { return }
        // Nested in the latest open popover holding its opener or itself.
        let opener = source.flatMap { presenter?.views[$0] }
        let ancestor = agentOpen.filter { _, open in
            guard let outer = open.popover else { return false }
            return opener.map { $0.isDescendant(of: outer) } == true || pop.isDescendant(of: outer)
        }.max { $0.value.order < $1.value.order }?.key
        if pop.props["popover"] != "manual" {
            var keep = Set<String>(), current = ancestor
            while let name = current, keep.insert(name).inserted { current = agentOpen[name]?.ancestor }
            for (other, entry) in agentOpen where entry.popover?.props["popover"] != "manual" && !keep.contains(other) { drop(other) }
        }
        opened += 1
        let entry = Lifted(source: source, popover: pop, order: opened, ancestor: ancestor)
        agentOpen[name] = entry
        presenter?.popoverEvents.changed(pop, open: true)
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
        guard ExactEnv.agentMode, let presenter, let name = node.props["contextPopover"], eligible(node, contextMenu: true) else { return false }
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
            return agentOpen.values.contains { entry in entry.source.flatMap { presenter.views[$0] }?.isNativeButton != true }
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
    func unmounted() { resetConfirmation(); sheets.reset() }
    var inTransition: Bool {
        if sheets.inTransition { return true }
        guard let owner = confirmation else { return false }
        return owner.finishing || owner.alert.isBeingPresented || owner.alert.isBeingDismissed
    }
    func observation() -> [String: Any]? {
        #if os(iOS)
        if let open = context.observation() { return open }
        #endif
        if confirmation == nil, let open = agentOpen.values.max(by: { $0.order < $1.order }), let pop = open.popover {
            return ["kind": "popover", "source": open.source.map { Int($0) as Any } ?? NSNull(), "popover": Int(pop.id), "phase": "open"]
        }
        guard let owner = confirmation else { return sheets.observation }
        return ["kind": "confirmation", "source": owner.source.map { Int($0.id) as Any } ?? NSNull(),
                "popover": owner.popover.map { Int($0.id) as Any } ?? NSNull(), "phase": inTransition ? "transition" : "open",
                "actionStyle": owner.alert.actions.first?.style == .destructive ? "destructive" : "default",
                "presentation": owner.style.rawValue,
                "actions": owner.actions.count]
    }
    /// LLP 1080.001 D3: the views this host adds — a node's overlay button,
    /// an open popover's top layer — and the popovers it hides or lifts.
    func inspectionOwns(_ view: UIView) -> Bool {
        overlays.values.contains { $0 === view } || agentOpen.values.contains { $0.layer === view } || sheets.inspectionOwns(view)
    }
    func hides(_ node: NodeView) -> Bool {
        presenter?.carrying("popover").contains { $0 === node } == true || presenter?.carrying("tag:dialog").contains { $0 === node } == true
    }
    func projects(_ node: NodeView) -> Bool { sheets.owns(node) || agentOpen.values.contains { $0.popover === node } }

    func ownsConfirmationNode(_ node: NodeView) -> Bool {
        var ancestor: UIView? = node
        while let view = ancestor {
            // A dialog laid out in its sheet holds ordinary nodes (LLP 1115.000 D11.5).
            if let n = view as? NodeView, isConfirmation(n), n.props["popover"] != nil || isDialog(n) { return !isSheet(n) }
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
               closes(node, pop, modal: owner.modal) {
                finish(owner, chosen: nil); return true
            }
            return false
        }
        guard let name = target(of: node), let pop = presenter?.views.values.first(where: {
            $0.props["id"] == name && isConfirmation($0) && ($0.props["popover"] != nil || isDialog($0))
        }) else { return nil }
        // The close button of a dialog in its sheet takes the ordinary tap.
        if sheets.owns(pop), !opens(node, pop) { return nil }
        guard opens(node, pop) else { return false }
        return openConfirmation(from: node, popover: pop, modal: showsModal(node))
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
    /// `showModal(id)` from an action (LLP 1115 D6): a `dialog` or an
    /// alertdialog popover by its `id`, presented with no invoker (a context
    /// menu's item asks before it deletes): a confirmation as its alert, any
    /// other `dialog` as a sheet (LLP 1115.000 D11.5). A menu that is still
    /// leaving holds UIKit's presentation; this waits out up to two seconds.
    func showModal(_ name: String, attempt: Int = 0) {
        guard let pop = popover(named: name) else {
            presenter?.session?.log("showModal \(name) refused: no dialog or popover has that id"); return
        }
        guard isConfirmation(pop) else {
            presenter?.session?.log("showModal \(name) refused: iOS presents a dialog, or a role=alertdialog popover as its alert"); return
        }
        if openModally(pop) != nil || sheets.owns(pop) { presenter?.session?.log("showModal \(name) ignored: dialog is already open"); return }
        if confirmation == nil, showable(pop), let controller = controller(for: pop), busy(controller), attempt < 20 {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in self?.showModal(name, attempt: attempt + 1) }
            return
        }
        if !openConfirmation(from: nil, popover: pop, modal: true), openModally(pop) == nil, !sheets.owns(pop) {
            presenter?.session?.log("showModal \(name): nothing presented")
        }
    }
    /// `close(id)` from an action: the confirmation by its `id`, as its
    /// cancel; a dialog in its sheet, as its own close button.
    func closeModal(_ name: String) {
        guard let pop = popover(named: name), openModally(pop) == true else { return }
        hideConfirmation(pop)
    }
    /// Ends a confirmation however it is open: its alert, a reopen queued
    /// behind a leaving one, or its sheet
    private func hideConfirmation(_ pop: NodeView) {
        if let owner = confirmation, owner.popover === pop, !owner.finishing { finish(owner, chosen: nil) }
        else if pendingConfirmation?.popover === pop { pendingConfirmation = nil; presenter?.popoverEvents.changed(pop, open: false) }
        else if sheets.isOpen(pop) { sheets.close(pop) }
    }
    /// The agent's `back` (LLP 1116.006 D4): the open confirmation's cancel,
    /// as a keyboard's Escape or a tap outside it chooses it. nil with none
    /// open; false while it is still moving.
    func dismissPopover() -> Bool {
        guard let (name, _) = agentOpen.filter({ $0.value.popover?.props["popover"] != "manual" }).max(by: { $0.value.order < $1.value.order }) else { return false }
        drop(name); return true
    }
    func agentCancel() -> Bool? {
        if dismissPopover() { return true }
        guard let owner = confirmation else { return nil }
        guard !inTransition, valid(owner) else { return false }
        finish(owner, chosen: nil)
        return true
    }

    /// The controller a presentation from `view` goes over: its own, or the
    /// dialog sheet that is already up over it.
    private func controller(for view: UIView) -> UIViewController? {
        var responder: UIResponder? = view
        while responder != nil && !(responder is UIViewController) { responder = responder?.next }
        guard let found = responder as? UIViewController else { return nil }
        if let top = sheets.top, top !== found, found.presentedViewController != nil, top.view.window === found.view.window { return top }
        return found
    }
    /// A confirmation's rows: its children, its actions (buttons with a
    /// `press`) and its cancels (buttons that only close it).
    private func rows(of pop: NodeView, modal: Bool) -> (children: [NodeView], actions: [NodeView], cancels: [NodeView]) {
        let children = pop.container.subviews.compactMap { $0 as? NodeView }
        return (children, children.filter { $0.isButton && $0.handlers.contains("press") },
                children.filter { $0.isButton && !$0.handlers.contains("press") && closes($0, pop, modal: modal) })
    }
    /// Why `pop` is not a confirmation, or nil when it is one: text, actions
    /// that also close it and at most one cancel, and, for a `dialog`, a
    /// `closedby` the confirmation of its shape has (LLP 1115.000 D13.4).
    /// An alert has no outside and always has its Cancel, so its is
    /// `closerequest`, what lowering implies for the shape. An action sheet
    /// and a popover are dismissed by a tap outside, so theirs is `any`.
    private func unconfirmable(_ pop: NodeView, modal: Bool) -> String? {
        let (children, actions, cancels) = rows(of: pop, modal: modal)
        if actions.isEmpty { return "no action (a button with press)" }
        if cancels.count > 1 { return "\(cancels.count) cancels; at most one hide-only button" }
        // Only an alertdialog is a prompt; a plain `dialog` with a field is a
        // form, its sheet (DialogSheetIOS.swift).
        let asks = pop.props["accessibilityRole"] == "alertdialog"
        if !children.allSatisfy({ child in child.kind == "text" || asks && child.isPromptField || actions.contains { $0 === child } || cancels.contains { $0 === child } }) {
            return "only text, text fields, actions and one cancel may be its rows"
        }
        if !actions.allSatisfy({ closes($0, pop, modal: modal) }) {
            return modal && isDialog(pop) ? "each action must also close it (command=close)" : "each action must also hide it (popovertargetaction=hide)"
        }
        // A prompt's field counts with its text: what it asks explains it.
        // It asks one thing, as lowering holds it to (rules/DEFERRED.md).
        let fields = asks ? children.filter(\.isPromptField).count : 0
        if fields > 1 { return "a prompt has one text field" }
        let alert = ConfirmationShape.isAlert(texts: children.filter { $0.kind == "text" && !title(of: $0).isEmpty }.count + fields,
                                              actions: actions.count, cancels: cancels.count)
        // Only an alert takes text fields; an action sheet or popover has none.
        if fields > 0, !alert || isDialog(pop) && pop.props["closedby"] == "any" {
            return "a text field is only an alert's (a prompt: one cancel, at most three buttons, no closedby=any)"
        }
        guard isDialog(pop), pop.props["closedby"] != "any" else { return nil }
        let closedby = pop.props["closedby"] ?? "closerequest"
        if closedby == "none" { return "closedby is none: an alert always has its Cancel" }
        return alert ? nil : "closedby is \(closedby), and only an alert has no outside"
    }
    /// The input `node`'s props were written: the prompt field standing in
    /// for it, if one shows, takes them (`PromptField.write`).
    func promptInputChanged(_ node: NodeView) {
        confirmation?.fields.first { $0.node === node }?.write()
    }
    /// A prompt: an alertdialog with a text field (`PromptField`). Every
    /// action may be disabled while it opens (Save, until a name is typed):
    /// its field and its cancel are still there.
    private func prompts(_ pop: NodeView) -> Bool {
        pop.props["accessibilityRole"] == "alertdialog" && pop.container.subviews.contains { ($0 as? NodeView)?.isPromptField == true }
    }
    /// LLP 1115.000 D11.5: a `dialog` that is not a confirmation by its rows
    /// is a sheet, its content laid out by the kernel (DialogSheetIOS.swift).
    func isSheet(_ node: NodeView) -> Bool {
        isDialog(node) && node.props["popover"] == nil && (sheets.owns(node) || unconfirmable(node, modal: true) != nil)
    }
    private func busy(_ controller: UIViewController) -> Bool {
        controller.presentedViewController != nil || controller.isBeingDismissed || controller.isBeingPresented
    }
    /// Whether a confirmation is open modally, nil when it is not open: a
    /// reopen waiting on its predecessor's dismissal counts, a finishing
    /// alert does not
    func openModally(_ pop: NodeView) -> Bool? {
        if let pending = pendingConfirmation, pending.popover === pop { return pending.modal }
        if let owner = confirmation, owner.popover === pop, !owner.finishing { return owner.modal }
        return sheets.isOpen(pop) ? true : nil
    }
    private func showsModal(_ source: NodeView) -> Bool { presenter?.invocation(of: source)?.action == .showModal }
    /// `anchorItem`: with no `source`, what UIKit says a popover presents from
    /// (`UIMenuLeaf.presentationSourceItem`), such as the button whose menu
    /// held the row that asked.
    private func openConfirmation(from source: NodeView?, popover pop: NodeView, modal: Bool, anchorItem: AnyObject? = nil,
                                  dispatchPress: Bool = true, announce: Bool = true) -> Bool {
        var modal = modal
        // Both invoker actions fire, as on the web: the source's press first
        // (Session.press applies its batch synchronously), then what the
        // surviving source invokes now, which its press may have retargeted.
        if dispatchPress, let source, source.handlers.contains("press") {
            guard confirmation == nil || confirmation?.finishing == true, !sheets.owns(pop), eligible(source), live(pop) else { return false }
            let token = source.incarnation
            presenter?.press(source.id)
            guard source.incarnation == token, presenter?.views[source.id] === source,
                  let now = presenter?.invocation(of: source), now.opens else { return false }
            if now.target !== pop {
                guard isConfirmation(now.target) else {
                    // UIKit opens a button's menu only from its own tap.
                    presenter?.session?.log("confirmation \(pop.props["id"] ?? "?") refused: its press retargeted the button to a menu")
                    return false
                }
                return openConfirmation(from: source, popover: now.target, modal: now.action == .showModal, dispatchPress: false, announce: announce)
            }
            modal = now.action == .showModal
        }
        if confirmation?.finishing == true {
            guard !isOpen(pop), showable(pop), source.map({ eligible($0) }) ?? true,
                  unconfirmable(pop, modal: modal) == nil, prompts(pop) || rows(of: pop, modal: modal).actions.contains(where: { choosable($0, in: pop) }) else { return false }
            if let previous = pendingConfirmation { presenter?.popoverEvents.changed(previous.popover, open: false) }
            pendingConfirmation = PendingConfirmation(pop, source, modal: modal)
            presenter?.popoverEvents.changed(pop, open: true)
            return true
        }
        func openable() -> Bool {
            guard confirmation == nil, !sheets.owns(pop) else { return false }
            guard let source else { return showable(pop) }
            return eligible(source) && live(pop) && source.window != nil
        }
        guard openable() else { return false }
        // Refused shapes are said, never silently dropped: the tap opens nothing.
        func refuse(_ why: String) -> Bool {
            presenter?.session?.log("confirmation \(pop.props["id"] ?? "?") refused: \(why)")
            return false
        }
        // The split by shape (LLP 1115 D6, LLP 1115.000 D11.5): a `dialog`
        // that is not a confirmation is a sheet; a popover that is not one
        // has no other presentation here.
        if let why = unconfirmable(pop, modal: modal) {
            guard isDialog(pop), pop.props["popover"] == nil else { return refuse(why) }
            guard let controller = controller(for: source ?? pop), !busy(controller) else { return false }
            return sheets.show(pop, from: source, over: controller)
        }
        let (children, actions, cancels) = rows(of: pop, modal: modal)
        let boundary = isDialog(pop) ? pop.superview : nil
        // A disabled choice shows dimmed; the others stay choosable.
        guard prompts(pop) || actions.contains(where: { eligible($0, inertBoundary: boundary) }) else { return refuse("every action is disabled") }
        guard let controller = controller(for: source ?? pop), !busy(controller) else { return false }
        let presented = actions.map { Confirmation.Presented($0, title: title(of: $0), enabled: eligible($0, inertBoundary: boundary)) }
        let texts = children.filter { $0.kind == "text" }.map(title(of:)).filter { !$0.isEmpty }
        let fields = children.filter(\.isPromptField)
        let label = pop.props["accessibilityLabel"].flatMap { $0.isEmpty ? nil : $0 }
        let style = Self.style(texts: texts.count + fields.count, actions: actions.count, cancels: cancels.count,
                               compact: controller.traitCollection.horizontalSizeClass != .regular,
                               outside: isDialog(pop) && pop.props["closedby"] == "any")
        // The alert's title is its aria-label ("Remove book?"), else its
        // first line; the message is the rest. A sheet is headed by its
        // label, its text the message (LLP 1115 D6).
        let words = style == .alert ? ConfirmationShape.alertText(label: label, lines: texts)
            : (title: label, message: texts.isEmpty ? nil : texts.joined(separator: "\n"))
        let owner = Confirmation(host: self, source: source, popover: pop, modal: modal, actions: presented, style: style,
                                 title: words.title, message: words.message)
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
            owner.alert.addAction(alertAction)
            owner.buttons.append(alertAction)
        }
        if let cancel = cancels.first {
            owner.alert.addAction(UIAlertAction(title: title(of: cancel), style: .cancel) { [weak self, weak owner] _ in
                if let owner { self?.select(owner, nil) }
            })
        }
        // A prompt: each field is UIKit's, standing in for the authored one,
        // and Return chooses its first action that is not destructive, as a
        // hand-built rename's field delegate does, while it is enabled.
        if style == .alert, let presenter {
            let lead = actions.firstIndex(where: { $0.props["destructive"] != "true" })
            owner.fields = fields.map { PromptField($0, presenter: presenter) }
            for field in owner.fields {
                if let lead {
                    field.returned = { [weak self, weak owner] in
                        guard let self, let owner, owner.buttons[lead].isEnabled else { return }
                        self.select(owner, lead)
                    }
                }
                owner.alert.addTextField { field.attach($0) }
            }
        }
        // The action drawn bold is the author's alone: UIKit's alert has no
        // preferred action until one is set, as a hand-built one and React
        // Native's `Alert.prompt` have none.
        if style == .alert, let preferred = Self.preferredAction(actions, cancel: cancels.first, prompt: !fields.isEmpty) {
            owner.alert.preferredAction = owner.buttons[preferred]
        }
        #if !os(tvOS)
        if style == .popover {
            guard let presentation = owner.alert.popoverPresentationController else { return false }
            presentation.delegate = owner
            if let source { anchor(presentation, at: source, area: PositionArea.of(pop)) }
            else if let item = anchorItem as? UIPopoverPresentationControllerSourceItem {
                // Placed as at an invoker (`anchor`), at the item UIKit names
                presentation.sourceItem = item
                Self.place(presentation, PositionArea.of(pop))
            }
            else {
                // No invoker: arrowless, over the middle of its screen.
                presentation.sourceView = controller.view
                presentation.sourceRect = CGRect(x: controller.view.bounds.midX, y: controller.view.bounds.midY, width: 0, height: 0)
                presentation.permittedArrowDirections = []
            }
        }
        #endif
        confirmation = owner
        if announce { presenter?.popoverEvents.changed(pop, open: true) }
        controller.present(owner.alert, animated: !ExactEnv.agentFreezes) { [weak self, weak owner] in
            guard let self, let owner, owner.finishing else { return }
            if self.confirmation === owner { self.dismissConfirmation(owner) }
            else { owner.alert.dismiss(animated: false) }
        }
        return true
    }
    /// The alert's preferred action, drawn bold and taking a hardware
    /// Return: the Mac's default button (`AlertsMac.defaultRow`, LLP 1115.000
    /// D7.3), the one `filled` action that is not destructive, when exactly
    /// one is; emphasis never picks among several. A prompt's Return is its
    /// first action that is not destructive (`PromptField.returned`), so
    /// there the filled action is preferred only when it is that one: the
    /// action drawn bold is always the one Return presses.
    static func preferredAction(_ actions: [NodeView], cancel: NodeView? = nil, prompt: Bool) -> Int? {
        let destructive = actions.map { $0.props["destructive"] == "true" }
        let filled = actions.indices.filter { !destructive[$0] && actions[$0].face?.style == "filled" }
        // A filled cancel is one of the candidates on the Mac, and never the default.
        guard filled.count == 1, prompt || cancel?.face?.style != "filled" else { return nil }
        return !prompt || destructive.firstIndex(of: false) == filled[0] ? filled[0] : nil
    }
    /// LLP 1115 D6, by the HIG: a confirmation — explanatory text, a
    /// cancel, at most three buttons — is an alert, centred, its Cancel
    /// kept (React Native's `Alert.alert`, SwiftUI's `.alert`). Anything
    /// else is an action sheet: UIKit's unanchored one on a compact screen
    /// when it has a cancel to show (UIKit draws no cancel in a popover), else a
    /// popover, which a tap outside dismisses. An alert has no outside (LLP
    /// 1115.000 D13.4), so a `dialog` of the alert's shape whose author wrote
    /// `closedby="any"` (`outside`) is the dismissible form too.
    private static func style(texts: Int, actions: Int, cancels: Int, compact: Bool, outside: Bool) -> Confirmation.Style {
        if !outside, ConfirmationShape.isAlert(texts: texts, actions: actions, cancels: cancels) { return .alert }
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
        // UIKit's defaults for a sheet presented from a control: the whole
        // invoker, an arrow on the side UIKit picks, never over the invoker.
        // An authored area overrides.
        presentation.sourceRect = anchor.bounds
        Self.place(presentation, area)
    }
    #endif

    #if !os(tvOS)
    /// LLP 1021 §5: the sheet's side of its invoker, from its popover's
    /// `position-area`; `none` keeps UIKit's defaults. UIKit places a popover
    /// by the arrow directions it permits — `.down` puts it above its source,
    /// `.up` below, `.left` to its right (`right span-bottom`) — centred on
    /// the source where it fits. `center` (the invoker's own cell) permits
    /// no arrow and may cover it: UIKit centres it across the invoker and
    /// picks its vertical position.
    static func place(_ presentation: UIPopoverPresentationController, _ area: String) {
        guard area != "none" else { return }
        if area == "center" {
            presentation.permittedArrowDirections = []
            presentation.canOverlapSourceViewRect = true
        } else {
            presentation.permittedArrowDirections = area.hasPrefix("top") ? .down : area.hasPrefix("right") ? .left : .up
            presentation.canOverlapSourceViewRect = false
        }
    }
    #endif

    private func opensConfirmation(_ row: NodeView) -> Bool {
        presenter?.invocation(of: row).map { $0.opens && isConfirmation($0.target) } ?? false
    }
    /// The menu grammar, extracted (LLP 1021 D3): button rows become
    /// actions, a row that opens another menu its submenu (§5,
    /// "Submenus"); any other row is a section boundary. A row's action
    /// presses it, then presents the confirmation or dialog it opens. `path` holds the
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
            } else if row.handlers.contains("press") || opensConfirmation(row) {
                let id = row.id, token = row.incarnation
                let image = image(of: row)
                let action = UIAction(title: title(of: row), image: image) { [weak self] action in
                    // The row this menu showed, not a recycled view's next node,
                    // and still enabled: a disabled button does nothing (HTML)
                    func current() -> NodeView? {
                        guard let row = self?.presenter?.views[id], row.incarnation == token, !row.disabled else { return nil }
                        return row
                    }
                    guard let self, let presenter = self.presenter, let row = current() else { return }
                    if row.handlers.contains("press") { presenter.press(id) }
                    // Once the menu has gone, what the surviving row opens, from
                    // where UIKit says the menu's presentations come from.
                    guard current() != nil, self.opensConfirmation(row), let invocation = presenter.invocation(of: row) else { return }
                    let target = invocation.target, targetToken = target.incarnation
                    #if os(tvOS)
                    let anchorItem: AnyObject? = nil
                    #else
                    let anchorItem: AnyObject? = action.presentationSourceItem
                    #endif
                    DispatchQueue.main.async {
                        guard current() != nil, target.incarnation == targetToken, presenter.views[target.id] === target else { return }
                        _ = self.openConfirmation(from: nil, popover: target, modal: invocation.action == .showModal, anchorItem: anchorItem, dispatchPress: false)
                    }
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
    /// The items of the popover `row` opens as a submenu: one it shows or
    /// toggles (`Invocation`), not a confirmation, not on `path` (a cycle is
    /// no submenu), with at least one item.
    private func submenu(of row: NodeView, path: [NodeView]) -> [UIMenuElement]? {
        guard let invocation = presenter?.invocation(of: row), invocation.action == .show || invocation.action == .toggle else { return nil }
        let sub = invocation.target
        guard !isConfirmation(sub), !path.contains(where: { $0 === sub }) else { return nil }
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

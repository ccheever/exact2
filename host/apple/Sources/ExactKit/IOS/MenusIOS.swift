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
// fire, D1 — so the menu shows rows refreshed by that press.
#if os(iOS) || os(tvOS)
import UIKit

final class MenuHost {
    private(set) weak var presenter: Presenter?
    private var touchObserver: PopoverTouch?
    private var overlays: [UInt32: UIButton] = [:]
    /// The target and press shape each overlay's menu was built for.
    private var menuShapes: [UInt32: String] = [:]
    #if os(iOS)
    /// Native buttons that are pull-downs (`nativeMenu`), by node.
    private var nativeMenus: [UInt32: NativeButtonIOS] = [:]
    #endif
    private var confirmation: Confirmation?
    /// A press another invoker activates (`activating`): it invokes nothing.
    var invocationOwner: NodeView?
    /// A press whose caller presents what its command opens (a pull-down, a
    /// confirmation): `invoke` still runs a command that closes.
    var presentationOwner: NodeView?
    /// Logical visibility can reopen before UIKit finishes dismissing the old alert.
    private struct PendingConfirmation {
        let popover: NodeView, source: NodeView?, modal: Bool
        let incarnation: UInt64, sourceIncarnation: UInt64?
        var fields: [PromptField.Snapshot] = []
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
    /// The layers lifted over a dialog's sheet: outside the viewport and the route mounts.
    var sheetLayers: [UIView] { agentOpen.values.sorted { $0.order < $1.order }.map(\.layer).filter { $0.sheet != nil && $0.superview != nil } }
    func logicalParent(of view: UIView) -> UIView? {
        #if os(iOS)
        if let parent = context.logicalParent(of: view) { return parent }
        #endif
        return agentOpen.values.first { $0.popover === view }?.parent
    }
    /// A captured press belongs to this opening, even if the same node reopens.
    func pressLifetime(of view: UIView) -> (() -> Bool)? {
        if let open = sheets.pressLifetime(of: view) { return open }
        guard let entry = agentOpen.values.first(where: { $0.popover === view }) else { return nil }
        return { [weak self, weak entry] in
            guard let entry else { return false }
            return self?.agentOpen.values.contains { $0 === entry } == true
        }
    }
    /// Whether a confirmation's button was chosen and its press has not yet
    /// landed: it lands once UIKit has dismissed the alert.
    var choosing: Bool { confirmation?.finishing == true }
    /// Whether an open prompt's field (UIKit's, in the alert) is being edited.
    var promptEditing: Bool { confirmation?.fields.contains { $0.editing } == true }
    var nativeConfirmation: NodeView? { pendingConfirmation?.popover ?? (confirmation?.finishing == false ? confirmation?.popover : nil) }
    /// How many popovers the agent has opened: each one's place in the stack.
    private var opened = 0
    private final class Lifted {
        var placement = PositionArea.History()
        /// Anchored to its opener at least once.
        var placed = false
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
        let layer: PopoverLayer
        #if os(iOS)
        var relay: MountRelay?
        #endif
        init(source: UInt32?, popover: NodeView, order: Int, ancestor: String?, presenter: Presenter) {
            self.source = source; self.popover = popover; self.order = order; self.ancestor = ancestor
            layer = PopoverLayer(presenter: presenter, order: order, popover: popover)
            parent = popover.superview
            index = parent?.subviews.firstIndex(of: popover) ?? 0
            center = popover.center
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
    func isConfirmation(_ node: NodeView) -> Bool {
        isDialog(node) || node.props["accessibilityRole"] == "alertdialog"
    }
    /// The `id` of what `source` invokes, as HTML resolves it (`Invocation`).
    private func target(of source: NodeView) -> String? { presenter?.invocation(of: source)?.target.props["id"] }
    private func closes(_ source: NodeView, _ target: NodeView, modal: Bool) -> Bool {
        presenter?.invocation(of: source)?.closes(target, modally: modal) ?? false
    }
    func opens(_ source: NodeView, _ target: NodeView) -> Bool {
        presenter?.invocation(of: source).map { $0.target === target && $0.opens } ?? false
    }

    /// After a batch: hide every popover, and lay a transparent button
    /// whose primary action is the system menu over every invoker of one.
    /// It is in the paint foreground, above the invoker's children however
    /// they are ranked (a positioned chip), and yields only to one that is
    /// interactive itself (`InvokerButton`).
    func sync() {
        // Paint ordinary menus for the agent; confirmations use UIKit with either input carrier (LLP 1021 D4).
        guard let presenter else { return }
        let surface = presenter.modals.coordinateView ?? presenter.viewport
        if touchObserver?.view !== surface {
            if let observer = touchObserver { observer.view?.removeGestureRecognizer(observer) }
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
            if !isConfirmation(v), ExactEnv.agentPaints || v.props["id"].map({ agentOpen[$0] != nil }) == true {
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
        var live = Set<UInt32>(), liveNative = Set<UInt32>()
        for v in presenter.carrying("popovertarget") + presenter.carrying("commandfor").filter({ $0.props["popovertarget"] == nil }) {
            guard let target = target(of: v), let pop = popovers[target],
                  // A row that only hides its popover (a menu item closing
                  // itself, the spec's way) is not an invoker.
                  opens(v, pop)
            else { continue }
            if v.isNativeButton {
                #if os(iOS)
                if pop.props["accessibilityRole"] == "menu", !isConfirmation(pop), let native = presenter.controls.controls[v.id] as? NativeButtonIOS {
                    let heading = pop.props["accessibilityLabel"] ?? ""
                    nativeMenu(native, of: v, popover: pop, shape: "\(target)|\(heading)", heading: heading)
                    nativeMenus[v.id] = native; liveNative.insert(v.id)
                }
                #endif
                continue // Its own primary action invokes after activation (D13).
            }
            // A custom invoker's popover that is no menu opens as itself, from
            // the invoker, as a native button's does (LLP 1021 D3: only a menu
            // is the platform's pull-down); its tap invokes after its press
            // (`activating`, `invoke`). tvOS's remote has no such path: the overlay stays.
            #if os(iOS)
            if !pullsDown(pop) { continue }
            #endif
            live.insert(v.id)
            if isConfirmation(pop) {
                let button = overlays[v.id] ?? InvokerButton(type: .custom)
                (button as? InvokerButton)?.invoker = v
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
                if button.superview !== v { v.addSubview(button); button.setPaintForeground() }
                button.frame = v.bounds; overlays[v.id] = button
                continue
            }
            let button = overlays[v.id] ?? {
                #if os(iOS)
                let b = MenuButton(type: .custom)
                b.focus = focus
                #else
                let b = InvokerButton(type: .custom)
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
            (button as? MenuButton)?.toggled = { [weak self, weak presenter, weak pop] open in
                if let pop { presenter?.popoverEvents.changed(pop, open: open) }
                if !open, let pop, self?.presentedMenu[pop.id] != nil { self?.presentedMenu = [:] }
            }
            #endif
            if button.superview !== v { v.addSubview(button); button.setPaintForeground() }
            (button as? InvokerButton)?.invoker = v
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
        for (id, b) in nativeMenus where !liveNative.contains(id) || presenter.controls.controls[id] !== b {
            Self.endNativeMenu(b); nativeMenus[id] = nil
        }
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
    func invoke(_ source: NodeView, bubbled: Bool = false) {
        guard source !== invocationOwner else { return }
        #if os(iOS)
        let custom = source.kind == "button" && (source.props["href"] ?? "").isEmpty
        #else
        let custom = false
        #endif
        // A bar item's `command="show-modal"` (`BarPress`): the page hides
        // its button while the bar shows it, and what it opens presents with
        // no invoker, as a pull-down's confirmation does. A bar item that
        // opens a popover is its pull-down (`pullDown`), never this.
        let barModal = !eligible(source) && eligible(source, inBar: true)
            && presenter?.invocation(of: source)?.action == .showModal
        // A bar item's popover that was no menu when its item was made (a
        // pull-down item opens its own): with a press of its own, after it,
        // whatever the press made it; without one the bar opens it (`openFromBar`).
        if source !== presentationOwner, !eligible(source), eligible(source, inBar: true), let invocation = presenter?.invocation(of: source),
           invocation.opensPopover, !isConfirmation(invocation.target) {
            _ = openFromBar(source)
            return
        }
        guard custom || source.isNativeButton || barModal || closesPresentedContent(source), barModal || eligible(source),
              let invocation = presenter?.invocation(of: source), let name = invocation.target.props["id"] else { return }
        let pop = invocation.target, action = invocation.action
        if source === presentationOwner, action == .show || action == .showModal || action == .toggle && !isOpen(pop) { return }
        if bubbled, !usableForBubbledInvocation(pop, from: source) { return }
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

    /// Only an invoker behind the modal is restricted to targets inside it.
    private func usableForBubbledInvocation(_ target: NodeView, from source: NodeView) -> Bool {
        guard let dialog = sheets.topDialog else { return true }
        func inside(_ node: NodeView) -> Bool {
            var view: UIView? = node
            while let current = view {
                if current === dialog { return true }
                view = presenter?.listenerParent(of: current) ?? current.superview
            }
            return false
        }
        return inside(source) || !target.inert && inside(target)
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
            for button in nativeMenus.values where button.popover === pop { button.dismissMenu() }
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

    /// Whether the button over `node` is highlighted: it holds the press look.
    func overlayHoldsPress(_ node: NodeView) -> Bool {
        (overlays[node.id] as? InvokerButton).map { $0.isHighlighted && $0.pressing === node } ?? false
    }
    func live(_ node: NodeView) -> Bool { presenter?.views[node.id] === node }
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
            self.pressAsTouched(invoker)
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
            completion(self.items(of: invocation.target, opener: invoker))
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
            let projectedSection = contextMenu && (view as? NodeView).map { presenter?.groupedLists?.hidSection($0) == true } == true
            if view.isHidden, !projectedSection, !(inBar && presenter?.navigation.hiddenForBar(view) == true), !groupedProjectionShows(node, behind: view) {
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
            && owner.actions.allSatisfy { presented(owner, $0) } && cancelStands(owner) }
        guard let source = owner.source else { return false }
        let modal = isDialog(pop)
        return eligible(source, inertBoundary: modal ? source : nil) && live(pop) && source.window != nil
            && (!owner.invoked || opens(source, pop)) && isConfirmation(pop)
            && presenter?.navigation.routeKey(containing: source) == owner.route
            && owner.actions.allSatisfy { presented(owner, $0) } && cancelStands(owner)
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
    /// The cancel still shows what the alert shows for it: live, in its
    /// popover, closing it, its title and its own `disabled` unchanged.
    private func cancelStands(_ owner: Confirmation) -> Bool {
        guard let entry = owner.cancel else { return true }
        guard let cancel = entry.node, let pop = owner.popover, live(cancel) else { return false }
        return cancel.isDescendant(of: pop) && closes(cancel, pop, modal: owner.modal) && title(of: cancel) == entry.title
            && !cancel.disabled == entry.enabled
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
        let shown = pop.confirmationRows.filter(\.isPromptField)
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
    func cancelled(_ owner: Confirmation) {
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
    func retire(over controller: UIViewController, removing route: NodeView) -> () -> Void {
        let endSheets = sheets.retire(over: controller, removing: route)
        if let pending = pendingConfirmation, presenter?.authored(pending.popover, inside: route) == true {
            pendingConfirmation = nil
            presenter?.popoverEvents.changed(pending.popover, open: false)
        }
        guard let owner = confirmation, ViewPresentation.isPresented(owner.alert, above: controller) else { return endSheets }
        // A selected action still belongs to its dismissal callback and its validity checks.
        guard !owner.finishing else { return endSheets }
        confirmation = nil
        if let pop = owner.popover {
            if pendingConfirmation == nil, live(pop), presenter?.authored(pop, inside: route) == false {
                pendingConfirmation = PendingConfirmation(pop, owner.source, modal: owner.modal)
                pendingConfirmation?.fields = owner.fields.compactMap { $0.snapshot() }
            } else {
                presenter?.popoverEvents.changed(pop, open: false)
            }
        }
        owner.finishing = true
        owner.fields.forEach { $0.end(relocating: pendingConfirmation.map { $0.popover === owner.popover } == true) }
        // The route's presenting controller dismisses the whole stack once.
        // Removing route content is not HTML dialog.close(), so sends no close.
        return { [owner] in
            _ = owner
            endSheets()
        }
    }
    func preservesFocus(_ id: UInt32) -> Bool { sheets.preservesFocus(id) || pendingConfirmation?.fields.contains { $0.node?.id == id && $0.node === presenter?.views[id] && $0.node?.incarnation == $0.incarnation } == true }
    func resumeAfterRouteDismissal() {
        guard let presenter, !presenter.modals.inTransition, !presenter.navigation.inTransition, sheets.resumeAfterRouteDismissal(),
              confirmation == nil, let pending = pendingConfirmation else { return }
        pendingConfirmation = nil
        let pop = pending.popover
        let source = pending.source.flatMap { live($0) && $0.incarnation == pending.sourceIncarnation && $0.window != nil ? $0 : nil }
        if pop.incarnation != pending.incarnation ||
            !openConfirmation(from: source, popover: pop, modal: pending.modal, dispatchPress: false, announce: false, restoring: pending.fields) {
            presenter.popoverEvents.changed(pop, open: false)
        }
    }
    func reset() {
        focus.reset()
        resetConfirmation()
        sheets.reset()
        overlays.values.forEach {
            #if os(iOS)
            ($0 as? MenuButton)?.dismissMenu()
            #endif
            $0.removeFromSuperview()
        }
        overlays.removeAll()
        #if os(iOS)
        nativeMenus.values.forEach(Self.endNativeMenu); nativeMenus.removeAll()
        #endif
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
    /// Whether `box`, a `dialog` or popover, is given back to layout now.
    func isLaidOut(_ box: NodeView) -> Bool { laidOut.contains(box.id) }
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
    /// Whether `node` is inside a `dialog` or popover covered while closed:
    /// the kernel has not laid it out, and nothing in it shows.
    func inClosedBox(_ node: NodeView) -> Bool {
        var at: UIView? = node
        while let view = at {
            if let box = view as? NodeView, box.props["popover"] != nil || isDialog(box) { return !laidOut.contains(box.id) && !showsInViews(box) }
            at = view.superview
        }
        return false
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
    /// Revisit only the hosts captured at opening, including after UIKit detaches one.
    func rehostPopovers() {
        for entry in agentOpen.values.sorted(by: { $0.order < $1.order }) { lift(entry) }
    }
    func replacePopoverHost(_ old: UIViewController, with controller: UIViewController) {
        for entry in agentOpen.values { entry.layer.replaceHost(old, with: controller) }
    }

    private func lift(_ entry: Lifted) {
        guard let presenter, let pop = entry.popover else { return }
        #if os(iOS)
        if entry.relay == nil { entry.relay = MountRelay(presenter, mount: entry.layer) }
        #endif
        entry.layer.mount(in: presenter)
        if pop.superview !== entry.layer { entry.layer.addSubview(pop) }
        if pop.isHidden { pop.isHidden = false }
        var placement = entry.layer.bounds
        if let top = presenter.keyboardTop {
            let end = entry.layer.convert(CGPoint(x: 0, y: top), from: nil).y
            placement.size.height = max(0, min(placement.maxY, end) - placement.minY)
        }
        defer {
            if presenter.keyboardTop != nil {
                let height = pop.bounds.height
                let y = max(placement.minY + height / 2, min(pop.center.y, placement.maxY - height / 2))
                if pop.center.y != y { pop.center.y = y }
            }
        }
        let opener = entry.source.flatMap { presenter.views[$0] }
        guard let source = opener, source.window != nil else {
            // An opener out of the window for now (its sheet leaving) keeps the place it gave; one gone recentres it.
            if !entry.placed || opener == nil { pop.center = CGPoint(x: placement.midX, y: placement.midY) }
            return
        }
        let anchorView: UIView = source.isNativeButton ? presenter.controls.controls[source.id] ?? source : source
        var anchor = anchorView.convert(anchorView.bounds, to: entry.layer)
        // A bar item's popover opens from the item, not its hidden button (`agentOpen(barItem:)`).
        if case .success(let (bar, item))? = presenter.navigation.barItem(source) { anchor = bar.convert(item, to: entry.layer) }
        let size = pop.bounds.size
        let placed = PositionArea.painted(pop, rtl: presenter.documentDirection == "rtl")
        let at = PositionArea.origin(placed.area, anchor: anchor, size: size,
                                    margins: PositionArea.margins(of: pop),
                                    fallbacks: placed.fallbacks,
                                    // The page's document less the bands past its area (status bar, home indicator).
                                    overflow: PositionArea.overflow(of: pop, page: presenter.viewport.convert(CGRect(origin: .zero, size: presenter.viewport.contentSize).inset(by: presenter.pageMargins), to: entry.layer)),
                                    rtl: presenter.documentDirection == "rtl",
                                    history: &entry.placement, in: placement)
        let center = CGPoint(x: at.x + size.width / 2, y: at.y + size.height / 2)
        entry.placed = true
        if pop.center != center { pop.center = center }
    }
    /// Out of the top layer, hidden, back where it was; the popovers nested
    /// in it first, as hiding a popover hides those above it in the stack.
    func drop(_ name: String) {
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
            presenter?.regrid(pop) // a frame op while it was up was placed from the top layer
            // Its centre as it was, its origin on a whole pixel at its size now.
            let s = presenter?.gridScale ?? 1, size = pop.bounds.size
            pop.center = CGPoint(x: PixelGrid.round(entry.center.x - size.width / 2, scale: s) + size.width / 2,
                                 y: PixelGrid.round(entry.center.y - size.height / 2, scale: s) + size.height / 2)
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
        if nativeMenus.values.contains(where: { $0.popover === node && $0.isMenuOpen }) { return true }
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
        let inBar = sequence(first: touched, next: { $0.superview }).contains { $0 is UINavigationBar }
        // Its DOM ancestors: out of a dialog's sheet or a lifted popover to the authored parent.
        var touchedNodes: [NodeView] = [], at: UIView? = touched
        while let view = at {
            if let node = view as? NodeView { touchedNodes.append(node) }
            at = presenter?.listenerParent(of: view) ?? view.superview
        }
        for (name, entry) in agentOpen.sorted(by: { $0.value.order > $1.value.order }) {
            guard agentOpen[name] != nil, entry.popover?.props["popover"] != "manual", !kept.contains(name) else { continue }
            // A touch on an eligible button that invokes it now is not outside
            // it (HTML's light dismiss); the button that opened it may since
            // name something else.
            if let pop = entry.popover, touchedNodes.contains(where: { (eligible($0) || eligible($0, inBar: true)) && presenter?.invocation(of: $0)?.target === pop }) { continue }
            // A touch on the bar item that opened it is not outside it: the item's own tap toggles it (`openFromBar`).
            if inBar, let id = entry.source, let source = presenter?.views[id], case .success(let (bar, item))? = presenter?.navigation.barItem(source),
               item.contains(touched.convert(CGPoint(x: touched.bounds.midX, y: touched.bounds.midY), to: bar)) { continue }
            drop(name)
        }
    }

    /// Dismiss before resolving the tap; its accepted press owns invocation.
    func agentTap(_ node: NodeView, landing: UIView? = nil) {
        guard ExactEnv.agentPaints else { return }
        lightDismiss(landing ?? node)
        #if os(tvOS)
        // tvOS retains its existing agent presentation; Select is unchanged.
        guard !node.isNativeButton, let presenter else { return }
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
        #endif
    }
    func agentShow(_ pop: NodeView, named name: String, from row: UInt32?) {
        layOut(pop)
        guard let presenter, live(pop) else { return }
        // A row of an open menu opens a popover that is no menu as the menu's own invoker would (LLP 1021's menu rule).
        let holder = { (id: UInt32?) in self.agentOpen.values.first { m in
            m.popover?.props["accessibilityRole"] == "menu" && id.flatMap { presenter.views[$0] }.map { $0.isDescendant(of: m.popover!) } == true } }
        var source = row, seen = Set<ObjectIdentifier>() // past any submenu, to what opened the outermost menu
        while pop.props["accessibilityRole"] != "menu", let m = holder(source), seen.insert(ObjectIdentifier(m)).inserted { source = m.source }
        // Nested in the latest open popover holding its opener or itself.
        let opener = source.flatMap { presenter.views[$0] }
        let ancestor = agentOpen.filter { _, open in
            guard let outer = open.popover, outer !== pop else { return false }
            return opener.map { $0.isDescendant(of: outer) } == true || pop.isDescendant(of: outer)
        }.max { $0.value.order < $1.value.order }?.key
        if pop.props["popover"] != "manual" {
            var keep = Set<String>(), current = ancestor
            while let name = current, keep.insert(name).inserted { current = agentOpen[name]?.ancestor }
            for (other, entry) in agentOpen where entry.popover?.props["popover"] != "manual" && !keep.contains(other) { drop(other) }
        }
        opened += 1
        let entry = Lifted(source: source, popover: pop, order: opened, ancestor: ancestor, presenter: presenter)
        entry.layer.rehost = { [weak self] in self?.rehostPopovers() }
        agentOpen[name] = entry
        presenter.popoverEvents.changed(pop, open: true)
        lift(entry)
        presenter.controls.syncShown()
        if let field = Self.autofocus(in: pop) { presenter.focusNode(field) }
    }
    /// Paint the context popover after its source's handler ran. Read the
    /// live run's props because the handler may replace it.
    @discardableResult
    func agentContext(_ node: NodeView, inline inlineID: UInt32? = nil) -> Bool {
        // Its own action may have unmounted or disabled it, or renamed its popover.
        guard ExactEnv.agentMode, let presenter, eligible(node, contextMenu: true) else { return false }
        let props: [String: String]
        if let inlineID {
            guard presenter.textHost(inlineID) === node, !presenter.inlineInert(inlineID), let run = presenter.inlineText(inlineID) else { return false }
            props = run.props
        } else { props = node.props }
        guard let name = props["contextPopover"] else { return false }
        // `agentShow` nests it in the open popovers holding `node`, as HTML's
        // stack does, and closes every other auto popover, as a tap outside
        // them would; a manual one stays.
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
        // Under native presentation a closed popover is UIKit's to open.
        if !ExactEnv.agentPaints { return false }
        let painted = (presenter.carrying("popover") + presenter.carrying("tag:dialog").filter { $0.props["popover"] == nil }).filter { !isConfirmation($0) }
        let invoker = sequence(first: node as UIView, next: { presenter.listenerParent(of: $0) })
            .compactMap { $0 as? NodeView }.first { presenter.invocation(of: $0) != nil }
        return painted.contains { pop in node === pop || node.isDescendant(of: pop) || (pop.props["id"] != nil && invoker.flatMap { target(of: $0) } == pop.props["id"]) }
    }
    /// Under native presentation (`--touch platform`), the popover `node`
    /// is a row of, which UIKit shows as its menu (D3), and the row: what a
    /// finger picks. Nil for anything else.
    func nativeRow(_ node: NodeView) -> (popover: NodeView, row: NodeView)? {
        guard ExactEnv.agentMode, !ExactEnv.agentPaints, let presenter else { return nil }
        // The innermost popover holding it: a submenu's rows are its own.
        let holding = presenter.carrying("popover").filter { pop in
            node.isDescendant(of: pop) && !isConfirmation(pop) && pop.props["id"].map { agentOpen[$0] == nil } != false
        }
        guard let pop = holding.first(where: { inner in !holding.contains { $0 !== inner && $0.isDescendant(of: inner) } }) else { return nil }
        var at: UIView = node
        while let up = at.superview, up !== pop.container { at = up }
        guard let row = at as? NodeView, row.superview === pop.container, row.props["popover"] == nil else { return nil }
        return (pop, row)
    }
    /// For `node`, a row (an action or the cancel) of the confirmation UIKit
    /// presents, the alert and the `UIAlertAction` it shows for the row, with
    /// its title as presented: what a finger picks under `--touch platform`
    /// (`Agent.aim`), or why none can be. Nil for any other node.
    func shownAction(_ node: NodeView) -> Result<(alert: UIAlertController, action: UIAlertAction, title: String), AimRefusal>? {
        guard let owner = confirmation, let pop = owner.popover, node.isDescendant(of: pop) else { return nil }
        var at: UIView = node
        while !(owner.actions.contains { $0.node === at } || owner.cancel?.node === at), let up = at.superview, up !== pop.container { at = up }
        let index = owner.actions.firstIndex { $0.node === at }
        guard index != nil || owner.cancel.map({ $0.node === at }) == true else { return nil }
        // UIKit draws no cancel in a popover.
        if index == nil, owner.style == .popover { return .failure(AimRefusal(why: "it is the cancel of a popover, which shows no cancel button: a tap outside the popover dismisses it")) }
        if owner.finishing || owner.dismissing || owner.alert.isBeingPresented || owner.alert.isBeingDismissed {
            return .failure(AimRefusal(why: "the alert is still moving; `clock settle` first"))
        }
        guard let action = index.map({ owner.buttons[$0] }) ?? owner.cancelButton, let entry = index.map({ owner.actions[$0] }) ?? owner.cancel else {
            return .failure(AimRefusal(why: "the alert shows no button for it"))
        }
        return .success((owner.alert, action, entry.title))
    }
    /// Under the agent, the rows each popover's menu was last given, in
    /// order, by popover id: where a finger finds a row in the menu UIKit
    /// shows (`Agent.shownRow`). UIKit puts no identifier of an action on
    /// the element it shows, so a row is its place among these.
    var given: [UInt32: [GivenRow]] = [:]
    /// Under the agent, the popovers (by id, with their incarnations) of
    /// the menu UIKit was last given to present: the one a menu opened
    /// with, and its submenus. A row of any other popover is not in the menu
    /// UIKit shows, however alike their rows read. Emptied when an
    /// invoker's menu ends; a bar item's pull-down reports no end, and its
    /// rows no longer showing is the refusal then.
    var presentedMenu: [UInt32: UInt64] = [:]
    private static func autofocus(in view: UIView) -> NodeView? {
        for sub in view.subviews {
            if let node = sub as? NodeView, let value = node.props["autofocus"], value != "false" { return node }
            if let found = autofocus(in: sub) { return found }
        }
        return nil
    }
    func unmounted() { focus.reset(); resetConfirmation(); sheets.reset() }
    var inTransition: Bool {
        if sheets.inTransition || pendingConfirmation != nil { return true }
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
        else if let pending = pendingConfirmation, pending.popover === pop {
            pendingConfirmation = nil
            presenter?.popoverEvents.changed(pop, open: false)
            if pending.modal { pop.queueDialogClose() }
            coverClosed()
        }
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
    /// dialog sheet or route presentation that is already up over it.
    func controller(for view: UIView) -> UIViewController? {
        guard let found = ViewPresentation.controller(for: view) else { return nil }
        if let top = sheets.top ?? presenter?.modals.owner, top !== found,
           found.presentedViewController != nil, top.view.window === found.view.window { return top }
        return found
    }
    /// A confirmation's rows: its children, its actions (buttons with a
    /// `press`) and its cancels (buttons that only close it).
    private func rows(of pop: NodeView, modal: Bool) -> (children: [NodeView], actions: [NodeView], cancels: [NodeView]) {
        let children = pop.confirmationRows
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
        // Only an alertdialog's alert with no field may have no action: its
        // one button tells ("Couldn't save." and OK), as lowering holds it to.
        if actions.isEmpty, !alert || !asks || fields > 0 { return "no action (a button with press)" }
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
        pop.props["accessibilityRole"] == "alertdialog" && pop.confirmationRows.contains(where: \.isPromptField)
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
    func openConfirmation(from source: NodeView?, popover pop: NodeView, modal: Bool, anchorItem: AnyObject? = nil,
                                  dispatchPress: Bool = true, announce: Bool = true, restoring: [PromptField.Snapshot] = []) -> Bool {
        var modal = modal
        // Both invoker actions fire, as on the web: the source's press first
        // (Session.press applies its batch synchronously), then what the
        // surviving source invokes now, which its press may have retargeted.
        if dispatchPress, let source, pressTarget(of: source) != nil {
            guard confirmation == nil || confirmation?.finishing == true, !sheets.owns(pop), eligible(source), live(pop) else { return false }
            let token = source.incarnation
            pressAsTouched(source)
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
                  unconfirmable(pop, modal: modal) == nil, prompts(pop) || Self.choosable(rows(of: pop, modal: modal).actions, { choosable($0, in: pop) }) else { return false }
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
        guard prompts(pop) || Self.choosable(actions, { eligible($0, inertBoundary: boundary) }) else { return refuse("every action is disabled") }
        guard let controller = controller(for: source ?? pop), !busy(controller) else { return false }
        let presented = actions.map { Confirmation.Presented($0, title: title(of: $0), enabled: eligible($0, inertBoundary: boundary)) }
        let texts = children.filter { $0.kind == "text" }.map(title(of:)).filter { !$0.isEmpty }
        let fields = children.filter(\.isPromptField)
        let label = pop.props["accessibilityLabel"].flatMap { $0.isEmpty ? nil : $0 }
        let style = Self.style(texts: texts.count + fields.count, actions: actions.count, cancels: cancels.count,
                               compact: controller.traitCollection.horizontalSizeClass != .regular,
                               // UIKit's dismissible form, an action sheet, drops a
                               // lone cancel in a popover: the one-button alert stays one.
                               outside: isDialog(pop) && pop.props["closedby"] == "any" && !actions.isEmpty)
        // The alert's title is its aria-label ("Remove book?"), else its
        // first line; the message is the rest. A sheet is headed by its
        // label, its text the message (LLP 1115 D6).
        let words = style == .alert ? ConfirmationShape.alertText(label: label, lines: texts)
            : (title: label, message: texts.isEmpty ? nil : texts.joined(separator: "\n"))
        let owner = Confirmation(host: self, source: source, popover: pop, modal: modal, actions: presented,
                                 cancel: cancels.first.map { Confirmation.Presented($0, title: title(of: $0), enabled: !$0.disabled) }, style: style,
                                 title: words.title, message: words.message)
        // A native action's tint is its accent (LLP 1069.011.000 D5); the
        // alert has one tint, the first action's, else its one button's. Unsaid, it is UIKit's
        // (LLP 1115 D4): a `color` the action only inherits is not its own.
        let lead = actions.first ?? cancels[0]
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
        if let cancel = owner.cancel {
            let button = UIAlertAction(title: cancel.title, style: .cancel) { [weak self, weak owner] _ in
                if let owner { self?.select(owner, nil) }
            }
            button.isEnabled = cancel.enabled
            owner.alert.addAction(button)
            owner.cancelButton = button
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
                owner.alert.addTextField {
                    field.attach($0)
                    if let state = restoring.first(where: { $0.node === field.node }) { field.restore(state) }
                }
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
        owner.observeDisappearance()
        if announce { presenter?.popoverEvents.changed(pop, open: true) }
        controller.present(owner.alert, animated: !ExactEnv.agentFreezes) { [weak self, weak owner] in
            guard let self, let owner else { return }
            owner.fields.forEach { $0.restoreSelection() }
            guard owner.finishing else { return }
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
    /// Something can be chosen: an action that is choosable, or none at
    /// all, the one-button alert whose button is its cancel.
    private static func choosable(_ actions: [NodeView], _ test: (NodeView) -> Bool) -> Bool {
        actions.isEmpty || actions.contains(where: test)
    }
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
    /// presses it, then presents the confirmation or dialog it opens, or any other popover as
    /// if by `opener`, the menu's own invoker (exact2's menu rule, LLP 1021: the menu closing
    /// doesn't take it along). `path` holds the popovers above `pop`, whose rows opened it.
    func items(of pop: NodeView, path: [NodeView] = [], opener: NodeView? = nil) -> [UIMenuElement] {
        var sections: [[UIMenuElement]] = [[]]
        // Under the agent, what UIKit was given, in order (`given`).
        var rows: [GivenRow] = []
        defer { if ExactEnv.agentMode { given[pop.id] = rows } }
        // A menu's own items (not a submenu's) begin what UIKit presents.
        if ExactEnv.agentMode {
            if path.isEmpty { presentedMenu = [:] }
            presentedMenu[pop.id] = pop.incarnation
        }
        for case let row as NodeView in pop.container.subviews {
            // A context menu's preview (§5.1) is not one of its items.
            if row.props["contextPreview"] == "true" { continue }
            // A submenu authored inside its parent is no row of it.
            if row.props["popover"] != nil { continue }
            let face = label(of: row)
            if let children = submenu(of: row, path: path + [pop], opener: opener) {
                // UIMenu has no disabled state: a disabled opener is a dimmed action.
                let element: UIMenuElement = row.props["disabled"] == "true"
                    ? UIAction(title: face.title, image: image(of: row), attributes: .disabled) { _ in }
                    : UIMenu(title: face.title, image: image(of: row), children: children)
                element.subtitle = face.subtitle
                sections[sections.count - 1].append(element)
                rows.append(GivenRow(row, title: element.title, subtitle: element.subtitle))
            } else if row.handlers.contains("press") || opensConfirmation(row) || presenter?.invocation(of: row).map({ $0.opens && $0.target.props["accessibilityRole"] != "menu" }) == true {
                let id = row.id, token = row.incarnation
                let image = image(of: row)
                let action = UIAction(title: face.title, image: image) { [weak self] action in
                    // The row this menu showed, not a recycled view's next node,
                    // and still enabled: a disabled button does nothing (HTML)
                    func current() -> NodeView? {
                        guard let row = self?.presenter?.views[id], row.incarnation == token, !row.disabled else { return nil }
                        return row
                    }
                    guard let self, let presenter = self.presenter, let row = current() else { return }
                    if row.handlers.contains("press") { self.presenting(row) { presenter.press(id) } }
                    // Once the menu has gone, what the surviving row opens, from
                    // where UIKit says the menu's presentations come from.
                    guard current() != nil, let invocation = presenter.invocation(of: row), invocation.opens else { return }
                    let target = invocation.target, targetToken = target.incarnation, from = opener?.id
                    if !self.isConfirmation(target), target.props["accessibilityRole"] != "menu", let name = target.props["id"] {
                        return DispatchQueue.main.async {
                            guard current() != nil, target.incarnation == targetToken else { return }
                            if self.isOpen(target) { if invocation.action == .toggle { self.drop(name) } } else { self.agentShow(target, named: name, from: from) }
                        }
                    }
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
                action.subtitle = face.subtitle
                if row.props["accessibilityChecked"] == "true" { action.state = .on }
                if row.props["disabled"] == "true" { action.attributes.insert(.disabled) }
                if row.props["destructive"] == "true" { action.attributes.insert(.destructive) }
                sections[sections.count - 1].append(action)
                rows.append(GivenRow(row, title: action.title, subtitle: action.subtitle))
            } else if !(sections.last?.isEmpty ?? true) {
                sections.append([])
            }
        }
        let filled = sections.filter { !$0.isEmpty }
        if filled.count <= 1 { return filled.first ?? [] }
        return filled.map { UIMenu(options: .displayInline, children: $0) }
    }
    /// The items of the popover `row` opens as a submenu: a menu it shows or
    /// toggles (`Invocation`), not a confirmation, not on `path` (a cycle is
    /// no submenu), with at least one item.
    private func submenu(of row: NodeView, path: [NodeView], opener: NodeView?) -> [UIMenuElement]? {
        guard let invocation = presenter?.invocation(of: row), invocation.action == .show || invocation.action == .toggle else { return nil }
        let sub = invocation.target
        guard !isConfirmation(sub), sub.props["accessibilityRole"] == "menu", !path.contains(where: { $0 === sub }) else { return nil }
        let children = items(of: sub, path: path, opener: opener)
        return children.isEmpty ? nil : children
    }
}
#endif

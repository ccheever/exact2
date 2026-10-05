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
    private var overlays: [UInt32: UIButton] = [:]
    /// The target and press shape each overlay's menu was built for.
    private var menuShapes: [UInt32: String] = [:]
    private var confirmation: Confirmation?
    /// Under the agent, the popovers open in their painted presentation
    /// (LLP 1021 D4), by `id`: each in the top layer while open, as macOS's
    /// and the web's are, out of its parent and back at its index after.
    private var agentOpen: [String: Lifted] = [:]
    private final class Lifted {
        let source: UInt32
        weak var popover: NodeView?
        weak var parent: UIView?
        var index: Int
        let center: CGPoint
        let layer = TopLayer(frame: .zero)
        init(source: UInt32, popover: NodeView) {
            self.source = source; self.popover = popover
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
        weak var host: MenuHost?
        weak var source: NodeView?
        weak var popover: NodeView?
        let route: String?
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
        init(host: MenuHost, source: NodeView, popover: NodeView, actions: [Presented], title: String?, message: String) {
            self.host = host; self.source = source; self.popover = popover; self.actions = actions
            route = host.presenter?.navigation.routeKey(containing: source)
            alert = UIAlertController(title: title, message: message.isEmpty ? nil : message, preferredStyle: .actionSheet)
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
        return source.props["popovertarget"] == target.props["id"] && source.props["popovertargetaction"] == "hide"
    }
    private func opens(_ source: NodeView, _ target: NodeView) -> Bool {
        self.target(of: source) == target.props["id"] && (isDialog(target)
            ? source.props["command"] == "show-modal" : source.props["popovertargetaction"] != "hide")
    }

    /// After a batch: hide every popover, and lay a transparent button
    /// whose primary action is the system menu over every invoker of one.
    func sync() {
        // Ordinary menus retain LLP 1021 D4's existing agent presentation.
        // Confirmations use the same UIKit owner under either input carrier.
        guard let presenter else { return }
        if let owner = confirmation, !valid(owner) { resetConfirmation() }
        var popovers: [String: NodeView] = [:]
        for (name, entry) in agentOpen {
            guard let pop = entry.popover, presenter.views[pop.id] === pop, pop.props["id"] == name,
                  pop.props["popover"] != nil else { drop(name); continue }
        }
        for v in presenter.carrying("popover") + presenter.carrying("tag:dialog").filter({ $0.props["popover"] == nil }) {
            // The agent's painted presentation: in the top layer while open,
            // hidden while closed, as on the web and macOS, so a closed
            // popover covers nothing (`agentTap`).
            if ExactEnv.agentMode && !isConfirmation(v) {
                if v.props["popover"] != nil {
                    if let name = v.props["id"], let entry = agentOpen[name], entry.popover === v { lift(entry) }
                    else if !v.isHidden { v.isHidden = true }
                }
                continue
            }
            v.isHidden = true
            if let name = v.props["id"] { popovers[name] = v }
        }
        var live = Set<UInt32>()
        for v in presenter.carrying("popovertarget") + presenter.carrying("commandfor").filter({ $0.props["popovertarget"] == nil }) {
            guard let target = target(of: v), let pop = popovers[target],
                  // A row that only hides its popover (a menu item closing
                  // itself, the spec's way) is not an invoker.
                  opens(v, pop)
            else { continue }
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
                let b = UIButton(type: .custom)
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
    }

    /// A live popover by its id.
    private func popover(named name: String) -> NodeView? {
        presenter?.views.values.first { $0.props["id"] == name && ($0.props["popover"] != nil || isDialog($0)) }
    }

    private func live(_ node: NodeView) -> Bool { presenter?.views[node.id] === node }
    private func eligible(_ node: NodeView, inertBoundary: UIView? = nil) -> Bool {
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
        guard let source = owner.source, let pop = owner.popover else { return false }
        let modal = isDialog(pop)
        return eligible(source, inertBoundary: modal ? source : nil) && live(pop) && source.window != nil
            && opens(source, pop) && isConfirmation(pop)
            && presenter?.navigation.routeKey(containing: source) == owner.route
            && owner.actions.allSatisfy { presented(owner, $0) }
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
    }

    /// Whether `node` is an open popover in the top layer (the agent's).
    func lifted(_ node: NodeView) -> Bool { agentOpen.values.contains { $0.popover === node } }
    /// A children op on `parent` while a child of it is in the top layer:
    /// the child stays there, its index kept for its return; one the op
    /// drops closes.
    func children(_ parent: UIView, _ wanted: [NodeView]) {
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
        let anchor = source.convert(source.bounds, to: entry.layer), size = pop.bounds.size
        let at = PositionArea.origin(PositionArea.of(pop), anchor: anchor, size: size,
                                    margins: PositionArea.margins(of: pop), in: entry.layer.bounds)
        let center = CGPoint(x: at.x + size.width / 2, y: at.y + size.height / 2)
        if pop.center != center { pop.center = center }
    }
    /// Out of the top layer, hidden, back where it was.
    private func drop(_ name: String) {
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
        return ExactEnv.agentMode && node.props["id"].map { agentOpen[$0] != nil } == true
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
        guard ExactEnv.agentMode, let presenter else { return }
        var byName: [String: NodeView] = [:]
        for pop in presenter.carrying("popover") where !isConfirmation(pop) {
            if let name = pop.props["id"] { byName[name] = pop }
        }
        for (name, entry) in agentOpen {
            guard let pop = byName[name] else { continue }
            if node === pop || node.isDescendant(of: pop) || node.id == entry.source || target(of: node) == name { continue }
            drop(name)
        }
        guard let name = target(of: node), let pop = byName[name], closes(node, pop) || opens(node, pop) else { return }
        let hides = closes(node, pop), source = node.id
        DispatchQueue.main.async { [weak self, weak presenter, weak pop] in
            guard let self, let presenter, let pop, presenter.views[pop.id] === pop else { return }
            if self.agentOpen[name] != nil { self.drop(name); return }
            guard !hides else { return }
            let entry = Lifted(source: source, popover: pop)
            self.agentOpen[name] = entry
            self.lift(entry)
            if let field = Self.autofocus(in: pop) { presenter.focusNode(field) }
        }
    }
    /// Whether a tap on `node` goes through the agent's painted popovers —
    /// one is open (the tap dismisses it first), or `node` is a painted
    /// popover, is in one, or opens or closes one — which only `agentTap`
    /// drives: a real touch bypasses it (LLP 1080.000 D7, stage 3).
    func agentPainted(_ node: NodeView) -> Bool {
        guard ExactEnv.agentMode, let presenter else { return false }
        if !agentOpen.isEmpty { return true }
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
        if confirmation == nil, ExactEnv.agentMode, let open = agentOpen.values.first, let pop = open.popover {
            return ["kind": "popover", "source": Int(open.source), "popover": Int(pop.id), "phase": "open"]
        }
        guard let owner = confirmation else { return nil }
        return ["kind": "confirmation", "source": owner.source.map { Int($0.id) as Any } ?? NSNull(),
                "popover": owner.popover.map { Int($0.id) as Any } ?? NSNull(), "phase": inTransition ? "transition" : "open",
                "actionStyle": owner.alert.actions.first?.style == .destructive ? "destructive" : "default",
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
    private func openConfirmation(from source: NodeView, popover pop: NodeView) -> Bool {
        guard confirmation == nil, eligible(source), live(pop), source.window != nil else { return false }
        if isDialog(pop) && pop.props["closedby"] != "any" {
            presenter?.session?.log("dialog refused: native confirmation currently requires closedby=any")
            return false
        }
        // Session.press applies its batch synchronously. Both invoker actions
        // fire, as on the web; read the updated rows only if these identities
        // survived that action (no elapsed-time guess or successor id).
        if source.handlers.contains("press") { presenter?.press(source.id) }
        guard confirmation == nil, eligible(source), live(pop), source.window != nil else { return false }
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
        var responder: UIResponder? = source
        while responder != nil && !(responder is UIViewController) { responder = responder?.next }
        guard let controller = responder as? UIViewController, controller.presentedViewController == nil,
              !controller.isBeingDismissed, !controller.isBeingPresented else { return false }
        let presented = actions.map { Confirmation.Presented($0, title: title(of: $0), enabled: eligible($0, inertBoundary: boundary)) }
        let texts = children.filter { $0.kind == "text" }
        // A chooser (several actions, no explanatory text) is titled by its
        // aria-label; a confirmation keeps its text as its only heading, as
        // the native prompts it matches have no title row.
        let heading = texts.isEmpty && actions.count > 1 ? pop.props["accessibilityLabel"] : nil
        let owner = Confirmation(host: self, source: source, popover: pop, actions: presented,
                                 title: heading, message: texts.map(title(of:)).joined(separator: "\n"))
        // A native action's tint is its accent (LLP 1069.011.000 D5); the
        // alert has one tint, the first action's.
        let lead = actions[0]
        owner.alert.view.tintColor = lead.isNativeButton
            ? lead.channels("accent_color").map { TextEngine.color($0) } ?? .systemBlue
            : lead.color("text_color", .systemBlue)
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
        guard let presentation = owner.alert.popoverPresentationController else { return false }
        presentation.sourceView = source
        // A labelled row anchors at its text; an icon control uses its box.
        let labels = source.container.subviews.compactMap { $0 as? NodeView }.filter { $0.kind == "text" }
        let labelBox = labels.reduce(CGRect.null) { $0.union($1.convert($1.bounds, to: source)) }
        presentation.sourceRect = labelBox.isNull ? source.bounds : CGRect(x: labelBox.minX, y: 0, width: labelBox.width, height: source.bounds.height)
        presentation.permittedArrowDirections = []
        presentation.canOverlapSourceViewRect = true
        Self.place(presentation, PositionArea.of(pop), source: source)
        presentation.delegate = owner
        #endif
        confirmation = owner
        controller.present(owner.alert, animated: !ExactEnv.agentFreezes)
        return true
    }

    #if !os(tvOS)
    /// LLP 1021 §5: the sheet's side of its invoker, from its popover's
    /// `position-area`. `none` keeps the arrowless placement above (D2's
    /// below-left, native Messages' prompts). UIKit places a popover by the
    /// arrow directions it permits — `.down` puts it above its source,
    /// `.up` below — centred on the source rect where it fits, so a centred
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
            presentation.permittedArrowDirections = area.hasPrefix("top") ? .down : .up
            presentation.canOverlapSourceViewRect = false
        }
    }
    #endif

    /// The menu grammar, extracted (LLP 1021 D3): button rows become
    /// actions; any other row is a section boundary.
    func items(of pop: NodeView) -> [UIMenuElement] {
        var sections: [[UIMenuElement]] = [[]]
        for case let row as NodeView in pop.container.subviews {
            if row.handlers.contains("press") {
                let id = row.id
                let image = image(of: row)
                let action = UIAction(title: title(of: row), image: image) { [weak self] _ in
                    self?.presenter?.press(id)
                }
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
        return v.container.subviews
            .compactMap { ($0 as? NodeView).map(title(of:)) }
            .filter { !$0.isEmpty }
            .joined(separator: " ")
    }
}
#endif

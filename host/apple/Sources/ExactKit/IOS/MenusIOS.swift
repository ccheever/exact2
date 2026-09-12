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

#if os(iOS)
import UIKit

final class MenuHost {
    private weak var presenter: Presenter?
    private var overlays: [UInt32: UIButton] = [:]
    private var confirmation: Confirmation?

    /// An alertdialog-shaped popover has one action and one hide-only cancel.
    /// Retain this owner until native dismissal completes; ids alone are not
    /// enough because a restart can reuse them for unrelated controls.
    private final class Confirmation: NSObject, UIPopoverPresentationControllerDelegate {
        weak var host: MenuHost?
        weak var source: NodeView?
        weak var popover: NodeView?
        weak var action: NodeView?
        let route: String?
        let alert: UIAlertController
        var finishing = false
        init(host: MenuHost, source: NodeView, popover: NodeView, action: NodeView, message: String) {
            self.host = host; self.source = source; self.popover = popover; self.action = action
            route = host.presenter?.navigation.routeKey(containing: source)
            alert = UIAlertController(title: nil, message: message, preferredStyle: .actionSheet)
        }
        func adaptivePresentationStyle(for controller: UIPresentationController) -> UIModalPresentationStyle { .none }
        func popoverPresentationControllerDidDismissPopover(_ controller: UIPopoverPresentationController) {
            host?.cancelled(self)
        }
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
        for v in presenter.views.values where v.props["popover"] != nil || isDialog(v) {
            if ExactEnv.agentMode && !isConfirmation(v) { continue }
            v.isHidden = true
            if let name = v.props["id"] { popovers[name] = v }
        }
        var live = Set<UInt32>()
        for v in presenter.views.values {
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
            let invokerId = v.id
            let hasPress = v.handlers.contains("press")
            button.menu = UIMenu(children: [
                UIDeferredMenuElement.uncached { [weak self, weak pop] completion in
                    if hasPress { self?.presenter?.press(invokerId) }
                    // The runner applies the press on its own thread; the
                    // items are read after it has (80 ms is invisible under
                    // the menu's own presentation).
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) {
                        completion(pop.map { self?.items(of: $0) ?? [] } ?? [])
                    }
                }
            ])
        }
        for (id, b) in overlays where !live.contains(id) {
            b.removeFromSuperview()
            overlays[id] = nil
        }
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
        guard let source = owner.source, let pop = owner.popover, let action = owner.action else { return false }
        let modal = isDialog(pop)
        return eligible(source, inertBoundary: modal ? source : nil) && live(pop)
            && eligible(action, inertBoundary: modal ? pop.superview : nil) && source.window != nil
            && opens(source, pop) && closes(action, pop)
            && action.isDescendant(of: pop) && isConfirmation(pop)
            && presenter?.navigation.routeKey(containing: source) == owner.route
    }
    private func cancelled(_ owner: Confirmation) {
        // UIKit also delivers this after a selected action's handler. That
        // notification must not cancel the action awaiting the next turn.
        guard confirmation === owner, !owner.finishing else { return }
        owner.finishing = true
        confirmation = nil
    }
    private func finish(_ owner: Confirmation, confirmed: Bool) {
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
                let action = confirmed && self.valid(owner) ? owner.action : nil
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
    }
    func unmounted() { resetConfirmation() }
    var inTransition: Bool {
        guard let owner = confirmation else { return false }
        return owner.finishing || owner.alert.isBeingPresented || owner.alert.isBeingDismissed
    }
    func observation() -> [String: Any]? {
        guard let owner = confirmation else { return nil }
        return ["kind": "confirmation", "source": owner.source.map { Int($0.id) as Any } ?? NSNull(),
                "popover": owner.popover.map { Int($0.id) as Any } ?? NSNull(), "phase": inTransition ? "transition" : "open",
                "actionStyle": owner.alert.actions.first?.style == .destructive ? "destructive" : "default"]
    }
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
            if owner.action === node { finish(owner, confirmed: true); return true }
            if let pop = owner.popover, node.isDescendant(of: pop), !node.disabled,
               closes(node, pop) {
                finish(owner, confirmed: false); return true
            }
            return false
        }
        guard let name = target(of: node), let pop = presenter?.views.values.first(where: {
            $0.props["id"] == name && isConfirmation($0) && ($0.props["popover"] != nil || isDialog($0))
        }) else { return nil }
        guard opens(node, pop) else { return false }
        return openConfirmation(from: node, popover: pop)
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
        let actions = children.filter { $0.kind == "button" && $0.handlers.contains("press") }
        let cancels = children.filter { $0.kind == "button" && !$0.handlers.contains("press") && closes($0, pop) }
        guard actions.count == 1, cancels.count == 1, let action = actions.first, let cancel = cancels.first,
              children.allSatisfy({ $0.kind == "text" || $0 === action || $0 === cancel }),
              [action, cancel].allSatisfy({ closes($0, pop) }),
              eligible(action, inertBoundary: isDialog(pop) ? pop.superview : nil) else { return false }
        var responder: UIResponder? = source
        while responder != nil && !(responder is UIViewController) { responder = responder?.next }
        guard let controller = responder as? UIViewController, controller.presentedViewController == nil,
              !controller.isBeingDismissed, !controller.isBeingPresented else { return false }
        let owner = Confirmation(host: self, source: source, popover: pop, action: action,
                                 message: children.filter { $0.kind == "text" }.map(title(of:)).joined(separator: "\n"))
        owner.alert.view.tintColor = action.color("text_color", .systemBlue)
        let actionStyle: UIAlertAction.Style = action.props["destructive"] == "true" ? .destructive : .default
        owner.alert.addAction(UIAlertAction(title: title(of: action), style: actionStyle) { [weak self, weak owner] _ in
            if let owner { self?.finish(owner, confirmed: true) }
        })
        owner.alert.addAction(UIAlertAction(title: title(of: cancel), style: .cancel) { [weak self, weak owner] _ in
            if let owner { self?.finish(owner, confirmed: false) }
        })
        guard let presentation = owner.alert.popoverPresentationController else { return false }
        presentation.sourceView = source
        // A labelled row anchors at its text; an icon control uses its box.
        let labels = source.container.subviews.compactMap { $0 as? NodeView }.filter { $0.kind == "text" }
        let labelBox = labels.reduce(CGRect.null) { $0.union($1.convert($1.bounds, to: source)) }
        presentation.sourceRect = labelBox.isNull ? source.bounds : CGRect(x: labelBox.minX, y: 0, width: labelBox.width, height: source.bounds.height)
        presentation.permittedArrowDirections = []
        presentation.canOverlapSourceViewRect = true
        presentation.delegate = owner
        confirmation = owner
        controller.present(owner.alert, animated: !ExactEnv.agentFreezes)
        return true
    }

    /// The menu grammar, extracted (LLP 1021 D3): button rows become
    /// actions; any other row is a section boundary.
    private func items(of pop: NodeView) -> [UIMenuElement] {
        var sections: [[UIMenuElement]] = [[]]
        for case let row as NodeView in pop.container.subviews {
            if row.handlers.contains("press") {
                let id = row.id
                let action = UIAction(title: title(of: row)) { [weak self] _ in
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

    private func title(of v: NodeView) -> String {
        if v.kind == "text" { return v.paragraphSpec().runs.map(\.text).joined() }
        return v.container.subviews
            .compactMap { ($0 as? NodeView).map(title(of:)) }
            .filter { !$0.isEmpty }
            .joined(separator: " ")
    }
}
#endif

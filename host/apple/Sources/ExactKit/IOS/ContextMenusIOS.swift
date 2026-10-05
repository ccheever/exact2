// @ref LLP 1021 §5.1 — the context menu on UIKit: a node naming a popover
// with `contextPopover` gets a UIContextMenuInteraction. UIKit owns the long
// press, the lift of the node itself (its targeted preview), the menu's
// placement and the morph. The popover's menu rows are the UIMenu (D3's
// extraction), and its `contextPreview` row, moved out of the hidden popover
// into the preview controller's view, is the preview, sized by its layout.
// Tapping the preview presses that row (UIKit's commit): a press that
// navigates is pushed without its own animation and committed with `.pop`,
// so the preview grows into the destination; any other is `.dismiss`.
// The node's own `contextmenu` action fires as the menu is about to show,
// before its rows or preview are read (D1's "both fire"), so a popover a
// list's rows share shows the row pressed. Under the agent this host is off
// and the popover opens painted (D4, `MenuHost.agentContext`).
#if os(iOS)
import UIKit

final class ContextMenuHost: NSObject, UIContextMenuInteractionDelegate {
    private weak var presenter: Presenter?
    private weak var menus: MenuHost?
    private var interactions: [UInt32: UIContextMenuInteraction] = [:]
    /// The preview's controller: its view holds the lifted row.
    final class PreviewController: UIViewController {
        override func loadView() { view = UIView() }
    }
    /// One presentation, from its configuration to the end of its dismissal.
    final class Open {
        weak var source: NodeView?
        weak var popover: NodeView?
        let name: String
        let point: CGPoint
        /// Whether the source's own `contextmenu` has fired (once, lazily).
        var fired = false
        weak var configuration: UIContextMenuConfiguration?
        weak var interaction: UIContextMenuInteraction?
        /// The lifted row; the popover it is a row of, the container it
        /// belongs in (its popover's, or another's a children op moved it
        /// to), its index there and its frame there (`home`, the last frame
        /// op's); whether a children op has dropped it.
        weak var preview: NodeView?
        weak var homePopover: NodeView?
        weak var container: UIView?
        var inPopover = true
        var index = 0
        var home = CGRect.zero
        var dropped = false
        var controller: PreviewController?
        /// Whether the preview was tapped, and whether its press navigated.
        var committed = false, navigated = false
        init(source: NodeView, popover: NodeView, name: String, point: CGPoint) {
            self.source = source; self.popover = popover; self.name = name; self.point = point
        }
    }
    private(set) var open: Open?

    init(presenter: Presenter, menus: MenuHost) {
        self.presenter = presenter; self.menus = menus
    }

    private func live(_ node: NodeView) -> Bool { presenter?.views[node.id] === node }
    private func popover(named name: String) -> NodeView? {
        presenter?.carrying("popover").first { $0.props["id"] == name }
    }
    private func ours(_ view: UIView) -> [UIContextMenuInteraction] {
        view.interactions.compactMap { $0 as? UIContextMenuInteraction }.filter { $0.delegate === self }
    }

    /// After a batch: one interaction on each node naming a popover, its
    /// plain long press (the `contextmenu` recognizer) off while it has one.
    func sync() {
        guard let presenter else { return }
        // A source gone, or naming another popover, ends its menu: the row
        // goes back now, as `willEnd` may never come for it. A commit's
        // animation still holds the row; its `willEnd` returns it.
        if let owner = open, !owner.committed, !(owner.source.map { live($0) && $0.props["contextPopover"] == owner.name } ?? false) {
            owner.interaction?.dismissMenu()
            restore(owner)
            open = nil
        }
        var wanted: [UInt32: NodeView] = [:]
        if !ExactEnv.agentMode {
            for v in presenter.carrying("contextPopover") where v.props["contextPopover"]?.isEmpty == false { wanted[v.id] = v }
        }
        for (id, interaction) in interactions where wanted[id] == nil || interaction.view !== wanted[id] {
            // A commit in flight keeps its interaction until it completes.
            if let owner = open, owner.committed, owner.interaction === interaction { continue }
            if let view = interaction.view {
                view.removeInteraction(interaction)
                (view as? NodeView)?.contextRecognizer?.isEnabled = true
            }
            interactions[id] = nil
        }
        for (id, v) in wanted {
            if interactions[id] == nil {
                // A recycled view may still hold one from its last id.
                ours(v).forEach { v.removeInteraction($0) }
                let interaction = UIContextMenuInteraction(delegate: self)
                v.addInteraction(interaction)
                interactions[id] = interaction
            }
            v.contextRecognizer?.isEnabled = false
        }
    }

    /// A frame op on the lifted row: the frame it has in its popover, which
    /// is where it returns, and its size the preview's.
    func framed(_ node: NodeView) {
        guard let owner = open, owner.preview === node else { return }
        owner.home = Self.box(node)
        place(owner)
    }
    /// A view's layout box in its parent, untransformed.
    private static func box(_ view: UIView) -> CGRect {
        CGRect(x: view.center.x - view.bounds.width / 2, y: view.center.y - view.bounds.height / 2,
               width: view.bounds.width, height: view.bounds.height)
    }
    /// The lifted row at the controller's origin.
    private func place(_ owner: Open) {
        guard let preview = owner.preview, let controller = owner.controller, preview.superview === controller.view else { return }
        // By its center: its layout box, whatever transform it carries.
        preview.center = CGPoint(x: preview.bounds.width / 2, y: preview.bounds.height / 2)
        if controller.preferredContentSize != preview.bounds.size { controller.preferredContentSize = preview.bounds.size }
    }

    /// Whether `node` is the row lifted into a preview: a children op on its
    /// popover leaves it there.
    func lifts(_ node: NodeView) -> Bool { open?.preview === node }
    func children(_ parent: UIView, _ wanted: [NodeView]) {
        guard let owner = open, let preview = owner.preview else { return }
        if let i = wanted.firstIndex(where: { $0 === preview }) {
            owner.container = parent; owner.index = i; owner.dropped = false
            owner.inPopover = parent === owner.homePopover?.container
        } else if parent === owner.container {
            owner.dropped = true
        }
    }

    /// The source's `contextmenu`, once, before the rows or the preview are read.
    private func fire(_ owner: Open) {
        guard !owner.fired else { return }
        owner.fired = true
        guard let presenter, let source = owner.source, live(source), source.handlers.contains("contextmenu"), !source.disabled else { return }
        let box = source.contentBox()
        let sample = PointerSample(x: Double(owner.point.x - box.minX), y: Double(owner.point.y - box.minY), buttons: 1, pressure: 0.5, type: "touch", id: 2)
        presenter.contextmenu(source.id, line: sample.line)
    }
    /// The popover as it is now, if its source still names it.
    private func current(_ owner: Open) -> NodeView? {
        // Its own action, which has run, may have disabled, hidden or moved it.
        guard let source = owner.source, live(source), source.props["contextPopover"] == owner.name,
              menus?.eligible(source) == true, let pop = popover(named: owner.name) else { return nil }
        owner.popover = pop
        return pop
    }
    private func log(_ line: String) { presenter?.session?.log(line) }

    func contextMenuInteraction(_ interaction: UIContextMenuInteraction, configurationForMenuAtLocation location: CGPoint) -> UIContextMenuConfiguration? {
        guard let menus, let source = interaction.view as? NodeView, live(source),
              let name = source.props["contextPopover"], menus.eligible(source) else { return nil }
        guard let pop = popover(named: name) else { log("context menu \(name) refused: no popover has that id"); return nil }
        // A press released before its menu showed never reached `willEnd`.
        if let previous = open { restore(previous) }
        let owner = Open(source: source, popover: pop, name: name, point: location)
        owner.interaction = interaction
        open = owner
        let configuration = UIContextMenuConfiguration(identifier: NSNumber(value: source.id), previewProvider: { [weak self, weak owner] in
            guard let self, let owner else { return nil }
            return self.lift(owner)
        }, actionProvider: { [weak self, weak owner] _ in
            guard let self, let owner else { return nil }
            return self.menu(owner)
        })
        owner.configuration = configuration
        return configuration
    }

    /// The popover's menu rows as the UIMenu, titled by its `aria-label`.
    func menu(_ owner: Open) -> UIMenu? {
        fire(owner)
        guard let menus, let pop = current(owner) else { return nil }
        let items = menus.items(of: pop)
        return items.isEmpty ? nil : UIMenu(title: pop.props["accessibilityLabel"] ?? "", children: items)
    }

    /// The preview row, moved into a controller sized by its layout; nil
    /// leaves UIKit's own (the lifted node) as the preview.
    func lift(_ owner: Open) -> UIViewController? {
        fire(owner)
        // Asked again, the same controller: the row is already in it.
        if owner.preview != nil, let controller = owner.controller { return controller }
        guard let pop = current(owner) else { return nil }
        let rows = pop.container.subviews.compactMap { $0 as? NodeView }.filter { $0.props["contextPreview"] == "true" }
        guard rows.count <= 1 else { log("context menu \(owner.name) refused a preview: \(rows.count) contextPreview rows; at most one"); return nil }
        guard let row = rows.first, !row.isHidden, row.bounds.width > 0, row.bounds.height > 0 else { return nil }
        let controller = PreviewController()
        owner.homePopover = pop
        owner.container = pop.container; owner.inPopover = true
        owner.index = pop.container.subviews.firstIndex(of: row) ?? 0
        owner.home = Self.box(row)
        owner.preview = row
        owner.controller = controller
        controller.view.addSubview(row)
        place(owner)
        return controller
    }

    /// The lifted row back in its popover, hidden with it.
    private func restore(_ owner: Open) {
        guard let row = owner.preview else { return }
        owner.preview = nil
        row.removeFromSuperview()
        // Into the container it belongs in; its popover's as it is now, as
        // a material change may have replaced the one it left.
        let popover = owner.homePopover.flatMap { live($0) ? $0 : nil }
        let home = owner.inPopover ? popover?.container : owner.container
        if live(row), !owner.dropped, let parent = home {
            parent.insertSubview(row, at: min(owner.index, parent.subviews.count))
            // Its layout box, set under whatever transform it carries.
            row.bounds.size = owner.home.size
            row.center = CGPoint(x: owner.home.midX, y: owner.home.midY)
        }
    }

    /// UIKit lifts the node itself, clipped to its corners.
    private func targeted(_ configuration: UIContextMenuConfiguration) -> UITargetedPreview? {
        guard let owner = open, let source = owner.source, live(source), source.window != nil,
              (configuration.identifier as? NSNumber)?.uint32Value == source.id else { return nil }
        let parameters = UIPreviewParameters()
        let radius = source.layer.cornerRadius
        if radius > 0 { parameters.visiblePath = UIBezierPath(roundedRect: source.bounds, cornerRadius: radius) }
        return UITargetedPreview(view: source, parameters: parameters)
    }
    func contextMenuInteraction(_ interaction: UIContextMenuInteraction, configuration: UIContextMenuConfiguration, highlightPreviewForItemWithIdentifier identifier: NSCopying) -> UITargetedPreview? {
        targeted(configuration)
    }
    func contextMenuInteraction(_ interaction: UIContextMenuInteraction, previewForHighlightingMenuWithConfiguration configuration: UIContextMenuConfiguration) -> UITargetedPreview? {
        targeted(configuration)
    }
    func contextMenuInteraction(_ interaction: UIContextMenuInteraction, previewForDismissingMenuWithConfiguration configuration: UIContextMenuConfiguration) -> UITargetedPreview? {
        // After a commit that navigated, the source is under the new screen.
        open?.navigated == true ? nil : targeted(configuration)
    }

    /// The preview tapped: its row's press, as a tap on it would be.
    func contextMenuInteraction(_ interaction: UIContextMenuInteraction, willPerformPreviewActionForMenuWith configuration: UIContextMenuConfiguration, animator: UIContextMenuInteractionCommitAnimating) {
        animator.preferredCommitStyle = .dismiss
        // The row still a row of the popover its source names, pressable and
        // enabled where it is authored (it is not in that tree while lifted).
        guard let owner = open, let presenter, let row = owner.preview, live(row), row.handlers.contains("press"),
              let pop = owner.homePopover, live(pop), current(owner) === pop, !owner.dropped,
              owner.inPopover,
              let source = owner.source, committable(row, in: pop, from: source) else { return }
        owner.committed = true
        let before = presenter.navigation.activeKey
        // Held until the stack has applied it: a push deferred behind a
        // transition (`pendingSync`) is still the commit's.
        presenter.navigation.unanimated = true
        presenter.press(row.id)
        if !presenter.navigation.inTransition || presenter.navigation.activeKey == before { presenter.navigation.unanimated = false }
        // A press that unmounted its source, or renamed its popover, without
        // navigating: `sync` left the row to the commit, which may not end.
        if presenter.navigation.activeKey == before, !(owner.source.map { live($0) && $0.props["contextPopover"] == owner.name } ?? false) {
            restore(owner)
            open = nil
            sync()
            return
        }
        // UIKit grows the preview into the screen only when the commit adds
        // an animation: at `.pop` with none it shrinks it away, as `.dismiss`
        // would (measured on iOS 27). The pushed screen's layout is the one.
        if presenter.navigation.activeKey != before {
            owner.navigated = true
            animator.preferredCommitStyle = .pop
            animator.addAnimations { [weak presenter] in presenter?.session?.view?.layoutIfNeeded() }
            // The row back when the morph is done, if `willEndFor` has not
            // (the press may have unmounted its source); then the held
            // interaction goes, if its source no longer wants one.
            animator.addCompletion { [weak self, weak owner] in
                guard let self, let owner, self.open === owner else { return }
                self.restore(owner)
                self.open = nil
                self.sync()
            }
        }
    }

    /// The row's authored ancestry, from its popover up, as `MenuHost.eligible`
    /// reads it but for the popover's own hiding: nothing disabled, inert or
    /// hidden (the row itself included), in the active route, and the source
    /// still eligible.
    private func committable(_ row: NodeView, in pop: NodeView, from source: NodeView) -> Bool {
        guard let presenter, let menus, !row.disabled, !row.isHidden, row.props["inert"] != "true", menus.eligible(source),
              !presenter.navigation.isInactiveRoute(containing: pop) else { return false }
        var view: UIView? = pop
        while let current = view {
            if let node = current as? NodeView, node.disabled || node.props["inert"] == "true" { return false }
            if current.isHidden, current !== pop { return false }
            if current === pop, pop.style["display"]?.string == "none" { return false }
            view = current.superview
        }
        return true
    }

    /// Showing: the focus is set aside, or UIKit's type-to-select brings
    /// the software keyboard up over the menu (MenuFocusIOS.swift).
    func contextMenuInteraction(_ interaction: UIContextMenuInteraction, willDisplayMenuFor configuration: UIContextMenuConfiguration, animator: UIContextMenuInteractionAnimating?) {
        menus?.focus.setAside()
    }

    func contextMenuInteraction(_ interaction: UIContextMenuInteraction, willEndFor configuration: UIContextMenuConfiguration, animator: UIContextMenuInteractionAnimating?) {
        let focus: () -> Void = { [weak self] in self?.menus?.focus.restore() }
        if let animator { animator.addCompletion(focus) } else { focus() }
        guard let owner = open, owner.configuration === configuration else { return }
        let finish = { [weak self, weak owner] in
            guard let self, let owner, self.open === owner else { return }
            self.restore(owner)
            self.open = nil
            self.sync()
        }
        if let animator { animator.addCompletion(finish) } else { finish() }
    }

    func reset() {
        for interaction in interactions.values {
            interaction.dismissMenu()
            if let view = interaction.view { view.removeInteraction(interaction); (view as? NodeView)?.contextRecognizer?.isEnabled = true }
        }
        interactions.removeAll()
        if let owner = open { restore(owner); open = nil }
    }

    /// `state.navigation.popover` while the menu is up.
    func observation() -> [String: Any]? {
        // Only once it is showing: a configuration UIKit asked for may end
        // as a tap, and nothing tells this host.
        guard let owner = open, owner.fired else { return nil }
        return ["kind": "contextmenu", "source": owner.source.map { Int($0.id) as Any } ?? NSNull(),
                "popover": owner.popover.map { Int($0.id) as Any } ?? NSNull(),
                "preview": owner.preview.map { Int($0.id) as Any } ?? NSNull(),
                "phase": owner.committed ? "commit" : "open"]
    }
}
#endif

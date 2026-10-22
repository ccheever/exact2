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
// list's rows share shows the row pressed. While the agent paints, this host
// is off and the popover opens painted (D4, `MenuHost.agentContext`).
#if os(iOS)
import UIKit

final class ContextMenuHost: NSObject, UIContextMenuInteractionDelegate, UIEditMenuInteractionDelegate {
    private weak var presenter: Presenter?
    private weak var menus: MenuHost?
    private var projected: [ObjectIdentifier: UIContextMenuInteraction] = [:]
    private var interactions: [UInt32: UIContextMenuInteraction] = [:]
    private lazy var keyboardInteraction = UIEditMenuInteraction(delegate: self)
    /// A sheet's mount's interaction, while a logical ancestor of its route
    /// names a menu (LLP 1123 D2): UIKit gives a long press to the
    /// interactions of the views it falls in, and the ancestor's own is on a
    /// view behind the sheet.
    private var mountInteractions: [ObjectIdentifier: UIContextMenuInteraction] = [:]
    private var recognizers: [ObjectIdentifier: Set<ObjectIdentifier>] = [:]
    /// Views parked with an interaction kept on them (`parked`).
    private var parkedViews = NSHashTable<UIView>.weakObjects()
    /// The preview's controller: its view holds the lifted row.
    final class PreviewController: UIViewController {
        override func loadView() { view = UIView() }
    }
    /// One presentation, from its configuration to the end of its dismissal.
    final class Open {
        weak var source: NodeView?
        weak var popover: NodeView?
        let name: String
        let inlineID: UInt32?
        let point: CGPoint
        /// Whether the source's own `contextmenu` has fired (once, lazily).
        var fired = false
        var toggled = false
        weak var configuration: UIContextMenuConfiguration?
        weak var interaction: UIContextMenuInteraction?
        var keyboardConfiguration: UIEditMenuConfiguration?
        /// The lifted row; the popover it is a row of, the container it
        /// belongs in (its popover's, or another's a children op moved it
        /// to), its index there and its frame there (`home`, the last frame
        /// op's); whether a children op has dropped it.
        weak var preview: NodeView?
        /// The view a long press in a sheet fell on, when the menu is a
        /// logical ancestor's behind the sheet: its preview lifts that.
        weak var touched: UIView?
        weak var homePopover: NodeView?
        weak var container: UIView?
        var inPopover = true
        var index = 0
        var home = CGRect.zero
        var dropped = false
        var controller: PreviewController?
        /// Whether the preview was tapped, and whether its press navigated.
        var committed = false, navigated = false
        init(source: NodeView, popover: NodeView, name: String, point: CGPoint, inlineID: UInt32? = nil) {
            self.source = source; self.popover = popover; self.name = name; self.point = point; self.inlineID = inlineID
        }
    }
    private(set) var open: Open?

    func isOpen(_ pop: NodeView) -> Bool { open?.popover === pop && open?.toggled == true }
    func close(_ pop: NodeView) {
        guard let owner = open, owner.popover === pop else { return }
        open = nil
        restore(owner)
        owner.interaction?.dismissMenu()
        if owner.keyboardConfiguration != nil { keyboardInteraction.dismissMenu() }
    }

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

    func syncProjected(_ view: UIView & GroupedMenuSurface) {
        let key = ObjectIdentifier(view)
        if let owner = open, owner.interaction?.view === view, owner.source !== view.menuSource {
            owner.interaction?.dismissMenu()
            restore(owner)
            open = nil
        }
        let wanted = !ExactEnv.agentPaints && view.menuSource?.props["contextPopover"]?.isEmpty == false
        if wanted, projected[key] == nil {
            let interaction = UIContextMenuInteraction(delegate: self)
            let before = companions(of: view)
            view.addInteraction(interaction)
            projected[key] = interaction
            recognizers[ObjectIdentifier(interaction)] = companions(of: view).subtracting(before).subtracting([ObjectIdentifier(interaction)])
        } else if !wanted, let interaction = projected.removeValue(forKey: key) {
            if open?.interaction === interaction, let owner = open { restore(owner); open = nil }
            interaction.dismissMenu()
            detach(interaction, from: view)
        }
    }

    /// After a batch: one interaction on each node naming a popover, its
    /// plain long press (the `contextmenu` recognizer) off while it has one.
    func sync() {
        guard let presenter else { return }
        // A source gone, or naming another popover, ends its menu: the row
        // goes back now, as `willEnd` may never come for it. A commit's
        // animation still holds the row; its `willEnd` returns it.
        if let owner = open, !owner.committed,
           !(owner.source.map { live($0) && contextName(owner) == owner.name } ?? false)
            || owner.keyboardConfiguration != nil && current(owner) == nil {
            open = nil
            owner.interaction?.dismissMenu()
            if owner.keyboardConfiguration != nil { keyboardInteraction.dismissMenu() }
            restore(owner)
        }
        var wanted: [UInt32: NodeView] = [:]
        if !ExactEnv.agentPaints {
            for v in presenter.textViews.values where v.hasInlineContextPopover { wanted[v.id] = v }
            for v in presenter.carrying("contextPopover") where v.props["contextPopover"]?.isEmpty == false { wanted[v.id] = v }
        }
        for (key, interaction) in projected {
            if let view = interaction.view as? (UIView & GroupedMenuSurface) { syncProjected(view) }
            else { projected[key] = nil; recognizers[ObjectIdentifier(interaction)] = nil }
        }
        syncMounts(presenter, sources: Array(wanted.values))
        for (id, interaction) in interactions where wanted[id] == nil || interaction.view !== wanted[id] {
            // A commit in flight keeps its interaction until it completes.
            if let owner = open, owner.committed, owner.interaction === interaction { continue }
            // A view destroyed with it has taken it along: its record goes too.
            if interaction.view == nil { recognizers[ObjectIdentifier(interaction)] = nil }
            if let view = interaction.view {
                detach(interaction, from: view)
                (view as? NodeView)?.contextRecognizer?.isEnabled = true
            }
            interactions[id] = nil
        }
        // A parked view lent to a new node keeps its interaction if the node
        // names a popover too, else it comes off.
        for case let v as NodeView in parkedViews.allObjects where presenter.views[v.id] === v {
            // Taken (live again under its new node's id), or never parked.
            parkedViews.remove(v)
            guard !wanted.values.contains(where: { $0 === v }) else { continue }
            ours(v).forEach { detach($0, from: v) }
            v.contextRecognizer?.isEnabled = true
        }
        for (id, v) in wanted {
            if interactions[id] == nil, let kept = ours(v).first, recognizers[ObjectIdentifier(kept)] != nil {
                // Lent from a parked row: the same interaction, now the new node's.
                interactions[id] = kept
            }
            if interactions[id] == nil {
                // Anything else of ours on the view goes first.
                ours(v).forEach { detach($0, from: v) }
                let interaction = UIContextMenuInteraction(delegate: self)
                let before = companions(of: v)
                v.addInteraction(interaction)
                interactions[id] = interaction
                // The recognizers and interactions UIKit adds with it, which
                // the node pool lets park with it.
                recognizers[ObjectIdentifier(interaction)] = companions(of: v).subtracting(before).subtracting([ObjectIdentifier(interaction)])
            }
            v.contextRecognizer?.isEnabled = (v.handlers.contains("contextmenu") && v.props["contextPopover"]?.isEmpty != false) || v.hasInlinePlainContextMenu
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
    /// Whether `interaction` is one this host installed.
    func owns(_ interaction: UIInteraction, on view: UIView) -> Bool {
        if (interaction as? UIContextMenuInteraction)?.delegate === self { return true }
        return came(ObjectIdentifier(interaction), on: view)
    }
    /// Whether `recognizer` came with one of this host's interactions on `view`.
    func owns(_ recognizer: UIGestureRecognizer, on view: UIView) -> Bool { came(ObjectIdentifier(recognizer), on: view) }
    private func came(_ id: ObjectIdentifier, on view: UIView) -> Bool {
        ours(view).contains { recognizers[ObjectIdentifier($0)]?.contains(id) == true }
    }
    /// One of ours off `view`, with the interactions UIKit added beside it:
    /// it leaves one of them (a click-presentation feedback generator) on
    /// the view when only the menu's is removed (measured), which would keep
    /// the view out of the node pool for good.
    private func detach(_ interaction: UIContextMenuInteraction, from view: UIView) {
        let added = recognizers.removeValue(forKey: ObjectIdentifier(interaction)) ?? []
        view.removeInteraction(interaction)
        for other in view.interactions where added.contains(ObjectIdentifier(other)) { view.removeInteraction(other) }
    }
    private func companions(of view: UIView) -> Set<ObjectIdentifier> {
        Set((view.gestureRecognizers ?? []).map(ObjectIdentifier.init) + view.interactions.map(ObjectIdentifier.init))
    }
    /// `view` parks in the node pool (NodePoolIOS): its interaction stays on
    /// it, no longer any node's (the delegate reads the node from the view,
    /// and a parked view is hidden, so no menu opens from it). The next sync
    /// gives it to the node the view is lent to if that one names a popover,
    /// else takes it off and turns the long press back on. Removing and
    /// adding it would cost what pooling saves: UIKit adds eight recognizers
    /// and two interactions with it.
    func parked(_ view: NodeView) {
        interactions = interactions.filter { $0.value.view !== view }
        if !ours(view).isEmpty { parkedViews.add(view) }
    }
    /// A parked view leaves the pool for good (evicted, its list gone, memory
    /// pressure): its interaction and what UIKit added with it go too.
    func dropped(_ view: NodeView) {
        parkedViews.remove(view)
        ours(view).forEach { detach($0, from: view) }
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
    func logicalParent(of view: UIView) -> UIView? {
        guard let owner = open, owner.preview === view, !owner.dropped else { return nil }
        return owner.inPopover ? owner.homePopover?.container : owner.container
    }
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
        guard let source = owner.source, live(source), !source.formDisabled else { return }
        source.deliverContext(at: owner.point, inline: owner.inlineID)
    }
    private func contextName(_ owner: Open) -> String? {
        guard let source = owner.source else { return nil }
        if let id = owner.inlineID {
            guard presenter?.textHost(id) === source, presenter?.inlineInert(id) == false else { return nil }
            return presenter?.inlineText(id)?.props["contextPopover"]
        }
        return source.props["contextPopover"]
    }

    /// The popover as it is now, if its source still names it.
    private func current(_ owner: Open) -> NodeView? {
        // A cell may have been reused while UIKit still holds its menu.
        if let cell = owner.interaction?.view as? GroupedMenuSurface, cell.menuSource !== owner.source { return nil }
        // Its own action, which has run, may have disabled, hidden or moved it.
        guard let source = owner.source, live(source), contextName(owner) == owner.name,
              menus?.eligible(source, contextMenu: true) == true, let pop = popover(named: owner.name) else { return nil }
        owner.popover = pop
        return pop
    }
    private func log(_ line: String) { presenter?.session?.log(line) }

    /// A mount takes an interaction while one of `sources` is a logical
    /// ancestor of its route, and gives it back when none is.
    private func syncMounts(_ presenter: Presenter, sources: [NodeView]) {
        var live: Set<ObjectIdentifier> = []
        for (mount, route) in presenter.modals.mounted {
            guard sources.contains(where: { $0 !== route && presenter.logicallyWithin(route, $0) }) else { continue }
            let key = ObjectIdentifier(mount)
            live.insert(key)
            if mountInteractions[key] == nil {
                let interaction = UIContextMenuInteraction(delegate: self)
                mount.addInteraction(interaction)
                mountInteractions[key] = interaction
            }
        }
        for (key, interaction) in mountInteractions where !live.contains(key) {
            interaction.view?.removeInteraction(interaction)
            mountInteractions[key] = nil
        }
    }

    func contextMenuInteraction(_ interaction: UIContextMenuInteraction, configurationForMenuAtLocation location: CGPoint) -> UIContextMenuConfiguration? {
        guard let menus, let view = interaction.view else { return nil }
        let source: NodeView, point: CGPoint, hit: UIView?
        if let node = view as? NodeView {
            source = node; point = location; hit = node.hitTest(location, with: nil)
        } else if let cell = view as? (UIView & GroupedMenuSurface), let found = cell.menuSource {
            source = found; point = location; hit = cell
        } else if let mount = view as? PresentationMount, let found = mount.relay?.listener(mount.hitTest(location, with: nil), contextMenu: true, { $0.props["contextPopover"]?.isEmpty == false }) {
            // A sheet's mount: its route's nearest logical ancestor with a
            // menu, unless a node in the sheet is nearer (`MountRelay`)
            source = found; point = mount.convert(location, to: found); hit = mount.hitTest(location, with: nil)
        } else { return nil }
        let run = source.inlineContextTarget(at: point)
        guard live(source), let name = run?.props["contextPopover"] ?? (run == nil ? source.props["contextPopover"] : nil),
              menus.eligible(source, contextMenu: true) else { return nil }
        // UIKit's row menu wins over text selection. An authored menu
        // nearer the hit still belongs to that node (LLP 1115.000 D11.6).
        if view is NodeView ? NodeView.nativeEditor(hit, below: view) : NodeView.nativeInputOwns(hit, below: view) { return nil }
        if let hit = hit as? NodeView,
           let nearest = hit.contextMenuSource(at: hit.convert(location, from: view)), nearest !== source { return nil }
        guard let pop = popover(named: name) else { log("context menu \(name) refused: no popover has that id"); return nil }
        // A press released before its menu showed never reached `willEnd`.
        if let previous = open { restore(previous) }
        let owner = Open(source: source, popover: pop, name: name, point: point, inlineID: run?.id)
        owner.interaction = interaction
        if !(view is NodeView) { owner.touched = hit }
        open = owner
        let configuration = UIContextMenuConfiguration(identifier: NSNumber(value: run?.id ?? source.id), previewProvider: { [weak self, weak owner] in
            guard let self, let owner else { return nil }
            return self.lift(owner)
        }, actionProvider: { [weak self, weak owner] _ in
            guard let self, let owner else { return nil }
            return self.menu(owner)
        })
        owner.configuration = configuration
        return configuration
    }


    /// UIKit's public programmatic menu API. It presents the same authored
    /// actions as a long press, without lifting a preview for keyboard input.
    func showKeyboard(_ source: NodeView, at point: CGPoint) -> Bool {
        guard !ExactEnv.agentMode, source.window != nil, let menus,
              menus.eligible(source, contextMenu: true), let name = source.props["contextPopover"],
              let pop = popover(named: name), !menus.isConfirmation(pop) else { return false }
        if let previous = open {
            open = nil
            previous.interaction?.dismissMenu()
            keyboardInteraction.dismissMenu()
            restore(previous)
        }
        let owner = Open(source: source, popover: pop, name: name, point: point)
        owner.fired = true // The keyboard dispatcher has already delivered contextmenu.
        guard menu(owner) != nil else { return false }
        let configuration = UIEditMenuConfiguration(identifier: NSNumber(value: source.id), sourcePoint: point)
        owner.keyboardConfiguration = configuration
        open = owner
        source.addInteraction(keyboardInteraction)
        menus.focus.setAside()
        keyboardInteraction.presentEditMenu(with: configuration)
        return true
    }

    func editMenuInteraction(_ interaction: UIEditMenuInteraction, menuFor configuration: UIEditMenuConfiguration,
                             suggestedActions: [UIMenuElement]) -> UIMenu? {
        guard let owner = open, owner.keyboardConfiguration === configuration else { return UIMenu(children: []) }
        return menu(owner) ?? UIMenu(children: [])
    }
    func editMenuInteraction(_ interaction: UIEditMenuInteraction, willPresentMenuFor configuration: UIEditMenuConfiguration,
                             animator: UIEditMenuInteractionAnimating) {
        guard let owner = open, owner.keyboardConfiguration === configuration, let pop = current(owner) else { return }
        owner.toggled = true
        presenter?.popoverEvents.changed(pop, open: true)
    }
    func editMenuInteraction(_ interaction: UIEditMenuInteraction, willDismissMenuFor configuration: UIEditMenuConfiguration,
                             animator: UIEditMenuInteractionAnimating) {
        guard let owner = open, owner.keyboardConfiguration === configuration else { return }
        animator.addCompletion { [weak self, weak owner] in
            guard let self, let owner, self.open === owner else { return }
            self.open = nil
            self.restore(owner)
        }
    }

    /// The popover's menu rows as the UIMenu, titled by its `aria-label`.
    func menu(_ owner: Open) -> UIMenu? {
        fire(owner)
        guard let menus, let pop = current(owner) else { return nil }
        let items = menus.items(of: pop, opener: owner.source) // a row's popover opens from the pressed node (LLP 1021)
        return items.isEmpty ? nil : UIMenu(title: pop.props["accessibilityLabel"] ?? "", children: items)
    }

    /// The preview row, moved into a controller sized by its layout; nil
    /// leaves UIKit's own (the lifted node) as the preview.
    func lift(_ owner: Open) -> UIViewController? {
        fire(owner)
        // Asked again, the same controller: the row is already in it.
        if owner.preview != nil, let controller = owner.controller { return controller }
        guard let pop = current(owner) else { return nil }
        // Laid out first: a closed popover has no box (`MenuHost.layOut`).
        menus?.layOut(pop)
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
        presenter?.controls.syncShown()
        return controller
    }

    /// The lifted row back in its popover, hidden with it.
    private func restore(_ owner: Open) {
        menus?.coverClosed()
        if let pop = owner.popover, owner.toggled { owner.toggled = false; presenter?.popoverEvents.changed(pop, open: false) }
        if owner.keyboardConfiguration != nil {
            keyboardInteraction.view?.removeInteraction(keyboardInteraction)
            menus?.focus.restore()
        }
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
            presenter?.regrid(row) // a frame op while it was lifted was placed from the preview
        }
    }

    /// UIKit lifts the node itself, clipped to its corners.
    private func targeted(_ configuration: UIContextMenuConfiguration) -> UITargetedPreview? {
        guard let owner = open, let source = owner.source, live(source), (owner.touched ?? source).window != nil,
              (configuration.identifier as? NSNumber)?.uint32Value == (owner.inlineID ?? source.id) else { return nil }
        let parameters = UIPreviewParameters()
        let shown = owner.touched.flatMap { $0.window == nil ? nil : $0 } ?? source
        if let id = owner.inlineID, let run = presenter?.inlineText(id) {
            let path = UIBezierPath()
            source.inlineRects(run).forEach { path.append(UIBezierPath(rect: $0)) }
            parameters.visiblePath = path
        }
        let radius = shown.layer.cornerRadius
        if radius > 0, parameters.visiblePath == nil { parameters.visiblePath = UIBezierPath(roundedRect: shown.bounds, cornerRadius: radius) }
        return UITargetedPreview(view: shown, parameters: parameters)
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
        if presenter.navigation.activeKey == before, !(owner.source.map { live($0) && contextName(owner) == owner.name } ?? false) {
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
        guard let presenter, let menus, !row.disabled, !row.isHidden, row.props["inert"] != "true", menus.eligible(source, contextMenu: true),
              !presenter.navigation.isInactiveRoute(containing: pop) else { return false }
        var view: UIView? = pop
        while let current = view {
            if let node = current as? NodeView, node.disabled || node.props["inert"] == "true" { return false }
            if current.isHidden, current !== pop { return false }
            if current === pop, pop.style["display"]?.string == "none" { return false }
            view = presenter.eventParent(of: current)
        }
        return true
    }

    /// Showing: the focus is set aside, or UIKit's type-to-select brings
    /// the software keyboard up over the menu (MenuFocusIOS.swift).
    func contextMenuInteraction(_ interaction: UIContextMenuInteraction, willDisplayMenuFor configuration: UIContextMenuConfiguration, animator: UIContextMenuInteractionAnimating?) {
        menus?.focus.setAside()
        if let owner = open, let pop = owner.popover { owner.toggled = true; presenter?.popoverEvents.changed(pop, open: true) }
    }

    func contextMenuInteraction(_ interaction: UIContextMenuInteraction, willEndFor configuration: UIContextMenuConfiguration, animator: UIContextMenuInteractionAnimating?) {
        let focus: () -> Void = { [weak self] in self?.menus?.focus.restore() }
        if let animator { animator.addCompletion(focus) } else { focus() }
        guard let owner = open, owner.configuration === configuration else { return }
        if let pop = owner.popover, owner.toggled { owner.toggled = false; presenter?.popoverEvents.changed(pop, open: false) }
        let finish = { [weak self, weak owner] in
            guard let self, let owner, self.open === owner else { return }
            self.restore(owner)
            self.open = nil
            self.sync()
        }
        if let animator { animator.addCompletion(finish) } else { finish() }
    }

    func reset() {
        keyboardInteraction.dismissMenu()
        for interaction in projected.values {
            interaction.dismissMenu()
            if let view = interaction.view { detach(interaction, from: view) }
        }
        projected.removeAll()
        for interaction in interactions.values {
            interaction.dismissMenu()
            if let view = interaction.view { detach(interaction, from: view); (view as? NodeView)?.contextRecognizer?.isEnabled = true }
        }
        interactions.removeAll()
        // Parked views are no node's and out of the map: theirs come off too,
        // with what UIKit added beside them, before the records go.
        for case let view as NodeView in parkedViews.allObjects { ours(view).forEach { detach($0, from: view) } }
        parkedViews.removeAllObjects()
        recognizers.removeAll()
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

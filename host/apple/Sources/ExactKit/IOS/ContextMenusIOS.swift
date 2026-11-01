// @ref LLP 1021 §5.1 — the context menu on UIKit: a node naming a popover
// with `contextPopover` gets a UIContextMenuInteraction. UIKit owns the long
// press, the lift of the node or of the list cell that draws it (its targeted
// preview), the menu's placement and the morph. The popover's menu rows are the UIMenu (D3's
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
        /// Runs as its view appears (`viewIsAppearing`).
        var appearing: (() -> Void)?
        override func loadView() { view = UIView() }
        override func viewIsAppearing(_ animated: Bool) {
            super.viewIsAppearing(animated)
            appearing?()
        }
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
        /// The platter's colour under its lifted cell, read as the cell first
        /// lifts; the dismissal's preview reuses it, as UIKit dims what is
        /// beneath once the menu shows.
        var platter: UIColor?
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

    func syncProjected(_ view: UIView & MenuSurface) {
        let key = ObjectIdentifier(view)
        if let owner = open, owner.interaction?.view === view, owner.source !== view.menuSource {
            owner.interaction?.dismissMenu()
            restore(owner)
            open = nil
        }
        let wanted = !ExactEnv.agentMode && view.menuSource?.props["contextPopover"]?.isEmpty == false
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
        if !ExactEnv.agentMode {
            for v in presenter.textViews.values where v.hasInlineContextPopover { wanted[v.id] = v }
            for v in presenter.carrying("contextPopover") where v.props["contextPopover"]?.isEmpty == false { wanted[v.id] = v }
        }
        for (key, interaction) in projected {
            if let view = interaction.view as? (UIView & MenuSurface) { syncProjected(view) }
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
        if let cell = owner.interaction?.view as? MenuSurface, cell.menuSource !== owner.source { return nil }
        // Its own action, which has run, may have disabled, hidden or moved it.
        guard let source = owner.source, live(source), contextName(owner) == owner.name,
              eligible(source, of: owner), let pop = popover(named: owner.name) else { return nil }
        owner.popover = pop
        return pop
    }
    /// Whether the source may still show its menu: a source a surface draws
    /// in a bar (its title) is hidden from the page while the bar shows it.
    private func eligible(_ source: NodeView, of owner: Open) -> Bool {
        menus?.eligible(source, contextMenu: true, inBar: owner.interaction?.view is HeaderTitleView) == true
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
        } else if let surface = view as? (UIView & MenuSurface), let found = surface.menuSource {
            source = found; point = location; hit = surface
        } else if let mount = view as? PresentationMount, let found = mount.relay?.listener(mount.hitTest(location, with: nil), contextMenu: true, { $0.props["contextPopover"]?.isEmpty == false }) {
            // A sheet's mount: its route's nearest logical ancestor with a
            // menu, unless a node in the sheet is nearer (`MountRelay`)
            source = found; point = mount.convert(location, to: found); hit = mount.hitTest(location, with: nil)
        } else { return nil }
        let run = source.inlineContextTarget(at: point)
        guard live(source), let name = run?.props["contextPopover"] ?? (run == nil ? source.props["contextPopover"] : nil),
              menus.eligible(source, contextMenu: true, inBar: view is HeaderTitleView) else { return nil }
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
        // UIKit shows the preview through a portal from a container it keeps
        // transparent, so the row's text is never found showing
        // (`textIsVisible`): its paragraphs paint as the preview appears.
        controller.appearing = { [weak self, weak row] in
            guard let presenter = self?.presenter, let row else { return }
            var views: [UIView] = [row]
            while let view = views.popLast() {
                views.append(contentsOf: view.subviews)
                if let node = view as? NodeView, node.isParagraph { _ = presenter.textRasters.ensure(node, urgent: true) }
            }
        }
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

    /// The view UIKit lifts: the node, clipped to its corners or to the
    /// grounds it holds, a bar title's avatar, or the list cell that draws
    /// it, in its background's shape.
    private func targeted(_ configuration: UIContextMenuConfiguration) -> UITargetedPreview? {
        guard let owner = open, let source = owner.source, live(source), (owner.touched ?? source).window != nil,
              (configuration.identifier as? NSNumber)?.uint32Value == (owner.inlineID ?? source.id) else { return nil }
        let parameters = UIPreviewParameters()
        var shown: UIView = owner.touched.flatMap { $0.window == nil ? nil : $0 } ?? source
        // A bar title drawn with an avatar, whose menu has no preview row,
        // lifts the avatar alone, in its corners on no ground, as the
        // platform's chat lifts the photo in its title
        if let title = shown as? HeaderTitleView, !title.avatar.isHidden, let face = title.avatarFace, !previews(owner) {
            shown = title.avatar
            parameters.visiblePath = UIBezierPath(cgPath: BorderPaint.roundedRect(title.avatar.bounds, face.corners))
            parameters.backgroundColor = .clear
        }
        if let id = owner.inlineID, let run = presenter?.inlineText(id) {
            let path = UIBezierPath()
            source.inlineRects(run).forEach { path.append(UIBezierPath(rect: $0)) }
            parameters.visiblePath = path
        }
        // A row a list's cell draws lifts as UIKit lifts the cells it draws
        // itself: the cell, its selection's ground included, in its
        // background's shape (a sidebar's rounded rectangle, a plain list's
        // whole width, past a side safe area), on the ground beneath it.
        if parameters.visiblePath == nil, let cell = Self.cell(drawing: shown), let background = cell.backgroundConfiguration {
            shown = cell
            parameters.visiblePath = Self.shape(of: background, in: cell)
            let platter = owner.platter ?? Self.ground(beside: cell) ?? parameters.backgroundColor
            owner.platter = platter
            parameters.backgroundColor = platter
        }
        let radius = shown.layer.cornerRadius
        if radius > 0, parameters.visiblePath == nil { parameters.visiblePath = UIBezierPath(roundedRect: shown.bounds, cornerRadius: radius) }
        // A node that paints no ground, whose text and images lie on the
        // grounds the nodes inside it paint, lifts in their shape: a chat
        // balloon, its body and its tail, as the platform lifts its own.
        if parameters.visiblePath == nil, let node = shown as? NodeView, !Self.paintsGround(node), let grounds = Self.grounds(in: node) {
            parameters.visiblePath = grounds
        }
        // Lifted over the window: in its own superview, a strip that scrolls
        // sideways would clip the card as it grows. Its centre is the view's,
        // and its transform the view's with each ancestor's scale and
        // rotation, which the default target's container applies.
        guard let window = shown.window, let superview = shown.superview else { return UITargetedPreview(view: shown, parameters: parameters) }
        var transform = shown.transform
        for ancestor in sequence(first: superview, next: { $0.superview }) where ancestor !== window {
            let t = ancestor.transform
            transform = transform.concatenating(CGAffineTransform(a: t.a, b: t.b, c: t.c, d: t.d, tx: 0, ty: 0))
        }
        let target = UIPreviewTarget(container: window, center: superview.convert(shown.center, to: window), transform: transform)
        return UITargetedPreview(view: shown, parameters: parameters, target: target)
    }
    /// Whether the menu's popover has a preview row to show (`lift`).
    private func previews(_ owner: Open) -> Bool {
        current(owner)?.container.subviews.contains { ($0 as? NodeView)?.props["contextPreview"] == "true" && !$0.isHidden } ?? false
    }
    /// The grounds the nodes inside `view` paint, in `view`'s space, each in
    /// its clip path or its corners; nil when none paints one, or when a
    /// node inside that paints none and holds none lies off them (a
    /// paragraph, an image, a drawing).
    private static func grounds(in view: NodeView) -> UIBezierPath? {
        let shape = UIBezierPath()
        var content: [NodeView] = []
        var stack = view.subviews
        while let sub = stack.popLast() {
            guard !sub.isHidden, sub.alpha > 0.01 else { continue }
            stack.append(contentsOf: sub.subviews)
            guard let node = sub as? NodeView else { continue }
            if paintsGround(node) {
                let path = node.clipPath.map { UIBezierPath(cgPath: $0) } ?? node.roundedPath(in: node.bounds)
                let origin = view.convert(CGPoint.zero, from: node), x = view.convert(CGPoint(x: 1, y: 0), from: node), y = view.convert(CGPoint(x: 0, y: 1), from: node)
                path.apply(CGAffineTransform(a: x.x - origin.x, b: x.y - origin.y, c: y.x - origin.x, d: y.y - origin.y, tx: origin.x, ty: origin.y))
                shape.append(path)
            } else if !node.bounds.isEmpty, !node.subviews.contains(where: { $0 is NodeView }) {
                content.append(node)
            }
        }
        guard !shape.isEmpty, content.allSatisfy({ shape.contains(view.convert(CGPoint(x: $0.bounds.midX, y: $0.bounds.midY), from: $0)) }) else { return nil }
        return shape
    }
    /// Whether `node` paints a ground: a background colour, a gradient, a material or a border.
    private static func paintsGround(_ node: NodeView) -> Bool {
        (node.cgColor("background_color")?.alpha ?? 0) > 0 || node.style["background_image"] != nil || node.materialView != nil
            || ["border_width", "border_width_top", "border_width_right", "border_width_bottom", "border_width_left"].contains { node.number($0) > 0 }
    }
    /// The list cell drawing `view`: the cell itself, or the cell whose content carries it.
    private static func cell(drawing view: UIView) -> UICollectionViewCell? {
        if let cell = view as? UICollectionViewCell { return cell }
        guard let content = view.superview, let cell = content.superview as? UICollectionViewCell, cell.contentView === content else { return nil }
        return cell
    }
    /// A cell's background as its configuration draws it: the cell's bounds
    /// less the configuration's insets, with its corner radius.
    private static func shape(of background: UIBackgroundConfiguration, in cell: UICollectionViewCell) -> UIBezierPath {
        var insets = background.backgroundInsets
        let margins = cell.directionalLayoutMargins, edges = background.edgesAddingLayoutMarginsToBackgroundInsets
        if edges.contains(.top) { insets.top += margins.top }
        if edges.contains(.leading) { insets.leading += margins.leading }
        if edges.contains(.bottom) { insets.bottom += margins.bottom }
        if edges.contains(.trailing) { insets.trailing += margins.trailing }
        let rtl = cell.effectiveUserInterfaceLayoutDirection == .rightToLeft
        let box = cell.bounds.inset(by: UIEdgeInsets(top: insets.top, left: rtl ? insets.trailing : insets.leading, bottom: insets.bottom, right: rtl ? insets.leading : insets.trailing))
        return UIBezierPath(roundedRect: box, cornerRadius: background.cornerRadius)
    }
    /// The ground a list's cell lies on, read off the screen in the list's
    /// margin beside it: in a sidebar, UIKit's material, which UIKit's own
    /// lift copies from below the cell and no API names. Nil where the cell
    /// spans its list (a plain list's): no margin shows the ground, and the
    /// platter's default, the system background, is that list's ground.
    private static func ground(beside cell: UICollectionViewCell) -> UIColor? {
        guard let window = cell.window, let list = cell.superview else { return nil }
        let margin = cell.frame.minX - list.bounds.minX
        guard margin >= 2 else { return nil }
        let point = list.convert(CGPoint(x: cell.frame.minX - margin / 2, y: cell.frame.midY), to: window)
        var pixel: [UInt8] = [0, 0, 0, 0]
        let drawn = pixel.withUnsafeMutableBytes { bytes -> Bool in
            guard let context = CGContext(data: bytes.baseAddress, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
                                          space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
            // Flip to UIKit's top-left origin, with `point` on the pixel
            context.translateBy(x: -point.x, y: 1 + point.y)
            context.scaleBy(x: 1, y: -1)
            UIGraphicsPushContext(context)
            defer { UIGraphicsPopContext() }
            return window.drawHierarchy(in: window.bounds, afterScreenUpdates: false)
        }
        guard drawn, pixel[3] == 255 else { return nil }
        return UIColor(red: CGFloat(pixel[0]) / 255, green: CGFloat(pixel[1]) / 255, blue: CGFloat(pixel[2]) / 255, alpha: 1)
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
              let source = owner.source, committable(row, in: pop, from: source, of: owner) else { return }
        owner.committed = true
        let before = presenter.navigation.activeKey
        // Held until the stack has applied it: a push deferred behind a
        // transition (`pendingSync`) is still the commit's.
        presenter.navigation.unanimated = true
        // The preview opens what its source row opens: a sidebar selects that row, as its press would
        if let lists = presenter.groupedLists { lists.open(from: source) { presenter.press(row.id) } } else { presenter.press(row.id) }
        if !presenter.navigation.inTransition || presenter.navigation.activeKey == before { presenter.navigation.unanimated = false }
        // It navigated if it pushed a screen, or if a split's detail now
        // shows its sidebar row's: a route shown in the place of the one
        // before keeps its key.
        let navigated = presenter.navigation.activeKey != before || presenter.sidebarSelection(source) == source.id
        // A press that unmounted its source, or renamed its popover, without
        // navigating: `sync` left the row to the commit, which may not end.
        if !navigated, !(owner.source.map { live($0) && contextName(owner) == owner.name } ?? false) {
            restore(owner)
            open = nil
            sync()
            return
        }
        // UIKit grows the preview into the screen only when the commit adds
        // an animation: at `.pop` with none it shrinks it away, as `.dismiss`
        // would (measured on iOS 27). That animation is the new screen's
        // layout (`layoutIfNeeded`).
        if navigated {
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
    private func committable(_ row: NodeView, in pop: NodeView, from source: NodeView, of owner: Open) -> Bool {
        guard let presenter, !row.disabled, !row.isHidden, row.props["inert"] != "true", eligible(source, of: owner),
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

/// A view that draws a node it is not and takes that node's long press: a
/// grouped list's cell for a row UIKit draws, a navigation bar's title for its
/// route's heading group. Its menu is the node's (`syncProjected`).
package protocol MenuSurface: AnyObject {
    var menuSource: NodeView? { get }
}

extension Presenter {
    package func syncMenu(on view: UIView & MenuSurface) {
        menus.context.syncProjected(view)
    }
}
#endif

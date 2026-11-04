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
import AVFoundation
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
        /// The preview of an inline run UIKit lifts (`targeted`), taken as the
        /// finger lands: its picture and path stay the same from the lift to the
        /// dismissal.
        var runPreview: UITargetedPreview?
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
        // A run's picture as the finger lands: UIKit asks for the configuration
        // before a press it starts has drawn (measured), so the picture holds
        // none, such as a tappable ancestor's dim
        if let run, let container = Self.container(of: source), let lines = RunLines(run, in: source, over: container) {
            owner.runPreview = Self.runPreview(lines, of: source)
        }
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
        guard !ExactEnv.agentPaints, source.window != nil, let menus,
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

    /// The lifted row back in its popover, hidden with it, and a run's picture
    /// off the screen: UIKit leaves it in the window once it has returned it
    /// (filmed and logged, iOS 27.1).
    private func restore(_ owner: Open) {
        menus?.coverClosed()
        owner.runPreview?.view.removeFromSuperview()
        owner.runPreview = nil
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

    /// UIKit lifts the node itself, live, clipped to its outline (LLP 1021
    /// §5.1): it hides the node, lifts it and returns it. A run of text cannot
    /// lift its paragraph, which UIKit would hide whole: it lifts a picture of
    /// what lies under the run's lines (`runPreview`), and the paragraph stays
    /// shown. Both rise over `container(of:)`.
    private func targeted(_ configuration: UIContextMenuConfiguration) -> UITargetedPreview? {
        guard let owner = open, let source = owner.source, live(source),
              (configuration.identifier as? NSNumber)?.uint32Value == (owner.inlineID ?? source.id) else { return nil }
        if let id = owner.inlineID {
            // Nil where the run no longer shows lines, its text gone
            guard let run = presenter?.inlineText(id), let container = Self.container(of: source),
                  let lines = RunLines(run, in: source, over: container) else { return nil }
            // The picture the finger landed on, else one taken now, from the run's place now
            guard let preview = owner.runPreview ?? Self.runPreview(lines, of: source) else { return nil }
            owner.runPreview = preview
            return preview.retargetedPreview(with: lines.target)
        }
        // A view the touch fell on that has left the window gives way to the node
        let shown = owner.touched.flatMap { $0.window == nil ? nil : $0 } ?? source
        let parameters = UIPreviewParameters()
        parameters.visiblePath = Self.rounded(shown)
        guard let target = Self.target(shown) else { return nil }
        return UITargetedPreview(view: shown, parameters: parameters, target: target)
    }

    /// `view`'s outline in its own coordinates where its corners are rounded,
    /// else nil. One radius, round or Apple's continuous, is UIKit's own
    /// rounded rectangle, which UIKit keeps through the lift's morph: UIKit
    /// applies any other path only after the morph's first frames, which show
    /// the node square (measured, iOS 27.1). One radius within the tolerance the
    /// layer's own (`applyBoxLayer`) has.
    private static func rounded(_ view: UIView) -> UIBezierPath? {
        guard let node = view as? NodeView else {
            return view.layer.cornerRadius > 0 ? UIBezierPath(roundedRect: view.bounds, cornerRadius: view.layer.cornerRadius) : nil
        }
        let corners = node.cornerSizes(in: node.bounds), shape = node.style["corner_shape"]
        if let corner = corners.first, corner.width > 0, corners.allSatisfy({ abs($0.width - corner.width) < 0.01 && abs($0.height - corner.width) < 0.01 }),
           shape == nil || CornerShape(shape)?.isAppleContinuous == true {
            return UIBezierPath(roundedRect: node.bounds, cornerRadius: corner.width)
        }
        return corners.contains(where: { $0.width > 0 }) ? node.roundedPath(in: node.bounds) : nil
    }

    /// A run's lines where the screen shows them in `container`: their boxes,
    /// and their outline as UIKit outlines text lines, with the padding UIKit
    /// gives them (`UIPreviewParameters(textLineRects:)`), in the coordinates of
    /// the outline's area. Where the padding reaches past the box the run sits
    /// in (`box(of:below:)`), it extends the box: the lift keeps UIKit's
    /// footprint and holds none of what lies outside the box, such as the page
    /// around a card.
    private struct RunLines {
        let boxes: [CGRect], outline: UIBezierPath, area: CGRect, container: UIView
        /// The box the padding reaches past, whose look it extends over the area
        let box: CALayer?
        /// The part of the area the box surely fills, in the area's
        /// coordinates: its bounds inset until their corners lie inside its
        /// outline (`innerRect`), and inside its border. The padding past
        /// `inner` extends what lies within it (`reflected`, `strips`).
        let inner: CGRect?

        /// Nil where the run shows no lines.
        init?(_ run: InlineText, in source: NodeView, over container: UIView) {
            let boxes = source.inlineRects(run).map { source.convert($0, to: container) }.filter { !$0.isEmpty }
            guard let outline = UIPreviewParameters(textLineRects: boxes.map { NSValue(cgRect: $0) }).visiblePath?.copy() as? UIBezierPath,
                  !outline.bounds.isEmpty else { return nil }
            let area = outline.bounds
            outline.apply(CGAffineTransform(translationX: -area.minX, y: -area.minY))
            var box: CALayer?, inner: CGRect?
            if let found = ContextMenuHost.box(of: source, below: container), !ContextMenuHost.covers(found.layer, area, in: container.layer) {
                let shape = ContextMenuHost.rounded(found) ?? UIBezierPath(rect: found.bounds)
                // Inside its border, then placed: a rotated box's placed rectangle is no longer upright, and a rectangle inside it is taken
                let placed = UIBezierPath(rect: ContextMenuHost.innerRect(of: shape).inset(by: ContextMenuHost.borders(of: found)))
                placed.apply(ContextMenuHost.placement(of: found, in: container))
                let fills = ContextMenuHost.innerRect(of: placed).intersection(area)
                if !fills.isNull, fills.width >= 1, fills.height >= 1 { (box, inner) = (found.layer, fills.offsetBy(dx: -area.minX, dy: -area.minY)) }
            }
            (self.boxes, self.outline, self.area, self.container, self.box, self.inner) = (boxes, outline, area, container, box, inner)
        }

        /// The lines' place, where a preview of them rises and returns
        var target: UIPreviewTarget { UIPreviewTarget(container: container, center: CGPoint(x: area.midX, y: area.midY)) }

        /// Where what lies under the run is read, in the container's
        /// coordinates: `inner` where there is a box, since the padding past
        /// `inner` extends what lies within it, else the whole area
        var reach: CGRect { inner.map { $0.offsetBy(dx: area.minX, dy: area.minY) } ?? area }

        /// The strips along `inner`'s edges, a pixel thick, each with the part
        /// of the area it stretches over: a side's band, or a corner
        var strips: [(from: CGRect, to: CGRect)] {
            guard let inner else { return [] }
            let bounds = CGRect(origin: .zero, size: area.size), pixel = 1 / container.traitCollection.displayScale
            // Per axis: the span before `inner`, `inner` itself and the span after it, each with the part of `inner` it takes
            typealias Span = (from: ClosedRange<CGFloat>, to: ClosedRange<CGFloat>)
            let columns: [Span] = [(inner.minX...inner.minX + pixel, bounds.minX...inner.minX), (inner.minX...inner.maxX, inner.minX...inner.maxX), (inner.maxX - pixel...inner.maxX, inner.maxX...bounds.maxX)]
            let rows: [Span] = [(inner.minY...inner.minY + pixel, bounds.minY...inner.minY), (inner.minY...inner.maxY, inner.minY...inner.maxY), (inner.maxY - pixel...inner.maxY, inner.maxY...bounds.maxY)]
            func rect(_ x: ClosedRange<CGFloat>, _ y: ClosedRange<CGFloat>) -> CGRect {
                CGRect(x: x.lowerBound, y: y.lowerBound, width: x.upperBound - x.lowerBound, height: y.upperBound - y.lowerBound)
            }
            return columns.enumerated().flatMap { i, column in rows.enumerated().compactMap { j, row in
                guard i != 1 || j != 1, column.to.upperBound > column.to.lowerBound, row.to.upperBound > row.to.lowerBound else { return nil }
                return (rect(column.from, row.from), rect(column.to, row.to))
            } }
        }
    }

    /// `shape`'s bounds inset half a point at a time until their corners lie
    /// inside it
    private static func innerRect(of shape: UIBezierPath) -> CGRect {
        var rect = shape.bounds
        while rect.width > 1, rect.height > 1 {
            let r = rect.insetBy(dx: 0.01, dy: 0.01)
            if [CGPoint(x: r.minX, y: r.minY), CGPoint(x: r.maxX, y: r.minY), CGPoint(x: r.minX, y: r.maxY), CGPoint(x: r.maxX, y: r.maxY)].allSatisfy(shape.contains) { break }
            rect = rect.insetBy(dx: 0.5, dy: 0.5)
        }
        return rect
    }

    /// The box a paragraph's runs sit in, below `container`: the paragraph
    /// itself where it paints a fill or a background image, else the nearest
    /// ancestor that does or that clips what it holds. A node's fill is its
    /// style's, wherever the host paints it (its layer, its corners' fill
    /// layer, its own drawing).
    private static func box(of paragraph: NodeView, below container: UIView) -> UIView? {
        sequence(first: paragraph as UIView, next: { $0.superview }).prefix(while: { $0 !== container }).first { view in
            let node = view as? NodeView
            let paints = node.map { ($0.cgColor("background_color")?.alpha ?? 0) > 0 || $0.style["background_image"] != nil } ?? ((view.layer.backgroundColor?.alpha ?? 0) > 0)
            return paints || view !== paragraph && (view.layer.masksToBounds || node?.overflowClips == true)
        }
    }

    /// The widths of `view`'s border: a node's from its style in whole device
    /// pixels, as its box draws them (`applyBoxLayer`), else its layer's
    private static func borders(of view: UIView) -> UIEdgeInsets {
        guard let node = view as? NodeView else { let w = view.layer.borderWidth; return UIEdgeInsets(top: w, left: w, bottom: w, right: w) }
        let uniform = node.number("border_width")
        func side(_ name: String) -> CGFloat { node.devicePixels(node.number("border_width_" + name, uniform)) }
        return UIEdgeInsets(top: side("top"), left: side("left"), bottom: side("bottom"), right: side("right"))
    }

    /// The transform from `view`'s coordinates to `container`'s: from its
    /// bounds' origin, along its axes as they land in the container.
    private static func placement(of view: UIView, in container: UIView) -> CGAffineTransform {
        let b = view.bounds.origin
        let o = view.convert(b, to: container), x = view.convert(CGPoint(x: b.x + 1, y: b.y), to: container), y = view.convert(CGPoint(x: b.x, y: b.y + 1), to: container)
        return CGAffineTransform(translationX: -b.x, y: -b.y).concatenating(CGAffineTransform(a: x.x - o.x, b: x.y - o.y, c: y.x - o.x, d: y.y - o.y, tx: o.x, ty: o.y))
    }

    /// A run's preview: a picture of its lines (`picture`), cut to their
    /// outline on no color, at the screen's scale, so its target adds no
    /// transform. The cut holds what UIKit shows of the picture before it
    /// applies the outline, which it does a few frames into the lift for one
    /// that is not a rounded rectangle (measured).
    private static func runPreview(_ lines: RunLines, of source: NodeView) -> UITargetedPreview? {
        guard let picture = picture(lines, of: source) else { return nil }
        let cut = CAShapeLayer()
        cut.path = lines.outline.cgPath
        picture.layer.mask = cut
        let parameters = UIPreviewParameters()
        parameters.visiblePath = lines.outline
        parameters.backgroundColor = .clear
        return UITargetedPreview(view: picture, parameters: parameters, target: lines.target)
    }

    /// The run's area as the screen shows it: its own words (`words`, else
    /// the screen's) over its padding (`padding`), else one snapshot of the
    /// screen across all of it, neighboring text included. Either way, the
    /// padding extends the box the run sits in past its edges (`reflected`,
    /// `RunLines.strips`).
    private static func picture(_ lines: RunLines, of source: NodeView) -> RunPicture? {
        let container = lines.container, area = lines.area
        guard let padding = padding(lines, of: source) else {
            guard let screen = container.resizableSnapshotView(from: area, afterScreenUpdates: false, withCapInsets: .zero) else { return nil }
            let strips = lines.strips
            guard let slot = screen.layer.contents, !strips.isEmpty else { return RunPicture(padding: screen) }
            // Past the box, the screen's own picture stretched from its edge: layers
            // sharing its contents, each cut to a pixel's strip. A strip snapshotted
            // on its own is too thin for a blur's radius and shows what lies behind
            // it unblurred, and a reflection would mirror the run's words, which the
            // screen's picture holds (filmed).
            let padding = UIView(frame: CGRect(origin: .zero, size: area.size))
            padding.addSubview(screen)
            for (from, to) in strips {
                let strip = CALayer()
                (strip.contents, strip.frame, strip.magnificationFilter) = (slot, to, .nearest)
                strip.contentsRect = CGRect(x: from.minX / area.width, y: from.minY / area.height, width: from.width / area.width, height: from.height / area.height)
                padding.layer.addSublayer(strip)
            }
            return RunPicture(padding: padding)
        }
        let picture = RunPicture(padding: padding)
        let runArea = lines.boxes.reduce(CGRect.null) { $0.union($1) }
        let only = CAShapeLayer(), path = UIBezierPath()
        for box in lines.boxes { path.append(UIBezierPath(rect: box.offsetBy(dx: -runArea.minX, dy: -runArea.minY))) }
        only.path = path.cgPath
        if let words = words(lines, in: runArea, of: source) {
            words.mask = only
            picture.layer.addSublayer(words)
        } else if let run = container.resizableSnapshotView(from: runArea, afterScreenUpdates: false, withCapInsets: .zero) {
            run.frame = runArea.offsetBy(dx: -area.minX, dy: -area.minY)
            run.layer.mask = only
            picture.addSubview(run)
        }
        return picture
    }

    /// The run's words over `runArea` as the paragraph shows them, with no
    /// pixel copied: layers that show the paragraph's own text raster
    /// (`textRasterLayer`) cut to the run, which the render server composites
    /// as it does the paragraph, at the opacity and in the dynamic range it
    /// shows them in. A one-color raster is that color masked by its coverage
    /// (`TintInk`). Nil where the paragraph shows its words otherwise (a
    /// shadow, an HDR shadow's cast, vibrancy, rolling numerals, a drawing of
    /// its own) or not upright in the container, and the caller takes the
    /// screen's.
    private static func words(_ lines: RunLines, in runArea: CGRect, of source: NodeView) -> CALayer? {
        let space = lines.container.layer
        guard source.layer.contents == nil, source.vibrancyView == nil, let ink = source.textRasterLayer, ink.superlayer != nil, ink.textCast == nil,
              ink.shadowOpacity == 0, ink.sublayers?.isEmpty ?? true,
              ink.filters == nil, ink.compositingFilter == nil, ink.contentsRect == CGRect(x: 0, y: 0, width: 1, height: 1), !ink.bounds.isEmpty else { return nil }
        let tint = (ink as? TintInk)?.backgroundColor
        guard let image = tint == nil ? ink.contents : ink.mask?.contents, tint == nil || ink.mask?.contentsRect == CGRect(x: 0, y: 0, width: 1, height: 1) else { return nil }
        // Upright: the ink's axes land on the container's
        let o = ink.convert(CGPoint.zero, to: space), x = ink.convert(CGPoint(x: 1, y: 0), to: space), y = ink.convert(CGPoint(x: 0, y: 1), to: space)
        guard abs(x.y - o.y) < 0.001, abs(y.x - o.x) < 0.001 else { return nil }
        let shown = space.convert(runArea, to: ink).intersection(ink.bounds)
        guard !shown.isNull, !shown.isEmpty else { return nil }
        let b = ink.bounds, cut = CGRect(x: (shown.minX - b.minX) / b.width, y: (shown.minY - b.minY) / b.height, width: shown.width / b.width, height: shown.height / b.height)
        let words = CALayer(), area = lines.area
        words.frame = runArea.offsetBy(dx: -area.minX, dy: -area.minY)
        words.opacity = sequence(first: ink, next: { $0.superlayer }).prefix { $0 !== space }.reduce(1) { $0 * $1.opacity }
        let face = CALayer()
        face.frame = ink.convert(shown, to: space).offsetBy(dx: -runArea.minX, dy: -runArea.minY)
        takeRange(of: ink, into: face)
        if let tint {
            face.backgroundColor = tint
            let coverage = CALayer()
            coverage.frame = face.bounds
            (coverage.contents, coverage.contentsRect, coverage.contentsScale) = (image, cut, ink.contentsScale)
            face.mask = coverage
        } else {
            (face.contents, face.contentsRect, face.contentsScale) = (image, cut, ink.contentsScale)
        }
        words.addSublayer(face)
        return words
    }

    /// The run's padding: what lies under its paragraph across its area,
    /// without the paragraph's other text. It comes from `underneath`: one flat
    /// color, or copies of the layers there (`copies`), the paragraph's by its
    /// fill alone, with a material shown live between the copies composited
    /// under it and over it. Past the box the run sits in, the copies under
    /// the material are reflected (`reflected`), while the copies over the
    /// material, and a material other than the box's own, stay within
    /// `inner`. Nil where only a snapshot shows what lies there, or the
    /// paragraph paints more than its layer's fill.
    private static func padding(_ lines: RunLines, of source: NodeView) -> UIView? {
        let container = lines.container, area = lines.area, reach = lines.reach
        guard !source.boxDrawn, source.boxFill == nil, source.style["background_image"] == nil, source.layer.borderWidth == 0, let parent = source.superview else { return nil }
        // From the nearest view under the paragraph that covers the reach opaquely, or the box the padding extends: nothing behind it shows
        let root = sequence(first: parent, next: { $0.superview }).first {
            $0.layer.opacity >= 1 && ($0.layer.backgroundColor?.alpha ?? 0) >= 1 && ($0.layer === lines.box || covers($0.layer, reach, in: container.layer))
        } ?? source.window ?? container
        switch underneath(root.layer, across: reach, in: container.layer, exceptFor: source.layer, within: lines.box) {
        case .screen:
            return nil
        case .color(let color):
            let padding = UIView(frame: CGRect(origin: .zero, size: area.size))
            padding.layer.backgroundColor = color
            return padding
        case .layers(let material):
            let padding = UIView(frame: CGRect(origin: .zero, size: area.size))
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            defer { CATransaction.commit() }
            let (under, over) = copies(of: root.layer, across: reach, in: container.layer, paragraph: source.layer, splitAt: material?.layer)
            let within = UIView(frame: lines.inner ?? padding.bounds)
            within.clipsToBounds = true
            // A side's copies placed as the root's layers are, in the area's coordinates less the origin of the view that holds them
            let placement = Self.placement(of: root, in: container).concatenating(CGAffineTransform(translationX: -area.minX, y: -area.minY))
            func placed(_ copy: CALayer, at origin: CGPoint) -> CALayer {
                let layer = CALayer()
                // Its sublayer transform applies about its anchor point
                (layer.anchorPoint, layer.frame) = (.zero, CGRect(x: -origin.x, y: -origin.y, width: area.width, height: area.height))
                layer.sublayerTransform = CATransform3DMakeAffineTransform(placement)
                (copy.anchorPoint, copy.position, copy.transform) = (.zero, root.layer.bounds.origin, CATransform3DIdentity)
                layer.addSublayer(copy)
                return layer
            }
            if let under {
                let below = UIView(frame: padding.bounds), layer = placed(under, at: .zero)
                below.layer.addSublayer(lines.inner.map { reflected(layer, inside: $0) } ?? layer)
                padding.addSubview(below)
            }
            if let material {
                // Live between the copies, it blurs those under it as the render server composites the platter: no
                // snapshot waits for it. The box's own spreads over all of the padding, which it then blurs past the box.
                let live = UIVisualEffectView(effect: material.effect)
                let frame = material.convert(material.bounds, to: container).offsetBy(dx: -area.minX, dy: -area.minY)
                if let inner = lines.inner, frame.insetBy(dx: -0.5, dy: -0.5).contains(inner) {
                    live.frame = padding.bounds
                    padding.addSubview(live)
                } else {
                    live.frame = frame.offsetBy(dx: -within.frame.minX, dy: -within.frame.minY)
                    if #available(iOS 26.0, *) { live.cornerConfiguration = material.cornerConfiguration } else { (live.layer.cornerRadius, live.clipsToBounds) = (material.layer.cornerRadius, true) }
                    within.addSubview(live)
                }
            }
            if let over {
                let above = UIView(frame: within.bounds)
                above.layer.addSublayer(placed(over, at: within.frame.origin))
                within.addSubview(above)
            }
            if !within.subviews.isEmpty { padding.addSubview(within) }
            return padding
        }
    }

    /// Copies of `root` and of what it holds across `area` (in `space`), in
    /// the order the render server composites them: `under` what comes before
    /// `material`, and `over` what comes after it, its content included,
    /// between which the padding shows the material live. A layer whose
    /// children fall on both sides is on both, its paint on the side it comes
    /// in on and its geometry alone (`shell`) on the other. A layer that
    /// reaches no part of the area is copied only to hold what does, and an
    /// effect view only through what it holds.
    private static func copies(of root: CALayer, across area: CGRect, in space: CALayer, paragraph: CALayer, splitAt material: CALayer?) -> (under: CALayer?, over: CALayer?) {
        var past = false
        func add(_ child: CALayer?, to holder: inout CALayer?, for layer: CALayer) {
            guard let child else { return }
            if holder == nil { holder = shell(of: layer) }
            holder?.addSublayer(child)
        }
        // `around` is the area in the superlayer's coordinates, where `frame` holds the layer's bounds
        func walk(_ layer: CALayer, around: CGRect) -> (under: CALayer?, over: CALayer?) {
            guard !layer.isHidden, layer.opacity > 0 else { return (nil, nil) }
            let inside = layer.frame.intersects(around)
            var under: CALayer?, over: CALayer?
            if let view = layer.delegate as? UIVisualEffectView {
                if layer === material { past = true } else if effect(of: view) == .material { return (nil, nil) }
                guard inside else { return (nil, nil) }
                let held = walk(view.contentView.layer, around: space.convert(area, to: layer))
                add(held.under, to: &under, for: layer)
                add(held.over, to: &over, for: layer)
                return (under, over)
            }
            if inside {
                if past { over = replica(of: layer, paragraph: paragraph) } else { under = replica(of: layer, paragraph: paragraph) }
            }
            guard layer !== paragraph, let sublayers = layer.sublayers, !sublayers.isEmpty, inside || !layer.masksToBounds else { return (under, over) }
            let local = space.convert(area, to: layer)
            for sublayer in composited(sublayers) {
                let held = walk(sublayer, around: local)
                add(held.under, to: &under, for: layer)
                add(held.over, to: &over, for: layer)
            }
            return (under, over)
        }
        return walk(root, around: root.superlayer.map { space.convert(area, to: $0) } ?? area)
    }

    /// A layer with `layer`'s geometry, clip, opacity and mask and nothing it
    /// paints, to hold copies of what it holds
    private static func shell(of layer: CALayer) -> CALayer {
        let copy = CALayer()
        (copy.bounds, copy.position, copy.anchorPoint, copy.zPosition, copy.transform, copy.sublayerTransform) = (layer.bounds, layer.position, layer.anchorPoint, layer.zPosition, layer.transform, layer.sublayerTransform)
        (copy.masksToBounds, copy.cornerRadius, copy.maskedCorners, copy.cornerCurve, copy.opacity) = (layer.masksToBounds, layer.cornerRadius, layer.maskedCorners, layer.cornerCurve, layer.opacity)
        copy.mask = layer.mask.flatMap { bitmapsOnly($0) ? whole($0) : nil }
        return copy
    }

    /// A copy of `layer` without what it holds (`valueCopy`), with its
    /// `zPosition`, dynamic range and mask, which the render server composites
    /// as it does the original, no pixel drawn. The paragraph is copied by its
    /// fill alone. Speed before fidelity: what a layer draws some other way is
    /// left out.
    private static func replica(of layer: CALayer, paragraph: CALayer?) -> CALayer {
        if layer === paragraph {
            let copy = shell(of: layer)
            copy.backgroundColor = layer.backgroundColor
            return copy
        }
        let copy = layer.valueCopy()
        copy.zPosition = layer.zPosition
        takeRange(of: layer, into: copy)
        copy.mask = layer.mask.map(whole)
        return copy
    }

    /// A copy of `layer` and all it holds, unpruned: a mask, which lies outside
    /// the tree the area is measured in
    private static func whole(_ layer: CALayer) -> CALayer {
        let copy = replica(of: layer, paragraph: nil)
        for sublayer in layer.sublayers ?? [] where !sublayer.isHidden { copy.addSublayer(whole(sublayer)) }
        return copy
    }

    /// `copy` asks for the dynamic range `layer` asks for, with its headroom
    /// and tone mapping (`applyDynamicRange`, `mapsExtendedRange`)
    private static func takeRange(of layer: CALayer, into copy: CALayer) {
        if #available(iOS 26.0, *) {
            (copy.preferredDynamicRange, copy.contentsHeadroom) = (layer.preferredDynamicRange, layer.contentsHeadroom)
        } else {
            copy.wantsExtendedDynamicRangeContent = layer.wantsExtendedDynamicRangeContent
        }
        if #available(iOS 18.0, *) { copy.toneMapMode = layer.toneMapMode }
    }

    /// `content` with the padding past `inner` mirroring what lies just inside
    /// it, the way reflect padding extends an image: `content` clipped to `inner`,
    /// in a replicator layer per side that shows what it holds and its mirror
    /// image about that side, which falls only past it. The render server
    /// draws the copies.
    private static func reflected(_ content: CALayer, inside inner: CGRect) -> CALayer {
        let bounds = content.frame, clip = CALayer()
        clip.frame = inner
        clip.masksToBounds = true
        content.frame = bounds.offsetBy(dx: -inner.minX, dy: -inner.minY)
        clip.addSublayer(content)
        // A replicator's instance transform applies about its center
        let center = CGPoint(x: bounds.midX, y: bounds.midY)
        var layer: CALayer = clip
        for (edge, vertical, past) in [(inner.minX, false, inner.minX > bounds.minX), (inner.maxX, false, inner.maxX < bounds.maxX),
                                       (inner.minY, true, inner.minY > bounds.minY), (inner.maxY, true, inner.maxY < bounds.maxY)] where past {
            let mirror = CAReplicatorLayer()
            mirror.frame = bounds
            mirror.instanceCount = 2
            mirror.instanceTransform = CATransform3DMakeAffineTransform(vertical
                ? CGAffineTransform(a: 1, b: 0, c: 0, d: -1, tx: 0, ty: 2 * (edge - center.y))
                : CGAffineTransform(a: -1, b: 0, c: 0, d: 1, tx: 2 * (edge - center.x), ty: 0))
            mirror.addSublayer(layer)
            layer = mirror
        }
        return layer
    }

    /// A run's picture (`picture`): its padding, with the run's words over it
    /// unless the padding is the screen's, which holds them. The padding fades
    /// as UIKit returns the picture (`willEndFor`, `fadePadding`), so it lands
    /// as the screen shows it then.
    private final class RunPicture: UIView {
        let padding: UIView
        init(padding: UIView) {
            self.padding = padding
            super.init(frame: CGRect(origin: .zero, size: padding.bounds.size))
            padding.frame = bounds
            addSubview(padding)
        }
        required init?(coder: NSCoder) { nil }

        /// The padding faded: a live material by losing its effect, which
        /// UIKit animates, and the rest by its opacity, though never a view
        /// that holds a live material, since fading one breaks the effect
        /// until the fade ends (`UIVisualEffectView`)
        func fadePadding() { Self.fade(padding) }

        private static func fade(_ view: UIView) {
            if let live = view as? UIVisualEffectView { live.effect = nil; return }
            guard view.subviews.contains(where: holdsMaterial) else { view.alpha = 0; return }
            view.subviews.forEach(fade)
        }

        private static func holdsMaterial(_ view: UIView) -> Bool { view is UIVisualEffectView || view.subviews.contains(where: holdsMaterial) }
    }

    /// What lies under a paragraph across a run's area.
    private enum Underneath {
        /// One flat color: nothing paints there but fills that cover all of it.
        case color(CGColor)
        /// Layers Core Animation draws, which their copies show (`copies`), and
        /// at most one material (a blur or glass), which a live copy shows
        case layers(material: UIVisualEffectView?)
        /// What only a snapshot of the screen shows: something only the render
        /// server draws (`drawnByRenderServer`), vibrancy, a second material,
        /// or contents a copy must not share (`shareable`)
        case screen
    }

    /// What lies under `paragraph` across `area` (in `space`), in one walk of
    /// `root`'s layers in the order they are composited. A layer counts as only its fill where
    /// it is sure to paint nothing else; the color is flat where every fill
    /// there, from the last opaque one that covers the whole area up, covers it
    /// too, at full opacity. The box the padding extends (`box`) counts as
    /// covering the area. Anything else is copied (`copies`), never guessed.
    private static func underneath(_ root: CALayer, across area: CGRect, in space: CALayer, exceptFor paragraph: CALayer, within box: CALayer?) -> Underneath {
        var flat = false, fills: [CGColor] = [], material: UIVisualEffectView?
        func paint(_ layer: CALayer, opacity: Float) {
            if layer !== paragraph, paintsBeyondItsFill(layer) { flat = false; return }
            guard let fill = layer.backgroundColor, fill.alpha > 0 else { return }
            let whole = opacity >= 1 && (layer === box || covers(layer, area, in: space))
            if whole, fill.alpha >= 1 { fills = [fill]; flat = true } else if whole, flat { fills.append(fill) } else { flat = false }
        }
        // Nil where only a snapshot shows something of `layer`'s in the area; else whether any of it lies there.
        // `around` is the area in the superlayer's coordinates, where `frame` holds the layer's bounds.
        func visit(_ layer: CALayer, around: CGRect, opacity: Float) -> Bool? {
            guard !layer.isHidden, layer.opacity > 0 else { return false }
            let alpha = opacity * layer.opacity, inside = layer.frame.intersects(around)
            // An effect view: only what it holds walked, a material shown live (`padding`)
            if let view = layer.delegate as? UIVisualEffectView {
                guard inside else { return false }
                switch effect(of: view) {
                case .material:
                    guard material == nil else { return nil }
                    (material, flat) = (view, false)
                case .vibrancy:
                    return nil
                case .container:
                    break
                }
                return visit(view.contentView.layer, around: space.convert(area, to: layer), opacity: alpha).map { _ in true }
            }
            if inside {
                if drawnByRenderServer(layer) || layer !== paragraph && !shareable(layer) { return nil }
                paint(layer, opacity: alpha)
            }
            if layer === paragraph { return true }
            guard let sublayers = layer.sublayers, !sublayers.isEmpty, inside || !layer.masksToBounds else { return inside }
            var reaches = inside
            let local = space.convert(area, to: layer)
            for sublayer in composited(sublayers) {
                guard let reached = visit(sublayer, around: local, opacity: alpha) else { return nil }
                reaches = reaches || reached
            }
            return reaches
        }
        guard visit(root, around: root.superlayer.map { space.convert(area, to: $0) } ?? area, opacity: 1) != nil else { return .screen }
        if flat, let color = composite(fills) { return .color(color) }
        return .layers(material: material)
    }

    /// What an effect view is to the walk: a material (a blur or glass) the
    /// padding shows live, vibrancy only a snapshot shows, or anything else
    /// (a container of other effect views, such as a glass group or a slot,
    /// or no effect), walked through
    private enum Effect { case material, vibrancy, container }

    private static func effect(of view: UIVisualEffectView) -> Effect {
        if view.effect is UIBlurEffect { return .material }
        if #available(iOS 26.0, *), view.effect is UIGlassEffect { return .material }
        return view.effect is UIVibrancyEffect ? .vibrancy : .container
    }

    /// Whether a copy may share `layer`'s contents and its mask's: each none
    /// or a bitmap. A view's backing store is neither, and a copy on screen
    /// must not share one: on macOS, a committed copy sharing one left the
    /// view blank (`PaintCapture.swift`, `PressLookMac.swift`).
    private static func shareable(_ layer: CALayer) -> Bool {
        isBitmap(layer.contents) && (layer.mask.map(bitmapsOnly) ?? true)
    }

    /// Whether `layer`, its mask and all they hold have no contents but bitmaps
    private static func bitmapsOnly(_ layer: CALayer) -> Bool {
        isBitmap(layer.contents) && (layer.mask.map(bitmapsOnly) ?? true) && (layer.sublayers ?? []).allSatisfy(bitmapsOnly)
    }

    private static func isBitmap(_ contents: Any?) -> Bool { contents.map { CFGetTypeID($0 as CFTypeRef) == CGImage.typeID } ?? true }

    /// `sublayers` in the order Core Animation composites them: by `zPosition`
    /// (a `z-index`'s rank, `PaintOrder`), then as listed
    private static func composited(_ sublayers: [CALayer]) -> [CALayer] {
        sublayers.enumerated().sorted { ($0.element.zPosition, $0.offset) < ($1.element.zPosition, $1.offset) }.map(\.element)
    }

    /// Whether `layer`'s fill covers all of `area` (in `space`): unmasked and
    /// upright there, its bounds hold the area clear of its rounded corners.
    private static func covers(_ layer: CALayer, _ area: CGRect, in space: CALayer) -> Bool {
        let b = layer.bounds, corner = [CGPoint(x: b.minX, y: b.minY), CGPoint(x: b.maxX, y: b.minY), CGPoint(x: b.minX, y: b.maxY)].map { layer.convert($0, to: space) }
        guard layer.mask == nil, abs(corner[0].y - corner[1].y) < 0.01, abs(corner[0].x - corner[2].x) < 0.01 else { return false }
        let bounds = layer.convert(b, to: space), r = layer.cornerRadius
        return bounds.contains(area) && (r == 0 || bounds.insetBy(dx: r, dy: 0).contains(area) || bounds.insetBy(dx: 0, dy: r).contains(area))
    }

    /// Whether only the render server draws `layer`: filters, video, Metal, a
    /// 3D transform, and content another process draws (a web view, an
    /// embedded platform view), which no copy of its properties shows. An
    /// effect view's own filters never get here (`effect(of:)`).
    private static func drawnByRenderServer(_ layer: CALayer) -> Bool {
        if let node = layer.delegate as? NodeView, ["iframe", "native", "video"].contains(node.kind) { return true }
        return layer is CAMetalLayer || layer is AVPlayerLayer || layer is AVSampleBufferDisplayLayer || layer is AVCaptureVideoPreviewLayer
            || layer.filters?.isEmpty == false || layer.backgroundFilters?.isEmpty == false || layer.compositingFilter != nil
            || !CATransform3DIsAffine(layer.transform)
    }

    /// Fills over one another, the first opaque, as one color: each over the
    /// ones before it (source over, in extended sRGB).
    private static func composite(_ fills: [CGColor]) -> CGColor? {
        guard let space = CGColorSpace(name: CGColorSpace.extendedSRGB) else { return nil }
        var rgb: [CGFloat]?
        for fill in fills {
            guard let c = fill.converted(to: space, intent: .defaultIntent, options: nil)?.components, c.count == 4 else { return nil }
            rgb = rgb.map { under in (0..<3).map { c[$0] * c[3] + under[$0] * (1 - c[3]) } } ?? Array(c.prefix(3))
        }
        return rgb.flatMap { CGColor(colorSpace: space, components: $0 + [1]) }
    }

    /// Whether `layer` may paint more than its fill: text, a picture, a drawn
    /// box, a border, a shadow, a mask or a layer that draws itself.
    private static func paintsBeyondItsFill(_ layer: CALayer) -> Bool {
        if layer.borderWidth > 0 || layer.shadowOpacity > 0 || layer.mask != nil { return true }
        if let node = layer.delegate as? NodeView {
            return node.isParagraph || node.boxDrawn || node.kind == "image" || node.kind == "canvas" || node.symbolView != nil || node.style["background_image"] != nil
        }
        return layer.contents != nil || layer is CAShapeLayer || layer is CATextLayer || layer is CAGradientLayer || layer is CAReplicatorLayer || layer is CAEmitterLayer
    }

    /// The view UIKit highlights, lifts and returns a preview over: the view of
    /// the controller that holds `view` (its route's, a sheet's, or the root
    /// controller's in an app with no routes), which the bars stay above.
    /// UIKit's own is the view's superview, and an ancestor that clips, such as
    /// a strip that scrolls sideways, then cuts the preview, its corners and its
    /// shadow as it grows (as it cuts a collection view's cell). The superview
    /// where no controller holds it; nil outside a window.
    private static func container(of view: UIView) -> UIView? {
        guard view.window != nil, let parent = view.superview else { return nil }
        return sequence(first: parent, next: { $0.superview }).first(where: { $0.next is UIViewController }) ?? parent
    }

    /// Where UIKit highlights, lifts and returns `view` itself: over
    /// `container(of:)`, centered on the view, with the view's transform, as
    /// UIKit's own target for a view carries it (`UITargetedPreview(view:parameters:)`),
    /// and each ancestor's scale and rotation; converting the center takes the
    /// translations.
    private static func target(_ view: UIView) -> UIPreviewTarget? {
        guard let container = container(of: view), let parent = view.superview else { return nil }
        var transform = view.transform
        for ancestor in sequence(first: parent, next: { $0.superview }) {
            if ancestor === container { break }
            let t = ancestor.transform
            transform = transform.concatenating(CGAffineTransform(a: t.a, b: t.b, c: t.c, d: t.d, tx: 0, ty: 0))
        }
        transform.tx = 0
        transform.ty = 0
        return UIPreviewTarget(container: container, center: view.convert(CGPoint(x: view.bounds.midX, y: view.bounds.midY), to: container), transform: transform)
    }

    func contextMenuInteraction(_ interaction: UIContextMenuInteraction, configuration: UIContextMenuConfiguration, highlightPreviewForItemWithIdentifier identifier: NSCopying) -> UITargetedPreview? {
        targeted(configuration)
    }
    func contextMenuInteraction(_ interaction: UIContextMenuInteraction, configuration: UIContextMenuConfiguration, dismissalPreviewForItemWithIdentifier identifier: NSCopying) -> UITargetedPreview? {
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
        // A run's picture lands without its padding, which would otherwise show
        // until the completion, about a second after the menu starts closing
        // (filmed and logged), covering the text beside the run or what has
        // changed under it since
        if let picture = owner.runPreview?.view as? RunPicture { animator?.addAnimations { picture.fadePadding() } }
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

// LLP 1021 D2–D4: button menus project to NSMenu, and so does an
// alertdialog popover of the chooser's shape ("The chooser": actions, at most
// one hide-only cancel, text rows as its message), popped up against its
// invoker by its `position-area` (ChooserMac.swift); a picked item presses on
// the next turn, once. Other popovers move their actual subtree into
// the session's top layer, including native editors. The agent uses that
// same painted presentation for every shape.
#if os(macOS)
import AppKit

final class MenuHost: NSObject {
    private(set) weak var presenter: Presenter?
    private final class Entry {
        let popover: NodeView
        weak var source: NodeView?
        weak var parent: NSView?
        weak var previous: NSView?
        var index: Int
        let selection: NSRange?
        let value: String?
        var frame: NSRect
        let layer = PopoverLayer(frame: .zero)
        var menu: NSMenu?
        /// An alertdialog presented as a menu: what each action showed.
        var confirmation: Confirmation?
        init(_ popover: NodeView, source: NodeView, previous: NSView?) {
            self.popover = popover; self.source = source; self.previous = previous
            parent = popover.superview
            index = parent?.subviews.firstIndex(of: popover) ?? 0
            frame = popover.frame; value = popover.props["popover"]
            selection = ((previous as? NSTextField)?.currentEditor() as? NSTextView)?.selectedRange()
                ?? (previous as? NSTextView)?.selectedRange()
        }
    }
    private var entries: [Entry] = []
    /// Confirmations whose chosen item awaits its turn (ChooserMac.swift),
    /// and picked button-menu items: each batch revalidates them, and one
    /// that stops showing what is there is cancelled for good.
    var choosing: [Confirmation] = []
    private var picking: [Pick] = []
    /// Each popover's presentation count: a choice made in one presentation
    /// never dispatches once the popover has been presented again.
    private var presentations: [UInt32: Int] = [:]
    func presentation(of pop: NodeView) -> Int { presentations[pop.id] ?? 0 }
    private var popovers: [UInt32: NodeView] = [:]
    /// LLP 1080.001 D3: an open popover's layer, and the popovers this host
    /// hides while closed or lifts while open.
    func inspectionOwns(_ view: NSView) -> Bool { entries.contains { $0.layer === view } }
    func hides(_ node: NodeView) -> Bool { node.props["popover"] != nil || node.props["semanticTag"] == "dialog" }
    func projects(_ node: NodeView) -> Bool { entries.contains { $0.popover === node } }
    private var pointerDown: (button: Int, ancestor: UInt32?)?
    private var escapeHeld = false
    var presented: [NodeView] { entries.filter { $0.menu == nil }.map(\.popover) }
    init(presenter: Presenter) { self.presenter = presenter }

    func isOpen(_ node: NodeView) -> Bool { entries.contains { $0.popover === node } }
    /// The alertdialog the open menu presents, if one does.
    var presentedConfirmation: Confirmation? { entries.last?.confirmation }
    func owns(_ node: NodeView) -> Bool { entries.contains { $0.popover === node && $0.menu == nil } }
    /// Input inheritance follows the authored tree after top-layer reparenting.
    func parent(of view: NSView) -> NSView? {
        entries.first(where: { $0.popover === view && $0.menu == nil })?.parent ?? view.superview
    }
    func contains(_ ancestor: NSView, _ view: NSView) -> Bool {
        var next: NSView? = view
        while let current = next {
            if current === ancestor { return true }
            next = parent(of: current)
        }
        return false
    }
    /// HTML's sequential focus scope follows the invoker, even when the
    /// popover was authored elsewhere. Never add a focus trap here.
    func following(_ source: NodeView) -> [NodeView] {
        entries.filter { $0.source === source && $0.menu == nil }.map(\.popover)
    }
    func live(_ node: NodeView) -> Bool { presenter?.views[node.id] === node }
    private func connected(_ view: NSView) -> Bool {
        guard let presenter else { return false }
        return contains(presenter.root, view) || presenter.dialogs.presented.contains { contains($0, view) }
    }
    func hidden(_ view: NSView) -> Bool {
        sequence(first: view, next: { self.parent(of: $0) }).contains {
            $0.isHidden || ($0 as? NodeView)?.style["display"]?.string == "none"
        }
    }
    private func focusOwner(_ window: NSWindow) -> NSView? {
        guard let view = window.firstResponder as? NSView else { return nil }
        return (view as? NSTextView).flatMap { $0.isFieldEditor ? $0.delegate as? NSView : nil } ?? view
    }
    private func refresh() {
        guard let presenter else { return }
        presenter.navigation.refreshInputGates(Array(presenter.views.values))
        presenter.topLayerChanged()
        presenter.syncKeyViewLoop()
        presenter.syncAccessibility()
        presenter.shortcuts.sync()
    }
    /// Capture identity before app code runs; a replacement with the same id
    /// must never receive an old invoker's deferred presentation.
    func command(_ source: NodeView, fromNativeMenu: Bool = false) -> (() -> Void)? {
        guard let presenter, source.isButton, !source.disabled, !source.inert,
              fromNativeMenu || !source.isHiddenOrHasHiddenAncestor || presenter.toolbar.contains(source),
              let name = source.props["popovertarget"],
              let pop = presenter.carrying("popover").first(where: { $0.props["id"] == name }) else { return nil }
        let action = source.props["popovertargetaction"] ?? "toggle"
        return { [weak self, weak source, weak pop] in
            guard let self, let source, let pop, self.live(source), self.live(pop),
                  source.props["popovertarget"] == name, pop.props["id"] == name,
                  (source.props["popovertargetaction"] ?? "toggle") == action,
                  !source.inert, !source.disabled else { return }
            if action == "hide" || (action != "show" && self.isOpen(pop)) { self.close(pop) }
            else { self.show(pop, from: source) }
        }
    }
    func opens(_ source: NodeView, _ pop: NodeView) -> Bool {
        source.props["popovertarget"] == pop.props["id"] && source.props["popovertargetaction"] != "hide"
    }
    /// `source` can still open `pop` in this presenter's window: live,
    /// enabled, not inert, shown (or a toolbar's), and pointing at it.
    func invokes(_ source: NodeView, _ pop: NodeView) -> Bool {
        guard let presenter, let window = presenter.viewport.window else { return false }
        return live(source) && live(pop) && source.window === window && !source.disabled && !source.inert
            && opens(source, pop) && (!hidden(source) || presenter.toolbar.contains(source))
    }
    /// A choice awaiting its turn is checked as each batch lands and as a
    /// popover is presented again: one a change has invalidated is
    /// cancelled for good, even if a later batch undoes the change.
    private func revalidate() {
        for owner in choosing where !valid(owner) { owner.cancel() }
        choosing.removeAll { $0.chosen == nil }
        for pick in picking where !valid(pick) { pick.cancelled = true }
        picking.removeAll { $0.cancelled }
    }
    func show(_ pop: NodeView, from source: NodeView) {
        guard let presenter, !isOpen(pop), live(pop), live(source), pop.props["popover"] != nil,
              connected(pop), connected(source), !pop.inert, !source.inert, !source.disabled,
              pop.style["display"]?.string != "none", let window = presenter.viewport.window,
              !hidden(source) || presenter.toolbar.contains(source) else { return }
        // Auto popovers may nest through the authored tree or their invoker;
        // opening an unrelated one dismisses the existing branch.
        let ancestor = entries.last { contains($0.popover, source) || contains($0.popover, pop) }
        while let last = entries.last, last !== ancestor { close(last.popover, restoreFocus: false) }
        guard live(pop), live(source), connected(pop), connected(source) else { return }
        let entry = Entry(pop, source: source, previous: focusOwner(window))
        presentations[pop.id, default: 0] += 1
        revalidate()
        if !ExactEnv.agentMode {
            if isConfirmation(pop) {
                // A shape the menu cannot present is said, and stays painted.
                if let owner = confirmation(of: pop, from: source) { entry.confirmation = owner; entry.menu = menu(of: owner) }
            } else if isMenuShaped(pop) { entry.menu = menu(of: pop, from: source) }
        }
        entries.append(entry)
        if let menu = entry.menu {
            // Leave native tracking until the click and its app batch finish.
            DispatchQueue.main.async { [weak self, weak source, weak entry] in
                guard let self, let entry, self.entries.contains(where: { $0 === entry }) else { return }
                guard let source, self.live(source), self.live(entry.popover), source.window === window,
                      !source.inert, !source.disabled, entry.confirmation.map(self.valid) ?? true
                else { self.close(entry.popover); return }
                menu.popUp(positioning: nil, at: self.popUpPoint(menu, entry.popover, in: source), in: source)
                // Escape or a click outside chose nothing. A chosen item has
                // only been recorded: it presses on the next turn, after this
                // stack and AppKit's tracking of `source` have unwound.
                self.close(entry.popover, cancelling: false)
                if let owner = entry.confirmation { DispatchQueue.main.async { owner.finished = true } }
            }
        } else {
            entry.layer.autoresizingMask = [.width, .height]
            presenter.viewport.addSubview(entry.layer)
            entry.layer.setPaintForeground()
            entry.layer.addSubview(pop)
            pop.isHidden = false
            layout()
            refresh()
            // A plain popover leaves focus alone unless it has autofocus.
            if let target = presenter.carrying("autofocus").first(where: {
                contains(pop, $0) && $0.props["autofocus"] == "true" && !$0.inert
                    && !$0.disabled && !$0.isHiddenOrHasHiddenAncestor
            }) { window.makeFirstResponder(presenter.keyView(of: target)) }
        }
    }
    /// LLP 1021 §5.1: a context menu, its popover's menu rows as an NSMenu
    /// popped up at the click, after the node's own `contextmenu` has run
    /// (both fire), on the next turn as a button menu's. Its preview row is
    /// not an item: a Mac's context menu has no preview. Under the agent the
    /// popover opens painted, anchored to the node (D4).
    func context(_ source: NodeView, at point: NSPoint) {
        guard let name = source.props["contextPopover"] else { return }
        DispatchQueue.main.async { [weak self, weak source] in
            guard let self, let presenter = self.presenter, let source, self.live(source), source.props["contextPopover"] == name,
                  !source.disabled, !source.inert, !self.hidden(source), source.window === presenter.viewport.window else { return }
            guard let pop = presenter.carrying("popover").first(where: { $0.props["id"] == name && !self.isConfirmation($0) }) else {
                presenter.session?.log("context menu \(name) refused: no popover has that id")
                return
            }
            while let last = self.entries.last { self.close(last.popover, restoreFocus: false) }
            if ExactEnv.agentMode { self.show(pop, from: source); return }
            // A new presentation only if it has an item: it retires picks of the last.
            guard pop.container.subviews.contains(where: { ($0 as? NodeView).map { $0.isButton && $0.props["contextPreview"] != "true" } ?? false }) else { return }
            self.presentations[pop.id, default: 0] += 1
            self.revalidate()
            let menu = self.menu(of: pop)
            menu.popUp(positioning: nil, at: point, in: source)
        }
    }
    /// Where a menu presenting `pop` pops up in `source`: its top-left, by
    /// the popover's `position-area` and margins as its painted box would
    /// sit, the menu's own size the box, clamped to the viewport (§5).
    func popUpPoint(_ menu: NSMenu, _ pop: NodeView, in source: NodeView) -> NSPoint {
        let bounds = presenter.map { source.convert($0.viewport.bounds, from: $0.viewport) } ?? .infinite
        return PositionArea.origin(PositionArea.of(pop), anchor: source.bounds, size: menu.size,
                                   margins: PositionArea.margins(of: pop), in: bounds)
    }
    func close(_ pop: NodeView, restoreFocus: Bool = true, cancelling: Bool = true) {
        guard let index = entries.firstIndex(where: { $0.popover === pop }) else { return }
        while entries.count > index + 1, let last = entries.last { close(last.popover, restoreFocus: restoreFocus) }
        let entry = entries[index], window = presenter?.viewport.window
        let hadFocus = window.flatMap(focusOwner).map { contains(pop, $0) } ?? false
        entries.remove(at: index)
        // Reset, unmount, a changed row: the menu ends and nothing it showed dispatches.
        if cancelling { entry.confirmation?.cancel() }
        entry.menu?.cancelTracking()
        pop.isHidden = pop.props["popover"] != nil || pop.style["display"]?.string == "none"
        if entry.menu == nil {
            pop.removeFromSuperview()
            entry.layer.removeFromSuperview()
            if live(pop), let parent = entry.parent {
                let siblings = parent.subviews
                if entry.index < siblings.count { parent.addSubview(pop, positioned: .below, relativeTo: siblings[entry.index]) }
                else { parent.addSubview(pop) }
                pop.frame = entry.frame
            }
            refresh()
        }
        if hadFocus, let window {
            if restoreFocus, let previous = entry.previous, previous.window === window,
               !previous.isHiddenOrHasHiddenAncestor, presenter?.dialogs.blocks(previous) != true,
               !((previous as? NodeView)?.inert ?? false) {
                window.makeFirstResponder(previous)
                if let range = entry.selection, let editor = window.firstResponder as? NSTextView { editor.setSelectedRange(range) }
            } else { window.makeFirstResponder(presenter?.viewport) }
        }
    }
    func reset() {
        while let last = entries.last { close(last.popover, restoreFocus: false) }
        choosing.forEach { $0.cancel() }; choosing.removeAll()
        picking.forEach { $0.cancelled = true }; picking.removeAll()
        popovers.removeAll(); presentations.removeAll()
        pointerDown = nil; escapeHeld = false
    }
    func children(_ parent: NSView, _ wanted: [NodeView]) {
        for entry in entries {
            let index = wanted.firstIndex(where: { $0 === entry.popover })
            if (entry.parent === parent) != (index != nil) { close(entry.popover) }
            else if let index { entry.index = index }
        }
    }
    func frame(_ node: NodeView, _ frame: NSRect) -> Bool {
        guard let entry = entries.first(where: { $0.popover === node && $0.menu == nil }) else { return false }
        entry.frame = frame
        layout()
        return true
    }
    func sync() {
        guard let presenter else { return }
        for entry in entries {
            guard let source = entry.source, live(source), live(entry.popover), connected(source), connected(entry.popover),
                  source.window === presenter.viewport.window, presenter.viewport.window != nil,
                  entry.value == entry.popover.props["popover"], !entry.popover.inert,
                  entry.parent.map(hidden) != true,
                  entry.popover.style["display"]?.string != "none",
                  !hidden(source) || presenter.toolbar.contains(source),
                  entry.menu == nil || (entry.confirmation.map(valid) ?? isMenuShaped(entry.popover))
            else { close(entry.popover); continue }
        }
        revalidate()
        var changed = false
        let current = presenter.carrying("popover")
        for pop in popovers.values where live(pop) && pop.props["popover"] == nil {
            let hidden = pop.style["display"]?.string == "none" || pop.routeInert
                || (pop.props["semanticTag"] == "dialog" && !presenter.dialogs.owns(pop))
            if pop.isHidden != hidden { pop.isHidden = hidden; changed = true }
        }
        popovers = Dictionary(uniqueKeysWithValues: current.map { ($0.id, $0) })
        for pop in current {
            let hidden = !owns(pop) || pop.style["display"]?.string == "none"
            if pop.isHidden != hidden { pop.isHidden = hidden; changed = true }
        }
        if changed { refresh() }
        layout()
    }
    func layout() {
        guard let presenter else { return }
        for entry in entries where entry.menu == nil {
            guard let source = entry.source else { continue }
            entry.layer.frame = presenter.viewport.bounds
            let anchor = source.convert(source.bounds, to: entry.layer)
            var box = entry.frame
            box.origin = PositionArea.origin(PositionArea.of(entry.popover), anchor: anchor, size: box.size,
                                             margins: PositionArea.margins(of: entry.popover), in: entry.layer.bounds)
            if entry.popover.frame != box { entry.popover.frame = box }
        }
    }
    /// Shared by the local event monitor and direct agent event delivery.
    func key(_ event: NSEvent) -> Bool {
        guard event.keyCode == 53, let window = presenter?.viewport.window, event.window === window else { return false }
        if escapeHeld {
            if event.type == .keyUp { escapeHeld = false }
            return true
        }
        guard let owner = focusOwner(window), let viewport = presenter?.viewport,
              owner === viewport || owner.isDescendant(of: viewport) else { return false }
        guard let last = entries.last, (window.firstResponder as? NSTextInputClient)?.hasMarkedText() != true else { return false }
        if event.type == .keyDown, !event.isARepeat {
            escapeHeld = true
            close(last.popover)
        }
        return true
    }
    func pointer(_ event: NSEvent) {
        guard !entries.isEmpty, event.window === presenter?.viewport.window,
              [.leftMouseDown, .leftMouseUp, .rightMouseDown, .rightMouseUp].contains(event.type),
              let content = event.window?.contentView else { return }
        let hit = content.hitTest(content.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow)
        let ancestor = entries.last(where: { entry in
            guard let hit else { return false }
            if contains(entry.popover, hit) { return true }
            return sequence(first: hit, next: { self.parent(of: $0) }).contains {
                ($0 as? NodeView)?.props["popovertarget"] == entry.popover.props["id"]
            }
        })?.popover.id
        if event.type == .leftMouseDown || event.type == .rightMouseDown {
            pointerDown = (event.buttonNumber, ancestor)
        } else {
            defer { pointerDown = nil }
            guard let down = pointerDown, down.button == event.buttonNumber, down.ancestor == ancestor else { return }
            while let last = entries.last, last.popover.id != ancestor { close(last.popover, restoreFocus: false) }
        }
    }
    var observation: [String: Any]? {
        entries.last.map { entry in
            var o: [String: Any] = ["popover": Int(entry.popover.id), "source": entry.source.map { Int($0.id) as Any } ?? NSNull(), "phase": "open"]
            if let owner = entry.confirmation { o["kind"] = "confirmation"; o["actions"] = owner.actions.count }
            return o
        }
    }

    func isMenuShaped(_ pop: NodeView) -> Bool {
        let rows = pop.container.subviews.compactMap { $0 as? NodeView }.filter { $0.props["contextPreview"] != "true" }
        func textOnly(_ node: NodeView) -> Bool {
            node.kind == "text" && node.container.subviews.compactMap { $0 as? NodeView }.allSatisfy(textOnly)
        }
        return !rows.isEmpty && rows.allSatisfy { row in
            if row.props["semanticTag"] == "hr" { return true }
            guard row.isButton else { return false }
            if row.isNativeButton { return true }
            if let face = row.face, face.fits, !face.raster { return true }
            // Text, and at most one image: the item's title and its image.
            let content = row.container.subviews.compactMap { $0 as? NodeView }
            let images = content.filter { $0.kind == "image" && $0.container.subviews.isEmpty }
            return images.count <= 1 && content.allSatisfy { textOnly($0) || images.contains($0) }
        }
    }
    /// `pop`'s button menu, opened from `source` (nil only when a test
    /// builds one with no invoker).
    func menu(of pop: NodeView, from source: NodeView? = nil) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false
        let once = Picked()
        // The popover's `aria-label` titles the menu ("Open location in").
        if let heading = pop.props["accessibilityLabel"], !heading.isEmpty { menu.addItem(.sectionHeader(title: heading)) }
        for case let row as NodeView in pop.container.subviews {
            if row.props["contextPreview"] == "true" { continue } // a context menu's preview (§5.1)
            if row.props["semanticTag"] == "hr" { menu.addItem(.separator()); continue }
            guard row.isButton else { continue }
            let item = NSMenuItem(title: title(of: row), action: #selector(pick(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = Pick(row, in: pop, from: source, presentation: presentation(of: pop),
                                          title: item.title, once: once)
            item.state = row.props["accessibilityChecked"] == "true" ? .on : .off
            // As a chooser's: a hidden or inert row is shown, never chosen.
            item.isEnabled = !row.disabled && !row.inert && shown(row, in: pop)
            item.image = image(of: row)
            menu.addItem(item)
        }
        return menu
    }
    /// One menu's items share this: the first item taken is its only one.
    private final class Picked { var taken = false }
    /// An item's row as the menu showed it, in which presentation, opened
    /// from which invoker.
    private final class Pick: NSObject {
        weak var row: NodeView?
        weak var pop: NodeView?
        weak var source: NodeView?
        let invoked: Bool
        let presentation: Int
        let title: String
        let once: Picked
        var cancelled = false
        init(_ row: NodeView, in pop: NodeView, from source: NodeView?, presentation: Int, title: String, once: Picked) {
            self.row = row; self.pop = pop; self.source = source; invoked = source != nil
            self.presentation = presentation; self.title = title; self.once = once
        }
    }
    /// Still the row the menu showed: live, in its popover, enabled, shown,
    /// under the same title (a reused id is not that row); its invoker still
    /// opening that popover, which has not been presented again since.
    private func valid(_ pick: Pick) -> Bool {
        guard !pick.cancelled, let row = pick.row, let pop = pick.pop, live(row), live(pop),
              row.isDescendant(of: pop), row.isButton, !row.disabled, !row.inert,
              shown(row, in: pop), title(of: row) == pick.title,
              presentation(of: pop) == pick.presentation else { return false }
        guard pick.invoked else { return true }
        return pick.source.map { invokes($0, pop) } ?? false
    }
    /// An item picked: recorded now, its row pressed on the next main-queue
    /// turn, once, as a chooser's is (ChooserMac.swift): AppKit sends the
    /// action inside `popUp`, still tracking the menu in the invoker, and
    /// the press's batch may unmount that invoker.
    @objc private func pick(_ sender: NSMenuItem) {
        guard let pick = sender.representedObject as? Pick, !pick.once.taken else { return }
        pick.once.taken = true
        picking.append(pick)
        DispatchQueue.main.async { [weak self, pick] in
            guard let self else { return }
            self.picking.removeAll { $0 === pick }
            guard self.valid(pick), let row = pick.row else { return }
            pick.cancelled = true
            self.presenter?.press(row.id, fromNativeMenu: true)
        }
    }
    /// Not hidden by the page — `node`'s or an ancestor's `display: none`
    /// or hiding — though `pop` is hidden in place while a menu presents it.
    func shown(_ node: NodeView, in pop: NodeView) -> Bool {
        !sequence(first: node as NSView, next: { self.parent(of: $0) }).contains { view in
            (view as? NodeView)?.style["display"]?.string == "none" || (view.isHidden && view !== pop)
        }
    }
    func title(of v: NodeView) -> String {
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

/// Transparent space passes through, so light dismiss never swallows the
/// outside click or makes the rest of the page modal.
private final class PopoverLayer: NSView {
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = raisedHit(super.hitTest(point), point)
        return hit === self ? nil : hit
    }
}
#endif

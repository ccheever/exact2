// LLP 1021 D2–D4: button menus project to NSMenu, and so does an
// alertdialog popover of the chooser's shape ("The chooser": actions, at most
// one hide-only cancel, text rows as its message), anchored below its
// invoker (ChooserMac.swift). Other popovers move their actual subtree into
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
    private func hidden(_ view: NSView) -> Bool {
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
        if !ExactEnv.agentMode {
            if isConfirmation(pop) {
                // A shape the menu cannot present is said, and stays painted.
                if let owner = confirmation(of: pop, from: source) { entry.confirmation = owner; entry.menu = menu(of: owner) }
            } else if isMenuShaped(pop) { entry.menu = menu(of: pop) }
        }
        entries.append(entry)
        if let menu = entry.menu {
            // Leave native tracking until the click and its app batch finish.
            DispatchQueue.main.async { [weak self, weak source, weak entry] in
                guard let self, let entry, self.entries.contains(where: { $0 === entry }) else { return }
                guard let source, self.live(source), self.live(entry.popover), source.window === window,
                      !source.inert, !source.disabled, entry.confirmation.map(self.valid) ?? true
                else { self.close(entry.popover); return }
                menu.popUp(positioning: nil, at: NSPoint(x: 0, y: source.bounds.height + 2), in: source)
                // Escape or a click outside chose nothing. A chosen item's
                // action has been sent by the next turn; none is taken after.
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
    func close(_ pop: NodeView, restoreFocus: Bool = true, cancelling: Bool = true) {
        guard let index = entries.firstIndex(where: { $0.popover === pop }) else { return }
        while entries.count > index + 1, let last = entries.last { close(last.popover, restoreFocus: restoreFocus) }
        let entry = entries[index], window = presenter?.viewport.window
        let hadFocus = window.flatMap(focusOwner).map { contains(pop, $0) } ?? false
        entries.remove(at: index)
        // Reset, unmount, a changed row: the menu ends and nothing it showed dispatches.
        if cancelling { entry.confirmation?.finished = true }
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
        popovers.removeAll()
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
            box.origin.x = max(0, min(anchor.minX, entry.layer.bounds.width - box.width))
            box.origin.y = max(0, min(anchor.maxY, entry.layer.bounds.height - box.height))
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
        let rows = pop.container.subviews.compactMap { $0 as? NodeView }
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
    func menu(of pop: NodeView) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false
        // The popover's `aria-label` titles the menu ("Open location in").
        if let heading = pop.props["accessibilityLabel"], !heading.isEmpty { menu.addItem(.sectionHeader(title: heading)) }
        for case let row as NodeView in pop.container.subviews {
            if row.props["semanticTag"] == "hr" { menu.addItem(.separator()); continue }
            guard row.isButton else { continue }
            let item = NSMenuItem(title: title(of: row), action: #selector(pick(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = NSNumber(value: row.id)
            item.state = row.props["accessibilityChecked"] == "true" ? .on : .off
            item.isEnabled = !row.disabled
            item.image = image(of: row)
            menu.addItem(item)
        }
        return menu
    }
    @objc private func pick(_ sender: NSMenuItem) {
        if let id = (sender.representedObject as? NSNumber)?.uint32Value { presenter?.press(id, fromNativeMenu: true) }
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

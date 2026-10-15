// LLP 1021 D2–D4: button menus project to NSMenu, and so does an
// alertdialog popover of the chooser's shape ("The chooser": actions, at most
// one hide-only cancel, text rows as its message), popped up against its
// invoker by its `position-area` (ChooserMac.swift); a picked item presses on
// the next turn, once. One in the alert shape (text, a cancel, at most three
// buttons) is `NSAlert` instead, under the agent too (LLP 1115.000 D7.6,
// AlertsMac.swift). Any other popover is an `NSPopover` holding its actual
// subtree, native editors included (LLP 1115.002 D7.3, PopoversMac.swift).
// The driver's own chrome paints it in the session's top layer instead, as
// it was, and paints a menu's shape there under either chrome.
#if os(macOS)
import AppKit

final class AccessibleMenuItem: NSMenuItem {
    weak var owner: NodeView?
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        if attribute == .enabled, owner?.ariaUnavailable == true { return false }
        return super.accessibilityAttributeValue(attribute)
    }
}

final class MenuHost: NSObject {
    private(set) weak var presenter: Presenter?
    final class Entry {
        let popover: NodeView
        weak var source: NodeView?
        let sourced: Bool
        /// The run of `source` that opened it as its context menu (X77): its anchor.
        let run: UInt32?
        weak var parent: NSView?
        weak var previous: NSView?
        var index: Int
        let selection: NSRange?
        let value: String?
        var frame: NSRect
        var placement = PositionArea.History()
        let layer = PopoverLayer(frame: .zero)
        var menu: NSMenu?
        /// AppKit's popover, while one shows it (PopoversMac.swift).
        var native: NativePopover?
        var hasFallbacks: Bool
        var focusedItem: NSMenuItem?
        var tracking = false
        /// An alertdialog presented as a menu: what each action showed.
        var confirmation: Confirmation?
        init(_ popover: NodeView, source: NodeView?, previous: NSView?, run: UInt32?) {
            self.popover = popover; self.source = source; sourced = source != nil; self.previous = previous; self.run = run
            parent = popover.superview
            index = parent?.subviews.firstIndex(of: popover) ?? 0
            frame = popover.frame; value = popover.props["popover"]
            hasFallbacks = popover.style["position_try_fallbacks"] != nil
            selection = ((previous as? NSTextField)?.currentEditor() as? NSTextView)?.selectedRange()
                ?? (previous as? NSTextView)?.selectedRange()
        }
    }
    private(set) var entries: [Entry] = []
    /// Painted in the page's top layer, not AppKit's popover: the driver's
    /// own chrome, and a test of that presentation.
    var painted = ExactEnv.authoredChrome
    /// Popovers AppKit is still taking away (PopoversMac.swift).
    var leaving: [NativePopover] = []
    /// The click with which AppKit closed an invoker's own popover.
    var dismissedByClick: (source: ObjectIdentifier, click: Int)?
    /// Confirmations whose chosen item awaits its turn (ChooserMac.swift),
    /// and picked button-menu items: each batch revalidates them, and one
    /// that stops showing what is there is cancelled for good.
    var choosing: [Confirmation] = []
    private var picking: [Pick] = []
    private var nativeTracking = 0
    private var afterTracking: [() -> Void] = []
    /// Main-queue work continues during native tracking; only the chosen
    /// item's action waits until popUp has unwound, then runs next turn.
    func afterNativeTracking(_ work: @escaping () -> Void) {
        if nativeTracking > 0 { afterTracking.append(work) }
        else { DispatchQueue.main.async(execute: work) }
    }
    private func track(_ menu: NSMenu, positioning item: NSMenuItem? = nil, at point: NSPoint, in source: NodeView) {
        nativeTracking += 1
        menu.popUp(positioning: item, at: point, in: source)
        nativeTracking -= 1
        if nativeTracking == 0 {
            let work = afterTracking; afterTracking.removeAll()
            for action in work { DispatchQueue.main.async(execute: action) }
        }
    }
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
    /// Modal accessibility includes an external popover opened from the
    /// modal, and popovers opened from that popover, as the focus scope does.
    func presented(within ancestor: NSView) -> [NodeView] {
        func belongs(_ view: NSView, seen: inout Set<ObjectIdentifier>) -> Bool {
            if contains(ancestor, view) { return true }
            for entry in entries where contains(entry.popover, view) {
                guard seen.insert(ObjectIdentifier(entry.popover)).inserted, let source = entry.source else { continue }
                if belongs(source, seen: &seen) { return true }
            }
            return false
        }
        return presented.filter { view in
            var seen: Set<ObjectIdentifier> = []
            return belongs(view, seen: &seen)
        }
    }
    init(presenter: Presenter) { self.presenter = presenter }

    func isOpen(_ node: NodeView) -> Bool { entries.contains { $0.popover === node } }
    /// Open as a popover, here or as the alert a confirmation becomes
    func showsAsPopover(_ node: NodeView) -> Bool { isOpen(node) || presenter?.alerts.isOpenAsPopover(node) == true }
    /// Open by `show-modal`, which HTML's popover commands leave open
    private func showsModally(_ node: NodeView) -> Bool { presenter?.alerts.isOpen(node) == true && !showsAsPopover(node) }
    /// `entry` out of the open popovers while `body` runs (PopoversMac.swift).
    func apart(_ entry: Entry, _ body: () -> Void) {
        guard let index = entries.firstIndex(where: { $0 === entry }) else { return body() }
        entries.remove(at: index)
        body()
        entries.insert(entry, at: min(index, entries.count))
    }
    /// A focus command in the opening action chooses AppKit's initial row.
    /// The authored subtree stays hidden while NSMenu presents its items.
    func focus(_ node: NodeView) -> Bool {
        guard let entry = entries.last(where: { $0.menu != nil && contains($0.popover, node) }),
              let item = entry.menu?.items.first(where: { ($0 as? AccessibleMenuItem)?.owner === node }) else { return false }
        guard !entry.tracking else {
            presenter?.session?.log("focus refused: NSMenu accepts an initial row in the opening action, not a new highlight during tracking")
            return true
        }
        if item.isEnabled { entry.focusedItem = item }
        return true
    }
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
        // One in a window of its own is a Tab loop of its own, as AppKit's popover is.
        entries.filter { $0.source === source && $0.menu == nil && $0.native == nil }.map(\.popover)
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
    func focusOwner(_ window: NSWindow) -> NSView? {
        guard let view = window.firstResponder as? NSView else { return nil }
        return (view as? NSTextView).flatMap { $0.isFieldEditor ? $0.delegate as? NSView : nil } ?? view
    }
    func refresh() {
        guard let presenter else { return }
        presenter.navigation.refreshInputGates(Array(presenter.views.values))
        presenter.topLayerChanged()
        presenter.syncKeyViewLoop()
        presenter.syncAccessibility()
        presenter.shortcuts.sync()
    }
    /// The popover `source` shows, hides or toggles, as HTML resolves it
    /// (`Invocation`); nil for a dialog's commands, which `DialogHost` runs.
    private func popoverInvocation(_ source: NodeView) -> Invocation? {
        guard let invocation = presenter?.invocation(of: source),
              [.show, .hide, .toggle].contains(invocation.action) else { return nil }
        return invocation
    }
    /// A native invoker anchors to its actual button frame, including AppKit's
    /// alignment insets, rather than the node's layout box (D13); a run of
    /// a paragraph, to its lines' bounds, as an inline element's box is.
    func anchor(_ source: NodeView, in view: NSView, run: UInt32? = nil) -> NSRect {
        // A button the window's bar shows has no box in the page (LLP 1115.002 D2).
        if let bar = presenter?.toolbar.anchor(of: source, in: view) { return bar }
        if let run, let text = presenter?.inlineText(run) {
            let lines = source.inlineRects(text).reduce(CGRect.null) { $0.union($1) }
            if !lines.isNull { return source.convert(lines, to: view) }
        }
        if let button = presenter?.controls.controls[source.id] as? NativeButtonMac {
            return button.convert(button.bounds, to: view)
        }
        return source.convert(source.bounds, to: view)
    }
    /// What activating `source` does once its own handler has run: the
    /// surviving button's invocation then, as HTML resolves the target after
    /// the event's listeners (a handler may retarget it, or give it one).
    func command(_ source: NodeView, fromNativeMenu: Bool = false) -> (() -> Void)? {
        guard let presenter, source.isButton, !source.disabled, !source.inert,
              fromNativeMenu || !source.isHiddenOrHasHiddenAncestor || presenter.toolbar.contains(source),
              popoverInvocation(source) != nil || source.handlers.contains("press") else { return nil }
        return { [weak self, weak source] in
            guard let self, let source, self.live(source), !source.inert, !source.disabled,
                  let invocation = self.popoverInvocation(source), self.live(invocation.target),
                  !self.showsModally(invocation.target) else { return }
            let pop = invocation.target
            if invocation.action == .hide || (invocation.action == .toggle && self.isOpen(pop)) { self.close(pop) }
            // AppKit closed it with this very click on its invoker: a toggle leaves it closed.
            else if invocation.action == .show || !self.closedByThisClick(source) { self.show(pop, from: source) }
        }
    }
    func opens(_ source: NodeView, _ pop: NodeView) -> Bool {
        popoverInvocation(source).map { $0.opens && $0.target === pop } ?? false
    }
    /// `source` can still open `pop` in this presenter's window: live,
    /// enabled, not inert, shown (or a toolbar's), and, for an invoker, pointing at it.
    func invokes(_ source: NodeView, _ pop: NodeView, requiringTarget: Bool = true) -> Bool {
        guard let presenter, presenter.viewport.window != nil else { return false }
        return live(source) && live(pop) && presenter.hosts(source.window) && !source.disabled && !source.inert
            && (!requiringTarget || opens(source, pop)) && (!hidden(source) || presenter.toolbar.contains(source))
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
    func action(_ name: String, _ args: [Any]) {
        guard let presenter, let id = args.first as? String,
              let pop = presenter.carrying("popover").first(where: { $0.props["id"] == id }) else {
            self.presenter?.session?.log("\(name) refused: no popover has that id"); return
        }
        let sourceId = args.count > 1 ? args[1] as? String : nil
        let source = sourceId.flatMap { id in presenter.views.values.first { $0.props["id"] == id } }
        if sourceId != nil && source == nil { presenter.session?.log("\(name) refused: no source has that id"); return }
        if showsModally(pop) { presenter.session?.log("\(name) ignored: \(id) is open as a modal dialog"); return }
        let open = showsAsPopover(pop)
        if name == "hidePopover" || name == "togglePopover" && open { close(pop) }
        else if !open { show(pop, from: source) }
    }
    func show(_ pop: NodeView, from source: NodeView?, run: UInt32? = nil) {
        guard let presenter, !isOpen(pop), live(pop), source.map(live) ?? true, pop.props["popover"] != nil,
              connected(pop), source.map(connected) ?? true, !pop.inert, source.map({ !$0.inert && !$0.disabled }) ?? true,
              pop.style["display"]?.string != "none", let page = presenter.viewport.window,
              source.map({ !hidden($0) || presenter.toolbar.contains($0) }) ?? true else { return }
        let window = source?.window.flatMap { presenter.hosts($0) ? $0 : nil } ?? page
        for old in leaving where old.node === pop { old.finish() }
        // Only auto popovers form a dismissible stack. A manual popover does
        // not close that stack when shown, or get closed by another auto.
        if pop.props["popover"] != "manual" {
            let ancestor = entries.last { entry in entry.value != "manual" && (source.map { contains(entry.popover, $0) } == true || contains(entry.popover, pop)) }
            for entry in entries.reversed() {
                if entry === ancestor { break }
                if entry.value != "manual" { close(entry.popover, restoreFocus: false) }
            }
        }
        guard live(pop), source.map(live) ?? true, connected(pop), source.map(connected) ?? true else { return }
        // A confirmation in the alert shape is the platform's alert, a sheet
        // on the window (LLP 1115.000 D7.6, AlertsMac.swift); a chooser is
        // the menu below.
        if isConfirmation(pop), presenter.alerts.isOpen(pop) || presenter.alerts.present(pop, from: source, popover: true) { return }
        let entry = Entry(pop, source: source, previous: focusOwner(window), run: run)
        presentations[pop.id, default: 0] += 1
        revalidate()
        if !ExactEnv.agentMode, let source {
            if isConfirmation(pop) {
                // A shape the menu cannot present is said, and stays painted.
                if let owner = confirmation(of: pop, from: source) { entry.confirmation = owner; entry.menu = menu(of: owner) }
            } else if isMenuShaped(pop) { entry.menu = menu(of: pop, from: opens(source, pop) ? source : nil) }
        }
        entries.append(entry)
        // A menu's shape under the driver stays the painted menu it was.
        let windowed = wantsWindow(pop)
        presenter.popoverEvents.changed(pop, open: true)
        if let menu = entry.menu {
            // Leave native tracking until the click and its app batch finish.
            RunLoop.main.perform(inModes: [.common]) { [weak self, weak source, weak entry] in
                guard let self, let entry, self.entries.contains(where: { $0 === entry }) else { return }
                guard let source, self.live(source), self.live(entry.popover), source.window === window,
                      !source.inert, !source.disabled, entry.confirmation.map(self.valid) ?? true
                else { self.close(entry.popover); return }
                entry.tracking = true
                self.track(menu, positioning: entry.focusedItem, at: self.popUpPoint(menu, entry.popover, in: source), in: source)
                entry.tracking = false
                // Escape or a click outside chose nothing. A chosen item has
                // only been recorded: it presses on the next turn, after this
                // stack and AppKit's tracking of `source` have unwound.
                self.close(entry.popover, cancelling: false)
                if let owner = entry.confirmation { DispatchQueue.main.async { owner.finished = true } }
            }
        } else if windowed, let source, present(entry, from: source, in: window) {
            refresh()
        } else {
            paint(entry, in: window)
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
    /// popover opens painted, anchored to the node (D4). `run` names a run
    /// of `source` whose own `contextPopover` it is (X77).
    func context(_ source: NodeView, at point: NSPoint, run: UInt32? = nil) {
        guard let name = contextPopover(source, run: run) else { return }
        RunLoop.main.perform(inModes: [.common]) { [weak self, weak source] in
            guard let self, let presenter = self.presenter, let source, self.live(source), self.contextPopover(source, run: run) == name,
                  !source.disabled, !source.inert, !self.hidden(source), presenter.hosts(source.window),
                  run.map(presenter.inlineEnabled) ?? true else { return }
            guard let pop = presenter.carrying("popover").first(where: { $0.props["id"] == name && !self.isConfirmation($0) }) else {
                presenter.session?.log("context menu \(name) refused: no popover has that id")
                return
            }
            while let last = self.entries.last { self.close(last.popover, restoreFocus: false) }
            if ExactEnv.agentMode { self.show(pop, from: source, run: run); return }
            // A new presentation only if it has an item: it retires picks of the last.
            guard pop.container.subviews.contains(where: { ($0 as? NodeView).map { $0.isButton && $0.props["contextPreview"] != "true" } ?? false }) else { return }
            self.presentations[pop.id, default: 0] += 1
            self.revalidate()
            let menu = self.menu(of: pop)
            let entry = Entry(pop, source: source, previous: source.window.flatMap(self.focusOwner), run: run)
            entry.menu = menu; entry.tracking = true
            self.entries.append(entry)
            presenter.popoverEvents.changed(pop, open: true)
            self.track(menu, at: point, in: source)
            entry.tracking = false
            if self.entries.contains(where: { $0 === entry }) { self.close(pop, cancelling: false) }
        }
    }
    /// The popover `source`'s context menu shows, or its run `run`'s.
    private func contextPopover(_ source: NodeView, run: UInt32?) -> String? {
        guard let run else { return source.props["contextPopover"] }
        return presenter?.textHost(run) === source ? presenter?.inlineText(run)?.props["contextPopover"] : nil
    }
    /// Where a menu presenting `pop` pops up in `source`: its top-left, by
    /// the popover's `position-area` and margins as its painted box would
    /// sit, the menu's own size the box, clamped to the viewport (§5).
    func popUpPoint(_ menu: NSMenu, _ pop: NodeView, in source: NodeView) -> NSPoint {
        let bounds = presenter.map { source.convert($0.viewport.bounds, from: $0.viewport) } ?? .infinite
        return PositionArea.origin(PositionArea.of(pop), anchor: anchor(source, in: source), size: menu.size,
                                   margins: PositionArea.margins(of: pop), in: bounds)
    }
    func close(_ pop: NodeView, restoreFocus: Bool = true, cancelling: Bool = true) {
        if presenter?.alerts.close(pop) == true { return }
        guard let index = entries.firstIndex(where: { $0.popover === pop }) else { return }
        // Hiding an auto stack never hides a manual popover above it.
        if entries[index].value != "manual" {
            for entry in entries[(index + 1)...].reversed() where entry.value != "manual" {
                close(entry.popover, restoreFocus: restoreFocus)
            }
        }
        // AppKit's popover held the keyboard in its own window, and the
        // window under it kept its first responder: nothing to give back.
        let entry = entries[index], window = entry.native == nil ? presenter?.viewport.window : nil
        let hadFocus = window.flatMap(focusOwner).map { contains(pop, $0) } ?? false
        entries.remove(at: index)
        presenter?.popoverEvents.changed(pop, open: false)
        // Reset, unmount, a changed row: the menu ends and nothing it showed dispatches.
        if cancelling { entry.confirmation?.cancel() }
        entry.menu?.cancelTracking()
        pop.isHidden = pop.props["popover"] != nil || pop.style["display"]?.string == "none"
        if let native = entry.native {
            dismiss(native)
            refresh()
        } else if entry.menu == nil {
            restore(entry)
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
        for old in leaving { old.finish() }
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
        // In AppKit's popover its box is the popover's content, whatever margins the block put before it.
        if entry.native != nil { let box = NSRect(origin: .zero, size: frame.size); if node.frame != box { node.frame = box }; return true }
        entry.frame = frame
        layout()
        return true
    }
    func sync() {
        guard let presenter else { return }
        for entry in entries {
            let source = entry.source
            guard (!entry.sourced || source != nil), source.map(live) ?? true, live(entry.popover), source.map(connected) ?? true, connected(entry.popover),
                  source.map({ presenter.hosts($0.window) }) ?? true, presenter.viewport.window != nil,
                  entry.value == entry.popover.props["popover"], !entry.popover.inert,
                  (entry.value == "manual" || entry.parent.map(hidden) != true),
                  entry.popover.style["display"]?.string != "none",
                  entry.value == "manual" || source.map({ !hidden($0) || presenter.toolbar.contains($0) }) ?? true,
                  entry.menu == nil || (entry.confirmation.map(valid) ?? isMenuShaped(entry.popover))
            else { close(entry.popover); continue }
            syncPresentation(entry)
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
        for entry in entries where entry.menu == nil && entry.native == nil {
            entry.layer.frame = entry.layer.superview?.bounds ?? presenter.viewport.bounds
            let anchor = entry.source.map { anchor($0, in: entry.layer, run: entry.run) } ?? .zero
            var box = entry.frame
            box.origin = PositionArea.origin(PositionArea.of(entry.popover), anchor: anchor, size: box.size,
                                             margins: PositionArea.margins(of: entry.popover),
                                             fallbacks: entry.popover.style["position_try_fallbacks"]?.string ?? "none",
                                             history: &entry.placement, in: entry.layer.bounds)
            if !entry.sourced { box.origin = NSPoint(x: (entry.layer.bounds.width - box.width) / 2, y: (entry.layer.bounds.height - box.height) / 2) }
            if entry.popover.frame != box { entry.popover.frame = box }
        }
    }
    /// Shared by the local event monitor and direct agent event delivery.
    func key(_ event: NSEvent) -> Bool {
        guard event.keyCode == 53, let window = event.window, presenter?.hosts(window) == true else { return false }
        if escapeHeld {
            if event.type == .keyUp { escapeHeld = false }
            return true
        }
        // The focus in this session's viewport, or in no view (the window
        // itself, after `blur()`) when this session hears the window's keys:
        // the web's close request reaches an auto popover with BODY focused,
        // and of several sessions in the window one takes it.
        guard let viewport = presenter?.viewport else { return false }
        // A sheet's window and a popover's hold this session's nodes alone.
        if window !== viewport.window {
        } else if let owner = focusOwner(window) {
            guard owner === viewport || owner.isDescendant(of: viewport) else { return false }
        } else if presenter?.session?.view?.ownsShortcutFocus() == false { return false }
        guard let last = entries.last(where: { $0.value != "manual" }),
              (window.firstResponder as? NSTextInputClient)?.hasMarkedText() != true else { return false }
        if event.type == .keyDown, !event.isARepeat {
            escapeHeld = true
            close(last.popover)
        }
        return true
    }
    func pointer(_ event: NSEvent) {
        guard !entries.isEmpty, presenter?.hosts(event.window) == true,
              [.leftMouseDown, .leftMouseUp, .rightMouseDown, .rightMouseUp].contains(event.type),
              let content = event.window?.contentView else { return }
        let hit = content.hitTest(content.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow)
        let ancestor = entries.last(where: { entry in
            guard entry.value != "manual" else { return false }
            guard let hit else { return false }
            if contains(entry.popover, hit) { return true }
            // An eligible invoker of it: a press there toggles it, and is not outside it.
            return sequence(first: hit, next: { self.parent(of: $0) }).contains {
                guard let node = $0 as? NodeView, !node.disabled, !node.inert else { return false }
                return self.popoverInvocation(node)?.target === entry.popover
            }
        })?.popover.id
        if event.type == .leftMouseDown || event.type == .rightMouseDown {
            pointerDown = (event.buttonNumber, ancestor)
        } else {
            defer { pointerDown = nil }
            guard let down = pointerDown, down.button == event.buttonNumber, down.ancestor == ancestor else { return }
            for entry in entries.reversed() where entry.value != "manual" {
                if entry.popover.id == ancestor { break }
                close(entry.popover, restoreFocus: false)
            }
        }
    }
    var observation: [String: Any]? {
        entries.last.map { entry in
            var o: [String: Any] = ["popover": Int(entry.popover.id), "source": entry.source.map { Int($0.id) as Any } ?? NSNull(), "phase": "open"]
            if entry.native != nil { o["presentation"] = "NSPopover" }
            if let owner = entry.confirmation { o["kind"] = "confirmation"; o["actions"] = owner.actions.count }
            return o
        }
    }

    func isMenuShaped(_ pop: NodeView) -> Bool {
        // A popover inside the menu (a submenu authored in its parent) is no
        // row of it: closed, it is `display: none` on the web.
        let rows = pop.container.subviews.compactMap { $0 as? NodeView }
            .filter { $0.props["contextPreview"] != "true" && $0.props["popover"] == nil }
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
    /// LLP 1021 §5, "Submenus": the popover `row` opens as a submenu, a
    /// menu-shaped one (D3) it shows or toggles (`Invocation`),
    /// not a confirmation, and not already on `path` (the popovers from the
    /// presented one down to `row`'s): a row that names one of those is an
    /// item, as one opening any other popover is.
    func submenu(of row: NodeView, path: [NodeView]) -> NodeView? {
        guard let invocation = popoverInvocation(row), invocation.opens else { return nil }
        let sub = invocation.target
        guard !isConfirmation(sub), !path.contains(where: { $0 === sub }), isMenuShaped(sub) else { return nil }
        return sub
    }
    /// `pop`'s button menu, opened from `source` (nil for a context menu,
    /// and when a test builds one with no invoker).
    func menu(of pop: NodeView, from source: NodeView? = nil) -> NSMenu {
        let menu = items(of: pop, path: [], from: source, presentation: presentation(of: pop), once: Picked())
        // The popover's `aria-label` titles the menu ("Open location in"). A
        // submenu has none: the item that opens it names it.
        if let heading = pop.props["accessibilityLabel"], !heading.isEmpty { menu.insertItem(.sectionHeader(title: heading), at: 0) }
        return menu
    }
    // MARK: A window toolbar's menu item (LLP 1115.002 D2)

    /// The popover a toolbar's menu item shows for `source`: the one it
    /// shows or toggles (`Invocation`), when it is a button menu's shape (D3)
    /// and no confirmation.
    func toolbarPopover(of source: NodeView) -> NodeView? {
        guard let invocation = popoverInvocation(source), invocation.opens,
              !isConfirmation(invocation.target), isMenuShaped(invocation.target) else { return nil }
        return invocation.target
    }
    /// The menu of a toolbar's menu item for `source`. AppKit pops it up
    /// from its own item, so nothing here tracks it: it is empty until it
    /// opens, and filled then from the popover's rows as they are
    /// (`menuNeedsUpdate`).
    func toolbarMenu(for source: NodeView) -> NSMenu {
        let menu = InvokerMenu()
        menu.source = source
        menu.autoenablesItems = false
        menu.delegate = self
        return menu
    }
    /// A toolbar item's menu, filled as it opens: the button's own `press`
    /// first, as a click on it in the page runs before its menu shows (it
    /// may make the rows), then the popover's rows now. A chosen row presses
    /// on the next turn, once, as a button menu's does (`pick`).
    func fill(_ menu: InvokerMenu) {
        menu.removeAllItems()
        guard let presenter, let source = menu.source, live(source), !source.inert, !source.disabled,
              presenter.toolbar.contains(source) else { return }
        if source.handlers.contains("press") {
            presenter.commitPendingEdit()
            presenter.onPress?(source.id)
        }
        guard live(source), !source.inert, !source.disabled else { return }
        guard let pop = toolbarPopover(of: source) else {
            // Its handler made it invoke something other than a menu: that
            // runs as a press would run it, after the empty menu.
            DispatchQueue.main.async { [weak self, weak presenter, weak source] in
                guard let self, let presenter, let source else { return }
                presenter.dialogs.command(source, fromNativeMenu: true)?()
                self.command(source, fromNativeMenu: true)?()
            }
            return
        }
        guard connected(pop), !pop.inert, pop.style["display"]?.string != "none" else { return }
        // An open auto popover closes for it, as for any other shown.
        for entry in entries.reversed() where entry.value != "manual" { close(entry.popover, restoreFocus: false) }
        presentations[pop.id, default: 0] += 1
        revalidate()
        let built = self.menu(of: pop, from: source)
        for item in built.items {
            built.removeItem(item)
            menu.addItem(item)
        }
    }
    private func menuItem(of row: NodeView, action: Selector?) -> NSMenuItem {
        let item = AccessibleMenuItem(title: title(of: row), action: action, keyEquivalent: "")
        item.owner = row
        if #available(macOS 14.4, *), row.isNativeButton {
            item.title = row.face?.shown ?? ""
            item.subtitle = row.face?.subtitle
        }
        return item
    }
    /// The menu of `pop`, reached through `path`'s openers (each row that
    /// opened the next popover): an item per button row, a separator per
    /// `hr`, and a submenu per row that opens a menu-shaped popover.
    private func items(of pop: NodeView, path: [Step], from source: NodeView?, presentation: Int, once: Picked) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false
        let popovers = path.compactMap(\.parent) + [pop]
        for case let row as NodeView in pop.container.subviews {
            if row.props["contextPreview"] == "true" { continue } // a context menu's preview (§5.1)
            if row.props["popover"] != nil { continue } // a submenu authored inside its parent
            if row.props["semanticTag"] == "hr" { menu.addItem(.separator()); continue }
            guard row.isButton else { continue }
            if let sub = submenu(of: row, path: popovers) {
                let item = menuItem(of: row, action: nil)
                item.submenu = items(of: sub, path: path + [Step(row, in: pop)], from: source, presentation: presentation, once: once)
                item.isEnabled = !row.disabled && !row.inert && shown(row, in: popovers + [sub])
                item.image = image(of: row)
                menu.addItem(item)
                continue
            }
            let item = menuItem(of: row, action: #selector(pick(_:)))
            item.target = self
            item.representedObject = Pick(row, in: pop, path: path, from: source, presentation: presentation,
                                          title: title(of: row), once: once)
            item.state = row.props["accessibilityChecked"] == "true" ? .on : .off
            // As a chooser's: a hidden or inert row is shown, never chosen.
            item.isEnabled = !row.disabled && !row.inert && shown(row, in: popovers)
            item.image = image(of: row)
            menu.addItem(item)
        }
        return menu
    }
    /// One menu's items share this, its submenus' too: the first item taken
    /// is its only one.
    private final class Picked { var taken = false }
    /// A row that opened a submenu, in the popover that holds it.
    private struct Step {
        weak var opener: NodeView?
        weak var parent: NodeView?
        init(_ opener: NodeView, in parent: NodeView) { self.opener = opener; self.parent = parent }
    }
    /// An item's row as the menu showed it, in which presentation, opened
    /// from which invoker, through which submenus.
    private final class Pick: NSObject {
        weak var row: NodeView?
        weak var pop: NodeView?
        weak var source: NodeView?
        let path: [Step]
        let invoked: Bool
        let presentation: Int
        let title: String
        let once: Picked
        var cancelled = false
        init(_ row: NodeView, in pop: NodeView, path: [Step], from source: NodeView?, presentation: Int, title: String, once: Picked) {
            self.row = row; self.pop = pop; self.path = path; self.source = source; invoked = source != nil
            self.presentation = presentation; self.title = title; self.once = once
        }
    }
    /// Still the row the menu showed: live, in its popover, enabled, shown,
    /// under the same title (a reused id is not that row); each submenu's
    /// opener still in its popover, enabled and shown, opening the next;
    /// the invoker still opening the presented popover, which has not been
    /// presented again since.
    private func valid(_ pick: Pick) -> Bool {
        guard !pick.cancelled, let row = pick.row, let pop = pick.pop, live(row), live(pop),
              row.isDescendant(of: pop), row.isButton, !row.disabled, !row.inert,
              title(of: row) == pick.title else { return false }
        var popovers: [NodeView] = []
        for (i, step) in pick.path.enumerated() {
            let next = i + 1 < pick.path.count ? pick.path[i + 1].parent : pop
            guard let opener = step.opener, let parent = step.parent, let next, live(opener), live(parent),
                  opener.isDescendant(of: parent), opens(opener, next), submenu(of: opener, path: popovers + [parent]) === next,
                  !opener.disabled, !opener.inert else { return false }
            popovers.append(parent)
        }
        popovers.append(pop)
        guard let root = popovers.first, presentation(of: root) == pick.presentation,
              pick.path.allSatisfy({ $0.opener.map { shown($0, in: popovers) } ?? false }), shown(row, in: popovers) else { return false }
        guard pick.invoked else { return true }
        return pick.source.map { invokes($0, root) } ?? false
    }
    /// An item picked: recorded now, its row pressed on the next main-queue
    /// turn, once, as a chooser's is (ChooserMac.swift): AppKit sends the
    /// action inside `popUp`, still tracking the menu in the invoker, and
    /// the press's batch may unmount that invoker.
    @objc private func pick(_ sender: NSMenuItem) {
        guard let pick = sender.representedObject as? Pick, !pick.once.taken else { return }
        pick.once.taken = true
        picking.append(pick)
        afterNativeTracking { [weak self, pick] in
            guard let self else { return }
            self.picking.removeAll { $0 === pick }
            guard self.valid(pick), let row = pick.row else { return }
            pick.cancelled = true
            self.presenter?.press(row.id, fromNativeMenu: true)
        }
    }
    /// Not hidden by the page — `node`'s or an ancestor's `display: none`
    /// or hiding — though `pop` is hidden in place while a menu presents it.
    func shown(_ node: NodeView, in pop: NodeView) -> Bool { shown(node, in: [pop]) }
    /// The same, `pops` (a menu and the submenus down to `node`) hidden in place.
    func shown(_ node: NodeView, in pops: [NodeView]) -> Bool {
        !sequence(first: node as NSView, next: { self.parent(of: $0) }).contains { view in
            (view as? NodeView)?.style["display"]?.string == "none" || (view.isHidden && !pops.contains { $0 === view })
        }
    }
    func title(of v: NodeView) -> String {
        if v.kind == "text" { return v.paragraphSpec().runs.map(\.text).joined() }
        // A native button's children are its face, not views: its title, else its label.
        if v.isNativeButton { return [v.face?.shown, v.face?.subtitle].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " — ") }
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

/// The menu of a window toolbar's menu item: the authored button whose
/// popover it shows (LLP 1115.002 D2).
final class InvokerMenu: NSMenu {
    weak var source: NodeView?
}

extension MenuHost: NSMenuDelegate {
    func menuNeedsUpdate(_ menu: NSMenu) {
        if let menu = menu as? InvokerMenu { fill(menu) }
    }
    /// A key's search for its menu item fills no toolbar menu: a row's
    /// shortcut is the page's (ShortcutsMac.swift), and filling presses the
    /// button.
    func menuHasKeyEquivalent(_ menu: NSMenu, for event: NSEvent, target: AutoreleasingUnsafeMutablePointer<AnyObject?>, action: UnsafeMutablePointer<Selector?>) -> Bool { false }
}

/// Transparent space passes through, so light dismiss never swallows the
/// outside click or makes the rest of the page modal.
final class PopoverLayer: NSView {
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = raisedHit(super.hitTest(point), point)
        return hit === self ? nil : hit
    }
    // A hit inside a popover may forward through plain views, but never
    // beyond the top layer to the page's responder chain.
    override func mouseDown(with event: NSEvent) {}
    override func mouseDragged(with event: NSEvent) {}
    override func mouseUp(with event: NSEvent) {}
    override func rightMouseDown(with event: NSEvent) {}
    override func rightMouseDragged(with event: NSEvent) {}
    override func rightMouseUp(with event: NSEvent) {}
    override func otherMouseDown(with event: NSEvent) {}
    override func otherMouseDragged(with event: NSEvent) {}
    override func otherMouseUp(with event: NSEvent) {}
}
#endif

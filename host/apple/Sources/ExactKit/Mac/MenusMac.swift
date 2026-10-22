// LLP 1021 D2–D4: button menus project to NSMenu, and so does an
// alertdialog popover of the chooser's shape ("The chooser": actions, at most
// one hide-only cancel, text rows as its message), popped up against its
// invoker by its `position-area` (ChooserMac.swift); a picked item presses
// once, as the menu closes. One in the alert shape (text, a cancel, at most
// three buttons) is `NSAlert` instead, under the agent too (LLP 1115.000 D7.6,
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
        let bubbled: Bool
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
        init(_ popover: NodeView, source: NodeView?, previous: NSView?, run: UInt32?, bubbled: Bool = false) {
            self.popover = popover; self.source = source; sourced = source != nil; self.previous = previous; self.run = run
            self.bubbled = bubbled
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
    /// item's action waits until popUp has unwound and the menu's popover
    /// has closed (`closed`), then runs in that same turn, as AppKit sends a
    /// menu item's action before `popUp` returns. A next turn let VoiceOver
    /// ask where the focus was between the menu's close and the action: it
    /// re-read the page's focused field, at length, and never named the
    /// dialog the action opened (realinput-1017k H-9).
    func afterNativeTracking(_ work: @escaping () -> Void) {
        if nativeTracking > 0 { afterTracking.append(work) }
        else { DispatchQueue.main.async(execute: work) }
    }
    /// `click`, a pointer's press that asked for a context menu, pops it up as
    /// AppKit pops up a view's `menu(for:)`: its top left at the pointer, the
    /// pointer just outside it, so no row is highlighted and the press's
    /// release chooses nothing (a quick one leaves the menu open, a held one
    /// closes it). `popUp(positioning:at:in:)` is a pop-up button's
    /// presentation: it puts the pointer in the first row, where a release half
    /// a second after the press chooses it. The press is restated at `point`
    /// in `source`'s window, which one bubbled from a popover's own window is
    /// not in.
    private func track(_ menu: NSMenu, positioning item: NSMenuItem? = nil, at point: NSPoint, in source: NodeView,
                       click: NSEvent? = nil, entry: Entry, valid: () -> Bool, closed: () -> Void) {
        // The release can close this presentation or change what its source invokes.
        presenter?.pointerSource?.pointerReleased(click)
        guard entries.contains(where: { $0 === entry }), live(entry.popover), live(source),
              source.window != nil, source.presenter === presenter, valid() else { closed(); return }
        nativeTracking += 1
        if let click, [.rightMouseDown, .leftMouseDown].contains(click.type), let window = source.window,
           let press = NSEvent.mouseEvent(with: click.type, location: source.convert(point, to: nil), modifierFlags: click.modifierFlags,
                                          timestamp: click.timestamp, windowNumber: window.windowNumber, context: nil,
                                          eventNumber: click.eventNumber, clickCount: click.clickCount, pressure: click.pressure) {
            // The author's rows alone: AppKit appends Services to a contextual
            // menu whose view can send them (`validRequestor`), as a paragraph
            // can while the page has a selection.
            menu.allowsContextMenuPlugIns = false
            NSMenu.popUpContextMenu(menu, with: press, for: source)
        } else {
            menu.popUp(positioning: item, at: point, in: source)
        }
        nativeTracking -= 1
        closed()
        if nativeTracking == 0 {
            let work = afterTracking; afterTracking.removeAll()
            for action in work { action() }
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
    private var pointerDown: (button: Int, ancestor: UInt32?, press: Int)?
    private var pressCount = 0
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
    func parent(of view: NSView) -> NSView? { authoredParent(of: view) ?? view.superview }
    /// An open popover's authored parent, nil for any other view.
    func authoredParent(of view: NSView) -> NSView? {
        entries.first(where: { $0.popover === view && $0.menu == nil })?.parent
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
    func command(_ source: NodeView, fromNativeMenu: Bool = false, bubbled: Bool = false) -> (() -> Void)? {
        guard let presenter, source.isButton, !source.disabled, !(bubbled ? source.authoredInert : source.inert),
              fromNativeMenu || !source.isHiddenOrHasHiddenAncestor || presenter.toolbar.contains(source),
              popoverInvocation(source) != nil || source.handlers.contains("press") else { return nil }
        return { [weak self, weak source] in
            guard let self, let source else { return }
            let bubbled = bubbled && self.presenter?.dialogs.isBehindModal(source) == true
            guard self.live(source), !(bubbled ? source.authoredInert : source.inert), !source.disabled,
                  let invocation = self.popoverInvocation(source), self.live(invocation.target), !bubbled || !invocation.target.inert,
                  !self.showsModally(invocation.target) else { return }
            let pop = invocation.target
            if invocation.action == .hide || (invocation.action == .toggle && self.isOpen(pop)) { self.close(pop) }
            // AppKit closed it with this very click on its invoker: a toggle leaves it closed.
            else if invocation.action == .show || !self.closedByThisClick(source) { self.show(pop, from: source, bubbled: bubbled) }
        }
    }
    func opens(_ source: NodeView, _ pop: NodeView) -> Bool {
        popoverInvocation(source).map { $0.opens && $0.target === pop } ?? false
    }
    /// `source` can still open `pop` in this presenter's window: live,
    /// enabled, not inert, shown (or a toolbar's), and, for an invoker, pointing at it.
    func invokes(_ source: NodeView, _ pop: NodeView, requiringTarget: Bool = true, bubbled: Bool = false) -> Bool {
        guard let presenter, presenter.viewport.window != nil else { return false }
        let bubbled = bubbled && presenter.dialogs.isBehindModal(source)
        return live(source) && live(pop) && presenter.hosts(source.window) && !source.disabled
            && !(bubbled ? source.authoredInert : source.inert) && (!bubbled || !pop.inert)
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
    func show(_ pop: NodeView, from row: NodeView?, run: UInt32? = nil, contextMenu: Bool = false, bubbled: Bool = false) {
        // A row of a menu opens a popover that is no menu as the menu's own invoker would (LLP 1021's
        // menu rule): an open menu's, or the native menu's it was chosen from, which has closed by now.
        let menuRow = { (menu: NodeView?) in menu?.props["accessibilityRole"] == "menu" && row.map { self.contains(menu!, $0) } == true }
        let picked = pop.props["accessibilityRole"] != "menu" && menuOpener.pops.contains(where: { menuRow($0.pop) }) ? menuOpener.opener : nil
        var source = row, seen = Set<ObjectIdentifier>() // past any open submenu, to what opened the outermost menu
        while pop.props["accessibilityRole"] != "menu", let e = entries.last(where: { e in e.popover.props["accessibilityRole"] == "menu" && source.map { s in contains(e.popover, s) } == true }),
              seen.insert(ObjectIdentifier(e)).inserted { source = e.source }
        if source === row, let picked { source = picked }
        let bubbled = bubbled && source.map { presenter?.dialogs.isBehindModal($0) == true } == true
        guard let presenter, !isOpen(pop), live(pop), source.map(live) ?? true, pop.props["popover"] != nil,
              connected(pop), source.map(connected) ?? true, !pop.inert,
              source.map({ !(bubbled ? $0.authoredInert : $0.inert) && !(contextMenu ? $0.formDisabled : $0.disabled) }) ?? true,
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
        let entry = Entry(pop, source: source, previous: focusOwner(window), run: run, bubbled: bubbled)
        presentations[pop.id, default: 0] += 1
        revalidate()
        if !ExactEnv.agentMode, let source {
            if isConfirmation(pop) {
                // A shape the menu cannot present is said, and stays painted.
                if let owner = confirmation(of: pop, from: source, bubbled: entry.bubbled) { entry.confirmation = owner; entry.menu = menu(of: owner) }
            } else if isMenuShaped(pop) {
                menuOpener = MenuOpener(pop, opener: source) // what opened it, `showPopover(source=)` too
                entry.menu = menu(of: pop, from: opens(source, pop) ? source : nil, bubbled: entry.bubbled)
            }
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
                      !(entry.bubbled ? source.authoredInert : source.inert), !source.disabled,
                      !entry.bubbled || !entry.popover.inert, entry.confirmation.map(self.valid) ?? true
                else { self.close(entry.popover); return }
                entry.tracking = true
                // Escape or a click outside chose nothing. A chosen item has
                // only been recorded: it presses once AppKit's tracking of
                // `source` has unwound and the menu has closed.
                let requiresTarget = self.opens(source, entry.popover)
                self.track(menu, positioning: entry.focusedItem, at: self.popUpPoint(menu, entry.popover, in: source), in: source, entry: entry, valid: {
                    self.invokes(source, entry.popover, requiringTarget: requiresTarget, bubbled: entry.bubbled)
                        && (entry.confirmation.map(self.valid) ?? true)
                }) {
                    entry.tracking = false
                    if self.entries.contains(where: { $0 === entry }) { self.close(entry.popover, cancelling: false) }
                }
                if let owner = entry.confirmation { DispatchQueue.main.async { owner.finished = true } }
            }
        } else if windowed, let source, present(entry, from: source, in: window) {
            refresh()
        } else {
            paint(entry, in: window)
            layout()
            // A plain popover leaves focus alone unless it has autofocus.
            if let target = presenter.carrying("autofocus").first(where: {
                contains(pop, $0) && $0.props["autofocus"] == "true" && !$0.inert
                    && !$0.disabled && !$0.isHiddenOrHasHiddenAncestor
            }) { window.makeFirstResponder(presenter.keyView(of: target)) }
            refresh()
        }
    }
    /// LLP 1021 §5.1: a context menu, its popover's menu rows as an NSMenu
    /// popped up at the click, after the node's own `contextmenu` has run
    /// (both fire), on the next turn as a button menu's. Its preview row is
    /// not an item: a Mac's context menu has no preview. Under the agent the
    /// popover opens painted, anchored to the node (D4). `run` names a run
    /// of `source` whose own `contextPopover` it is (X77). `click` is the
    /// pointer's press that asked for it (`track`); a request from the
    /// keyboard has none, and its menu's top left is at `point`.
    func context(_ source: NodeView, at point: NSPoint, run: UInt32? = nil, click: NSEvent? = nil) {
        guard let name = contextPopover(source, run: run) else { return }
        RunLoop.main.perform(inModes: [.common]) { [weak self, weak source] in
            guard let self, let presenter = self.presenter, let source, self.live(source), self.contextPopover(source, run: run) == name,
                  !source.formDisabled, !source.inert, !self.hidden(source), presenter.hosts(source.window),
                  run.map({ !presenter.inlineInert($0) }) ?? true else { return }
            guard let pop = presenter.carrying("popover").first(where: { $0.props["id"] == name && !self.isConfirmation($0) }) else {
                presenter.session?.log("context menu \(name) refused: no popover has that id")
                return
            }
            // A source in an open popover nests its menu there, as HTML's
            // popover stack nests one shown with that source: the popovers
            // holding it stay open, every auto popover above them closes,
            // and a manual one stays.
            let holder = self.entries.lastIndex { $0.value != "manual" && self.contains($0.popover, source) }
            for entry in self.entries[(holder.map { $0 + 1 } ?? 0)...].reversed() where entry.value != "manual" {
                self.close(entry.popover, restoreFocus: false)
            }
            if ExactEnv.agentMode { self.show(pop, from: source, run: run, contextMenu: true); return }
            // A new presentation only if it has an item: it retires picks of the last.
            guard pop.container.subviews.contains(where: { ($0 as? NodeView).map { $0.isButton && $0.props["contextPreview"] != "true" } ?? false }) else { return }
            self.presentations[pop.id, default: 0] += 1
            self.revalidate()
            let menu = self.contextMenu(of: pop, on: source)
            let entry = Entry(pop, source: source, previous: source.window.flatMap(self.focusOwner), run: run)
            entry.menu = menu; entry.tracking = true
            self.entries.append(entry)
            presenter.popoverEvents.changed(pop, open: true)
            self.track(menu, at: point, in: source, click: click, entry: entry, valid: {
                self.contextPopover(source, run: run) == name && !source.formDisabled && !source.inert && !self.hidden(source)
                    && (run.map { !presenter.inlineInert($0) } ?? true)
            }) {
                entry.tracking = false
                if self.entries.contains(where: { $0 === entry }) { self.close(pop, cancelling: false) }
            }
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
                                   margins: PositionArea.margins(of: pop), overflow: .platform, in: bounds)
    }
    func close(_ pop: NodeView, restoreFocus: Bool = true, cancelling: Bool = true) {
        presenter?.mouseChain.bubbledPress?.cancel(in: pop)
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
        // Opened by VO-Space, which moves no focus, and closed with the focus
        // where it was: VoiceOver goes back to the trigger it pressed, as
        // from an `NSMenu` (`tellLanding`). AppKit's popover gives its own
        // window's focus back by itself.
        if !hadFocus, let window, let source = entry.source, let presenter, focusOwner(window) === entry.previous {
            presenter.landing.invoker = source
            presenter.landing.invokerFocus = entry.previous
            presenter.scheduleLanding()
        }
        entries.remove(at: index)
        presenter?.popoverEvents.changed(pop, open: false)
        // Reset, unmount, a changed row: the menu ends and nothing it showed dispatches.
        if cancelling { entry.confirmation?.cancel() }
        entry.menu?.cancelTracking()
        pop.isHidden = pop.props["popover"] != nil || pop.style["display"]?.string == "none"
        // The focus is given back to the trigger, never left on the window
        // the hidden popover leaves; VoiceOver is sent to it after
        // (`tellLanding`, Accessibility.swift).
        if hadFocus, let window {
            if restoreFocus, let previous = entry.previous, previous.window === window,
               !previous.isHiddenOrHasHiddenAncestor, presenter?.dialogs.blocks(previous) != true,
               !((previous as? NodeView)?.inert ?? false) {
                window.makeFirstResponder(previous)
                if let range = entry.selection, let editor = window.firstResponder as? NSTextView { editor.setSelectedRange(range) }
            } else { window.makeFirstResponder(presenter?.viewport) }
        }
        if let native = entry.native {
            dismiss(native)
            refresh()
        } else if entry.menu == nil {
            restore(entry)
            refresh()
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
            let anchor: NSRect = entry.source.map { self.anchor($0, in: entry.layer, run: entry.run) } ?? .zero
            var box = entry.frame
            // The page scrolls under a layer in its own window; a sheet's layer is its own extent.
            let document = presenter.viewport.documentView.flatMap { $0.window != nil && $0.window === entry.layer.window ? $0 : nil }
            let page = document.map { $0.convert($0.bounds, to: entry.layer) } ?? entry.layer.bounds
            box.origin = PositionArea.origin(PositionArea.of(entry.popover), anchor: anchor, size: box.size,
                                             margins: PositionArea.margins(of: entry.popover),
                                             fallbacks: entry.popover.style["position_try_fallbacks"]?.string ?? "none",
                                             overflow: PositionArea.overflow(of: entry.popover, page: page),
                                             rtl: presenter.documentDirection == "rtl",
                                             history: &entry.placement, in: entry.layer.bounds)
            if !entry.sourced { box.origin = NSPoint(x: (entry.layer.bounds.width - box.width) / 2, y: (entry.layer.bounds.height - box.height) / 2) }
            if entry.popover.frame != box { entry.popover.frame = box }
        }
    }
    /// Shared by the local event monitor and direct agent event delivery.
    func key(_ event: NSEvent) -> Bool {
        guard event.keyCode == 53, let window = event.window, window !== presenter?.alerts.sheet,
              presenter?.hosts(window) == true else { return false }
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
    /// HTML's popover light dismiss: a press outside closes the auto
    /// popovers above the one it began in, at its release, when it was
    /// released over the same one (the spec's "popover pointerdown target",
    /// https://html.spec.whatwg.org/multipage/popover.html#popover-light-dismiss).
    func pointer(_ event: NSEvent) {
        guard !entries.isEmpty, presenter?.hosts(event.window) == true,
              [.leftMouseDown, .leftMouseUp, .rightMouseDown, .rightMouseUp].contains(event.type),
              let window = event.window, window !== presenter?.alerts.sheet else { return }
        let (ancestor, hit) = clickedPopover(at: event.locationInWindow, in: window)
        if event.type == .leftMouseUp || event.type == .rightMouseUp { return release(event.buttonNumber, at: ancestor) }
        pressCount += 1
        pointerDown = (event.buttonNumber, ancestor, pressCount)
        guard hit else { return }
        settle(pressCount, button: event.buttonNumber, pressedAt: event.timestamp, in: window)
    }
    /// A view that tracks a press itself (NSTextView, a field editor,
    /// NSSearchField, an NSControl) takes the release from the queue in its
    /// own loop, in the event-tracking run loop mode, and no local monitor
    /// sees it. So the press is settled when the run loop is next in the
    /// default mode, after any such loop has ended. The application's
    /// current event is then the release only if nothing followed it: after
    /// a hand's press in a field or a text view, AppKit first sends the
    /// `mouseExited`/`mouseEntered` events of the tracking areas the view
    /// moved (measured with real input, macOS 27). The window server's
    /// record of the button says whether it was released after the press,
    /// and the pointer's position stands for the release's.
    private func settle(_ press: Int, button: Int, pressedAt at: TimeInterval, in window: NSWindow) {
        RunLoop.main.perform(inModes: [.default]) { [weak self, weak window] in
            guard let self, let window, self.pointerDown?.press == press else { return }
            if let taken = NSApp.currentEvent, taken.type == (button == 0 ? .leftMouseUp : .rightMouseUp),
               taken.window === window, taken.timestamp >= at {
                self.release(button, at: self.clickedPopover(at: taken.locationInWindow, in: window).ancestor)
            } else if case let hand = Self.handRelease(button, window), hand.time >= at {
                self.release(button, at: self.clickedPopover(at: hand.point, in: window).ancestor)
            } else if NSEvent.pressedMouseButtons & (1 << button) != 0 {
                // Still held, and no view tracking it: its release usually
                // comes through the monitor. Look again in a moment, in case a
                // loop takes it after all.
                RunLoop.main.add(Timer(timeInterval: 0.05, repeats: false) { [weak self, weak window] _ in
                    guard let self, let window else { return }
                    self.settle(press, button: button, pressedAt: at, in: window)
                }, forMode: .default)
            }
        }
    }
    /// When the window server last saw `button` released, on the clock of
    /// `NSEvent.timestamp`, and the pointer's position in `window` now. The
    /// window server records a hand's release and one posted as a device's
    /// (`CGEvent.post(tap: .cghidEventTap)`), never an event made in the
    /// application, as the driver's and the tests' are.
    static var handRelease: (_ button: Int, _ window: NSWindow) -> (time: TimeInterval, point: NSPoint) = { button, window in
        let type: CGEventType = button == 0 ? .leftMouseUp : .rightMouseUp
        return (ProcessInfo.processInfo.systemUptime - CGEventSource.secondsSinceLastEventType(.combinedSessionState, eventType: type),
                window.mouseLocationOutsideOfEventStream)
    }
    private func release(_ button: Int, at ancestor: UInt32?) {
        defer { pointerDown = nil }
        guard let down = pointerDown, down.button == button, down.ancestor == ancestor else { return }
        for entry in entries.reversed() where entry.value != "manual" {
            if entry.popover.id == ancestor { break }
            close(entry.popover, restoreFocus: false)
        }
    }
    /// The spec's "topmost clicked popover" at a point in `window`: the
    /// auto popover the view there is in or invokes, and whether any view
    /// of the content is there.
    private func clickedPopover(at point: NSPoint, in window: NSWindow) -> (ancestor: UInt32?, hit: Bool) {
        guard let content = window.contentView else { return (nil, false) }
        let hit = content.hitTest(content.superview?.convert(point, from: nil) ?? point)
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
        return (ancestor, hit != nil)
    }
    var observation: [String: Any]? {
        entries.last.map { entry in
            var o: [String: Any] = ["popover": Int(entry.popover.id), "source": entry.source.map { Int($0.id) as Any } ?? NSNull(), "phase": "open"]
            if entry.native != nil { o["presentation"] = "NSPopover" }
            if let owner = entry.confirmation { o["kind"] = "confirmation"; o["actions"] = owner.actions.count }
            return o
        }
    }

    /// Whether `pop` is menu-shaped (`NodeView.menuShaped`, LLP 1021 D3).
    func isMenuShaped(_ pop: NodeView) -> Bool { pop.menuShaped }
    /// LLP 1021 §5, "Submenus": the popover `row` opens as a submenu, a
    /// menu-shaped one (D3) it shows or toggles (`Invocation`),
    /// not a confirmation, and not already on `path` (the popovers from the
    /// presented one down to `row`'s): a row that names one of those is an
    /// item, as one opening any other popover is. It is a menu
    /// (`role="menu"`): any other popover a row opens opens beside the
    /// menu's invoker (LLP 1021's menu rule, `show`).
    func submenu(of row: NodeView, path: [NodeView]) -> NodeView? {
        guard let invocation = popoverInvocation(row), invocation.opens else { return nil }
        let sub = invocation.target
        guard !isConfirmation(sub), !path.contains(where: { $0 === sub }), isMenuShaped(sub), sub.props["accessibilityRole"] == "menu" else { return nil }
        return sub
    }
    /// `pop`'s menu as `source`'s context menu: a chosen row's popover opens
    /// from the pressed node, the record set before the menu is built so its
    /// beside-written submenus join it as they are.
    func contextMenu(of pop: NodeView, on source: NodeView) -> NSMenu {
        menuOpener = MenuOpener(pop, opener: source)
        return menu(of: pop)
    }
    /// `pop`'s button menu, opened from `source` (nil for a context menu,
    /// and when a test builds one with no invoker).
    func menu(of pop: NodeView, from source: NodeView? = nil, bubbled: Bool = false) -> NSMenu {
        if let source { menuOpener = MenuOpener(pop, opener: source) }
        let menu = items(of: pop, path: [], from: source, presentation: presentation(of: pop), once: Picked(), bubbled: bubbled)
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
    private func items(of pop: NodeView, path: [Step], from source: NodeView?, presentation: Int, once: Picked, bubbled: Bool) -> NSMenu {
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
                menuOpener.pops.append(Weak(pop: sub)) // a row chosen in it opens from the same opener
                item.submenu = items(of: sub, path: path + [Step(row, in: pop)], from: source, presentation: presentation, once: once, bubbled: bubbled)
                item.isEnabled = !row.disabled && !row.inert && shown(row, in: popovers + [sub])
                item.image = image(of: row)
                menu.addItem(item)
                continue
            }
            let item = menuItem(of: row, action: #selector(pick(_:)))
            item.target = self
            item.representedObject = Pick(row, in: pop, path: path, from: source, presentation: presentation,
                                          title: title(of: row), once: once, bubbled: bubbled)
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
    /// The native menu presented last and the invoker or node it came from:
    /// a row chosen in it presses after it has closed, and the popover that
    /// row opens opens from here (`show`, LLP 1021's menu rule).
    private struct Weak { weak var pop: NodeView? }
    private struct MenuOpener {
        var pops: [Weak] = []
        weak var opener: NodeView?
        init() {}
        init(_ pop: NodeView, opener: NodeView?) { pops = [Weak(pop: pop)]; self.opener = opener }
    }
    private var menuOpener = MenuOpener()
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
        let bubbled: Bool
        let presentation: Int
        let title: String
        let once: Picked
        var cancelled = false
        init(_ row: NodeView, in pop: NodeView, path: [Step], from source: NodeView?, presentation: Int, title: String, once: Picked, bubbled: Bool) {
            self.row = row; self.pop = pop; self.path = path; self.source = source; invoked = source != nil
            self.bubbled = bubbled
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
        return pick.source.map { invokes($0, root, bubbled: pick.bubbled) } ?? false
    }
    /// An item picked: recorded now, its row pressed once `popUp` has
    /// returned and the menu closed, once, as a chooser's is (ChooserMac.swift):
    /// AppKit sends the action inside `popUp`, still tracking the menu in the
    /// invoker, and the press's batch may unmount that invoker.
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

#if os(macOS)
import AppKit

/// HTML's top layer belongs to the session, not to NSApp or a second runner.
/// The actual subtree moves; fields, selection and app state remain the same.
/// A `dialog` that is no alert is AppKit's sheet on its window, holding that
/// subtree (LLP 1115.002 D7.2, SheetsMac.swift); under the driver's own
/// chrome it is painted in the page over a backdrop, as it was. A modal route
/// is a sheet by the same means (D7.4, RouteSheetMac.swift): an entry here
/// whose node is the route, which navigation shows and takes away.
final class DialogHost {
    private(set) weak var presenter: Presenter?
    final class Entry {
        let dialog: NodeView
        weak var parent: NSView?
        weak var previous: NSView?
        let index: Int
        let selection: NSRange?
        var frame: NSRect
        let backdrop = DialogBackdrop(frame: .zero)
        /// Its sheet, while AppKit presents it; nil while it is painted.
        var sheet: DialogSheet?
        /// A modal route in its sheet, not a `dialog`: what its sheet shows
        /// of its header, and the header's nodes hidden while it does.
        let route: Bool
        var header: SheetHeader?
        var stoodFor: [NodeView] = []
        init(_ dialog: NodeView, previous: NSView?, route: Bool = false) {
            self.dialog = dialog; self.previous = previous; self.route = route
            selection = ((previous as? NSTextField)?.currentEditor() as? NSTextView)?.selectedRange()
                ?? (previous as? NSTextView)?.selectedRange()
            parent = dialog.superview
            index = parent?.subviews.firstIndex(of: dialog) ?? 0
            frame = dialog.frame
        }
    }
    private(set) var entries: [Entry] = []
    /// Painted in the page, not AppKit's sheet: the driver's own chrome
    /// (`--chrome agent`), as it paints the header and the tablist. A test
    /// of the painted presentation says so.
    var painted = ExactEnv.authoredChrome
    /// A mouse went down on the window behind a sheet (SheetsMac.swift).
    var downBehind = false
    /// The routes' sheets are being reconciled, and a batch asked again meanwhile (RouteSheetMac.swift).
    var routeSync = (running: false, again: false)
    private var accessibilityBlocker: ObjectIdentifier?
    /// The native button drawn as the open dialog's default (DefaultButtonMac.swift).
    weak var drawnDefault: NativeButtonMac?
    var active: NodeView? { entries.last?.dialog }
    /// LLP 1080.001 D3: an open dialog's backdrop; the dialogs it lifts.
    func inspectionOwns(_ view: NSView) -> Bool { entries.contains { $0.backdrop === view } }
    func projects(_ node: NodeView) -> Bool { entries.contains { $0.dialog === node } }
    var presented: [NodeView] { entries.map(\.dialog) }
    init(_ presenter: Presenter) { self.presenter = presenter }

    func owns(_ node: NodeView) -> Bool { entries.contains { $0.dialog === node } }
    /// A presented dialog's authored parent, painted over its backdrop or in
    /// its sheet; nil for any other node. A modal route's is navigation's.
    func authoredParent(of node: NodeView) -> NSView? { entries.first { $0.dialog === node && !$0.route }?.parent }
    func presentsRoute(_ node: NodeView) -> Bool { entries.contains { $0.dialog === node && $0.route } }
    func isBehindModal(_ source: NSView) -> Bool {
        guard let active else { return false }
        return presenter?.menus.contains(active, source) != true
    }
    func blocks(_ node: NSView) -> Bool {
        guard let active else { return false }
        // An alert over the dialog is the top modal: its rows are its own.
        if let alert = presenter?.alerts.modal, presenter?.menus.contains(alert, node) == true { return false }
        // A route's sheet's own rows are the sheet's, as the route in it is.
        if entries.last?.sheet?.chrome?.owns(node) == true { return false }
        return presenter?.menus.contains(active, node) != true
    }
    private var accessibleActive: NodeView? {
        guard let active, active.accessibilityVisible, !active.accessibilityHiddenByProp else { return nil }
        return active
    }
    func accessibilityBlocks(_ node: NSView) -> Bool {
        guard let active = accessibleActive else { return false }
        return presenter?.menus.contains(active, node) != true
    }
    func live(_ node: NodeView) -> Bool { presenter?.views[node.id] === node }
    /// The window the open dialog is in: its sheet's, or the page's.
    var window: NSWindow? { active?.window ?? presenter?.viewport.window }
    func focusOwner(_ window: NSWindow) -> NSView? {
        guard let view = window.firstResponder as? NSView else { return nil }
        if let editor = view as? NSTextView, editor.isFieldEditor {
            return editor.delegate as? NSView ?? editor
        }
        return view
    }
    /// What activating `source` does to a dialog once its own handler has
    /// run: the surviving button's invocation then (`Invocation`).
    func command(_ source: NodeView, fromNativeMenu: Bool = false, bubbled: Bool = false) -> (() -> Void)? {
        guard !source.disabled, !(bubbled ? source.authoredInert : source.inert), (fromNativeMenu || !source.isHiddenOrHasHiddenAncestor || presenter?.toolbar.contains(source) == true),
              source.isButton, dialogInvocation(source) != nil || source.handlers.contains("press") else { return nil }
        return { [weak self, weak source] in
            guard let self, let source else { return }
            let bubbled = bubbled && self.isBehindModal(source)
            guard self.live(source), !source.disabled, !(bubbled ? source.authoredInert : source.inert),
                  let invocation = self.dialogInvocation(source), self.live(invocation.target),
                  !bubbled || !invocation.target.inert else { return }
            if invocation.action == .showModal { self.show(invocation.target) }
            // HTML's `close` does nothing to a dialog showing as a popover.
            else if self.presenter?.menus.showsAsPopover(invocation.target) == false {
                self.close(invocation.target)
            }
        }
    }
    private func dialogInvocation(_ source: NodeView) -> Invocation? {
        guard let invocation = presenter?.invocation(of: source),
              invocation.action == .showModal || invocation.action == .close else { return nil }
        return invocation
    }
    func show(_ dialog: NodeView) {
        guard let presenter, !owns(dialog), live(dialog), let window = presenter.viewport.window,
              let parent = dialog.superview, parent.isDescendant(of: presenter.root) || parent === presenter.root
                || entries.contains(where: { parent.isDescendant(of: $0.dialog) || parent === $0.dialog }) else { return }
        // A confirmation in the alert shape is the platform's alert, a sheet
        // on the window (LLP 1115.000 D7.6, AlertsMac.swift). HTML: showModal
        // on an open modal dialog does nothing.
        if presenter.alerts.isOpen(dialog) || presenter.alerts.present(dialog, from: nil, popover: false) { return }
        // Over the dialog that is open, when one is: its sheet's window.
        let front = self.window ?? window
        present(Entry(dialog, previous: focusOwner(front)), over: front)
    }
    /// `entry` joins the top layer: a dialog painted or in its sheet, a
    /// modal route in its sheet.
    func present(_ entry: Entry, over front: NSWindow) {
        guard let presenter else { return }
        let dialog = entry.dialog
        presenter.menus.reset()
        entries.append(entry)
        if painted, !entry.route {
            entry.backdrop.host = self
            entry.backdrop.autoresizingMask = [.width, .height]
            presenter.viewport.addSubview(entry.backdrop)
            entry.backdrop.addSubview(dialog)
        } else { prepare(entry, over: front) }
        dialog.isHidden = false
        layout()
        presenter.mouseSwipe.cancel(); presenter.mouseDrags.height.cancel()
        presenter.mouseDrags.transform.cancel(); presenter.mouseDrags.reorder.cancel()
        presenter.mouseLayoutPan.abandon(); presenter.selection.clear()
        presenter.session?.canvases.cancelMovedControls()
        refreshInputGates()
        presenter.syncKeyViewLoop()
        syncDefaultButton()
        presenter.syncAccessibility()
        presenter.shortcuts.sync()
        // The focus is in the sheet before it shows: it arrives with it.
        enterFocus()
        // AppKit brings the sheet in before `begin` returns, and the main
        // queue turns meanwhile: the dialog may have closed in it. A default
        // button arrives without its key equivalent (`DefaultButtonSheetMacTests`
        // holds that it has one), so it is given again.
        if entry.sheet != nil, begin(entry) { syncDefaultButton() }
    }
    func close(_ dialog: NodeView, restoreFocus: Bool = true, notify: Bool = true) {
        if presenter?.alerts.close(dialog) == true { return }
        guard entries.contains(where: { $0.dialog === dialog }) else { return }
        presenter?.mouseChain.bubbledPress?.cancel(in: dialog)
        // What stands on its sheet leaves first: an alert, a dialog opened over it.
        clear(above: dialog)
        guard let i = entries.firstIndex(where: { $0.dialog === dialog }) else { return }
        for popover in presenter?.menus.presented ?? [] where presenter?.menus.contains(dialog, popover) == true {
            presenter?.menus.close(popover, restoreFocus: false)
        }
        // The window the focus goes back to: the page's, or the one a sheet stood on.
        let window = entries[i].sheet?.over ?? presenter?.viewport.window
        let hadFocus = window.map { ownsFocus($0) } ?? false
        let entry = entries.remove(at: i)
        let wasTop = i == entries.count
        // Its sheet leaves with the dialog still in it, as a person watches it go.
        if let sheet = entry.sheet { end(entry, sheet) }
        // A route is shown and hidden by navigation.
        if !entry.route { dialog.isHidden = true }
        if entry.sheet == nil {
            dialog.removeFromSuperview()
            entry.backdrop.removeFromSuperview()
            if live(dialog), let parent = entry.parent {
                let siblings = parent.subviews
                parent.addSubview(dialog, positioned: .below, relativeTo: entry.index < siblings.count ? siblings[entry.index] : nil)
                dialog.frame = entry.frame
            }
        }
        refreshInputGates()
        presenter?.syncKeyViewLoop()
        syncDefaultButton()
        // What held the focus when it opened takes it back: the page under
        // a sheet gave it up as its nodes became inert, as under a painted one.
        if wasTop, restoreFocus, hadFocus, let window {
            if let previous = entry.previous, previous.window === window,
               !previous.isHiddenOrHasHiddenAncestor, !blocks(previous),
               !((previous as? NodeView)?.inert ?? false) {
                window.makeFirstResponder(previous)
                if let range = entry.selection, let editor = window.firstResponder as? NSTextView { editor.setSelectedRange(range) }
            } else if active != nil { enterFocus() }
            else { window.makeFirstResponder(presenter?.viewport) }
        }
        presenter?.syncAccessibility()
        presenter?.shortcuts.sync()
        if notify, !entry.route { dialog.queueDialogClose() }
        presenter?.afterBatch { [weak presenter] in presenter?.toolbar.sync() }
    }
    func reset() {
        presenter?.alerts.reset()
        // A route's sheet is not brought back by the batch its leaving sends: navigation still names its route.
        let was = routeSync.running
        routeSync.running = true
        while let dialog = active { close(dialog, restoreFocus: false, notify: false) }
        routeSync = (was, false)
    }
    /// A children op still addresses the authored parent, even while a dialog
    /// is in the top layer. Removing that edge retires its presentation.
    func children(_ parent: NSView, _ wanted: [NodeView]) {
        // A route's sheet leaves after the batch (`syncRoutes`), never inside it.
        for entry in entries where !entry.route && (entry.parent === parent) != wanted.contains(where: { $0 === entry.dialog }) {
            close(entry.dialog, notify: false)
        }
    }
    func frame(_ node: NodeView, _ frame: NSRect) -> Bool {
        guard let entry = entries.first(where: { $0.dialog === node }) else { return false }
        // In its sheet the frame is in the sheet's own space (its block's).
        if entry.sheet != nil { if node.frame != frame { node.frame = frame }; return true }
        entry.frame = frame
        layout()
        return true
    }
    func sync() {
        guard let presenter else { return }
        presenter.alerts.sync()
        for entry in entries where !entry.route {
            let parent = entry.parent
            let connected = parent === presenter.root || parent?.isDescendant(of: presenter.root) == true
                || entries.contains { other in other !== entry && (parent === other.dialog || parent?.isDescendant(of: other.dialog) == true) }
            if !live(entry.dialog) || entry.dialog.props["semanticTag"] != "dialog" || !connected || presenter.viewport.window == nil {
                close(entry.dialog, notify: false)
            }
        }
        var changed = false
        for dialog in presenter.carrying("tag:dialog") where dialog.props["popover"] == nil {
            let hidden = !owns(dialog) || dialog.style["display"]?.string == "none"
            if dialog.isHidden != hidden { dialog.isHidden = hidden; changed = true }
        }
        if changed || accessibleActive.map(ObjectIdentifier.init) != accessibilityBlocker { refreshInputGates() }
        layout()
        if active != nil || drawnDefault != nil { syncDefaultButton() }
        if let window, active != nil,
           ownsFocus(window), focusOwner(window).map({ blocks($0) }) ?? true { enterFocus() }
    }
    private func refreshInputGates() {
        guard let presenter else { return }
        accessibilityBlocker = accessibleActive.map(ObjectIdentifier.init)
        presenter.navigation.refreshInputGates(Array(presenter.views.values))
        presenter.topLayerChanged()
    }
    func layout() {
        guard let presenter else { return }
        fitSheets()
        for entry in entries where entry.sheet == nil {
            entry.backdrop.frame = presenter.viewport.bounds
            var frame = entry.frame
            // Without authored insets the HTML modal is centred in its viewport.
            let style = entry.dialog.style
            if let left = length(style["left"], entry.backdrop.bounds.width) { frame.origin.x = left }
            else if let right = length(style["right"], entry.backdrop.bounds.width) { frame.origin.x = entry.backdrop.bounds.width - right - frame.width }
            else {
                frame.origin.x = max(0, (entry.backdrop.bounds.width - frame.width) / 2)
            }
            if let top = length(style["top"], entry.backdrop.bounds.height) { frame.origin.y = top }
            else if let bottom = length(style["bottom"], entry.backdrop.bounds.height) { frame.origin.y = entry.backdrop.bounds.height - bottom - frame.height }
            else {
                frame.origin.y = max(0, (entry.backdrop.bounds.height - frame.height) / 2)
            }
            entry.dialog.frame = frame
        }
    }
    private func length(_ value: BatchValue?, _ basis: CGFloat) -> CGFloat? {
        value?.length(basis: basis)
    }
    /// The dialog's stops in tree order, and how many of them come before
    /// `start` when it is a responder in the dialog.
    private func stops(_ dialog: NodeView, start: NSView? = nil) -> (views: [(view: NSView, order: Int)], before: Int?) {
        guard let presenter else { return ([], nil) }
        var views: [(view: NSView, order: Int)] = [], before: Int?
        func walk(_ node: NodeView) {
            guard !node.inert, !node.isHidden else { return }
            let view = presenter.keyView(of: node)
            if Presenter.tabbable(node) { views.append((view, node.tabOrder)) }
            else if node === start || view === start { before = views.count }
            for run in node.syncInlineKeyViews() {
                if run.canBecomeKeyView { views.append((run, run.tabOrder)) }
                else if run === start { before = views.count }
            }
            for case let child as NodeView in node.container.subviews { walk(child) }
            for popover in presenter.menus.following(node) { walk(popover) }
        }
        walk(dialog)
        return (views, before)
    }
    private func candidates(_ dialog: NodeView) -> [NSView] { candidates(stops(dialog).views) }
    private func candidates(_ views: [(view: NSView, order: Int)]) -> [NSView] {
        views.enumerated().sorted { a, b in
            let ai = a.element.order, bi = b.element.order
            let ap = ai > 0 ? ai : Int.max, bp = bi > 0 ? bi : Int.max
            return ap == bp ? a.offset < b.offset : ap < bp
        }.map { $0.element.view } + (entries.last?.sheet?.chrome?.stops ?? []) // then a route's sheet's own buttons
    }
    func enterFocus() {
        guard let presenter, let dialog = active, let window = dialog.window else { return }
        let candidates = candidates(dialog)
        let autofocus = presenter.carrying("autofocus").first {
            ($0 === dialog || $0.isDescendant(of: dialog)) && $0.props["autofocus"] == "true" && !$0.inert && !$0.disabled && !$0.isHiddenOrHasHiddenAncestor
        }
        let focus = autofocus.map { presenter.keyView(of: $0) } ?? candidates.first ?? dialog
        window.makeFirstResponder(focus)
    }
    private func ownsFocus(_ window: NSWindow) -> Bool {
        guard let owner = focusOwner(window), let viewport = presenter?.viewport else { return true }
        // A sheet's window holds this session's dialog and nothing else.
        if entries.contains(where: { $0.sheet?.window === window }) { return true }
        return owner === viewport || owner.isDescendant(of: viewport) || owner === presenter?.session?.view
    }
    /// Shared by real key delivery and the agent's direct NSWindow delivery.
    func key(_ event: NSEvent) -> Bool {
        guard let dialog = active, event.window === dialog.window else { return false }
        if let window = dialog.window, !ownsFocus(window) { return false }
        // Composition owns Escape and Tab until the input method commits it.
        if (dialog.window?.firstResponder as? NSTextInputClient)?.hasMarkedText() == true { return false }
        // Escape, and Command-period, which AppKit gives a sheet's Cancel too.
        let period = event.type == .keyDown && event.keyCode == 47
            && event.modifierFlags.intersection([.command, .control, .option, .shift]) == .command
        if event.keyCode == 53 || period {
            if event.type == .keyDown, !event.isARepeat, dialog.props["closedby"] != "none" { dismiss(dialog) }
            return true
        }
        // Return is the default button's, when the dialog has one (LLP 1115.000 D7.3).
        if pressDefault(event, in: dialog) { return true }
        if event.type == .keyDown, event.keyCode == 48 {
            // A node that holds the focus and is no stop (the dialog itself, a `tabindex=-1` box) is where Tab
            // starts, as on the page (KeyViewLoopMac.swift's starts): the stop after it in tree order, Shift-Tab
            // the one before it, and past either end the trap's first or last. With the focus given up by a control
            // that became disabled, the start is that control (`Presenter.tabStart`).
            let start = dialog.window.flatMap(focusOwner) ?? presenter?.tabStart
            let (views, before) = stops(dialog, start: start)
            let choices = candidates(views)
            guard let window = dialog.window, !choices.isEmpty else { enterFocus(); return true }
            let backwards = event.modifierFlags.contains(.shift)
            if let before {
                window.makeFirstResponder(backwards ? (before > 0 ? views[before - 1].view : choices[choices.count - 1])
                    : (before < views.count ? views[before].view : choices[0]))
                return true
            }
            let current = focusOwner(window).flatMap { owner -> Int? in
                var ancestor: NSView? = owner
                while let view = ancestor {
                    if let index = choices.firstIndex(where: { $0 === view
                        || ($0 as? NSTextField)?.currentEditor() === window.firstResponder }) { return index }
                    ancestor = view.superview
                }
                return nil
            }
            let index = current.map { ($0 + (backwards ? choices.count - 1 : 1)) % choices.count }
                ?? (backwards ? choices.count - 1 : 0)
            window.makeFirstResponder(choices[index])
            return true
        }
        if let window = dialog.window, focusOwner(window).map({ blocks($0) }) ?? true { enterFocus() }
        return false
    }
    func outside() {
        if let dialog = active, dialog.props["closedby"] == "any" { dismiss(dialog) }
    }
    /// The platform's dismissal (Escape, Command-period, a click behind a
    /// `closedby="any"` sheet): a dialog closes; a modal route goes back as
    /// its stack goes, which the app decides (BackMac.swift).
    private func dismiss(_ dialog: NodeView) {
        guard entry(ofRoute: dialog) != nil else { return close(dialog) }
        if let refused = presenter?.navigation.dismiss(dialog) { presenter?.session?.log("modal dismissal refused: \(refused)") }
    }
    var observation: [String: Any]? {
        presenter?.alerts.observation ?? entries.last { !$0.route }.map {
            ["kind": "dialog", "dialog": Int($0.dialog.id), "closedby": $0.dialog.props["closedby"] ?? "closerequest", "phase": "open",
             "presentation": $0.sheet == nil ? "painted" : "sheet"]
        }
    }
}

final class DialogBackdrop: NSView {
    weak var host: DialogHost?
    private var downOutside = false
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) { NSColor.black.withAlphaComponent(0.18).setFill(); dirtyRect.fill() }
    /// A backdrop is not a control: the click that activates its window does
    /// not also dismiss the dialog (LLP 1115.000 D7.9). The agent's does.
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { NodeView.everyClickCounts }
    private func outside(_ event: NSEvent) -> Bool {
        guard let parent = superview else { return false }
        return hitTest(parent.convert(event.locationInWindow, from: nil)) === self
    }
    override func mouseDown(with event: NSEvent) { downOutside = outside(event) }
    override func mouseUp(with event: NSEvent) {
        defer { downOutside = false }
        if downOutside, outside(event) { host?.outside() }
    }
    override func scrollWheel(with event: NSEvent) {}
}
#endif

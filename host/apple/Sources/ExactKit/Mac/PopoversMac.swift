// @ref LLP 1115.002 D7.3 — a popover that is not a menu is an `NSPopover`.
// A popover whose rows are a menu's is the `NSMenu` it was (MenusMac.swift),
// and one in the alert shape the alert (AlertsMac.swift). Any other is
// AppKit's popover: a window of its own beside its invoker, with AppKit's
// arrow, material and shadow, which may leave the window it was opened in
// (the painted one was clipped to it). The popover's own subtree moves into
// it, as a dialog's into its sheet (SheetsMac.swift), laid out in a
// containing block that is a maximum on each axis, and the popover's content
// size is what comes back.
//
// Measured first, in a window really made key (macOS 27.0.1;
// `~/audits/exact2-no-tells-2026-10-11/mac-sheets/`):
// - Its window is its content plus 13 points a side, the arrow inside that
//   margin; AppKit moves it to another edge when the one asked for has no
//   room on the screen.
// - It takes the keyboard: its first text field holds the caret as it
//   arrives, Tab stays inside it, and the window under it keeps its own
//   first responder, which has the keyboard again when the popover closes.
// - `.transient` closes on Escape, on a click anywhere else (the click
//   still reaches what it was on), when another window becomes key and when
//   the app is no longer active; a click on the button it is anchored to
//   closes it and does not press that button. `.applicationDefined` closes
//   for none of those.
//
// So `popover="auto"` is `.transient` and `popover="manual"` is
// `.applicationDefined`. What the page's own rules close (Escape after the
// `key` handlers, a click outside as the driver sends one, the invoker's
// toggle, a batch that takes the popover or its invoker away) still closes
// it, through the same door as before; what AppKit closes itself closes the
// page's popover with it. The edge is the popover's `position-area`: above
// for `top`, beside for `right`, below otherwise; one whose author wrote
// `left` or `top` points at that point of the viewport instead (LLP 1021
// D2), as a hand-built app points one at a click. Its margin on the side
// that faces its invoker stands between the two (AppKit's popover has none
// of its own, so the rectangle it is anchored to is moved out by it); the
// popover itself is the size of its border box.
//
// The driver's own chrome (`--chrome agent`) paints it in the top layer as
// before, and a menu's shape is painted there under either chrome: over the
// page, or in the sheet of the dialog its invoker is in.
#if os(macOS)
import AppKit

/// One popover as AppKit shows it: the `NSPopover`, the view that holds the
/// subtree, and where the subtree goes back to.
final class NativePopover: NSObject, NSPopoverDelegate {
    let popover = NSPopover()
    let content = BlockView(frame: .zero)
    weak var host: MenuHost?
    weak var node: NodeView?
    /// The popover's box in the page, for its return.
    let home: NSRect
    private var finished = false
    var window: NSWindow? { content.window }

    init(host: MenuHost, node: NodeView, home: NSRect) {
        self.host = host; self.node = node; self.home = home
        super.init()
        let controller = NSViewController()
        content.frame = NSRect(origin: .zero, size: home.size)
        controller.view = content
        popover.contentViewController = controller
        popover.contentSize = home.size
        popover.delegate = self
    }
    /// Where it points while shown, by its margins (the gap to its invoker):
    /// the view it was shown against, and the rectangle for given margins in it.
    var positioning: (view: NSView, rect: (PositionArea.Margins) -> NSRect)?
    var margins = PositionArea.Margins()
    /// Its margins changed while shown: the gap to its invoker follows.
    func reanchor(_ margins: PositionArea.Margins) {
        guard margins != self.margins else { return }
        self.margins = margins
        if let positioning, popover.isShown { popover.positioningRect = positioning.rect(margins) }
    }
    /// Its insets moved while shown (LLP 1021 D2): it points at the new point.
    func repoint() {
        guard let positioning, popover.isShown else { return }
        let rect = positioning.rect(margins)
        if popover.positioningRect != rect { popover.positioningRect = rect }
    }
    /// The content's size, as layout returned it.
    func fit(_ size: CGSize) {
        guard size.width > 0, size.height > 0, popover.contentSize != size else { return }
        popover.contentSize = size
    }
    /// AppKit has taken the popover away: its own close, or this host's.
    func popoverDidClose(_ notification: Notification) { host?.closed(self) }
    /// The subtree goes back under its authored parent, where the kernel
    /// lays it out again. Once.
    func finish() {
        guard !finished else { return }
        finished = true
        host?.leaving.removeAll { $0 === self }
        guard let presenter = host?.presenter, let node else { return }
        if presenter.views[node.id] === node { node.frame = home }
        presenter.blocks.release(node)
    }
}

extension MenuHost {
    func wantsWindow(_ pop: NodeView) -> Bool {
        pop.style["position_try_fallbacks"] == nil && !painted
            && !(ExactEnv.agentMode && (isMenuShaped(pop) || isConfirmation(pop)))
    }

    func paint(_ entry: Entry, in window: NSWindow) {
        entry.layer.autoresizingMask = [.width, .height]
        layerHost(entry, in: window)?.addSubview(entry.layer)
        entry.layer.setPaintForeground()
        entry.layer.addSubview(entry.popover)
        entry.popover.isHidden = false
    }

    /// The view that owns a painted popover's layer: its invoker's split
    /// pane, page or sheet. `layout` moves the layer when that changes (a
    /// split collapsing takes the route back into the page).
    func layerHost(_ entry: Entry, in window: NSWindow?) -> NSView? {
        guard let presenter else { return nil }
        if let pane = presenter.navigation.split.pane(holding: entry.source) { return pane }
        guard let window else { return nil }
        return window === presenter.viewport.window ? presenter.viewport : window.contentView ?? presenter.viewport
    }

    func restore(_ entry: Entry) {
        let pop = entry.popover
        pop.removeFromSuperview()
        entry.layer.removeFromSuperview()
        if live(pop), let parent = entry.parent {
            let siblings = parent.subviews
            if entry.index < siblings.count { parent.addSubview(pop, positioned: .below, relativeTo: siblings[entry.index]) }
            else { parent.addSubview(pop) }
            pop.frame = entry.frame
        }
    }

    /// A style change moves the same open subtree between presentations;
    /// it neither toggles the popover nor gives it a new presentation identity.
    func syncPresentation(_ entry: Entry) {
        let pop = entry.popover, hasFallbacks = pop.style["position_try_fallbacks"] != nil
        guard entry.hasFallbacks != hasFallbacks else { return }
        entry.hasFallbacks = hasFallbacks
        guard entry.menu == nil, let presenter, presenter.viewport.window != nil,
              let source = entry.source, let window = source.window,
              wantsWindow(pop) != (entry.native != nil) else { return }
        let oldWindow = pop.window
        let focused = oldWindow.flatMap(focusOwner)
        let selection = (oldWindow?.firstResponder as? NSTextView)?.selectedRange()
        let wasKey = oldWindow?.isKeyWindow == true
        if let native = entry.native {
            // Closing only the container must not close the logical popover,
            // and no outgoing animation may later take its subtree back.
            entry.native = nil
            native.popover.animates = false
            dismiss(native)
            paint(entry, in: window)
        } else {
            // BlockHost must record the authored parent as the subtree's home.
            restore(entry)
            if !present(entry, from: source, in: window) { paint(entry, in: window) }
        }
        layout()
        refresh()
        if let focused, let destination = focused.window {
            if wasKey { destination.makeKey() }
            destination.makeFirstResponder(focused)
            if let selection, let editor = destination.firstResponder as? NSTextView { editor.setSelectedRange(selection) }
        }
    }

    /// The toolbar-item API chooses its own edge and has no gap parameter.
    static func usesToolbarAnchor(_ pop: NodeView) -> Bool {
        pop.style["position_area"] == nil && PositionArea.margins(of: pop) == .init()
    }

    /// Whether `window` is one of this session's popovers'.
    func hosts(_ window: NSWindow) -> Bool {
        entries.contains { $0.native?.window === window } || leaving.contains { $0.window === window }
    }
    /// The popovers AppKit shows, each a Tab loop of its own (KeyViewLoopMac.swift).
    var windowed: [NativePopover] { entries.compactMap(\.native) }

    /// Show `entry`'s popover as AppKit's, against `source`. False when
    /// there is nothing to anchor it to: it is painted then.
    func present(_ entry: Entry, from source: NodeView, in window: NSWindow) -> Bool {
        guard let presenter else { return false }
        let pop = entry.popover
        // At its authored `left`/`top` (LLP 1021 D2): a 1×1 rectangle at that
        // point of the viewport, as a hand-built app points a popover at a
        // click, read again as the insets change while it shows (`repoint`).
        // It is the window's, as the CSS box is the viewport's: not the
        // invoker's, which the point may lie outside of or scroll away from.
        let windowContent = window.contentView
        let pressed = { [weak presenter, weak pop, weak windowContent] () -> NSRect? in
            guard let presenter, let pop, let windowContent, let at = PositionArea.inset(of: pop, viewport: presenter.clientSize) else { return nil }
            return NSRect(origin: presenter.point(fromClient: at, in: windowContent), size: CGSize(width: 1, height: 1))
        }
        // A button in the window's bar has no box in the page: its item's.
        // AppKit shows no popover against an anchor it cannot see (measured:
        // a window not on screen, a hidden view, one scrolled out of sight).
        let view: NSView, invoker: NSRect
        if let windowContent, pressed() != nil, source.window != nil {
            (view, invoker) = (windowContent, windowContent.convert(self.anchor(source, in: source, run: entry.run), from: source))
        } else if let content = windowContent, let bar = presenter.toolbar.anchor(of: source, in: content) { (view, invoker) = (content, bar) }
        else if source.window != nil { (view, invoker) = (source, self.anchor(source, in: source, run: entry.run)) }
        else { return false }
        guard view.window?.isVisible == true else { return false }
        // Pointed at in `view`: one opened at its invoker and given insets
        // later points at them from the invoker's coordinates.
        let pointed = { [weak view, weak windowContent] () -> NSRect? in
            guard let view, let rect = pressed() else { return nil }
            return view === windowContent ? rect : view.convert(rect, from: windowContent)
        }
        let anchor = pointed() ?? invoker
        let native = NativePopover(host: self, node: pop, home: entry.frame)
        entry.native = native
        // AppKit's popover gives its own block and takes it back on its
        // finish: a region told before is told again if it is painted again,
        // and one not yet told is not told over AppKit's.
        entry.block = nil
        presenter.regionsUntold.removeValue(forKey: pop.id)
        native.popover.behavior = entry.value == "manual" ? .applicationDefined : .transient
        native.popover.animates = !ExactEnv.agentFreezes
        native.content.pointerTracking = { [weak presenter] in presenter?.trackPointer($0) }
        // As large as its content, at most what the screen can show beside AppKit's own margin.
        // A block's used size holds the box's margins, which are no part of the popover.
        let screen = (window.screen ?? NSScreen.main)?.visibleFrame.size ?? window.frame.size
        let maximum = CGSize(width: max(1, screen.width - 2 * Self.popoverMargin), height: max(1, screen.height - 2 * Self.popoverMargin))
        let margins = PositionArea.margins(of: pop), edge = Self.edge(PositionArea.of(pop))
        presenter.blocks.show(pop, in: native.content, block: .atMost(maximum)) { [weak native, weak pop] size in
            // Its margins as they are now: a change while open resizes it too.
            let margins = pop.map(PositionArea.margins(of:)) ?? margins
            native?.fit(CGSize(width: size.width - margins.left - margins.right, height: size.height - margins.top - margins.bottom))
            native?.reanchor(margins)
        }
        // A presenter with no session lays nothing out again: the popover keeps the page's box.
        if presenter.blocks.used[pop.id] == nil { pop.setFrameOrigin(.zero) }
        pop.isHidden = false
        // The author's margin stands between the popover and its invoker: the
        // anchor's rectangle moved out by it, in the window's content view,
        // since AppKit keeps a rectangle to the view it is given in.
        // A margin changed while it is shown moves the rectangle in the same view.
        let content = view.isFlipped ? window.contentView : nil
        let at: NSView = margins.gap(edge) > 0 ? content ?? view : view
        let pointing = { [weak view, weak at] (margins: PositionArea.Margins) -> NSRect in
            guard let view, let at else { return anchor }
            let gap = view.isFlipped ? margins.gap(edge) : 0
            var rect = pointed() ?? invoker
            switch edge {
            case .minY: rect.origin.y -= gap; rect.size.height += gap
            case .maxY: rect.size.height += gap
            case .maxX: rect.size.width += gap
            default: rect.origin.x -= gap; rect.size.width += gap
            }
            return at === view ? rect : at.convert(rect, from: view)
        }
        let rect = pointing(margins)
        native.positioning = (at, pointing)
        native.margins = margins
        if Self.usesToolbarAnchor(pop), pointed() == nil, let item = presenter.toolbar.items[source.id], window.toolbar?.items.contains(where: { $0 === item }) == true {
            native.positioning = nil // a toolbar item's popover points at the item
            native.popover.show(relativeTo: item)
        } else {
            native.popover.show(relativeTo: rect, of: at, preferredEdge: at.isFlipped ? edge : Self.unflipped(edge))
        }
        guard native.popover.isShown else {
            // AppKit showed none (its invoker is hidden or scrolled out of
            // sight): the page paints it, as it would have. Its subtree goes
            // home first, and is no open popover while it does: the batch
            // that takes its block back would show it in the page and count
            // its text twice.
            entry.native = nil
            pop.isHidden = true
            native.popover.close()
            apart(entry) { native.finish() }
            return false
        }
        // The Tab loop in it is this session's, never AppKit's geometric one.
        native.window?.autorecalculatesKeyViewLoop = false
        presenter.syncKeyViewLoop()
        // AppKit puts the caret in its first field; an `autofocus` the author wrote wins.
        if let target = presenter.carrying("autofocus").first(where: {
            contains(pop, $0) && $0.props["autofocus"] == "true" && !$0.inert && !$0.disabled && !$0.isHiddenOrHasHiddenAncestor
        }) { native.window?.makeFirstResponder(presenter.keyView(of: target)) }
        return true
    }
    /// What AppKit leaves around a popover's content, a side (measured).
    static let popoverMargin: CGFloat = 13
    /// The edge of the invoker a `position-area` names, in a flipped view
    /// (a node's): `top` above it, `right` beside it, anything else below.
    static func edge(_ area: String) -> NSRectEdge {
        area.hasPrefix("top") ? .minY : area.hasPrefix("right") ? .maxX : area.hasPrefix("left") ? .minX : .maxY
    }
    private static func unflipped(_ edge: NSRectEdge) -> NSRectEdge { edge == .minY ? .maxY : edge == .maxY ? .minY : edge }

    /// This host closes `native`: the page's rules did, or its own buttons.
    /// AppKit animates it away and says when it has gone (`closed`); one
    /// that is not shown, or shown with no animation, has gone already.
    func dismiss(_ native: NativePopover) {
        leaving.append(native)
        if native.popover.isShown { native.popover.close() }
        if !native.popover.isShown || !native.popover.animates { native.finish() }
    }
    /// AppKit says `native` has gone. Closed by AppKit itself (a click
    /// elsewhere, another window, the app left), the page's popover closes
    /// with it; a click on its own invoker did it without pressing the
    /// invoker, and where that press is still to come it opens nothing.
    func closed(_ native: NativePopover) {
        if let entry = entries.first(where: { $0.native === native }) {
            if let event = NSApp.currentEvent, [.leftMouseDown, .leftMouseUp].contains(event.type), let source = entry.source,
               let hit = event.window?.contentView?.superview?.hitTest(event.locationInWindow), contains(source, hit) {
                dismissedByClick = (ObjectIdentifier(source), event.eventNumber)
            }
            close(entry.popover, restoreFocus: false)
        }
        native.finish()
    }
    /// Whether the click being handled is the one with which AppKit closed
    /// `source`'s popover: its press toggles nothing open again.
    func closedByThisClick(_ source: NodeView) -> Bool {
        guard let mark = dismissedByClick, mark.source == ObjectIdentifier(source), let event = NSApp.currentEvent,
              [.leftMouseDown, .leftMouseUp].contains(event.type), event.eventNumber == mark.click else { return false }
        return true
    }
}
#endif

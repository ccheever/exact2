// DOM's `wheel` and `drop` on the Mac (studio diary R3, R19), beside the
// pointer's events (MouseChainMac.swift) and `contextmenu`'s point
// (NodeViewMac's `rightMouseDown`).
//
// A wheel's turn or a trackpad's scroll over a node is heard by every
// node from it up that declares `wheel` (a disabled box included, as a
// `<div disabled>` in Chrome; not a disabled form control or an inert node), innermost first, as the
// DOM's bubbles; one that calls `preventDefault()` keeps the scroll from
// happening (a canvas zooming on ⌘-scroll). A trackpad's pinch is a wheel
// with Control held and `deltaY` of -100 × the magnification, which is how
// a browser on a Mac delivers one, so one handler zooms on both hosts.
//
// Files dragged in from Finder land on the innermost node that declares
// `drop`: each a `doc:` handle minted for this session (LLP 1069.010 D1),
// of the types the manifest's `file_handlers` declares — the bound the
// pickers keep (D2) — and refused into the journal otherwise.
#if os(macOS)
import AppKit
import UniformTypeIdentifiers

extension Presenter {
    /// Chrome on macOS opens an unmodified ContextMenu keydown at the
    /// current focus, after its key handlers; Shift+F10 has no such default.
    func contextMenuKey(_ event: NSEvent, in owner: NSWindow? = nil) -> Bool {
        guard event.type == .keyDown, NodeView.keyName(event) == "ContextMenu",
              event.modifierFlags.intersection([.shift, .control, .option, .command]).isEmpty,
              let window = owner ?? event.window, window === viewport.window,
              (window.firstResponder as? NSTextInputClient)?.hasMarkedText() != true else { return false }
        if let run = window.firstResponder as? InlineKeyView, run.owner?.inlineContextMenu(run.id) == true { return true }
        let point: NSPoint
        var next: NSView?
        if let focus = keyTarget(window.firstResponder) {
            guard let middle = contextPoint(focus) else { return false }
            point = middle
            next = focus
        } else {
            // A handler removed the focus: Chrome hits the page at (1,1).
            // An embedded view/editor still owns its keys, even when its
            // responder is not one of our focusable nodes.
            let responder = window.firstResponder
            guard responder == nil || responder === window || responder === viewport
                    || responder === root || responder === session?.view,
                  session?.view?.ownsShortcutFocus() ?? true else { return false }
            let clip = viewport.contentView
            point = clip.convert(NSPoint(x: clip.bounds.minX + 1, y: clip.bounds.minY + 1), to: nil)
            next = viewport.hitTest(viewport.superview?.convert(point, from: nil) ?? point)
        }
        while let view = next {
            if let node = view as? NodeView, (node.field != nil || node.textArea != nil) { return false }
            if let node = view as? NodeView, views[node.id] === node, node.answersContextMenu { return contextMenu(node, at: point) }
            next = menus.parent(of: view)
        }
        return false
    }

    /// Where a request from the keyboard lands on `node` (the ContextMenu
    /// key at the focus, VoiceOver's show menu): the middle of `rect` in its
    /// space (its box when nil) as drawn, clipped by what clips it, in the
    /// window; nil when it takes no request or none of it is drawn.
    func contextPoint(_ node: NodeView, rect: NSRect? = nil) -> NSPoint? {
        guard !node.disabled, !node.inert, !node.isHiddenOrHasHiddenAncestor,
              !dialogs.blocks(node), node.field == nil, node.textArea == nil else { return nil }
        // AppKit's visibleRect clips the untransformed frame. Clip the
        // drawn anchor instead, including a frame translated into view.
        var drawn = node.drawnRect(rect ?? menus.anchor(node, in: node))
        var ancestor = node.superview
        while let view = ancestor {
            if let node = view as? NodeView, node.overflowClips {
                drawn = drawn.intersection(node.drawnRect(node.bounds))
            } else if let clip = view as? NSClipView {
                let rect = (clip.superview?.superview as? NodeView).map {
                    $0.drawnRect($0.convert(clip.bounds, from: clip))
                } ?? clip.convert(clip.bounds, to: nil)
                drawn = drawn.intersection(rect)
            }
            ancestor = view.superview
        }
        return drawn.isEmpty ? nil : NSPoint(x: drawn.midX, y: drawn.midY)
    }

    /// `node`'s `contextmenu` and `contextPopover` at window point `point`,
    /// as a request from the keyboard sends them: no button held.
    func contextMenu(_ node: NodeView, at point: NSPoint) -> Bool {
        let local = node.local(point), box = node.contentBox(), client = self.client(point)
        let sample = PointerSample(x: Double(local.x - box.minX), y: Double(local.y - box.minY),
                                   buttons: 0, pressure: 0, type: "mouse", id: 1,
                                   clientX: Double(client.x), clientY: Double(client.y))
        return node.dispatchContextMenu(at: node.convert(point, from: nil), sample: sample)
    }
}

extension NodeView {
    /// Whether its own `contextmenu` or `contextPopover` answers a context request.
    var answersContextMenu: Bool { handlers.contains("contextmenu") || props["contextPopover"]?.isEmpty == false }

    /// The context action runs before the menu is read, for pointer and
    /// keyboard alike. A context action cannot leak its preventDefault to
    /// another input; keydown cancellation was handled before this default.
    func dispatchContextMenu(at point: NSPoint, sample: PointerSample) -> Bool {
        guard let presenter, presenter.views[id] === self, !disabled, !inert else { return false }
        let action = handlers.contains("contextmenu"), menu = props["contextPopover"]?.isEmpty == false
        let outer = presenter.defaultPrevented
        presenter.defaultPrevented = false
        defer { presenter.defaultPrevented = outer }
        if action { presenter.mouseEvent(id, 10, sample.line) }
        if menu { presenter.menus.context(self, at: point) }
        return action || menu
    }

    /// A Control-click is the Mac's secondary click (X85): Chrome sends it as
    /// `contextmenu` and no `click`. AppKit asks the view under it for
    /// `menu(for:)` and, given none, sends `mouseDown`. Where a node from
    /// here up answers it (`contextMenuAuthored`), its down, drags and
    /// release take a right-click's path (`rightMouseDown`) with the primary
    /// button and Control the event holds, and nothing presses. Selectable
    /// text with none above had its menu from `menu(for:)` (#454); anywhere
    /// else it stays a click with Control held.
    func contextClick(_ event: NSEvent) -> Bool {
        contextClicked = event.type == .leftMouseDown && event.modifierFlags.contains(.control)
            && (contextMenuAuthored || inlineContextTarget(at: local(event.locationInWindow)) != nil)
        return contextClicked
    }

    /// Whether run `id`'s own `contextmenu` or `contextPopover` answers a context request.
    func inlineAnswersContextMenu(_ id: UInt32) -> Bool {
        guard let presenter, presenter.inlineEnabled(id), let run = presenter.inlineText(id) else { return false }
        return run.handlers.contains("contextmenu") || run.props["contextPopover"]?.isEmpty == false
    }

    /// The run a secondary click at `point` (this paragraph's space) reaches
    /// (X77): the nearest from the innermost run there up whose own
    /// `contextmenu` or `contextPopover` answers, as the web's `contextmenu`
    /// bubbles from the run's element to its first handler. Nil where none
    /// does, and the paragraph and its ancestors hear the click as before.
    func inlineContextTarget(at point: CGPoint) -> InlineText? {
        guard !inlineText.isEmpty, let presenter else { return nil }
        var run = inlineTarget(at: point)
        while let current = run {
            if inlineAnswersContextMenu(current.id) { return current }
            run = presenter.inlineText(current.parent)
        }
        return nil
    }

    /// A secondary click's `contextmenu` on the run under it (X77), after
    /// the button's `pointerdown`, from the run's first line's top left as
    /// Chrome measures an inline element's offset, then its
    /// `contextPopover`'s menu at the click. True ends the click, as
    /// `dispatchContextMenu` returning true does for a node.
    func inlineContextMenu(_ event: NSEvent) -> Bool {
        let point = local(event.locationInWindow)
        guard let run = inlineContextTarget(at: point), let line = inlineRects(run).first else { return false }
        var sample = pointerSample(event)
        sample.x = Double(point.x - line.minX); sample.y = Double(point.y - line.minY)
        return dispatchInlineContextMenu(run, at: convert(event.locationInWindow, from: nil), sample: sample)
    }

    /// Run `id`'s `contextmenu` and `contextPopover` for VoiceOver's show
    /// menu (X84), as the ContextMenu key sends a node's: at the middle of
    /// its first line, from that line's top left, no button held.
    func inlineContextMenu(_ id: UInt32) -> Bool {
        guard inlineAnswersContextMenu(id), let presenter, let run = presenter.inlineText(id),
              let line = inlineRects(run).first, let point = presenter.contextPoint(self, rect: line) else { return false }
        let local = self.local(point), client = presenter.client(point)
        let sample = PointerSample(x: Double(local.x - line.minX), y: Double(local.y - line.minY),
                                   buttons: 0, pressure: 0, type: "mouse", id: 1,
                                   clientX: Double(client.x), clientY: Double(client.y))
        return dispatchInlineContextMenu(run, at: convert(point, from: nil), sample: sample)
    }

    /// `run`'s `contextmenu`, then its `contextPopover`'s menu at `point`
    /// (this view's space), as `dispatchContextMenu` sends a node's.
    private func dispatchInlineContextMenu(_ run: InlineText, at point: NSPoint, sample: PointerSample) -> Bool {
        guard let presenter, presenter.views[id] === self, inlineAnswersContextMenu(run.id) else { return false }
        let outer = presenter.defaultPrevented
        presenter.defaultPrevented = false
        defer { presenter.defaultPrevented = outer }
        if run.handlers.contains("contextmenu") { presenter.mouseEvent(run.id, 10, sample.line) }
        if run.props["contextPopover"]?.isEmpty == false { presenter.menus.context(self, at: point, run: run.id) }
        return true
    }

    // Any button holds the pointer, as in a browser (review b5-b 1): the
    // secondary's moves and the middle button's down, moves and up are the
    // held node's pointer events too, `buttons` 2 or 4 (`pointerSample`).
    package override func rightMouseDragged(with event: NSEvent) {
        pointerDragged(event)
        if canvasInput?.pointer(event, phase: "move") != true { super.rightMouseDragged(with: event) }
    }
    package override func otherMouseDown(with event: NSEvent) {
        pointerPressed(event)
        if canvasInput?.pointer(event, phase: "down") != true { super.otherMouseDown(with: event) }
    }
    package override func otherMouseDragged(with event: NSEvent) {
        pointerDragged(event)
        if canvasInput?.pointer(event, phase: "move") != true { super.otherMouseDragged(with: event) }
    }
    package override func otherMouseUp(with event: NSEvent) {
        pointerReleased(event)
        if canvasInput?.pointer(event, phase: "up") != true { super.otherMouseUp(with: event) }
    }

    /// `wheel` at this node and its ancestors, once per event however many
    /// of the views on the way pass it up; true when a handler prevented
    /// its default, and the scroll stops here.
    func wheel(_ event: NSEvent) -> Bool {
        guard let presenter else { return false }
        if let (seen, prevented) = presenter.lastWheel, seen === event { return prevented }
        var path: [NodeView] = []
        var next: NSView? = self
        while let view = next {
            if let node = view as? NodeView, presenter.views[node.id] === node, node.handlers.contains("wheel"), !node.formDisabled, !node.inert {
                path.append(node)
            }
            next = view.superview
        }
        var prevented = false
        for node in path where presenter.views[node.id] === node {
            presenter.defaultPrevented = false
            presenter.mouseEvent(node.id, 37, node.wheelLine(event))
            prevented = prevented || presenter.defaultPrevented
        }
        presenter.defaultPrevented = false
        presenter.lastWheel = (event, prevented)
        return prevented
    }

    /// The `WheelEvent` line: the point from the content box, the deltas in
    /// CSS px with the web's sign (positive scrolls down and right), pixels
    /// as the mode, and the modifiers held.
    func wheelLine(_ event: NSEvent) -> String {
        let point = local(event.locationInWindow), box = contentBox()
        var dx = 0.0, dy = 0.0
        var held = event.modifierFlags
        if event.type == .magnify {
            dy = -Double(event.magnification) * 100
            held.insert(.control)
        } else {
            // A wheel's notch is a line; Chrome on a Mac scrolls 40 px for one.
            let scale = event.hasPreciseScrollingDeltas ? 1.0 : 40.0
            dx = -Double(event.scrollingDeltaX) * scale
            dy = -Double(event.scrollingDeltaY) * scale
        }
        return "\(Double(point.x - box.minX)),\(Double(point.y - box.minY)),\(dx),\(dy),0,\(KeyCodes.held(held))"
    }

    /// A node that hears `drop` takes files dragged over it; one that no
    /// longer does gives them back to whatever is under it.
    func syncDropTypes() {
        if handlers.contains("drop") { registerForDraggedTypes([.fileURL]) } else { unregisterDraggedTypes() }
    }

    /// The dragged files this node would take: file URLs of a type the
    /// manifest declares.
    private func droppable(_ info: NSDraggingInfo) -> [URL] {
        guard handlers.contains("drop"), !disabled, !inert else { return [] }
        let urls = info.draggingPasteboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL] ?? []
        return urls.filter(ExactDocuments.accepts)
    }

    package override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        droppable(sender).isEmpty ? [] : .copy
    }

    package override func draggingUpdated(_ sender: NSDraggingInfo) -> NSDragOperation {
        droppable(sender).isEmpty ? [] : .copy
    }

    package override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        drop(droppable(sender), at: sender.draggingLocation)
    }

    /// The innermost node from `view` up that hears `drop`: what a drag
    /// over it lands on (the agent's `tap … drop`).
    static func dropTarget(_ view: NSView) -> NodeView? {
        var next: NSView? = view
        while let view = next {
            if let node = view as? NodeView, node.handlers.contains("drop"), !node.formDisabled, !node.inert { return node }
            next = view.superview
        }
        return nil
    }

    /// Files dropped at a window point: each of a declared type minted for
    /// this session and delivered with the `DragEvent`'s line.
    func drop(_ urls: [URL], at windowPoint: NSPoint) -> Bool {
        guard let presenter, let session = presenter.session else { return false }
        let docs = urls.filter(ExactDocuments.accepts).compactMap { session.mintDocument($0) }
        guard !docs.isEmpty else {
            session.log("drop: refused: no file of a type this app declares")
            return false
        }
        let point = local(windowPoint), box = contentBox()
        let held = KeyCodes.held(NSEvent.modifierFlags)
        presenter.mouseEvent(id, 38, "\(Double(point.x - box.minX)),\(Double(point.y - box.minY)),\(held)\n" + docs.joined(separator: "\n"))
        return true
    }
}

extension NSControl {
    /// X85 on AppKit's own controls (a native button, checkbox, switch,
    /// radio, range, select or date field): a Control-click that a
    /// `contextmenu` or `contextPopover` from the control's node up answers
    /// takes the right-click's path from that node (`contextClick`), and the
    /// control's tracking never starts, so it checks, moves or opens
    /// nothing; AppKit sends the release on to the node. Chrome sends
    /// `contextmenu` from each, and changes none but a range, whose down
    /// moves its value first.
    func contextClicked(_ event: NSEvent) -> Bool {
        var view = superview
        while let at = view, !(at is NodeView) { view = at.superview }
        guard let node = view as? NodeView, node.contextClick(event) else { return false }
        node.rightMouseDown(with: event)
        return true
    }
}
#endif

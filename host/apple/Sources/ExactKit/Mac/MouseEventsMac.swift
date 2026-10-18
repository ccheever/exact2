// DOM's `wheel` and `drop` on the Mac (studio diary R3, R19), beside the
// pointer's events (MouseChainMac.swift) and `contextmenu`'s point
// (NodeViewMac's `rightMouseDown`).
//
// A wheel's turn or a trackpad's scroll over a node is heard by every
// node from it up the authored tree (`eventParent`) that declares `wheel`
// (a disabled box included, as a `<div disabled>` in Chrome; not a
// disabled form control or a node an `inert` makes inert; a modal dialog's
// ancestors, inert behind it, hear it), innermost first, as the DOM's
// bubbles; one that calls `preventDefault()` keeps the scroll from
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
              let window = owner ?? event.window, hosts(window),
              (window.firstResponder as? NSTextInputClient)?.hasMarkedText() != true else { return false }
        if let run = window.firstResponder as? InlineKeyView, run.owner?.inlineContextMenu(run.id) == true { return true }
        let point: NSPoint
        let start: NSView?
        if let focus = keyTarget(window.firstResponder) {
            guard let middle = contextPoint(focus) else { return false }
            point = middle
            start = focus
        } else {
            guard window === viewport.window else { return false }
            // A handler removed the focus: Chrome hits the page at (1,1).
            // An embedded view/editor still owns its keys, even when its
            // responder is not one of our focusable nodes.
            let responder = window.firstResponder
            guard responder == nil || responder === window || responder === viewport
                    || responder === root || responder === session?.view,
                  session?.view?.ownsShortcutFocus() ?? true else { return false }
            let clip = viewport.contentView
            point = clip.convert(NSPoint(x: clip.bounds.minX + 1, y: clip.bounds.minY + 1), to: nil)
            start = viewport.hitTest(viewport.superview?.convert(point, from: nil) ?? point)
        }
        guard let node = contextTarget(start) else { return false }
        return contextMenu(node, at: point, from: start)
    }

    /// The node a request from the keyboard at `view` reaches (the
    /// ContextMenu key, VoiceOver's show menu): the first from it up the
    /// authored tree (`eventParent`), whose `contextmenu` or `contextPopover`
    /// answers, as Chrome's `contextmenu` at the element bubbles to its
    /// first handler; nil at a field, which keeps its own menu.
    func contextTarget(_ view: NSView?) -> NodeView? {
        var next = view
        while let view = next {
            if let node = view as? NodeView {
                if node.field != nil || node.textArea != nil { return nil }
                if views[node.id] === node, node.answersContextMenu { return node }
            }
            next = eventParent(of: view)
        }
        return nil
    }

    /// Where a request from the keyboard lands on `node` (the ContextMenu
    /// key at the focus, VoiceOver's show menu): the middle of `rect` in its
    /// space (its box when nil) as drawn, clipped by what clips it, in the
    /// window; nil when it takes no request or none of it is drawn.
    func contextPoint(_ node: NodeView, rect: NSRect? = nil) -> NSPoint? {
        guard !node.formDisabled, !node.inert, !node.isHiddenOrHasHiddenAncestor,
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

    /// `node`'s `contextmenu` and `contextPopover` at `point` in the source's
    /// window (the node's when omitted), its sample with no button held.
    /// `click` is a pointer's press, which the menu pops up from.
    func contextMenu(_ node: NodeView, at point: NSPoint, from source: NSView? = nil, click: NSEvent? = nil) -> Bool {
        func converted(to window: NSWindow?) -> NSPoint {
            guard let from = source?.window ?? node.window, let window, from !== window else { return point }
            return window.convertPoint(fromScreen: from.convertPoint(toScreen: point))
        }
        let windowPoint = converted(to: node.window)
        let local = node.local(windowPoint), box = node.contentBox(), client = self.client(converted(to: viewport.window))
        let sample = PointerSample(x: Double(local.x - box.minX), y: Double(local.y - box.minY),
                                   buttons: 0, pressure: 0, type: "mouse", id: 1,
                                   clientX: Double(client.x), clientY: Double(client.y))
        return node.dispatchContextMenu(at: node.convert(windowPoint, from: nil), sample: sample,
                                        bubbled: source.map { $0 !== node } ?? false, click: click)
    }
}

extension NodeView {
    package override func mouseDown(with event: NSEvent) {
        let chain = holdPresenter?.mouseChain
        chain?.receivedDown(self, event: event)
        if let contact = chain?.bubbledPress {
            if contact.cancelled { return }
            if contact.reaches(self) { contact.down(on: self); return }
        }
        guard !inert else { return }
        if contextClick(event) { return rightMouseDown(with: event) }
        pointerPressed(event)
        if isSurfaceControl { _ = control("down", point: local(event.locationInWindow), timestamp: event.timestamp); return }
        if canvasInput?.pointer(event, phase: "down") == true { return }
        presenter?.leaves.pressed(self) // a held leaf's box was clicked: made now (LLP 1068 §5.1)
        presenter?.collections.pointerDown(id, event: event)
        presenter?.mouseChain.down(self, event: event)
        guard !disabled else { pressed = false; return }
        presenter?.interacting = id
        if let target = presenter?.svg.target(id, at: local(event.locationInWindow)) { svgPressed = target; return }
        if isParagraph, let run = inlineTarget(at: local(event.locationInWindow), handler: "press") {
            inlinePressed = run.id
            window?.makeFirstResponder(self)
            presenter?.selection.begin(self, event: event)
            return
        }
        if selectsText { presenter?.selection.press(self, event: event); return }
        // A descendant that took focus keeps it (LLP 1088 D7.3). Forward
        // through the responder chain: NSView's default retries covered siblings.
        let inner = (window?.firstResponder as? NodeView).map { $0 !== self && ($0.tabbable || $0.explicitTabIndex != nil) && (window?.contentView?.hitTest(event.locationInWindow)?.isDescendant(of: $0) ?? false) } ?? false
        if takesClickFocus, !inner { window?.makeFirstResponder(self) }
        if pressable { pressed = true } else { (chain?.responder(from: self, event: event) ?? (chain == nil ? nextResponder : nil))?.mouseDown(with: event) }
    }
    /// A paragraph's drag selects its text unless a press takes it: its
    /// own handler (a `<span onClick>`, spreadsheet F12) or an ancestor's.
    var selectsText: Bool { isParagraph && !pressable && !hasPressableAncestor }
    var hasPressableAncestor: Bool { hasPressableAncestor(crossingPresentations: true) }
    func hasPressableAncestor(crossingPresentations: Bool) -> Bool {
        var at: NSView = self, crossed = false
        while let parent = (crossingPresentations ? presenter?.presentationParent(of: at) : nil) ?? at.superview {
            crossed = crossed || parent !== at.superview
            if let node = parent as? NodeView,
               node.pressable || !crossed && (node.isSurfaceControl || node.isButton) { return true }
            at = parent
        }
        return false
    }
    package override func mouseDragged(with event: NSEvent) {
        if contextClicked { return rightMouseDragged(with: event) }
        pointerDragged(event)
        let chain = holdPresenter?.mouseChain
        if let contact = chain?.bubbledPress {
            contact.validate()
            if contact.cancelled { _ = chain?.drag(event); return }
            if contact.reaches(self) { contact.drag(on: self, event: event); return }
        }
        guard !inert else { return }
        if isSurfaceControl || ownsSurfaceControl { _ = control("move", point: local(event.locationInWindow), timestamp: event.timestamp); return }
        if canvasInput?.pointer(event, phase: "move") == true { return }
        inlinePressed = nil
        // A gesture that engages ends the press (the chain clears `pressed`).
        if presenter?.mouseChain.drag(event) == true { return }
        pressFollows(inside: pressInside(event.locationInWindow))
        if selectsText { presenter?.selection.drag(event) }
        else { (chain?.responder(from: self, event: event) ?? (chain == nil ? nextResponder : nil))?.mouseDragged(with: event) }
    }
    package override func rightMouseUp(with event: NSEvent) {
        pointerReleased(event)
        if canvasInput?.pointer(event, phase: "up") != true { super.rightMouseUp(with: event) }
    }
    package override func mouseUp(with event: NSEvent) {
        let chain = holdPresenter?.mouseChain, contact = chain?.bubbledPress
        defer { chain?.completed(contact, at: self) }
        if contextClicked { contextClicked = false; return rightMouseUp(with: event) }
        pointerReleased(event)
        contact?.validate()
        if contact?.cancelled == true {
            let taken = chain?.up(event) == true
            chain?.presenter?.interacting = 0
            if !taken, !inert { dispatchDblclick(dblclickTarget(event)) }
            return
        }
        if let contact, contact.reaches(self) { contact.up(on: self, event: event); return }
        guard !inert else { return }
        if isSurfaceControl || ownsSurfaceControl { _ = control("up", point: local(event.locationInWindow), timestamp: event.timestamp); finishPointerPress(); return }
        if canvasInput?.pointer(event, phase: "up") == true { return }
        defer { holdPresenter?.interacting = 0 }
        if presenter?.mouseChain.up(event) == true { return }
        presenter?.collections.releaseInteractionLater()
        let double = dblclickTarget(event)
        defer { dispatchDblclick(double) }
        if let target = svgPressed {
            svgPressed = nil
            // The element has no view of its own: the `svg`'s view stands for it.
            if !inert, presenter?.svg.target(id, at: local(event.locationInWindow)) == target {
                presenter?.pressHeld = KeyCodes.held(event.modifierFlags); presenter?.onPress?(target); presenter?.pressHeld = ""
            }
            return
        }
        if let run = inlinePressed {
            inlinePressed = nil
            if inlineTarget(at: local(event.locationInWindow), handler: "press")?.id == run { _ = activateInline(run) }
            return
        }
        if selectsText { presenter?.selection.end(self, event: event); return }
        guard !disabled else { pressed = false; return }
        guard pressed else { (chain?.responder(from: self, event: event) ?? (chain == nil ? nextResponder : nil))?.mouseUp(with: event); return }
        pressed = false
        if pressInside(event.locationInWindow) {
            let canvas = inputCanvas, ownerWindow = window
            presenter?.press(id, held: KeyCodes.held(event.modifierFlags))
            finishPress(canvas: canvas, window: ownerWindow, pointer: true)
        }
    }

    /// Whether its own `contextmenu` or `contextPopover` answers a context request.
    var answersContextMenu: Bool { handlers.contains("contextmenu") || props["contextPopover"]?.isEmpty == false }

    /// The context action runs before the menu is read, for pointer and
    /// keyboard alike. A context action cannot leak its preventDefault to
    /// another input; keydown cancellation was handled before this default.
    /// `click` is a pointer's press, which the menu pops up from.
    func dispatchContextMenu(at point: NSPoint, sample: PointerSample, bubbled: Bool = false, click: NSEvent? = nil) -> Bool {
        guard let presenter, presenter.views[id] === self, !formDisabled, !(bubbled ? authoredInert : inert) else { return false }
        let action = handlers.contains("contextmenu"), menu = props["contextPopover"]?.isEmpty == false
        let outer = presenter.defaultPrevented
        presenter.defaultPrevented = false
        defer { presenter.defaultPrevented = outer }
        if action { presenter.mouseEvent(id, 10, sample.line) }
        // In the browser this ancestor's popover opens behind the modal, inert.
        if menu, !inert { presenter.menus.context(self, at: point, click: click) }
        return action || menu
    }

    /// AppKit keeps the ordinary responder chain; only a lifted view's
    /// step into the top layer goes through its authored ancestors instead.
    func bubbleContextMenu(_ event: NSEvent) -> Bool {
        guard let presenter, let parent = presenter.eventParent(of: self), parent !== superview else { return false }
        var next: NSView? = parent
        while let view = next {
            if let node = view as? NodeView,
               node.dispatchContextMenu(at: node.convert(Self.point(event, in: node.window), from: nil),
                                        sample: node.pointerSample(event), bubbled: true, click: event) { return true }
            next = presenter.eventParent(of: view)
        }
        return false
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
        guard let presenter, !presenter.inlineInert(id), let run = presenter.inlineText(id) else { return false }
        return run.handlers.contains("contextmenu") || run.props["contextPopover"]?.isEmpty == false
    }

    /// The run a secondary click at `point` (this paragraph's space) reaches
    /// (X77): the nearest from the innermost run there up whose own
    /// `contextmenu` or `contextPopover` answers, as the web's `contextmenu`
    /// bubbles from the run's element to its first handler. Nil where none
    /// does, and the paragraph and its ancestors hear the click as before.
    func inlineContextTarget(at point: CGPoint) -> InlineText? {
        guard !inlineText.isEmpty else { return nil }
        return inlineContextTarget(from: inlineTarget(at: point))
    }

    /// The nearest run from `run` up whose own `contextmenu` or
    /// `contextPopover` answers; nil where none does.
    private func inlineContextTarget(from run: InlineText?) -> InlineText? {
        guard let presenter else { return nil }
        var run = run
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
        return dispatchInlineContextMenu(run, at: convert(event.locationInWindow, from: nil), sample: sample, click: event)
    }

    /// A request from the keyboard at run `id` (VoiceOver's show menu, X84;
    /// the ContextMenu key at the run), as the key sends a node's: at the
    /// middle of the run's first line, no button held, to the nearest run
    /// from it up that answers, from that run's first line's top left, else
    /// to the first node from this paragraph up that does, as Chrome's
    /// `contextmenu` at the run's element bubbles.
    func inlineContextMenu(_ id: UInt32) -> Bool {
        guard let presenter, !presenter.inlineInert(id), let run = presenter.inlineText(id),
              let line = inlineRects(run).first, let point = presenter.contextPoint(self, rect: line) else { return false }
        guard let target = inlineContextTarget(from: run), let first = inlineRects(target).first else {
            return presenter.contextTarget(self).map { presenter.contextMenu($0, at: point, from: self) } ?? false
        }
        let local = self.local(point), client = presenter.client(point)
        let sample = PointerSample(x: Double(local.x - first.minX), y: Double(local.y - first.minY),
                                   buttons: 0, pressure: 0, type: "mouse", id: 1,
                                   clientX: Double(client.x), clientY: Double(client.y))
        return dispatchInlineContextMenu(target, at: convert(point, from: nil), sample: sample)
    }

    /// Whether a request from the keyboard at run `id` reaches a run or
    /// node from it up whose `contextmenu` or `contextPopover` runs.
    func inlineOffersContextMenu(_ id: UInt32) -> Bool {
        guard let presenter, presenter.views[self.id] === self, !formDisabled, !inert,
              !presenter.inlineInert(id), let run = presenter.inlineText(id) else { return false }
        if inlineContextTarget(from: run) != nil { return true }
        return presenter.contextTarget(self).map { !$0.formDisabled && !$0.authoredInert } ?? false
    }

    /// `run`'s `contextmenu`, then its `contextPopover`'s menu at `point`
    /// (this view's space), as `dispatchContextMenu` sends a node's.
    private func dispatchInlineContextMenu(_ run: InlineText, at point: NSPoint, sample: PointerSample, click: NSEvent? = nil) -> Bool {
        guard let presenter, presenter.views[id] === self, inlineAnswersContextMenu(run.id) else { return false }
        let outer = presenter.defaultPrevented
        presenter.defaultPrevented = false
        defer { presenter.defaultPrevented = outer }
        if run.handlers.contains("contextmenu") { presenter.mouseEvent(run.id, 10, sample.line) }
        if run.props["contextPopover"]?.isEmpty == false { presenter.menus.context(self, at: point, run: run.id, click: click) }
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
    /// of the views on the way pass it up, until a handler stops its
    /// propagation; true when a handler prevented its default, and the
    /// scroll stops here (a stop alone does not stop the scroll).
    func wheel(_ event: NSEvent) -> Bool {
        guard let presenter else { return false }
        if let (seen, prevented) = presenter.lastWheel, seen === event { return prevented }
        var path: [NodeView] = []
        var next: NSView? = self
        while let view = next {
            if let node = view as? NodeView, presenter.views[node.id] === node, node.handlers.contains("wheel"), !node.formDisabled, !node.authoredInert {
                path.append(node)
            }
            next = presenter.eventParent(of: view)
        }
        var prevented = false
        presenter.propagating {
            for node in path where presenter.views[node.id] === node {
                presenter.defaultPrevented = false
                presenter.mouseEvent(node.id, 37, node.wheelLine(event))
                prevented = prevented || presenter.defaultPrevented
                if presenter.propagationStopped { break }
            }
        }
        presenter.defaultPrevented = false
        presenter.lastWheel = (event, prevented)
        return prevented
    }

    /// The `WheelEvent` line: the point from the content box, the deltas in
    /// CSS px with the web's sign (positive scrolls down and right), pixels
    /// as the mode, and the modifiers held.
    func wheelLine(_ event: NSEvent) -> String {
        // In this node's window: a wheel in a popover's bubbles to the page.
        let point = local(Self.point(event, in: window)), box = contentBox()
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

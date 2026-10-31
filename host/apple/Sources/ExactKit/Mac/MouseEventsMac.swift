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
// Files and folders dragged in from Finder land on the innermost node that
// declares `drop`, a field under the pointer included: each a `doc:` handle
// minted for this session (LLP 1069.010 D1), of any type (the 2026-10-11
// any-file waiver, extended to `drop`). `dragenter` and `dragleave` bracket
// the drag over each node that hears them (FileDragMac.swift).
#if os(macOS)
import AppKit
import UniformTypeIdentifiers

extension Presenter {
    /// Invoke the focused node's contextmenu or contextPopover after key handlers.
    func contextMenuKey(_ event: NSEvent, in owner: NSWindow? = nil) -> Bool {
        let name = NodeView.keyName(event), held = KeyCodes.held(event.modifierFlags)
        guard event.type == .keyDown, (name == "ContextMenu" && held.isEmpty || name == "F10" && held == "Shift+"),
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
            if let (node, at) = selection.keyboardMenuPoint(in: window) {
                // Selected text: at its first line, as Blink's
                // `ShowNonLocatedContextMenu` places a menu with no focus.
                point = at
                start = node
            } else {
                let clip = viewport.contentView
                point = clip.convert(NSPoint(x: clip.bounds.minX + 1, y: clip.bounds.minY + 1), to: nil)
                start = viewport.hitTest(viewport.superview?.convert(point, from: nil) ?? point)
            }
        }
        guard let node = contextTarget(start) else { return unansweredContextMenu(at: point, from: start, in: window) }
        return contextMenu(node, at: point, from: start)
    }

    /// A request from the keyboard that no node answers (the ContextMenu
    /// key, VoiceOver's show menu) gets the menu a right-click at `point`
    /// would show, as Chrome's own menu follows a `contextmenu` no handler
    /// prevented: the menu of the view drawn there or of the first view
    /// above it that has one (selected text's), else the session's view and
    /// the responders past it (an embedder's, an app's), which hear a
    /// secondary click's press and release. No node hears it, since the
    /// tree already declined, and selected text keeps its selection. It
    /// goes on the next turn, as a context menu's popover does
    /// (`MenuHost.context`), outside VoiceOver's request. False where the
    /// walk from `start` stopped at a field, which keeps its own menu.
    func unansweredContextMenu(at point: NSPoint, from start: NSView?, in window: NSWindow) -> Bool {
        guard contextUnanswered(start), let down = Self.keyboardClick(.rightMouseDown, at: point, in: window),
              let up = Self.keyboardClick(.rightMouseUp, at: point, in: window) else { return false }
        let found = keyboardMenu(for: down, in: window)
        RunLoop.main.perform(inModes: [.common]) { [weak self] in
            guard let self else { return }
            if let (view, menu) = found { return NSMenu.popUpContextMenu(menu, with: down, for: view) }
            let past = self.session?.view ?? self.viewport
            past.rightMouseDown(with: down)
            past.rightMouseUp(with: up)
        }
        return true
    }

    /// Whether a request from the keyboard at `point` would get a menu,
    /// judged without building one, since VoiceOver asks on every query:
    /// the view drawn there or one above it holds selected text or has a
    /// `menu`, below any native control (`keyboardMenu`). A responder past
    /// the session's view is not counted.
    func offersKeyboardMenu(at point: NSPoint, in window: NSWindow) -> Bool {
        var next = hit(point, in: window)
        while let view = next, view !== session?.view, !(view is NSControl) {
            if let node = view as? NodeView, node.isParagraph, !ExactEnv.agentMode, (selection.range(node)?.length ?? 0) > 0 { return true }
            if view.menu != nil { return true }
            next = view.superview
        }
        return false
    }

    private static func keyboardClick(_ type: NSEvent.EventType, at point: NSPoint, in window: NSWindow) -> NSEvent? {
        NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                           windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)
    }

    private func hit(_ point: NSPoint, in window: NSWindow) -> NSView? {
        guard let content = window.contentView else { return nil }
        return content.hitTest(content.superview?.convert(point, from: nil) ?? point)
    }

    /// The first view from the one drawn at `event`'s point up to the
    /// session's view whose `menu(for:)` gives a request from the keyboard
    /// a menu, as AppKit asks a right-click's view and then its superviews;
    /// nil when none does. The walk stops at a native control: its own menu
    /// (a `select`'s options, which its `NSPopUpButton` gives) is no context
    /// menu, and Chrome shows its page menu over a `select` too.
    private func keyboardMenu(for event: NSEvent, in window: NSWindow) -> (NSView, NSMenu)? {
        NodeView.keyboardMenuRequest = true
        defer { NodeView.keyboardMenuRequest = false }
        var next = hit(event.locationInWindow, in: window)
        while let view = next, view !== session?.view, !(view is NSControl) {
            if let menu = view.menu(for: event) { return (view, menu) }
            next = view.superview
        }
        return nil
    }

    /// Whether the walk `contextTarget` takes from `view` meets no field,
    /// which keeps its own menu.
    func contextUnanswered(_ view: NSView?) -> Bool {
        var next = view
        while let view = next {
            if let node = view as? NodeView, node.field != nil || node.textArea != nil { return false }
            next = eventParent(of: view)
        }
        return true
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
        let drawn = visiblePart(of: node.drawnRect(rect ?? menus.anchor(node, in: node)), in: node)
        return drawn.isEmpty ? nil : NSPoint(x: drawn.midX, y: drawn.midY)
    }

    /// `drawn`, a rect of `node` as drawn in its window, clipped by what
    /// clips `node`. AppKit's visibleRect clips the untransformed frame; this
    /// clips the drawn rect, including a frame translated into view.
    func visiblePart(of drawn: NSRect, in node: NodeView) -> NSRect {
        var drawn = drawn
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
        return drawn
    }

    /// `node`'s `contextmenu` and `contextPopover` at `point` in the source's
    /// window (the node's when omitted), with no button held.
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
        HeldState.input(at: self) // a press on a held node lets its hold go (HeldState.swift; the window's monitor, ExactViewMac)
        let chain = holdPresenter?.mouseChain
        chain?.receivedDown(self, event: event)
        if let contact = chain?.bubbledPress {
            if contact.cancelled { return }
            if contact.reaches(self) { contact.down(on: self); return }
        }
        guard !inert else { return }
        // A control-click focuses as it did before its menu, whatever its `pointerdown` says (its own way, LLP 1005).
        if contextClick(event) { if pointerdownHeard, takesClickFocus, !focusInside(event) { takeClickFocus() }; return rightMouseDown(with: event) }
        pointerPressed(event)
        // A prevented `pointerdown` keeps the focus where it is and starts no selection; the press still goes on.
        let keepsFocus = presenter?.mouseChain.prevented === event
        let endKeeping = keepingFocus(for: event)
        defer { endKeeping() }
        // One AppKit did not focus before its `mouseDown` (a `pointerdown` handler hears it:
        // `needsPanelToBecomeKey`) takes the click focus now, after that handler, unless it prevented it.
        if pointerdownHeard, takesClickFocus, !keepsFocus, !focusInside(event) { takeClickFocus() }
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
            if !keepsFocus { window?.makeFirstResponder(self); presenter?.selection.begin(self, event: event) }
            return
        }
        if selectsText { if !keepsFocus { presenter?.selection.press(self, event: event) }; return }
        // A descendant that took focus keeps it (LLP 1088 D7.3). Forward
        // through the responder chain: NSView's default retries covered siblings.
        let inner = focusInside(event)
        if takesClickFocus, !inner, !keepsFocus { window?.makeFirstResponder(self) }
        if pressable { pressed = true } else { (chain?.responder(from: self, event: event) ?? (chain == nil ? nextResponder : nil))?.mouseDown(with: event) }
    }
    /// A descendant that took focus keeps it (LLP 1088 D7.3): the click is in it.
    func focusInside(_ event: NSEvent) -> Bool {
        (window?.firstResponder as? NodeView).map { $0 !== self && ($0.tabbable || $0.explicitTabIndex != nil) && (window?.contentView?.hitTest(event.locationInWindow)?.isDescendant(of: $0) ?? false) } ?? false
    }
    /// The click focus, as AppKit gives it before a `mouseDown`: first
    /// responder, its window made key (a popover's, a panel that becomes key
    /// only if needed).
    func takeClickFocus() {
        window?.makeFirstResponder(self)
        if let window, !window.isKeyWindow, window.canBecomeKey { window.makeKey() }
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
        if selectsText, presenter?.mouseChain.preventedContact != true { presenter?.selection.drag(event) } // a prevented contact selects nothing
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
                presenter?.menus.activating(from: self, pressing: nil) {
                    presenter?.pressHeld = KeyCodes.held(event.modifierFlags); presenter?.pressClick = event
                    presenter?.onPress?(target); presenter?.pressHeld = ""; presenter?.pressClick = nil
                }
            }
            return
        }
        if let run = inlinePressed {
            inlinePressed = nil
            if inlineTarget(at: local(event.locationInWindow), handler: "press")?.id == run {
                presenter?.pressClick = event
                _ = activateInline(run)
                presenter?.pressClick = nil
            }
            return
        }
        if selectsText { presenter?.selection.end(self, event: event); return }
        guard !disabled else { pressed = false; return }
        guard pressed else { (chain?.responder(from: self, event: event) ?? (chain == nil ? nextResponder : nil))?.mouseUp(with: event); return }
        pressed = false
        if pressInside(event.locationInWindow) {
            let canvas = inputCanvas, ownerWindow = window
            presenter?.menus.activating(from: presenter?.mouseChain.pressSource ?? self, pressing: self) {
                presenter?.press(id, held: KeyCodes.held(event.modifierFlags), click: event)
            }
            finishPress(canvas: canvas, window: ownerWindow, pointer: true)
        }
    }

    /// Whether its own `contextmenu` or `contextPopover` answers a context request.
    var answersContextMenu: Bool { handlers.contains("contextmenu") || props["contextPopover"]?.isEmpty == false }

    /// The context action runs before the menu is read, for pointer and
    /// keyboard alike. A context action cannot leak its preventDefault to
    /// another input; keydown cancellation was handled before this default.
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
            if let node = presenter.contextTarget(self) { return presenter.contextMenu(node, at: point, from: self) }
            return window.map { presenter.unansweredContextMenu(at: point, from: self, in: $0) } ?? false
        }
        let local = self.local(point), client = presenter.client(point)
        let sample = PointerSample(x: Double(local.x - first.minX), y: Double(local.y - first.minY),
                                   buttons: 0, pressure: 0, type: "mouse", id: 1,
                                   clientX: Double(client.x), clientY: Double(client.y))
        return dispatchInlineContextMenu(target, at: convert(point, from: nil), sample: sample)
    }

    /// Whether a request from the keyboard at run `id` reaches a run or
    /// node from it up whose `contextmenu` or `contextPopover` runs, or,
    /// where none answers, a menu at the run's middle (`offersKeyboardMenu`).
    func inlineOffersContextMenu(_ id: UInt32) -> Bool {
        guard let presenter, presenter.views[self.id] === self, !formDisabled, !inert,
              !presenter.inlineInert(id), let run = presenter.inlineText(id) else { return false }
        if inlineContextTarget(from: run) != nil { return true }
        if let node = presenter.contextTarget(self) { return !node.formDisabled && !node.authoredInert }
        guard presenter.contextUnanswered(self), let line = inlineRects(run).first,
              let point = presenter.contextPoint(self, rect: line), let window else { return false }
        return presenter.offersKeyboardMenu(at: point, in: window)
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
        let endKeeping = keepingFocus(for: event)
        defer { endKeeping() }
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

    /// A node that hears `drop`, `dragenter` or `dragleave` is a destination
    /// for files dragged over it (FileDragMac.swift); one that no longer does
    /// gives them back to whatever is under it.
    func syncDropTypes() {
        if !handlers.isDisjoint(with: FileDrag.events) { registerForDraggedTypes([.fileURL]) } else { unregisterDraggedTypes() }
    }

    package override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation { FileDrag.moved(self, sender) ?? [] }
    package override func draggingUpdated(_ sender: NSDraggingInfo) -> NSDragOperation { FileDrag.moved(self, sender) ?? [] }
    package override func draggingExited(_ sender: NSDraggingInfo?) { FileDrag.exited(self, sender) }
    package override func performDragOperation(_ sender: NSDraggingInfo) -> Bool { FileDrag.perform(self, sender) ?? false }
    package override func draggingEnded(_ sender: NSDraggingInfo) { FileDrag.ended() }

    /// The innermost node from `view` up that hears `drop`: what a drag
    /// over it lands on, a field inside it included (the agent's `tap … drop`).
    static func dropTarget(_ view: NSView) -> NodeView? {
        var next: NSView? = view
        while let view = next {
            if let node = view as? NodeView, node.handlers.contains("drop"), !node.formDisabled, !node.inert { return node }
            next = view.superview
        }
        return nil
    }

    /// Files and folders dropped at a window point, of any type: each minted
    /// for this session and delivered with the `DragEvent`'s line.
    func drop(_ urls: [URL], at windowPoint: NSPoint) -> Bool {
        guard let presenter, let session = presenter.session else { return false }
        let docs = Self.droppedDocuments(urls, accepts: { ExactDocuments.accepts($0) },
                                         keep: { session.keepDocument($0, noting: true) }, mint: session.mintDocument)
        guard !docs.isEmpty else {
            session.log("drop: refused: no file could be opened")
            return false
        }
        presenter.mouseEvent(id, 38, dragLine(windowPoint) + "\n" + docs.joined(separator: "\n"))
        return true
    }

    /// The handles of dropped `urls`. A file of a type the manifest declares
    /// is a document the person chose (LLP 1069.010.000 D3): kept, it arrives
    /// as the path it is kept by, and joins Open Recent. Any other file, and a
    /// folder (a directory handle, its path undisclosed; `fs.stat` tells the
    /// two apart), is content: minted for this session alone, as a paste's is.
    static func droppedDocuments(_ urls: [URL], accepts: (URL) -> Bool, keep: (URL) -> String?, mint: (URL) -> String?) -> [String] {
        urls.filter(\.isFileURL).compactMap { url in (accepts(url) ? keep(url) : nil) ?? mint(url) }
    }

    /// A `DragEvent`'s point line: from the content box, and the modifiers held.
    func dragLine(_ windowPoint: NSPoint) -> String {
        let point = local(windowPoint), box = contentBox()
        return "\(Double(point.x - box.minX)),\(Double(point.y - box.minY)),\(KeyCodes.held(NSEvent.modifierFlags))"
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

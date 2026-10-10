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
        let point: NSPoint
        var next: NSView?
        if let focus = keyTarget(window.firstResponder) {
            guard !focus.disabled, !focus.inert, !focus.isHiddenOrHasHiddenAncestor,
                  !dialogs.blocks(focus), focus.field == nil, focus.textArea == nil else { return false }
            // AppKit's visibleRect clips the untransformed frame. Clip the
            // drawn anchor instead, including a frame translated into view.
            var drawn = focus.drawnRect(menus.anchor(focus, in: focus))
            var ancestor = focus.superview
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
            guard !drawn.isEmpty else { return false }
            point = NSPoint(x: drawn.midX, y: drawn.midY)
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
        let client = self.client(point)
        while let view = next {
            if let node = view as? NodeView, (node.field != nil || node.textArea != nil) { return false }
            if let node = view as? NodeView, views[node.id] === node,
               node.handlers.contains("contextmenu") || node.props["contextPopover"]?.isEmpty == false {
                let local = node.local(point), box = node.contentBox()
                let sample = PointerSample(x: Double(local.x - box.minX), y: Double(local.y - box.minY),
                                           buttons: 0, pressure: 0, type: "mouse", id: 1,
                                           clientX: Double(client.x), clientY: Double(client.y))
                return node.dispatchContextMenu(at: node.convert(point, from: nil), sample: sample)
            }
            next = menus.parent(of: view)
        }
        return false
    }
}

extension NodeView {
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
#endif

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

extension NodeView {
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
        guard handlers.contains("drop"), !formDisabled, !inert else { return [] }
        let urls = info.draggingPasteboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL] ?? []
        return urls.filter(ExactDocuments.accepts)
    }

    override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        droppable(sender).isEmpty ? [] : .copy
    }

    override func draggingUpdated(_ sender: NSDraggingInfo) -> NSDragOperation {
        droppable(sender).isEmpty ? [] : .copy
    }

    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
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

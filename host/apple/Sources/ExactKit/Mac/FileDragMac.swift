// A drag of files from outside the app (Finder, the desktop) over a window:
// DOM's `dragenter` and `dragleave` at each node that hears them, and its
// `drop` at the innermost node that declares `drop` (MouseEventsMac.swift).
//
// A node hears `dragenter` when the drag comes over its box from outside
// it, and `dragleave` when it leaves the box, is cancelled or leaves the
// window: moves between its own descendants are neither, which is what a
// web handler gets by ignoring an event whose `relatedTarget` it contains
// (T3 Code's chat column does). A drop a node takes ends the drag with no
// `dragleave`, as a browser's does (an overlay is cleared by the `drop`'s
// action); a drop no node takes, or one whose files none could be opened,
// is a leave at each node still entered. Every file drag is heard, whether
// or not a `drop` takes it; a disabled control or an inert node hears none.
//
// AppKit hands a drag to the deepest view under it that registered for its
// types, and a text field or text area registers for file URLs, inserting
// their paths. A file drag over one whose node or an ancestor declares
// `drop` goes to that node instead, as the web's ancestor listener prevents
// the drop's default; a text drag, or a file drag with no `drop` above, is
// the field's own.
#if os(macOS)
import AppKit

enum FileDrag {
    static let events: Set<String> = ["drop", "dragenter", "dragleave"]

    /// The nodes the drag is over that hear `dragenter` or `dragleave`,
    /// innermost first, and the drag's last window point.
    nonisolated(unsafe) private static var entered: [NodeView] = []
    nonisolated(unsafe) private static var last = NSPoint.zero
    /// Bumped by every callback: a pending exit's clear is stale once it moves.
    nonisolated(unsafe) private static var turn = 0

    /// The dragged file URLs, folders included; none for a drag of anything else.
    static func urls(_ info: NSDraggingInfo) -> [URL] {
        info.draggingPasteboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL] ?? []
    }

    /// The nodes hearing `dragenter` or `dragleave` whose box holds `point`
    /// in `window`, innermost first: what is under it and its ancestors.
    static func chain(at point: NSPoint, in window: NSWindow?) -> [NodeView] {
        guard let content = window?.contentView, content.bounds.contains(content.convert(point, from: nil)) else { return [] }
        var view = TableRowCell.whatIsThere { content.hitTest(content.superview?.convert(point, from: nil) ?? point) }
        var out: [NodeView] = []
        while let at = view {
            if let node = at as? NodeView, !node.inert, !node.formDisabled,
               node.handlers.contains("dragenter") || node.handlers.contains("dragleave") { out.append(node) }
            view = at.superview
        }
        return out
    }

    /// The drag is over `next`: the nodes it came over hear `dragenter`,
    /// then those it left `dragleave`, each innermost first, as the DOM
    /// fires the new target's enter (bubbling) before the old one's leave.
    static func update(_ next: [NodeView], at point: NSPoint) {
        let before = entered
        entered = next
        last = point
        for node in next where node.handlers.contains("dragenter") && !before.contains(where: { $0 === node }) {
            node.presenter?.mouseEvent(node.id, 55, node.dragLine(point))
        }
        for node in before where node.handlers.contains("dragleave") && !next.contains(where: { $0 === node }) {
            node.presenter?.mouseEvent(node.id, 56, node.dragLine(point))
        }
    }

    /// A destination's `draggingEntered` or `draggingUpdated` at `view`: the
    /// drag tracked, and `.copy` when a node from `view` up takes the drop.
    /// Nil when it carries no file, or no node takes it (a field's own then).
    static func moved(_ view: NSView, _ info: NSDraggingInfo) -> NSDragOperation? {
        guard !urls(info).isEmpty else { return nil }
        turn &+= 1
        update(chain(at: info.draggingLocation, in: view.window), at: info.draggingLocation)
        return NodeView.dropTarget(view) != nil ? .copy : nil
    }

    /// The drag left `view`. AppKit's last point as it leaves the window
    /// can still be inside it, so what is still under that point lets go a
    /// moment later unless another callback comes first (the drag moved into
    /// a registered child, or came back).
    static func exited(_ view: NSView, _ info: NSDraggingInfo?) {
        guard let info, !urls(info).isEmpty else { return }
        turn &+= 1
        let mine = turn, point = info.draggingLocation
        update(chain(at: point, in: view.window), at: point)
        guard !entered.isEmpty else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { if turn == mine { update([], at: last) } }
    }

    /// A destination's `performDragOperation` at `view`: the drop at the
    /// node that takes it, then the drag's end. Nil when no node takes it.
    static func perform(_ view: NSView, _ info: NSDraggingInfo) -> Bool? {
        let files = urls(info)
        guard !files.isEmpty, let target = NodeView.dropTarget(view) else { ended(); return nil }
        let ok = target.drop(files, at: info.draggingLocation)
        ended(dropped: ok)
        return ok
    }

    /// Whether a node from `view` up takes this drag (a field's forwarding).
    static func takes(_ view: NSView, _ info: NSDraggingInfo?) -> Bool {
        guard let info else { return false }
        return !urls(info).isEmpty && NodeView.dropTarget(view) != nil
    }

    /// The drag ended: cancelled, or dropped where nothing took it, every
    /// node still entered leaves; `dropped`, none does (the DOM's `drop` is
    /// the last event of a drag that lands).
    static func ended(dropped: Bool = false) {
        turn &+= 1
        if dropped { entered = [] } else if !entered.isEmpty { update([], at: last) }
    }

    // The agent's drag (`tap <target> dragenter <path…>`, `tap … drop`,
    // `tap … dragleave`): the session a Finder drag would run, without one.
    /// A drag of files comes over `point`, and stays.
    static func agentEnter(at point: NSPoint, in window: NSWindow) {
        turn &+= 1
        update(chain(at: point, in: window), at: point)
    }
}

// Forwarding a field's file drag (the header's last paragraph).
extension TextArea {
    package override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation { FileDrag.moved(self, sender) ?? super.draggingEntered(sender) }
    package override func draggingUpdated(_ sender: NSDraggingInfo) -> NSDragOperation { FileDrag.moved(self, sender) ?? super.draggingUpdated(sender) }
    package override func draggingExited(_ sender: NSDraggingInfo?) { FileDrag.exited(self, sender); super.draggingExited(sender) }
    package override func prepareForDragOperation(_ sender: NSDraggingInfo) -> Bool { FileDrag.takes(self, sender) || super.prepareForDragOperation(sender) }
    package override func performDragOperation(_ sender: NSDraggingInfo) -> Bool { FileDrag.perform(self, sender) ?? super.performDragOperation(sender) }
    package override func concludeDragOperation(_ sender: NSDraggingInfo?) { if !FileDrag.takes(self, sender) { super.concludeDragOperation(sender) } }
}

extension FieldEditor {
    override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation { FileDrag.moved(self, sender) ?? super.draggingEntered(sender) }
    override func draggingUpdated(_ sender: NSDraggingInfo) -> NSDragOperation { FileDrag.moved(self, sender) ?? super.draggingUpdated(sender) }
    override func draggingExited(_ sender: NSDraggingInfo?) { FileDrag.exited(self, sender); super.draggingExited(sender) }
    override func prepareForDragOperation(_ sender: NSDraggingInfo) -> Bool { FileDrag.takes(self, sender) || super.prepareForDragOperation(sender) }
    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool { FileDrag.perform(self, sender) ?? super.performDragOperation(sender) }
    override func concludeDragOperation(_ sender: NSDraggingInfo?) { if !FileDrag.takes(self, sender) { super.concludeDragOperation(sender) } }
}

// An input that is not being edited takes a drag itself (its cell's).
extension Field {
    override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation { FileDrag.moved(self, sender) ?? super.draggingEntered(sender) }
    override func draggingUpdated(_ sender: NSDraggingInfo) -> NSDragOperation { FileDrag.moved(self, sender) ?? super.draggingUpdated(sender) }
    override func draggingExited(_ sender: NSDraggingInfo?) { FileDrag.exited(self, sender); super.draggingExited(sender) }
    override func prepareForDragOperation(_ sender: NSDraggingInfo) -> Bool { FileDrag.takes(self, sender) || super.prepareForDragOperation(sender) }
    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool { FileDrag.perform(self, sender) ?? super.performDragOperation(sender) }
    override func concludeDragOperation(_ sender: NSDraggingInfo?) { if !FileDrag.takes(self, sender) { super.concludeDragOperation(sender) } }
}

extension Agent {
    /// `tap <target> drop <path…>`, `dragenter <path…>` and `dragleave`
    /// (studio diary R19): the drag a Finder drag would bring to the window
    /// point `p` over `v`, through the same chain, minting and events (no
    /// `dragleave` after a drop that lands). Nil for any other tap.
    func fileDragTap(_ req: [String: Any], _ v: NodeView, at p: NSPoint, in win: NSWindow, reply at: [Double]) -> [String: Any]? {
        if req["dragleave"] != nil {
            FileDrag.ended()
            session.presenter.settlePump()
            return ["tapped": Int(v.id), "dragleave": true, "delivery": "presenter"]
        }
        if let paths = req["dragenter"] as? [String] {
            guard !paths.isEmpty else { return ["malformed": true, "error": "dragenter: expected the dragged paths"] }
            FileDrag.agentEnter(at: p, in: win)
            session.presenter.settlePump()
            return ["tapped": Int(v.id), "at": at, "dragenter": paths.count, "delivery": "presenter"]
        }
        guard let paths = req["drop"] as? [String] else { return nil }
        guard let node = NodeView.dropTarget(win.contentView?.hitTest(p) ?? v) else {
            FileDrag.ended()
            return ["error": "view \(v.id) takes no drop: nothing under it declares `drop`"]
        }
        FileDrag.agentEnter(at: p, in: win)
        let ok = node.drop(paths.map { URL(fileURLWithPath: $0) }, at: p)
        FileDrag.ended(dropped: ok)
        session.presenter.settlePump()
        guard ok else { return ["error": "drop: refused: no file could be opened"] }
        return ["tapped": Int(node.id), "at": at, "drop": paths.count, "delivery": "presenter"]
    }
}
#endif

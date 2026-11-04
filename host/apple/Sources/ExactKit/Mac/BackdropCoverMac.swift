// A focused node under a later `backdrop-filter` node (LLP 1115.002 D7, the
// user's ruling of 2026-11-05). AppKit draws the system focus ring over the
// window's content, where the cover's filters never sample it, so the ring
// would show sharp through a dimmed, blurred cover; Chrome paints its ring
// in the page and blurs it with the rest. While a later backdrop node covers
// a ring's bounds the node draws no ring; it keeps the focus, and the ring
// comes back when the cover goes.
#if os(macOS)
import AppKit

enum BackdropCovers {
    /// Every node whose style has a `backdrop-filter`, in any window.
    nonisolated(unsafe) static let nodes = NSHashTable<NodeView>.weakObjects()

    /// A cover came, moved or went in `window`: the focus there asks for its
    /// ring again. A field's ring is its node's, not its editor's.
    static func changed(in window: NSWindow?) {
        guard let responder = window?.firstResponder as? NSView else { return }
        responder.noteFocusRingMaskChanged()
        guard !(responder is NodeView) else { return }
        var at = responder.superview
        while let view = at, !(view is NodeView) { at = view.superview }
        at?.noteFocusRingMaskChanged()
    }
}

extension NodeView {
    /// Keeps the registry and the focused ring in step with this node's
    /// backdrop; `applyBackdrop` calls it on every change of style or box.
    func backdropCoverChanged() {
        let covers = !backdropOperations.isEmpty
        let known = BackdropCovers.nodes.contains(self)
        if covers { BackdropCovers.nodes.add(self) } else { BackdropCovers.nodes.remove(self) }
        if covers || known { BackdropCovers.changed(in: window) }
    }

    package override func viewDidHide() {
        super.viewDidHide()
        if BackdropCovers.nodes.contains(self) { BackdropCovers.changed(in: window) }
    }

    package override func viewDidUnhide() {
        super.viewDidUnhide()
        if BackdropCovers.nodes.contains(self) { BackdropCovers.changed(in: window) }
    }
}

extension NSView {
    /// `ring`, a focus ring's mask bounds, or none while a cover is over it.
    func uncovered(_ ring: NSRect) -> NSRect { ringCovered(ring) ? .zero : ring }

    /// Whether a shown `backdrop-filter` node that paints over this view
    /// covers `ring` (this view's coordinates) in its window.
    func ringCovered(_ ring: NSRect) -> Bool {
        guard !ring.isEmpty, let window, BackdropCovers.nodes.count > 0 else { return false }
        let rect = convert(ring, to: nil)
        return BackdropCovers.nodes.allObjects.contains { cover in
            cover.window === window && !cover.isHiddenOrHasHiddenAncestor && !cover.cssVisibilityHidden
                && !cover.backdropOperations.isEmpty
                && cover.convert(cover.visibleRect, to: nil).contains(rect)
                && cover.paintsOver(self)
        }
    }

    /// Whether this view is composited over `other`, neither holding the
    /// other: where their branches meet, Core Animation orders sibling
    /// layers by `zPosition`, then by place (`PaintOrder.swift`).
    func paintsOver(_ other: NSView) -> Bool {
        let theirs = Array(sequence(first: other, next: \.superview))
        guard !theirs.contains(where: { $0 === self }) else { return false }
        var branch: NSView = self
        while let parent = branch.superview {
            if let index = theirs.firstIndex(where: { $0 === parent }) {
                guard index > 0 else { return false }
                let under = theirs[index - 1]
                let a = branch.layer?.zPosition ?? 0, b = under.layer?.zPosition ?? 0
                if a != b { return a > b }
                let order = parent.subviews
                return (order.firstIndex { $0 === branch } ?? 0) > (order.firstIndex { $0 === under } ?? 0)
            }
            branch = parent
        }
        return false
    }
}
#endif

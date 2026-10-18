#if os(macOS)
import AppKit

// The pointer's hover on the Mac, beside `Presenter.setHoverPath`: a native
// module's, and a node's `:hover` look (LLP 1121 D8), which every node under
// the pointer that has one shows at once, with no runner round trip and
// whether or not it hears `hover`.
extension Presenter {
    /// Native-module hover dispatch uses the same ancestor coverage.
    func hover(_ view: NodeView, _ over: Bool) {
        guard views[view.id] === view else { return }
        if over {
            var path: [NodeView] = []
            var current: NSView? = view
            while let node = current {
                if let node = node as? NodeView { path.append(node) }
                current = node.superview
            }
            setHoverPath(path)
        } else {
            setHoverPath(hoveredNodes.filter { !$0.isDescendant(of: view) })
        }
    }
    /// The `:hover` looks of the path from `leaf` in its own window (a
    /// sheet's, a popover's): it and its authored ancestors.
    func hoverLooks(from leaf: NodeView?) {
        var path: [NodeView] = []
        var view: NSView? = leaf
        while let current = view {
            if let node = current as? NodeView { path.append(node) }
            view = eventParent(of: current)
        }
        hoverLooks(path)
    }
    /// The `:hover` looks of the hover path `path` (innermost first, every
    /// ancestor): on for those that have one and show, off for the rest.
    func hoverLooks(_ path: [NodeView]) {
        let next = path.filter {
            // In the page, or in a sheet's or a popover's own window (`trackPointer`).
            views[$0.id] === $0 && $0.window != nil && !$0.inert && !$0.isHiddenOrHasHiddenAncestor
                && NodeView.look("hover", in: $0.style) != nil
        }
        for held in lookHovered {
            // The pointer leaving a held hover's node lets it go (HeldState.swift), as on the web.
            if let view = held.view, !next.contains(where: { $0 === view }) { view.heldInput("hover"); view.lookState(hovered: false) }
        }
        // The pointer coming over it lets one go too (moving within it does not).
        for view in next {
            if !lookHovered.contains(where: { $0.view === view }) { view.heldInput("hover") }
            view.lookState(hovered: true)
        }
        lookHovered = next.map { WeakNodeView(view: $0) }
    }
}
#endif

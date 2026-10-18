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
    /// The `:hover` looks of the hover path `path` (innermost first, every
    /// ancestor): on for those that have one and show, off for the rest.
    func hoverLooks(_ path: [NodeView]) {
        let next = path.filter {
            views[$0.id] === $0 && $0.isDescendant(of: viewport) && !$0.inert && !$0.isHiddenOrHasHiddenAncestor
                && NodeView.look("hover", in: $0.style) != nil
        }
        for held in lookHovered {
            if let view = held.view, !next.contains(where: { $0 === view }) { view.lookState(hovered: false) }
        }
        for view in next { view.lookState(hovered: true) }
        lookHovered = next.map { WeakNodeView(view: $0) }
    }
}
#endif

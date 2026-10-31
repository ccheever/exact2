// PROTO (rule 4, "Exact describes, UIKit drives"; notes `design/rule4-proto.md`). A route
// whose top edge moves (a large title collapsing) has the boxes that depend on that edge
// laid out a second time by the kernel, with the edge at the block's top
// (`Kernel::trial_clearance_frames`), and each such box arrives with both frames (`far`
// ops). UIKit moves the edge; in its own pass for the route's controller
// (`viewSafeAreaInsetsDidChange`), each box is placed between its two frames as far as the
// edge has moved, through the path a frame op takes. A box that reaches the top edge, and
// what it holds, is UIKit's to inset and is left alone, as a scroll view's content keeps
// its layout while UIKit collapses a title over it.
#if os(iOS)
import UIKit


extension Presenter {
    /// A `far` op: its route's frames replaced (`property` "reset", `id` the route), or a
    /// box's two frames (`property` "<route> x y w h", its frame now; the op's own frame,
    /// with the edge at the block's top).
    func farFrame(_ op: BatchOp) {
        if op.property == "reset" { edgeFrames[op.id] = [:]; return }
        let words = op.property.split(separator: " ")
        guard words.count == 5, let route = UInt32(words[0]) else { return }
        let n = words.dropFirst().compactMap { Double($0) }
        guard n.count == 4 else { return }
        edgeFrames[route, default: [:]][op.id] = (rest: CGRect(x: n[0], y: n[1], width: n[2], height: n[3]),
                                                  far: CGRect(x: op.x, y: op.y, width: op.w, height: op.h))
    }

    /// After a batch, which places each box at its resting frame: each moving route's
    /// boxes where the edge is now.
    func placeEdgesAfterBatch() {
        for route in edgeFrames.keys {
            guard let c = navigation.controllers[route] else { continue }
            // At rest, the batch placed what it moved and the rest stands
            if c.placedEdge != 0 { c.placedEdge = nil }
            placeEdge(c)
        }
    }

    /// The boxes of `c`'s route that depend on its top edge, each between its two frames
    /// as far as UIKit has moved the edge from its resting place (`restingTop`) toward the
    /// block's top.
    func placeEdge(_ c: RouteController) {
        guard let frames = edgeFrames[c.node.id], !frames.isEmpty, c.restingTop > 0, c.viewIfLoaded?.window != nil else { return }
        let t = Double(min(1, max(0, (c.restingTop - c.view.safeAreaInsets.top) / c.restingTop)))
        guard t != c.placedEdge else { return }
        c.placedEdge = t
        for (id, f) in frames {
            guard let v = views[id], !reachesTop(v, under: c.node) else { continue }
            var op = BatchOp(op: .frame, nodeID: id)
            op.x = f.rest.minX + (f.far.minX - f.rest.minX) * t
            op.y = f.rest.minY + (f.far.minY - f.rest.minY) * t
            op.w = f.rest.width + (f.far.width - f.rest.width) * t
            op.h = f.rest.height + (f.far.height - f.rest.height) * t
            applyGeometry(op)
        }
        regridMoved()
    }

    /// Whether `v`, or a box holding it under `root`, reaches the top edge.
    private func reachesTop(_ v: NodeView, under root: NodeView) -> Bool {
        var view: UIView? = v
        while let at = view, at !== root {
            if let node = at as? NodeView, node.reach.top > 0 { return true }
            view = at.superview
        }
        return false
    }
}
#endif

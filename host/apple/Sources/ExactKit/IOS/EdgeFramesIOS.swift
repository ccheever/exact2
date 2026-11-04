// PROTO (rule 4, "Exact describes, UIKit drives"; notes `design/rule4-proto.md`). A route
// whose top edge moves (a large title collapsing, the bar minimizing) has the boxes that depend on that edge
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
    /// box's frames along the edge's range (`property` "<route> x y w h[; t x y w h]…": its
    /// frame now, then at each knot `t` of the way to the block's top; the op's own frame,
    /// with the edge at the block's top).
    func farFrame(_ op: BatchOp) {
        if op.property.hasPrefix("reset") { edgeFrames[op.id] = [:]; NSLog("PROTO rule4 far pass %@", op.property); return }
        let parts = op.property.split(separator: ";").map { $0.split(separator: " ").compactMap { Double($0) } }
        guard let first = parts.first, first.count == 5, parts.dropFirst().allSatisfy({ $0.count == 5 }) else { return }
        let rect = { (n: ArraySlice<Double>) in CGRect(x: n[n.startIndex], y: n[n.startIndex + 1], width: n[n.startIndex + 2], height: n[n.startIndex + 3]) }
        let inner: [(t: Double, frame: CGRect)] = parts.dropFirst().map { (t: $0[0], frame: rect($0.dropFirst())) }
        edgeFrames[UInt32(first[0]), default: [:]][op.id] = [(t: 0, frame: rect(first.dropFirst()))] + inner + [(t: 1, frame: CGRect(x: op.x, y: op.y, width: op.w, height: op.h))]
        if !inner.isEmpty { NSLog("PROTO rule4 knots %d for %u", inner.count, op.id) } // PROTO: how far the range was halved
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
        guard c.restingTop > 0, c.viewIfLoaded?.window != nil else { return }
        guard let frames = edgeFrames[c.node.id], !frames.isEmpty else {
            noteUnforeseenMove(c)
            return
        }
        let t = Double(min(1, max(0, (c.restingTop - c.view.safeAreaInsets.top) / c.restingTop)))
        guard t != c.placedEdge else { return }
        c.placedEdge = t
        for (id, knots) in frames {
            guard let v = views[id], !reachesTop(v, under: c.node) else { continue }
            // Between the neighbouring knots around `t`, along which the box is linear in the edge
            let i = max(1, knots.firstIndex { $0.t >= t } ?? knots.count - 1)
            let (a, b) = (knots[i - 1], knots[i]), u = b.t > a.t ? (t - a.t) / (b.t - a.t) : 0
            var op = BatchOp(op: .frame, nodeID: id)
            op.x = a.frame.minX + (b.frame.minX - a.frame.minX) * u
            op.y = a.frame.minY + (b.frame.minY - a.frame.minY) * u
            op.w = a.frame.width + (b.frame.width - a.frame.width) * u
            op.h = a.frame.height + (b.frame.height - a.frame.height) * u
            applyGeometry(op)
        }
        regridMoved()
    }

    /// Registers `c`'s route as one whose top edge moves when UIKit has moved the edge outside a transition though
    /// nothing configured says it would (a bar minimizing under `.automatic`, where the system decides): its boxes
    /// follow from the far pass that registration owes, measured 0.7 to 5.5 ms after this first move on an iPhone 18 Pro (five runs).
    private func noteUnforeseenMove(_ c: RouteController) {
        let id = c.node.id
        guard !movingTops.contains(id), c.transitionCoordinator == nil, c.view.safeAreaInsets.top < c.restingTop - 0.5 else { return }
        movedTops.insert(id)
        movingTops.insert(id)
        deliveredMovingTops.insert(id)
        onMovingTops?([(id, true)])
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

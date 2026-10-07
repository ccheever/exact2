// CSS Scroll Anchoring (https://drafts.csswg.org/css-scroll-anchoring/) for a
// plain scroll container on the Apple hosts (#138 X23d). `overflow-anchor:
// auto` is CSS's default, so a browser keeps the visible content still when
// a box above the port changes size or is inserted: it selects an anchor node
// before layout and moves the scroll offset by the anchor's movement after.
// The web gets it from the browser; a list is the runner's (LLP 1010 §6.6).
// Here the presenter snapshots before each batch and restores after layout
// (LLP 1001), with the spec's selection, exclusions and suppression triggers.
// Not here: `overflow-anchor: none` (not a Contract property), priority
// candidates (the focused editable, find-in-page), and the inline axis.
#if os(iOS) || os(tvOS)
import UIKit
#else
import AppKit
#endif

enum ScrollAnchoring {
    /// The spec's candidate examination (Chromium's `ScrollAnchor::Examine`).
    enum Verdict: Equatable { case skip, select, descend }

    /// A box with no area, or none of it in the port, is skipped with its
    /// subtree; a fully visible one is the anchor; a partly visible one is
    /// descended into, and is the anchor when nothing inside it is.
    static func examine(_ rect: CGRect, port: CGRect) -> Verdict {
        guard rect.width > 0, rect.height > 0, rect.intersects(port) else { return .skip }
        return port.contains(rect) ? .select : .descend
    }

    /// The selection, in tree order. `rect` is in the scroller's content space.
    static func select<Node>(_ nodes: [Node], port: CGRect, children: (Node) -> [Node],
                             excluded: (Node) -> Bool, rect: (Node) -> CGRect) -> Node? {
        for node in nodes where !excluded(node) {
            switch examine(rect(node), port: port) {
            case .skip: continue
            case .select: return node
            case .descend:
                return select(children(node), port: port, children: children, excluded: excluded, rect: rect) ?? node
            }
        }
        return nil
    }

    /// The offset after the adjustment: moved by the anchor's movement and
    /// clamped to the range, as the browser's scroll does.
    static func adjusted(top: CGFloat, before: CGFloat, after: CGFloat, minimum: CGFloat, maximum: CGFloat) -> CGFloat {
        min(max(top + after - before, minimum), max(minimum, maximum))
    }

    /// The suppression triggers' properties (spec §2.4): a change to one on
    /// the anchor or a box between it and the scroller cancels the adjustment.
    static let layoutKeys: Set<String> = [
        "top", "left", "right", "bottom", "inset", "position_type",
        "margin_top", "margin_left", "margin_right", "margin_bottom",
        "padding_top", "padding_left", "padding_right", "padding_bottom",
        "width", "height", "min_width", "min_height", "max_width", "max_height",
        "transform", "translate", "translate_percent", "translate_z", "rotate", "scale",
    ]

    /// What a batch's style ops changed that suppresses anchoring.
    struct Changes {
        /// Boxes whose layout properties changed.
        var layout: Set<UInt32> = []
        /// Boxes that became, or stopped being, out of flow. The spec suppresses
        /// for one anywhere in the scroller (§2.4, "becomes or stops being
        /// absolutely positioned"), not only on the anchor's chain.
        var positioned: Set<UInt32> = []
        mutating func note(_ id: UInt32, from old: NodeStyle, to new: NodeStyle) {
            if ScrollAnchoring.layoutKeys.contains(where: { old[$0] != new[$0] }) { layout.insert(id) }
            // Out of flow is absolute or fixed (Chromium's `IsOutOfFlowPositioned`).
            let outOfFlow = { (s: NodeStyle) in ["absolute", "fixed"].contains(s["position_type"]?.string ?? "") }
            if outOfFlow(old) != outOfFlow(new) { positioned.insert(id) }
        }
        mutating func reset() { layout.removeAll(keepingCapacity: true); positioned.removeAll(keepingCapacity: true) }
    }

    /// Excluded subtrees (spec §2.3): `display: none`, a fixed or sticky box,
    /// and an absolute one whose containing block is outside the scroller.
    static func excluded(_ node: NodeView, scroller: NodeView) -> Bool {
        if node.style["display"]?.string == "none" { return true }
        switch node.style["position_type"]?.string {
        case "fixed", "sticky": return true
        case "absolute":
            var above = node.superview
            while let view = above, view !== scroller {
                if let n = view as? NodeView, containsAbsolute(n) { return false }
                above = view.superview
            }
            return !containsAbsolute(scroller)
        default: return false
        }
    }

    /// Whether a box is an absolute box's containing block: positioned, or
    /// transformed or filtered (CSS Positioned Layout §3.1, CSS Transforms).
    private static func containsAbsolute(_ n: NodeView) -> Bool {
        if let p = n.style["position_type"]?.string, p != "static" { return true }
        return ["transform", "translate", "translate_percent", "rotate", "scale", "filter"].contains { n.style[$0] != nil }
    }

    /// The anchor in `scroller`'s content view `space`, whose visible part is
    /// `port`. A nested scroller is a candidate, its content is not: that
    /// moves with its own offset.
    static func anchor(in scroller: NodeView, space: PlatformView, port: CGRect) -> (node: NodeView, y: CGFloat)? {
        // A view leaving with an exit animation stays in the window but is no
        // longer the presenter's: the DOM has removed it, so it is no candidate.
        let live = { (n: NodeView) in scroller.presenter?.views[n.id] === n }
        let roots = space.subviews.compactMap { $0 as? NodeView }
        let rect = { (n: NodeView) in n.convert(n.bounds, to: space) }
        guard let node = select(roots, port: port,
                                children: { $0.scroll == nil ? $0.container.subviews.compactMap { $0 as? NodeView } : [] },
                                excluded: { !live($0) || excluded($0, scroller: scroller) }, rect: rect) else { return nil }
        return (node, rect(node).minY)
    }

    /// The anchor's movement since it was chosen, or nil when the anchor is
    /// gone or a suppression trigger fired during the batch.
    static func shift(of anchor: (node: NodeView, y: CGFloat), in scroller: NodeView, space: PlatformView,
                      changes: Changes, live: (NodeView) -> Bool) -> CGFloat? {
        let node = anchor.node
        guard live(node), node.isDescendant(of: space), node.style["display"]?.string != "none" else { return nil }
        var above: PlatformView? = node
        while let view = above, view !== scroller {
            if let n = view as? NodeView, changes.layout.contains(n.id) { return nil }
            above = view.superview
        }
        // A box that became or stopped being absolutely positioned anywhere in
        // the scroller suppresses too.
        for id in changes.positioned where scroller.presenter?.views[id]?.isDescendant(of: space) == true { return nil }
        return node.convert(node.bounds, to: space).minY - anchor.y
    }
}

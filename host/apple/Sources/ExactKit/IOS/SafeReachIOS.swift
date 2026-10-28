// PROTO (safe-area design r2 §2.3, §5.3; not landing code). A route's root is laid out in its
// safe area (the kernel's clearance); after each batch, the surfaces that meet the safe area's
// edge reach the allocation's edge on that side as overflow. Their native views are outset with
// the bounds origin shifted, so their subviews stay where layout put them while the view covers
// the band and UIKit's safe area reaches the scroll views inside (LLP 1131: it does not reach a
// view drawn past its superview's box). The design computes reach in the kernel; this prototype
// computes it here from the laid-out frames.
#if os(iOS) || os(tvOS)
import UIKit

extension Presenter {
    /// Every shown route's reach, after a batch's frames.
    func reachSafeEdges() {
        for c in navigation.controllers.values where views[c.node.id] === c.node { reachSafeEdges(of: c) }
    }

    private func reachSafeEdges(of c: RouteController) {
        guard let view = c.viewIfLoaded, view.window != nil, case .edges(let e)? = navigation.covers[c.node.id] else {
            SafeReach.clear(c.node)
            return
        }
        let a = view.bounds
        let s = CGRect(x: a.minX + e.left, y: a.minY + e.top, width: max(0, a.width - e.left - e.right), height: max(0, a.height - e.top - e.bottom))
        let walk = SafeReach(view: view, allocation: a, safe: s, pixel: 1 / max(1, view.traitCollection.displayScale))
        _ = walk.visit(c.node, allowed: .all)
    }
}

/// One route's walk: which edges each box touches, which it reaches, and the outset applied.
struct SafeReach {
    let view: UIView, allocation: CGRect, safe: CGRect, pixel: CGFloat

    /// The edges a box or something inside it reaches.
    @discardableResult
    func visit(_ v: NodeView, allowed: UIRectEdge) -> UIRectEdge {
        guard !v.isHidden else { Self.apply(.zero, to: v); return [] }
        let r = v.convert(v.boxBounds, to: view)
        var touches: UIRectEdge = []
        if allowed.contains(.top), safe.minY > allocation.minY + pixel, abs(r.minY - safe.minY) <= pixel { touches.insert(.top) }
        if allowed.contains(.bottom), safe.maxY < allocation.maxY - pixel, abs(r.maxY - safe.maxY) <= pixel { touches.insert(.bottom) }
        if allowed.contains(.left), safe.minX > allocation.minX + pixel, abs(r.minX - safe.minX) <= pixel { touches.insert(.left) }
        if allowed.contains(.right), safe.maxX < allocation.maxX - pixel, abs(r.maxX - safe.maxX) <= pixel { touches.insert(.right) }
        let decorated = Self.decorated(v)
        let contained = Self.contained(v)
        // What may reach inside it: where it touches, not past a clip that does not extend or a `contain`ed edge,
        // and in a scroller only across its scrolling axis (scrolled content never reaches along it).
        var inside = touches.subtracting(contained)
        if decorated && Self.clips(v) { inside = [] }
        if v.scroll != nil {
            let axes = Self.axes(v)
            if axes.y { inside.subtract([.top, .bottom]) }
            if axes.x { inside.subtract([.left, .right]) }
        }
        var below: UIRectEdge = []
        for child in v.container.subviews.compactMap({ $0 as? NodeView }) { below.formUnion(visit(child, allowed: inside)) }
        let leaf = v.container.subviews.allSatisfy { !($0 is NodeView) } && v.scroll == nil
        let surface = v.scroll != nil || (Self.fills(v) && !leaf) || Self.clips(v)
        var reach = UIEdgeInsets.zero
        for edge in [UIRectEdge.top, .left, .bottom, .right] where touches.contains(edge) && !contained.contains(edge) {
            // A decorated box covers for what reaches inside it but paints nothing new: the prototype
            // keeps it in its box (the design separates paint from coverage).
            guard !decorated, surface || below.contains(edge) else { continue }
            switch edge {
            case .top: reach.top = r.minY - allocation.minY
            case .left: reach.left = r.minX - allocation.minX
            case .bottom: reach.bottom = allocation.maxY - r.maxY
            default: reach.right = allocation.maxX - r.maxX
            }
        }
        Self.apply(reach, to: v)
        var reached: UIRectEdge = below
        if reach.top > 0 { reached.insert(.top) }
        if reach.left > 0 { reached.insert(.left) }
        if reach.bottom > 0 { reached.insert(.bottom) }
        if reach.right > 0 { reached.insert(.right) }
        return reached
    }

    /// Outsets `v`'s frame by `reach` and shifts its bounds origin by as much; a scroller's scroll
    /// view follows its bounds, takes UIKit's adjustment on the axes it scrolls, and is offset by
    /// the reach across them.
    static func apply(_ reach: UIEdgeInsets, to v: NodeView) {
        guard reach != v.reach else { return }
        let base = v.frame.inset(by: v.reach)
        v.reach = reach
        v.bounds.origin = CGPoint(x: -reach.left, y: -reach.top)
        v.frame = base.inset(by: UIEdgeInsets(top: -reach.top, left: -reach.left, bottom: -reach.bottom, right: -reach.right))
        guard let sv = v.scroll else { return }
        sv.frame = v.scrollFrame
        let axes = Self.axes(v)
        // Across the axes it scrolls, the content keeps its laid-out place: Exact's offset (by delta, as bands are).
        let cross = UIEdgeInsets(top: axes.y ? 0 : reach.top, left: axes.x ? 0 : reach.left,
                                 bottom: axes.y ? 0 : reach.bottom, right: axes.x ? 0 : reach.right)
        let old = v.reachInset
        v.reachInset = cross
        sv.contentInset = UIEdgeInsets(top: sv.contentInset.top + cross.top - old.top, left: sv.contentInset.left + cross.left - old.left,
                                       bottom: sv.contentInset.bottom + cross.bottom - old.bottom, right: sv.contentInset.right + cross.right - old.right)
        // Along them, UIKit's adjustment; an authored axis bounces, so UIKit counts it as scrollable.
        let along = (axes.y && (reach.top > 0 || reach.bottom > 0)) || (axes.x && (reach.left > 0 || reach.right > 0))
        if along, sv.contentInsetAdjustmentBehavior == .never { sv.contentInsetAdjustmentBehavior = .automatic }
        if axes.x && (reach.left > 0 || reach.right > 0) { sv.alwaysBounceHorizontal = true }
        v.fitScroll()
    }

    /// The axes a scroller scrolls: one the author made `scroll`, or `auto` with content past its port, as
    /// UIKit's `.scrollableAxes` counts them. The other is its cross axis, where its content keeps its place.
    static func axes(_ v: NodeView) -> (x: Bool, y: Bool) {
        guard let sv = v.scroll else { return (false, false) }
        let box = v.boxBounds
        let x = sv.scrollsX && (v.style["overflow_x"]?.string == "scroll" || v.content.width > box.width + 0.5)
        let y = sv.scrollsY && (v.style["overflow_y"]?.string == "scroll" || v.content.height > box.height + 0.5)
        return (x, y)
    }

    static func clear(_ v: NodeView) {
        apply(.zero, to: v)
        for child in v.container.subviews.compactMap({ $0 as? NodeView }) { clear(child) }
    }

    /// A plain fill: a colour or a material.
    static func fills(_ v: NodeView) -> Bool {
        if v.materialKind != nil { return true }
        guard let c = v.channels("background_color") else { return false }
        return c.count < 4 || c[3] > 0
    }

    static func clips(_ v: NodeView) -> Bool {
        let x = v.style["overflow_x"]?.string ?? "visible", y = v.style["overflow_y"]?.string ?? "visible"
        return v.scroll == nil && (x != "visible" || y != "visible")
    }

    /// Paint that growing would change: a border, a corner radius, a shadow, an image, a clip path, a filter, a transform.
    static func decorated(_ v: NodeView) -> Bool {
        if v.number("border_width") > 0 { return true }
        for side in ["top", "right", "bottom", "left"] where v.number("border_width_" + side) > 0 { return true }
        if v.number("border_radius") > 0 { return true }
        for corner in ["top_left", "top_right", "bottom_left", "bottom_right"] where v.number("border_radius_" + corner) > 0 { return true }
        for key in ["box_shadow", "background_image", "clip_path", "filter", "transform"] {
            if let s = v.style[key]?.string, s != "none", !s.isEmpty { return true }
        }
        return false
    }

    /// The edges `-exact-safe-area="contain …"` keeps in.
    static func contained(_ v: NodeView) -> UIRectEdge {
        guard let raw = v.props["safeArea"], raw.hasPrefix("contain") else { return [] }
        let words = raw.split(separator: " ").dropFirst()
        if words.isEmpty { return .all }
        var out: UIRectEdge = []
        for w in words {
            switch w { case "top": out.insert(.top); case "right": out.insert(.right); case "bottom": out.insert(.bottom); case "left": out.insert(.left); default: break }
        }
        return out
    }
}
#endif

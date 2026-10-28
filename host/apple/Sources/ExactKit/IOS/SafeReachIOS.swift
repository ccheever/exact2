// PROTO (safe-area design r3 §2.3, §5.3; not landing code). A route's root is laid out in its
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
        // Its laid-out box: its bounds less its reach (origin shifted) or less what it takes (origin kept).
        let r = v.convert(v.boxBounds.inset(by: v.take), to: view)
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
        // A leaf that takes its edges (`-exact-safe-area="ignore …"`, §2.4) is laid out to the allocation's
        // edge with its content: its frame grows, its bounds origin stays, so what fills its bounds (a module's
        // view, its own layer) fills the band, and UIKit's safe area tells a native view where its controls go.
        if leaf {
            let taken = Self.ignored(v).intersection(touches)
            var take = UIEdgeInsets.zero
            if taken.contains(.top) { take.top = r.minY - allocation.minY }
            if taken.contains(.left) { take.left = r.minX - allocation.minX }
            if taken.contains(.bottom) { take.bottom = allocation.maxY - r.maxY }
            if taken.contains(.right) { take.right = allocation.maxX - r.maxX }
            Self.take(take, to: v)
            // Its ancestors cover what it takes, so UIKit's safe area reaches the view inside it.
            below.formUnion(taken)
        }
        let surface = v.scroll != nil || (Self.fills(v) && !leaf) || Self.clips(v)
        var reach = UIEdgeInsets.zero
        // An edge a leaf takes is laid out to the allocation already: it does not also reach there.
        let takes = leaf ? Self.ignored(v) : []
        for edge in [UIRectEdge.top, .left, .bottom, .right] where touches.contains(edge) && !contained.contains(edge) && !takes.contains(edge) {
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

    /// Outsets `v`'s frame by `reach` and shifts its bounds origin by as much. A reaching scroller's scroll view
    /// follows its bounds, and UIKit adjusts its content on the axes it scrolls (`.automatic`; one inside another
    /// scroller's content `.scrollableAxes`, its main axis bouncing so that it counts while short). UIKit's
    /// adjustment consumes the safe area it applies, so its content's subviews do not get it again; the rest
    /// (the cross axis) stays in the content's safe area, where a nested carousel takes it (`reconcile`).
    static func apply(_ reach: UIEdgeInsets, to v: NodeView) {
        if reach != v.reach {
            let base = v.frame.inset(by: v.reach)
            v.reach = reach
            v.bounds.origin = CGPoint(x: -reach.left, y: -reach.top)
            v.frame = base.inset(by: UIEdgeInsets(top: -reach.top, left: -reach.left, bottom: -reach.bottom, right: -reach.right))
        }
        guard let sv = v.scroll else { return }
        if sv.frame != v.scrollFrame { sv.frame = v.scrollFrame }
        if reach == .zero {
            if let set = v.reachBehavior, sv.contentInsetAdjustmentBehavior == set { sv.contentInsetAdjustmentBehavior = .never }
            v.reachBehavior = nil
        } else {
            let nested = Self.nested(v)
            let want: UIScrollView.ContentInsetAdjustmentBehavior = nested ? .scrollableAxes : .automatic
            if sv.contentInsetAdjustmentBehavior == .never || (nested && sv.contentInsetAdjustmentBehavior != want) { sv.contentInsetAdjustmentBehavior = want }
            v.reachBehavior = sv.contentInsetAdjustmentBehavior
            let axes = Self.axes(v)
            if nested, axes.x, !sv.alwaysBounceHorizontal { sv.alwaysBounceHorizontal = true }
        }
        v.fitScroll()
        reconcile(v)
    }

    /// The content's place on each edge a scroller reaches: UIKit's adjustment where UIKit applies one, and
    /// `contentInset` for the rest of the scroll view's own safe-area inset there (its cross axis), read from
    /// UIKit, never computed: the scroll view's `safeAreaInsets` less UIKit's part (`adjustedContentInset` less
    /// `contentInset`). Called again whenever UIKit's adjustment changes (`adjustedContentInsetDidChange`).
    static func reconcile(_ v: NodeView) {
        guard let sv = v.scroll else { return }
        let r = v.reach, safe = sv.safeAreaInsets, adj = sv.adjustedContentInset, ci = sv.contentInset
        func part(_ reach: CGFloat, _ safe: CGFloat, _ adjusted: CGFloat, _ content: CGFloat) -> CGFloat {
            reach > 0 ? max(0, safe - (adjusted - content)) : 0
        }
        let want = UIEdgeInsets(top: part(r.top, safe.top, adj.top, ci.top), left: part(r.left, safe.left, adj.left, ci.left),
                                bottom: part(r.bottom, safe.bottom, adj.bottom, ci.bottom), right: part(r.right, safe.right, adj.right, ci.right))
        let old = v.reachInset
        guard want != old else { return }
        v.reachInset = want
        // UIKit keeps the offset as an inset changes; content resting at its start stays there (CSS scrollLeft and
        // scrollTop 0), as it does when UIKit's own adjustment arrives (`NavigationBar`'s `atStart`).
        let atStart = (x: sv.contentOffset.x <= 0.5 - adj.left, y: sv.contentOffset.y <= 0.5 - adj.top)
        sv.contentInset = UIEdgeInsets(top: ci.top + want.top - old.top, left: ci.left + want.left - old.left,
                                       bottom: ci.bottom + want.bottom - old.bottom, right: ci.right + want.right - old.right)
        let now = sv.adjustedContentInset
        let o = CGPoint(x: atStart.x ? -now.left : sv.contentOffset.x, y: atStart.y ? -now.top : sv.contentOffset.y)
        if o != sv.contentOffset { sv.contentOffset = PixelGrid.offset(o, in: sv) }
    }

    /// Whether `v` lies in another scroller's scrolled content (the presenter's viewport is no author's).
    static func nested(_ v: NodeView) -> Bool {
        var up = v.superview
        while let u = up {
            if u is ScrollView, !(u is Viewport) { return true }
            up = u.superview
        }
        return false
    }

    /// The axes a scroller scrolls, from what the author wrote and never from how much content it holds: an
    /// axis whose overflow is `scroll`, or `auto` along its main axis (a row flex container's inline axis,
    /// otherwise its block axis). Its other axis is its cross axis.
    static func axes(_ v: NodeView) -> (x: Bool, y: Bool) {
        guard let sv = v.scroll else { return (false, false) }
        let row = v.style["display"]?.string == "flex" && !(v.style["flex_direction"]?.string ?? "row").hasPrefix("column")
        let x = sv.scrollsX && (v.style["overflow_x"]?.string == "scroll" || row)
        let y = sv.scrollsY && (v.style["overflow_y"]?.string == "scroll" || !row)
        return (x, y)
    }

    /// Outsets a leaf that takes its edges, its bounds origin kept.
    static func take(_ take: UIEdgeInsets, to v: NodeView) {
        guard take != v.take else { return }
        let base = v.frame.inset(by: v.take)
        v.take = take
        v.frame = base.inset(by: UIEdgeInsets(top: -take.top, left: -take.left, bottom: -take.bottom, right: -take.right))
    }

    /// The edges `-exact-safe-area="ignore …"` (or D4's `none`) takes.
    static func ignored(_ v: NodeView) -> UIRectEdge {
        guard let raw = v.props["safeArea"] else { return [] }
        if raw == "none" || raw == "ignore" { return .all }
        guard raw.hasPrefix("ignore ") else { return [] }
        var out: UIRectEdge = []
        for w in raw.split(separator: " ").dropFirst() {
            switch w { case "top": out.insert(.top); case "right": out.insert(.right); case "bottom": out.insert(.bottom); case "left": out.insert(.left); default: break }
        }
        return out
    }

    static func clear(_ v: NodeView) {
        take(.zero, to: v)
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

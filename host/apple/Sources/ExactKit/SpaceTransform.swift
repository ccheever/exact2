// CSS's 3D transforms on both Apple platforms (LLP 1077 D8): `rotate` about
// an axis, `translate`'s z, a parent's `perspective` about its
// `perspective-origin`, and `backface-visibility`. Core Animation's layer
// transform is CSS's matrix (WebKit draws CSS 3D with it); its sublayers
// flatten into its plane, which is CSS's initial `transform-style: flat`.
import QuartzCore
#if os(iOS)
import UIKit
#else
import AppKit
#endif

extension NodeView {
    /// The node's transform in space when it has one — a rotation out of the
    /// screen's plane or a z translation — about `origin` (from the layer's
    /// anchor), else nil and the 2D path applies. `shift` is what a lifted
    /// Arrange row already moved by its frame (macOS).
    func spaceTransform(origin d: CGPoint, scale s: CGFloat, shift: CGPoint = .zero) -> CATransform3D? {
        let axis = style["rotate_axis"]?.numbers ?? [0, 0, 1]
        let z = number("translate_z")
        guard axis.count == 3, (axis[0] != 0 || axis[1] != 0 || z != 0) else { return nil }
        var t = CATransform3DMakeTranslation(translate.x - shift.x + d.x, translate.y - shift.y + d.y, z)
        t = CATransform3DRotate(t, rotate * .pi / 180, axis[0], axis[1], axis[2])
        t = CATransform3DScale(t, s, s, 1)
        return CATransform3DTranslate(t, -d.x, -d.y, 0)
    }

    /// Whether the box stands in space: turned out of the screen's plane or
    /// moved along z. Only such a box is drawn off its frame in a way the
    /// platform's own hit test cannot see.
    var standsInSpace: Bool {
        let axis = style["rotate_axis"]?.numbers ?? [0, 0, 1]
        return (axis.count == 3 && (axis[0] != 0 || axis[1] != 0)) || number("translate_z") != 0
    }

    /// Where Core Animation draws the box, as a homography (row major, as
    /// `NodeView.map` takes it) from its bounds to its superview's
    /// coordinates: its layer's transform about its anchor point, then the
    /// holder's `sublayerTransform` (the parent's perspective) about the
    /// holder's anchor, each read from the layer as drawn. Nil where the
    /// platform's own geometry is already where the box is drawn: on iOS a
    /// box not in space (UIKit converts through a 2D transform); on macOS an
    /// untransformed box, and a child a canvas's surface places itself.
    var plane: [Double]? {
        #if os(iOS)
        let own: CALayer? = standsInSpace ? layer : nil
        #else
        // AppKit sees no layer transform at all: translate, rotate, scale, a
        // press, a layout transition's offset, and the 3D rows alike.
        let own = placement == nil && !placementHidden && !CATransform3DIsIdentity(layer?.transform ?? CATransform3DIdentity) ? layer : nil
        #endif
        // The superview's layer holds the perspective (`applyPerspective`);
        // AppKit attaches it as the superlayer only once the window draws,
        // and a superview with no layer (a root) holds none.
        guard let own, superview != nil else { return nil }
        func anchor(_ l: CALayer) -> CATransform3D {
            let b = l.bounds
            return CATransform3DMakeTranslation(b.minX + b.width * l.anchorPoint.x, b.minY + b.height * l.anchorPoint.y, 0)
        }
        var m = CATransform3DConcat(CATransform3DInvert(anchor(own)), own.transform)
        // Where the anchor sits in the superview: on macOS from the frame,
        // AppKit's own geometry, since the layer's position is in its
        // superlayer's space, which is not the superview's until a layerless
        // superview's subtree is attached and flipped.
        #if os(iOS)
        let position = own.position
        #else
        let position = CGPoint(x: frame.minX + frame.width * own.anchorPoint.x, y: frame.minY + frame.height * own.anchorPoint.y)
        #endif
        m = CATransform3DConcat(m, CATransform3DMakeTranslation(position.x, position.y, 0))
        if let holder = superview?.layer, !CATransform3DIsIdentity(holder.sublayerTransform) {
            let a = anchor(holder)
            m = CATransform3DConcat(CATransform3DConcat(CATransform3DConcat(m, CATransform3DInvert(a)), holder.sublayerTransform), a)
        }
        // A point (x, y, 0, 1) times the row-vector matrix, z dropped.
        return [m.m11, m.m21, m.m41, m.m12, m.m22, m.m42, m.m14, m.m24, m.m44].map { Double($0) }
    }

    /// A point in the superview's coordinates on the box in space, in the
    /// box's own: the ray under the point meets the box's plane. Nil when
    /// the box takes no hit there, as CSS has it: the plane is behind the
    /// viewer, or its back face is toward them and `backface-visibility`
    /// hides it (which hides its subtree with it).
    func unproject(_ p: CGPoint, plane h: [Double]) -> CGPoint? {
        hidesBack(h) ? nil : NodeView.onPlane(p, h)
    }

    /// Whether the box's back face is toward the viewer and
    /// `backface-visibility: hidden` hides it: it is not drawn, so neither
    /// it nor anything in it takes a hit.
    func hidesBack(_ h: [Double]? = nil) -> Bool {
        guard style["backface_visibility"]?.string == "hidden", let h = h ?? plane else { return false }
        // The sign of the plane's orientation where it is in front of the
        // viewer (the map's Jacobian is det(h) / w³, and w > 0 there).
        return h[0] * (h[4] * h[8] - h[5] * h[7]) - h[1] * (h[3] * h[8] - h[5] * h[6]) + h[2] * (h[3] * h[7] - h[4] * h[6]) < 0
    }

    /// The point on the plane under `p`, or nil when that is behind the viewer.
    static func onPlane(_ p: CGPoint, _ h: [Double]) -> CGPoint? {
        guard let inv = NodeView.invert(h) else { return nil }
        let q = NodeView.map(inv, p)
        guard q.x.isFinite, q.y.isFinite, h[6] * q.x + h[7] * q.y + h[8] > 0 else { return nil }
        return q
    }

    /// The 3D rows (LLP 1077 D8) go on again when any of them changed.
    func applySpace(changedFrom old: NodeStyle) {
        let rows = ["rotate_axis", "translate_z", "perspective", "perspective_origin", "backface_visibility"]
        if rows.contains(where: { style[$0] != old[$0] }) { applyTransform(); applyPerspective() }
    }

    /// `perspective` on the box that holds the children, about
    /// `perspective-origin`, and `backface-visibility` on the box itself.
    func applyPerspective() {
        #if os(iOS)
        let own: CALayer? = layer
        let holder: CALayer? = container.layer
        #else
        let own = layer
        let holder = container.layer
        #endif
        let hidden = style["backface_visibility"]?.string == "hidden"
        if own?.isDoubleSided == hidden { own?.isDoubleSided = !hidden }
        guard let holder else { return }
        let d = number("perspective")
        guard d > 0 else {
            // Its own children's holder, as the set below writes (never the
            // parent's, whose perspective is the parent's own).
            if !CATransform3DIsIdentity(holder.sublayerTransform) { holder.sublayerTransform = CATransform3DIdentity }
            return
        }
        // The vanishing point, from the holder's anchor.
        let axes = style["perspective_origin"]?.array ?? []
        func axis(_ i: Int, _ size: CGFloat) -> CGFloat {
            guard axes.count == 2 else { return size / 2 }
            if let points = axes[i].number { return points }
            if case .object(let o) = axes[i], let pct = o["pct"]?.number { return size * pct / 100 }
            return size / 2
        }
        let b = holder.bounds
        let anchor = CGPoint(x: b.minX + b.width * holder.anchorPoint.x, y: b.minY + b.height * holder.anchorPoint.y)
        let o = CGPoint(x: axis(0, bounds.width) - anchor.x, y: axis(1, bounds.height) - anchor.y)
        var p = CATransform3DIdentity
        p.m34 = -1 / d
        let t = CATransform3DConcat(CATransform3DConcat(CATransform3DMakeTranslation(-o.x, -o.y, 0), p), CATransform3DMakeTranslation(o.x, o.y, 0))
        if !CATransform3DEqualToTransform(holder.sublayerTransform, t) { holder.sublayerTransform = t }
    }
}

#if os(macOS)
// AppKit's geometry knows nothing of a layer's transform: its hit test and
// its conversions place every box at its frame. A transformed box — moved,
// turned, scaled, or in space — is drawn elsewhere, so these carry a point
// through each such box's plane, as the web's hit test does (LLP 1077 D8;
// Linux maps through the same plane).
extension NodeView {
    /// A hit-test point (in the superview's coordinates, AppKit's
    /// convention) moved to where AppKit's flat geometry finds what is drawn
    /// under it, when this box is transformed; nil when it takes no hit there.
    func spaceHit(_ point: NSPoint) -> NSPoint? {
        guard let h = plane, let sup = superview else { return point }
        return unproject(point, plane: h).map { convert($0, to: sup) }
    }

    /// `p` in `top`'s coordinates (the window's when nil) in this node's:
    /// AppKit's conversion, through the plane of each transformed box on the way.
    func descend(_ p: NSPoint, from top: NSView? = nil) -> NSPoint {
        var turned: [NodeView] = []
        var v: NSView? = self
        while let n = v, n !== top {
            if let node = n as? NodeView, node.plane != nil { turned.append(node) }
            v = n.superview
        }
        guard !turned.isEmpty else { return convert(p, from: top) }
        var p = p, at = top
        for node in turned.reversed() {
            guard let sup = node.superview, let h = node.plane else { continue }
            // Behind the viewer, the point is on no part of the box.
            p = NodeView.onPlane(sup.convert(p, from: at), h) ?? NSPoint(x: -1e9, y: -1e9)
            at = node
        }
        return convert(p, from: at)
    }

    /// `p` in this node's coordinates in `top`'s (the window's when nil):
    /// `descend` the other way.
    func ascend(_ p: NSPoint, to top: NSView?) -> NSPoint {
        var p = p, v: NSView = self
        while v !== top, let sup = v.superview {
            p = (v as? NodeView)?.plane.map { NodeView.map($0, p) } ?? v.convert(p, to: sup)
            v = sup
        }
        return v === top ? p : top?.convert(p, from: v) ?? v.convert(p, to: nil)
    }

    /// Whether this node or one above it is drawn off its frame.
    var drawnOffFrame: Bool {
        var v: NSView? = self
        while let n = v { if (n as? NodeView)?.plane != nil { return true }; v = n.superview }
        return false
    }
}
#endif

#if os(iOS)
typealias SpaceView = UIView
#else
typealias SpaceView = NSView
#endif

extension NodeView {
    /// `p` in this node's coordinates where it is drawn in `top`'s (the
    /// window's when nil): through each transformed box (UIKit's conversion
    /// carries them; AppKit's does not, so `ascend`), and through the
    /// placement of a child a canvas's surface placed (LLP 1014 D5), the
    /// canvas then carried out the same way — `local` the other way.
    func drawnPoint(_ p: CGPoint, in top: SpaceView? = nil) -> CGPoint {
        func out(_ v: NodeView, _ p: CGPoint, _ top: SpaceView?) -> CGPoint {
            #if os(iOS)
            return v.convert(p, to: top)
            #else
            return v.ascend(p, to: top)
            #endif
        }
        guard let placed = placedAncestor, let h = placed.placement, let canvas = placed.superview?.superview as? NodeView else {
            return out(self, p, top)
        }
        return canvas.drawnPoint(NodeView.map(h, out(self, p, placed)), in: top)
    }

    /// The bounds in `top`'s coordinates of `r` as drawn (`drawnPoint`).
    func drawnRect(_ r: CGRect, in top: SpaceView? = nil) -> CGRect {
        let corners = [CGPoint(x: r.minX, y: r.minY), CGPoint(x: r.maxX, y: r.minY), CGPoint(x: r.maxX, y: r.maxY), CGPoint(x: r.minX, y: r.maxY)]
            .map { drawnPoint($0, in: top) }
        let xs = corners.map { $0.x }, ys = corners.map { $0.y }
        return CGRect(x: xs.min()!, y: ys.min()!, width: xs.max()! - xs.min()!, height: ys.max()! - ys.min()!)
    }
}

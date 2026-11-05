// A scaled iframe on macOS. AppKit's geometry knows nothing of a layer's
// transform: the host carries its own hit test through a transformed box's
// plane (`SpaceTransform.swift`), but a web view converts each event's
// window point itself, by AppKit's geometry. Under a layer that scales it,
// the page would hear every press, hover and wheel where the unscaled box
// would be, where Chromium delivers them where the frame is drawn. So an
// iframe's 2D scale goes into view geometry instead: its web view lies in a
// holder whose bounds are the box and whose frame is the box scaled about
// `transform-origin`, and AppKit scales the page's drawing and its events
// alike. The node's layer draws the rest of the transform; the iframe's own
// box (a background or border on the element) is drawn unscaled.
#if os(macOS)
import AppKit

/// The web view's holder in a scaled iframe's node.
final class FrameScaleHolder: NSView {
    override var isFlipped: Bool { true }
}

extension NodeView {
    /// The part of the node's scale its web view takes by geometry: all of
    /// it for an iframe turned in no direction and not flying, else none.
    var frameGeometryScale: CGFloat {
        guard web != nil, rotate == 0, flightLook == nil, !standsInSpace, scale > 0 else { return 1 }
        return scale
    }

    /// The web view placed in the box: in a holder once the frame has been
    /// scaled (kept at scale 1 after, so the page is not moved between views
    /// again), else filling the box.
    func placeWeb() {
        guard let web else {
            for case let holder as FrameScaleHolder in subviews { holder.removeFromSuperview() }
            return
        }
        let s = frameGeometryScale
        var holder = web.superview as? FrameScaleHolder
        if holder == nil, s != 1 {
            // An empty holder stays from a web view a new partition replaced.
            let made = subviews.lazy.compactMap { $0 as? FrameScaleHolder }.first ?? FrameScaleHolder(frame: bounds)
            made.wantsLayer = true
            if made.superview !== self { addSubview(made, positioned: .below, relativeTo: web) }
            made.addSubview(web)
            holder = made
        }
        guard let holder else { web.frame = bounds; return }
        let o = transformOriginPoint
        holder.frame = NSRect(x: o.x - (o.x - bounds.minX) * s, y: o.y - (o.y - bounds.minY) * s,
                              width: bounds.width * s, height: bounds.height * s)
        holder.bounds = NSRect(origin: .zero, size: bounds.size)
        web.frame = holder.bounds
    }
}
#endif

// Boxes on the device's pixel grid on the Mac, as `IOS/PixelGridIOS.swift`
// draws them on iOS and as a browser paints them. Layout's frames are
// fractional (LLP 1001 §6). Drawn where they fall, a view's layer sits a
// fraction of a device pixel off the grid and Core Animation blends it, its
// edges and its text, across two device rows. Chromium rounds every painted
// rectangle's edges to device pixels at their absolute positions
// (`PhysicalRect::PixelSnappedIntRect`, each edge rounded half up by
// `LayoutUnit::Round`), so edges that meet in layout meet when drawn. Here
// too: each box keeps its absolute layout origin and its absolute drawn
// origin in whole device pixels, so the rounding is exact however deep the
// tree. An absolute edge is taken to Chromium's layout unit (1/64 of a CSS
// px) before it is rounded. A device pixel is the backing scale times the
// page zoom (`ZoomMac.swift`: 2.5 at 125% on a 2x display), so the scale
// here is not always whole, as iOS's is. A container AppKit places (the
// page, a dialog's or a popover's place, a table's cell) is an origin on
// the grid.
#if os(macOS)
import AppKit

/// An absolute origin: as layout placed it, in points of the page (CSS px),
/// and drawn, in device pixels of `scale`, so one drawn at another scale is
/// another origin.
package struct GridOrigin: Equatable {
    package var layoutX: CGFloat = 0, layoutY: CGFloat = 0
    package var pixelX = 0, pixelY = 0
    /// Device pixels a point; 0 before any.
    package var scale: CGFloat = 0
    package static let zero = GridOrigin()
}

package struct GridPlace: Equatable {
    /// The frame layout sent, in the parent's coordinates.
    package var layout: CGRect
    /// The frame drawn, in the parent's drawn coordinates.
    package var drawn: CGRect
    /// The box's own absolute origin, which its children are placed from.
    package var origin: GridOrigin
}

package enum PixelGrid {
    /// Layout units in `v` points, as Chromium's `LayoutUnit` takes a length.
    static func units(_ v: CGFloat) -> Int {
        let u = (v * 64).rounded()
        return u.isFinite ? Int(max(-1e15, min(1e15, u))) : 0
    }
    /// The device pixel `u` layout units round to, a half up. Exact at a
    /// whole scale: `u × scale / 64` is then a binary fraction.
    static func pixel(_ u: Int, scale: CGFloat) -> Int {
        let p = (CGFloat(u) * scale / 64 + 0.5).rounded(.down)
        return p.isFinite ? Int(max(-1e15, min(1e15, p))) : 0
    }
    /// The device pixel an absolute position of `v` points rounds to.
    static func pixel(_ v: CGFloat, scale: CGFloat) -> Int { pixel(units(v), scale: scale) }
    /// Device pixels a point; whole points for none.
    static func scale(_ s: CGFloat) -> CGFloat { s.isFinite && s > 0 ? s : 1 }

    /// `rect`, laid out in a parent whose absolute origin is `parent`, with
    /// each edge on its nearest device pixel.
    package static func place(_ rect: CGRect, in parent: GridOrigin, scale s: CGFloat) -> GridPlace {
        let k = scale(s)
        let x = parent.layoutX + rect.minX, y = parent.layoutY + rect.minY
        let px = pixel(x, scale: k), py = pixel(y, scale: k)
        let px1 = pixel(x + rect.width, scale: k), py1 = pixel(y + rect.height, scale: k)
        let drawn = CGRect(x: CGFloat(px - parent.pixelX) / k, y: CGFloat(py - parent.pixelY) / k,
                           width: CGFloat(px1 - px) / k, height: CGFloat(py1 - py) / k)
        return GridPlace(layout: rect, drawn: drawn, origin: GridOrigin(layoutX: x, layoutY: y, pixelX: px, pixelY: py, scale: k))
    }

    /// How far a box placed at `place` moves, drawn, for a layout
    /// displacement of `by` (a sticky box's): the difference of its rounded
    /// origins, so the moved box is on the grid too.
    package static func shift(_ place: GridPlace, by: CGPoint, scale s: CGFloat) -> CGPoint {
        let o = place.origin, k = o.scale > 0 ? o.scale : scale(s)
        return CGPoint(x: CGFloat(pixel(o.layoutX + by.x, scale: k) - o.pixelX) / k,
                       y: CGFloat(pixel(o.layoutY + by.y, scale: k) - o.pixelY) / k)
    }

    /// A content `length` from a box placed at `origin`, ending where its
    /// last edge is drawn: a scroller's extent, so its last pixel row is
    /// reachable.
    package static func length(_ length: CGFloat, from origin: GridOrigin, vertical: Bool, scale s: CGFloat) -> CGFloat {
        let k = origin.scale > 0 ? origin.scale : scale(s) // the pixels `origin` counts
        let end = vertical ? pixel(origin.layoutY + length, scale: k) - origin.pixelY : pixel(origin.layoutX + length, scale: k) - origin.pixelX
        return CGFloat(end) / k
    }

    /// `v` points on the nearest device pixel, a half up.
    package static func round(_ v: CGFloat, scale s: CGFloat) -> CGFloat {
        let k = scale(s)
        return CGFloat(pixel(v, scale: k)) / k
    }
}

extension Presenter {
    /// Device pixels a point of the page: the window's backing scale times
    /// the page zoom (`ExactSession.devicePixelScale`, which text line boxes
    /// round to too). Without a session, the page view's own.
    package var gridScale: CGFloat { session?.devicePixelScale ?? root.pixelScale ?? 1 }
    /// The scale to place a box under `parent` at: the parent's own, so a
    /// subtree stays on one grid until the scale pass reaches it; else the page's.
    func gridScale(under parent: GridOrigin) -> CGFloat { parent.scale > 0 ? parent.scale : gridScale }

    /// The absolute origin `view` is placed from: that of the node whose
    /// children it is among, else a container on the grid.
    func gridOrigin(above view: NSView) -> GridOrigin {
        guard let holder = view.superview else { return .zero }
        var ancestor: NSView? = holder
        while let a = ancestor, !(a is NodeView) { ancestor = a.superview }
        guard let node = ancestor as? NodeView, node.container === holder else { return .zero }
        return node.grid?.origin ?? .zero
    }

    /// Where `v` is drawn for a layout frame `rect`. A box a host places
    /// itself (a dialog in the top layer, a popover, a table's row in its
    /// cell) is an origin of its own, its children placed from its corner.
    func gridPlace(_ v: NodeView, _ rect: CGRect) -> GridPlace {
        if hostPlaces(v) { return GridPlace(layout: rect, drawn: rect, origin: GridOrigin(scale: PixelGrid.scale(v.superview?.pixelScale ?? gridScale))) }
        // A flying box is placed from its slot, where it lands.
        let at: NSView = flights[v.id].flatMap { $0.view === v ? $0.slot : nil } ?? v
        let origin = gridOrigin(above: at)
        return PixelGrid.place(rect, in: origin, scale: gridScale(under: origin))
    }
    private func hostPlaces(_ v: NodeView) -> Bool {
        dialogs.owns(v) || menus.owns(v) || v.superview is TableRowCell
    }

    /// A frame layout sent for `v`, drawn on the grid; a box whose origin
    /// moves moves its children's edges with it.
    func applyFrame(_ v: NodeView, _ rect: CGRect) {
        let place = gridPlace(v, rect)
        let moved = place.origin != v.grid?.origin
        v.grid = place
        defer { if moved { if isApplying { gridMoved.append(v) } else { regridChildren(of: v) } } }
        let frame = v.frameInScrollDocument(place.drawn)
        if flightFrame(v.id, frame) { return }
        // A host that places the box is given layout's frame, as before the grid.
        let laid = v.frameInScrollDocument(rect)
        if !dialogs.frame(v, laid) && !menus.frame(v, laid) && !tables.frame(v, laid) { v.frame = frame }
        v.arrangeShift = .zero
        v.textRasterGeometryChanged()
        v.scroll?.frame = v.bounds
        v.layoutField()
        v.layoutTextArea()
        v.metal?.frame = v.bounds
        v.overlay?.frame = v.bounds
        v.placeWeb()
        v.applyShadow()
        v.fitScroll()
        v.applyTransform()
        if v.surface != nil { v.applySurface() } // its size is the laid-out one plus `layoutResize`
        v.layoutContent()
        stickies.regridded(v.id)
    }

    /// `parent`'s children drawn again from its origin: those whose drawn
    /// frame changes, and theirs in turn.
    func regridChildren(of parent: NodeView) {
        for case let child as NodeView in parent.container.subviews {
            guard let old = child.grid, gridPlace(child, old.layout) != old else { continue }
            applyFrame(child, old.layout)
        }
    }

    /// The children of the boxes a batch moved, drawn again once its ops
    /// are applied, shallowest first: a child's own frame op may have placed
    /// it before its parent's, and each box is then placed from its parent's
    /// final origin once, not once for each ancestor that moved.
    func regridMoved() {
        func depth(_ v: NSView) -> Int { var d = 0, s = v.superview; while let x = s { d += 1; s = x.superview }; return d }
        while !gridMoved.isEmpty {
            var seen = Set<ObjectIdentifier>()
            let round = gridMoved.filter { seen.insert(ObjectIdentifier($0)).inserted }.map { ($0, depth($0)) }.sorted { $0.1 < $1.1 }
            gridMoved = []
            for (v, _) in round where v.grid != nil { regridChildren(of: v) }
        }
        regridSlots()
    }

    /// Each flight's slot, where its view lands, from its parent's final
    /// origin: the slot is no node, so no parent's pass reaches it.
    func regridSlots() {
        for f in flights.values {
            guard let view = f.view, let slot = f.slot, slot.superview != nil, let layout = view.grid?.layout else { continue }
            let origin = gridOrigin(above: slot)
            let place = PixelGrid.place(layout, in: origin, scale: gridScale(under: origin))
            view.grid = place
            _ = flightFrame(view.id, view.frameInScrollDocument(place.drawn))
        }
    }

    /// Every box drawn again on a new grid (another display's backing scale,
    /// another page zoom), parents first; a flying view is placed from its
    /// slot, and a scroller's extent follows its frame (`fitScroll`).
    func regridAll() {
        func depth(_ v: NSView) -> Int { var d = 0, s = v.superview; while let x = s { d += 1; s = x.superview }; return d }
        for (v, _) in views.values.map({ ($0, depth($0)) }).sorted(by: { $0.1 < $1.1 }) {
            if let layout = v.grid?.layout { applyFrame(v, layout) }
        }
        regridMoved()
    }

    /// `view`, newly among its parent's children (or the roots), drawn from there.
    func regrid(_ view: NodeView) {
        guard let old = view.grid, gridPlace(view, old.layout) != old else { return }
        applyFrame(view, old.layout)
    }
}
extension NodeView {
    /// How far below its drawn top layout put this box: the part of its
    /// absolute position the grid rounded away, under half a device pixel.
    /// Its paragraph is placed from layout's top, and each baseline rounds
    /// at its place on the page (`TextEngine.paintedLine`), as Chromium
    /// rounds it, not at its place in a box the grid moved.
    var gridDrop: CGFloat {
        guard let o = grid?.origin, o.scale > 0 else { return 0 }
        return o.layoutY - CGFloat(o.pixelY) / o.scale
    }
}
#endif

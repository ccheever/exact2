// Boxes on the device's pixel grid, as a browser paints them. Layout's frames
// are fractional (LLP 1001 §6). Drawn where they fall, UIKit blends a text
// raster or an edge across two device pixels: text blurs, and a 0.5 pt line
// covers two rows. A browser rounds every painted rectangle to device pixels
// at its absolute position (Chromium's `PhysicalRect::PixelSnappedIntRect`,
// each edge rounded half up by `LayoutUnit::Round`), so edges that meet in
// layout meet when drawn, and a box's size is the distance between its
// rounded edges. Here too: each box keeps its absolute layout origin and
// its absolute drawn origin in whole device pixels, so the rounding is exact
// however deep the tree. An absolute edge is taken to Chromium's layout unit
// (1/64 of a CSS pixel, a point here) before it is rounded, so a half that
// floating point left a hair short is a half; it is the absolute position
// that is taken there, once, so two boxes' edges that meet in layout are one
// value when rounded. A container UIKit places (a cell, the window) is an
// origin on the grid.
#if os(iOS) || os(tvOS)
import UIKit

/// An absolute origin: as layout placed it, in points, and drawn, in device
/// pixels of `scale`, so one drawn for another display is another origin.
package struct GridOrigin: Equatable {
    package var layoutX: CGFloat = 0, layoutY: CGFloat = 0
    package var pixelX = 0, pixelY = 0, scale = 0
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
    /// The device pixel `u` layout units round to, a half up.
    static func pixel(_ u: Int, scale: Int) -> Int {
        let n = 2 * u * scale + 64
        return n >= 0 ? n / 128 : -((-n + 127) / 128)
    }
    /// The device pixel an absolute position of `v` points rounds to.
    static func pixel(_ v: CGFloat, scale: Int) -> Int { pixel(units(v), scale: scale) }
    /// Device pixels a point, as a whole number (every iOS and tvOS display's is).
    static func scale(_ s: CGFloat) -> Int { max(1, Int(s.rounded())) }

    /// `rect`, laid out in a parent whose absolute origin is `parent`, with
    /// each edge on its nearest device pixel.
    package static func place(_ rect: CGRect, in parent: GridOrigin, scale s: CGFloat) -> GridPlace {
        let scale = scale(s), k = CGFloat(scale)
        let x = parent.layoutX + rect.minX, y = parent.layoutY + rect.minY
        let px = pixel(x, scale: scale), py = pixel(y, scale: scale)
        let px1 = pixel(x + rect.width, scale: scale), py1 = pixel(y + rect.height, scale: scale)
        let drawn = CGRect(x: CGFloat(px - parent.pixelX) / k, y: CGFloat(py - parent.pixelY) / k,
                           width: CGFloat(px1 - px) / k, height: CGFloat(py1 - py) / k)
        return GridPlace(layout: rect, drawn: drawn, origin: GridOrigin(layoutX: x, layoutY: y, pixelX: px, pixelY: py, scale: scale))
    }

    /// How far a box placed at `place` moves, drawn, for a layout
    /// displacement of `by` (a sticky box's): the difference of its rounded
    /// origins, so the moved box is on the grid too.
    package static func shift(_ place: GridPlace, by: CGPoint, scale s: CGFloat) -> CGPoint {
        let o = place.origin, scale = o.scale > 0 ? o.scale : scale(s), k = CGFloat(scale)
        return CGPoint(x: CGFloat(pixel(o.layoutX + by.x, scale: scale) - o.pixelX) / k,
                       y: CGFloat(pixel(o.layoutY + by.y, scale: scale) - o.pixelY) / k)
    }

    /// A content `length` from a box placed at `origin`, ending where its
    /// last edge is drawn: a scroller's extent, so its last pixel row is
    /// reachable.
    package static func length(_ length: CGFloat, from origin: GridOrigin, vertical: Bool, scale s: CGFloat) -> CGFloat {
        let scale = origin.scale > 0 ? origin.scale : scale(s) // the pixels `origin` counts
        let end = vertical ? pixel(origin.layoutY + length, scale: scale) - origin.pixelY : pixel(origin.layoutX + length, scale: scale) - origin.pixelX
        return CGFloat(end) / CGFloat(scale)
    }

    /// `v` on the nearest device pixel inside `low...high`, both ends taken
    /// onto the grid inward: a scroll offset UIKit will not clamp off it.
    package static func clamp(_ v: CGFloat, _ low: CGFloat, _ high: CGFloat, scale s: CGFloat) -> CGFloat {
        let k = CGFloat(scale(s))
        // Native inset sums can miss an integral pixel by a few ULP.
        func pixels(_ value: CGFloat) -> CGFloat {
            let value = value * k, nearest = value.rounded()
            return abs(value - nearest) <= max(1e-9, value.ulp * 8) ? nearest : value
        }
        let lo = pixels(low).rounded(.up) / k, hi = max(lo, pixels(high).rounded(.down) / k)
        return min(max(round(v, scale: s), lo), hi)
    }

    /// `v` points on the nearest device pixel, a half up.
    package static func round(_ v: CGFloat, scale s: CGFloat) -> CGFloat {
        let scale = scale(s)
        return CGFloat(pixel(v, scale: scale)) / CGFloat(scale)
    }

}

extension PixelGrid {
    /// Programmatic offsets use the scroller's display. Unclamped writes
    /// preserve a rubber band past the native range; in-range targets snap inward.
    package static func offset(_ point: CGPoint, in scroll: UIScrollView, clamped: Bool = true) -> CGPoint {
        let s = max(1, scroll.traitCollection.displayScale)
        let i = scroll.adjustedContentInset
        func snap(_ value: CGFloat, low: CGFloat, high: CGFloat) -> CGFloat {
            if !clamped, value < low || value > max(low, high) { return round(value, scale: s) }
            return clamp(value, low, high, scale: s)
        }
        return CGPoint(x: snap(point.x, low: -i.left, high: scroll.contentSize.width + i.right - scroll.bounds.width),
                       y: snap(point.y, low: -i.top, high: scroll.contentSize.height + i.bottom - scroll.bounds.height))
    }
    /// How far, in device pixels, an offset UIKit hands back may sit from the
    /// value it was given and still be that value: it returns one a ULP or so
    /// off (5/3 at scale 3 came back 1.6666666666666665). An exact compare
    /// took that for the port moving: a smooth correction restarted every
    /// frame and never advanced (SmoothCollectionIOSTests). A real move (a
    /// clamp, a drag, a reader's scroll) is a pixel or more.
    package static let readBackSlop: CGFloat = 0.01
    /// `a` and `b` the same offset of `scroll`'s, within `readBackSlop` of its pixels.
    package static func same(_ a: CGFloat, _ b: CGFloat, in scroll: UIScrollView) -> Bool {
        abs(a - b) <= readBackSlop / max(1, scroll.traitCollection.displayScale)
    }
    package static func same(_ a: CGPoint, _ b: CGPoint, in scroll: UIScrollView) -> Bool {
        same(a.x, b.x, in: scroll) && same(a.y, b.y, in: scroll)
    }
}

extension Presenter {
    /// Device pixels a point, of the display the page is drawn on. Before
    /// the page has one, whole points, which are on every display's grid.
    package var gridScale: CGFloat { max(1, root.traitCollection.displayScale) }
    /// The scale to place a box under `parent` at: the parent's own, so a
    /// flying subtree the display-scale pass left stays on one grid until it
    /// lands; else the display's.
    func gridScale(under parent: GridOrigin) -> CGFloat { parent.scale > 0 ? CGFloat(parent.scale) : gridScale }

    /// The absolute origin `view` is placed from: that of the node whose
    /// children it is among (for a row a grouped list carried into its cell,
    /// the node it was carried from), else a container on the grid (a cell,
    /// the window).
    func gridOrigin(above view: UIView) -> GridOrigin {
        func origin(_ holder: UIView) -> GridOrigin? {
            var ancestor: UIView? = holder
            while let a = ancestor, !(a is NodeView) { ancestor = a.superview }
            guard let node = ancestor as? NodeView, node.container === holder else { return nil }
            return node.grid?.origin ?? .zero
        }
        guard let holder = view.superview else { return .zero }
        if let found = origin(holder) { return found }
        guard groupedLists?.carriesRows == true, let home = groupedLists?.logicalParent(of: view) else { return .zero }
        return origin(home) ?? .zero
    }

    /// A frame layout sent for `v`, drawn on the grid; a box whose origin
    /// moves moves its children's edges with it. Inside an animation UIKit
    /// has open, it moves only if the last frame showed it (`moveOrPlace`).
    func applyFrame(_ v: NodeView, _ rect: CGRect) { moveOrPlace(v) { frame(v, rect) } }
    private func frame(_ v: NodeView, _ rect: CGRect) {
        let origin = gridOrigin(above: v)
        let place = PixelGrid.place(rect, in: origin, scale: gridScale(under: origin))
        let moved = place.origin != v.grid?.origin
        v.grid = place
        // A frame is set untransformed (UIKit's `frame` is undefined
        // under a transform); the presentation goes back on after.
        v.transform = .identity
        v.frame = groupedLists?.carriesRows == true ? groupedLists?.cellFrame(of: v, at: place) ?? place.drawn : place.drawn
        v.textRasterGeometryChanged()
        v.scroll?.frame = v.scrollFrame
        if v.fieldWaits, rect.width > 0 || rect.height > 0 { v.makeWaitingField() } // its box laid out (MenuHost.waitingFields)
        v.layoutField()
        v.layoutTextArea()
        v.metal?.frame = v.bounds
        v.overlay?.frame = v.bounds
        v.web?.frame = v.bounds
        v.fitScroll()
        v.applyTransform()
        if v.surface != nil { v.applySurface() } // its size is the laid-out one plus `layoutResize`
        menus.framed(v)
        stickies.regridded(v.id)
        if moved { if applying { gridMoved.append(v) } else { regridChildren(of: v) } }
    }

    /// `parent`'s children drawn again from its origin: those whose drawn
    /// frame changes, and theirs in turn.
    func regridChildren(of parent: NodeView) {
        let origin = parent.grid?.origin ?? .zero
        func regrid(_ child: NodeView) {
            guard let old = child.grid else { return }
            let place = PixelGrid.place(old.layout, in: origin, scale: gridScale(under: origin))
            guard place != old else { return }
            // Drawn where it was in its parent, which moved by whole pixels (a row below one inserted above it):
            // only the absolute origin its own children are placed from changes, and its box keeps its frame.
            if place.drawn == old.drawn {
                child.grid = place
                moveOrPlace(child) { child.placeTextRaster() } // its text's box is placed from its absolute origin
                menus.framed(child); stickies.regridded(child.id)
                regridChildren(of: child)
                return
            }
            applyFrame(child, old.layout)
        }
        for case let child as NodeView in parent.container.subviews { regrid(child) }
        // A grouped list's carried rows are its children too, in their cells.
        if groupedLists?.carriesRows == true { groupedLists?.carried(from: parent.container).forEach(regrid) }
        flats.regrid(parent: parent)
    }

    /// The children of the boxes a batch moved, drawn again before its flat
    /// leaves are flushed, shallowest first: a child's own frame op may have
    /// placed it before its parent's, and each box is then placed from its
    /// parent's final origin once, not once for each ancestor that moved.
    func regridMoved() {
        func depth(_ v: UIView) -> Int { var d = 0, s = v.superview; while let x = s { d += 1; s = x.superview }; return d }
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
            guard let slot = f.slot, slot.superview != nil else { continue }
            let rect = f.geometry.map { CGRect(x: $0.x, y: $0.y, width: $0.w, height: $0.h) } ?? f.view?.grid?.layout
            let origin = gridOrigin(above: slot)
            if let rect { slot.frame = PixelGrid.place(rect, in: origin, scale: gridScale(under: origin)).drawn }
        }
    }

    /// Every box drawn again on a new display's grid, parents first, each
    /// from its authored parent (carried rows go home for it, as before a
    /// batch); a flying view is left to its flight, which lands it. Then
    /// the flat leaves, and each scroller's end and offset.
    func regridAll() {
        groupedLists?.prepare()
        func depth(_ v: UIView) -> Int { var d = 0, s = v.superview; while let x = s { d += 1; s = x.superview }; return d }
        // A flying view's subtree is placed from it, which its landing places.
        func flying(_ v: UIView) -> Bool {
            var at: UIView? = v
            while let a = at { if let n = a as? NodeView, isFlying(n) { return true }; at = a.superview }
            return false
        }
        for (v, _) in views.values.map({ ($0, depth($0)) }).sorted(by: { $0.1 < $1.1 }) where !flying(v) {
            if let layout = v.grid?.layout { applyFrame(v, layout) }
        }
        flats.regridAll { !flying($0) }
        regridSlots()
        let s = gridScale
        for id in scrollers {
            guard let v = views[id], let sv = v.scrollView, !flying(v) else { continue }
            v.fitScroll()
            let i = sv.adjustedContentInset
            let offset = CGPoint(x: PixelGrid.clamp(sv.contentOffset.x, -i.left, sv.contentSize.width + i.right - sv.bounds.width, scale: s),
                                 y: PixelGrid.clamp(sv.contentOffset.y, -i.top, sv.contentSize.height + i.bottom - sv.bounds.height, scale: s))
            if sv.contentOffset != offset { sv.contentOffset = offset }
        }
        flats.flush()
        requestProjectionSync()
    }

    /// `view`, newly among its parent's children (or the roots), drawn from there.
    func regrid(_ view: NodeView) {
        let origin = gridOrigin(above: view)
        guard let old = view.grid, PixelGrid.place(old.layout, in: origin, scale: gridScale(under: origin)) != old else { return }
        applyFrame(view, old.layout)
    }
}
#endif

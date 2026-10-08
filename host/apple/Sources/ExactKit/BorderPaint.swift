// A box's border as the web paints it, on both Apple platforms (LLP 1053
// G2): each side in its own colour, over the area between the border box's
// curve and the padding box's, with the corners joined where Chrome joins
// them. The iOS layer keeps the uniform case (`applyBoxLayer`); everything
// else — sides that differ in colour or width, over square or rounded
// corners — draws here. Coordinates are the view's own, y down.
//
// The join: each side owns the quadrilateral from its two outer corners to
// its two inner corners, so the colour boundary at a corner is the line from
// the border box's corner to the padding box's — at an angle set by the two
// adjacent widths (CSS Backgrounds 3 §5.5; Chromium's
// `BoxBorderPainter::ClipBorderSidePolygon`). Where the padding box's corner
// is itself rounded, the line runs on to the chord of that inner curve, so
// the quadrilateral covers all of the corner's border. Sides that share a
// colour are one clip, so no seam shows where they meet.
import CoreGraphics
import QuartzCore

package enum BorderPaint {
    /// Percentages use the border box's width and height independently.
    static func radii(_ style: NodeStyle, in rect: CGRect, inset: CGFloat = 0) -> [CGSize] {
        ["top_left", "top_right", "bottom_right", "bottom_left"].map { name in
            let value = style["border_radius_" + name] ?? style["border_radius"]
            func length(_ basis: CGFloat) -> CGFloat {
                if case .object(let d) = value {
                    return max(0, basis * CGFloat(d["pct"]?.number ?? 0) / 100 + CGFloat(d["px"]?.number ?? 0) - inset)
                }
                return max(0, CGFloat(value?.number ?? 0) - inset)
            }
            return CGSize(width: length(rect.width + 2 * inset), height: length(rect.height + 2 * inset))
        }
    }

    /// A private material/media layer owns its mask. Keep the circular fast
    /// path; percentages on non-square boxes need the resolved elliptical path.
    @discardableResult
    static func clip(_ layer: CALayer, in rect: CGRect, radii: [CGSize]) -> CGFloat {
        let corners = reduced(radii, in: rect)
        let first = corners[0]
        let circular = first.width == first.height && corners.allSatisfy { $0 == first }
        let radius = circular ? first.width : 0
        layer.cornerRadius = radius
        layer.masksToBounds = true
        layer.mask = circular ? nil : ClipPath.mask(roundedRect(rect, corners))
        return radius
    }

    /// The outline `clip` masks with: nil when the radius says it.
    static func uncircular(in rect: CGRect, radii: [CGSize]) -> CGPath? {
        let corners = reduced(radii, in: rect)
        let first = corners[0]
        return first.width == first.height && corners.allSatisfy({ $0 == first }) ? nil : roundedRect(rect, corners)
    }

    /// A replaced element's content edge: reduce at the border edge first,
    /// then remove each adjacent border/padding inset from that corner.
    static func contentRadii(_ radii: [CGSize], outer: CGRect, inner: CGRect) -> [CGSize] {
        let r = reduced(radii, in: outer)
        let left = inner.minX - outer.minX, right = outer.maxX - inner.maxX
        let top = inner.minY - outer.minY, bottom = outer.maxY - inner.maxY
        return zip(r, [CGSize(width: left, height: top), CGSize(width: right, height: top),
                       CGSize(width: right, height: bottom), CGSize(width: left, height: bottom)])
            .map { CGSize(width: max(0, $0.width - $1.width), height: max(0, $0.height - $1.height)) }
    }

    /// Circle-to-cubic control distance for a quarter arc.
    private static let kappa: CGFloat = 0.5522847498

    /// CSS's radius reduction: every corner scaled by the one factor that
    /// keeps two neighbours from overlapping an edge. `radii` are top-left,
    /// top-right, bottom-right, bottom-left, as (horizontal, vertical).
    package static func reduced(_ radii: [CGSize], in rect: CGRect) -> [CGSize] {
        let sums = [radii[0].width + radii[1].width, radii[3].width + radii[2].width,
                    radii[0].height + radii[3].height, radii[1].height + radii[2].height]
        let edges = [rect.width, rect.width, rect.height, rect.height]
        var factor: CGFloat = 1
        for i in 0..<4 where sums[i] > 0 { factor = min(factor, max(0, edges[i]) / sums[i]) }
        return radii.map { CGSize(width: max(0, $0.width * factor), height: max(0, $0.height * factor)) }
    }

    /// A rectangle with an elliptical radius per corner, clockwise on screen;
    /// with a `corner-shape`, the kernel's outline (LLP 1077 D1).
    package static func roundedRect(_ r: CGRect, _ radii: [CGSize], shape: CornerShape? = nil) -> CGMutablePath {
        if let shape { return shape.outline(r, radii) }
        let p = CGMutablePath()
        let (tl, tr, br, bl) = (radii[0], radii[1], radii[2], radii[3])
        let k = kappa
        p.move(to: CGPoint(x: r.minX + tl.width, y: r.minY))
        p.addLine(to: CGPoint(x: r.maxX - tr.width, y: r.minY))
        p.addCurve(to: CGPoint(x: r.maxX, y: r.minY + tr.height),
                   control1: CGPoint(x: r.maxX - tr.width * (1 - k), y: r.minY),
                   control2: CGPoint(x: r.maxX, y: r.minY + tr.height * (1 - k)))
        p.addLine(to: CGPoint(x: r.maxX, y: r.maxY - br.height))
        p.addCurve(to: CGPoint(x: r.maxX - br.width, y: r.maxY),
                   control1: CGPoint(x: r.maxX, y: r.maxY - br.height * (1 - k)),
                   control2: CGPoint(x: r.maxX - br.width * (1 - k), y: r.maxY))
        p.addLine(to: CGPoint(x: r.minX + bl.width, y: r.maxY))
        p.addCurve(to: CGPoint(x: r.minX, y: r.maxY - bl.height),
                   control1: CGPoint(x: r.minX + bl.width * (1 - k), y: r.maxY),
                   control2: CGPoint(x: r.minX, y: r.maxY - bl.height * (1 - k)))
        p.addLine(to: CGPoint(x: r.minX, y: r.minY + tl.height))
        p.addCurve(to: CGPoint(x: r.minX + tl.width, y: r.minY),
                   control1: CGPoint(x: r.minX, y: r.minY + tl.height * (1 - k)),
                   control2: CGPoint(x: r.minX + tl.width * (1 - k), y: r.minY))
        p.closeSubpath()
        return p
    }

    /// Paint `box`'s border. `widths` and `colors` are top, right, bottom,
    /// left; `radii` the authored corner radii (top-left, top-right,
    /// bottom-right, bottom-left), reduced here as CSS reduces them.
    static func paint(_ ctx: CGContext, box: CGRect, widths: [CGFloat], colors: [CGColor], radii: [CGSize], shape: CornerShape? = nil) {
        let w = widths.map { max(0, $0) }
        guard w.contains(where: { $0 > 0 }), box.width > 0, box.height > 0 else { return }
        let outer = reduced(radii, in: box)
        let inner = CGRect(x: box.minX + w[3], y: box.minY + w[0],
                           width: max(0, box.width - w[3] - w[1]), height: max(0, box.height - w[0] - w[2]))
        // The padding box's radii: each outer radius less the adjacent widths.
        let innerRadii = reduced([
            CGSize(width: outer[0].width - w[3], height: outer[0].height - w[0]),
            CGSize(width: outer[1].width - w[1], height: outer[1].height - w[0]),
            CGSize(width: outer[2].width - w[1], height: outer[2].height - w[2]),
            CGSize(width: outer[3].width - w[3], height: outer[3].height - w[2]),
        ].map { CGSize(width: max(0, $0.width), height: max(0, $0.height)) }, in: inner)
        let ring = roundedRect(box, outer, shape: shape)
        ring.addPath(roundedRect(inner, innerRadii, shape: shape))

        // Visible sides, grouped by colour.
        var groups: [(CGColor, [Int])] = []
        for side in 0..<4 where w[side] > 0 && colors[side].alpha > 0 {
            if let i = groups.firstIndex(where: { $0.0 == colors[side] }) { groups[i].1.append(side) }
            else { groups.append((colors[side], [side])) }
        }
        let sided = (0..<4).filter { w[$0] > 0 }
        if groups.count == 1, groups[0].1.count == sided.count {
            // One colour for every side that has width: no joins to draw.
            ctx.saveGState()
            ctx.addPath(ring); ctx.setFillColor(groups[0].0); ctx.fillPath(using: .evenOdd)
            ctx.restoreGState()
            return
        }
        let quads = sideQuads(box: box, inner: inner, innerRadii: innerRadii)
        for (color, sides) in groups {
            ctx.saveGState()
            for side in sides { ctx.addLines(between: quads[side]); ctx.closePath() }
            ctx.clip(using: .winding)
            ctx.addPath(ring); ctx.setFillColor(color); ctx.fillPath(using: .evenOdd)
            ctx.restoreGState()
        }
    }

    /// Each side's quadrilateral (top, right, bottom, left): outer corner,
    /// inner corner, inner corner, outer corner. Each corner's join line is
    /// pushed a point past both ends along itself, so the clip's own edge
    /// antialiasing never lands on the border's outer or inner edge.
    static func sideQuads(box: CGRect, inner: CGRect, innerRadii: [CGSize]) -> [[CGPoint]] {
        let outerCorners = [CGPoint(x: box.minX, y: box.minY), CGPoint(x: box.maxX, y: box.minY),
                            CGPoint(x: box.maxX, y: box.maxY), CGPoint(x: box.minX, y: box.maxY)]
        let innerCorners = [CGPoint(x: inner.minX, y: inner.minY), CGPoint(x: inner.maxX, y: inner.minY),
                            CGPoint(x: inner.maxX, y: inner.maxY), CGPoint(x: inner.minX, y: inner.maxY)]
        // Unit vectors from each inner corner along its two edges, inward.
        let along: [(CGPoint, CGPoint)] = [((1, 0), (0, 1)), ((-1, 0), (0, 1)), ((-1, 0), (0, -1)), ((1, 0), (0, -1))]
            .map { (CGPoint(x: $0.0.0, y: $0.0.1), CGPoint(x: $0.1.0, y: $0.1.1)) }
        var joins: [(CGPoint, CGPoint)] = []
        for c in 0..<4 {
            let o = outerCorners[c]
            var p = innerCorners[c]
            let r = innerRadii[c]
            if r.width > 0 || r.height > 0 {
                // The inner curve's chord, from its end on one edge to its end on the other.
                let a = CGPoint(x: p.x + along[c].0.x * r.width, y: p.y + along[c].0.y * r.width)
                let b = CGPoint(x: p.x + along[c].1.x * r.height, y: p.y + along[c].1.y * r.height)
                if let hit = intersection(o, p, a, b) { p = hit }
            }
            let dx = p.x - o.x, dy = p.y - o.y, len = (dx * dx + dy * dy).squareRoot()
            if len > 0 {
                let e: CGFloat = 1
                joins.append((CGPoint(x: o.x - dx / len * e, y: o.y - dy / len * e),
                              CGPoint(x: p.x + dx / len * e, y: p.y + dy / len * e)))
            } else {
                joins.append((o, p))
            }
        }
        return (0..<4).map { side in
            let (a, b) = (joins[side], joins[(side + 1) % 4])
            return [a.0, a.1, b.1, b.0]
        }
    }

    /// Where the line through `a`–`b` meets the line through `c`–`d`.
    static func intersection(_ a: CGPoint, _ b: CGPoint, _ c: CGPoint, _ d: CGPoint) -> CGPoint? {
        let r = CGPoint(x: b.x - a.x, y: b.y - a.y), s = CGPoint(x: d.x - c.x, y: d.y - c.y)
        let den = r.x * s.y - r.y * s.x
        guard abs(den) > 1e-9 else { return nil }
        let t = ((c.x - a.x) * s.y - (c.y - a.y) * s.x) / den
        return CGPoint(x: a.x + r.x * t, y: a.y + r.y * t)
    }
}

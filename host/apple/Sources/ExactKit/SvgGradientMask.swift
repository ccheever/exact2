// @ref LLP 1055.000 D10 — a mask whose content is one rectangle filled
// with a gradient (a vignette, a fade) as Core Animation's own gradient
// layer, drawn by the GPU: no island. Luminance coverage is linear in the
// colour (`exact_svg_raster::mask_coverage`: 0.2126 R + 0.7152 G + 0.0722 B
// of the premultiplied sRGB colour, at most its alpha), so a stop's coverage
// is a white stop's alpha and the gradient between two stops interpolates
// it as the island's pixels would. The feature bench's F1 map (a full-size
// radial vignette over the map) spent ~95 ms of an iPhone's first frame
// rasterizing it: the gradient, the island's render and the coverage pass.
import CoreGraphics
import QuartzCore

enum SvgGradientMask {
    private static func nums(_ v: Any?) -> [Double] { (v as? [Any])?.map { ($0 as? NSNumber)?.doubleValue ?? .nan } ?? [] }

    /// A colour as the scene carries it: `[r,g,b,a]` (0–255) or a
    /// `light-dark()` pair.
    private static func rgba(_ v: Any?, dark: Bool) -> [Double]? {
        guard let a = v as? [Any] else { return nil }
        let c: [Double]
        if a.count == 2, let pair = a[dark ? 1 : 0] as? [Any] { c = pair.map { ($0 as? NSNumber)?.doubleValue ?? .nan } } else { c = nums(a) }
        return c.count == 4 && c.allSatisfy(\.isFinite) ? c : nil
    }

    /// The rectangle a scene path draws, when it is one: move, three lines
    /// and a close (or four lines), axis-aligned.
    private static func rect(_ v: Any?) -> CGRect? {
        let n = nums(v)
        var pts: [CGPoint] = [], i = 0, closed = false
        while i < n.count {
            switch Int(n[i]) {
            case 0 where i + 2 < n.count && pts.isEmpty, 1 where i + 2 < n.count:
                pts.append(CGPoint(x: n[i + 1], y: n[i + 2])); i += 3
            case 3: closed = true; i += 1
            default: return nil
            }
        }
        if pts.count == 5, pts[4] == pts[0] { pts.removeLast() }
        guard closed || pts.count == 4, pts.count == 4 else { return nil }
        // Four corners of an axis-aligned rectangle, in either order around it.
        let xs = Set(pts.map(\.x)), ys = Set(pts.map(\.y))
        guard xs.count == 2, ys.count == 2 else { return nil }
        for k in 0..<4 {
            let a = pts[k], b = pts[(k + 1) % 4]
            guard a.x == b.x || a.y == b.y else { return nil }
        }
        return CGRect(x: xs.min()!, y: ys.min()!, width: xs.max()! - xs.min()!, height: ys.max()! - ys.min()!)
    }

    /// The mask layer for `spec` (`SvgIsland.mask`'s), or `nil` when its
    /// content is not one plain gradient rectangle this can say.
    static func layer(_ spec: [String: Any], dark: Bool) -> CALayer? {
        let r = nums(spec["r"]), tt = nums(spec["t"])
        guard r.count == 4, r.allSatisfy(\.isFinite), var els = spec["c"] as? [Any] else { return nil }
        // Groups around the one shape (the mask's own, a content-units
        // transform) that only move and scale it.
        var inner = CGAffineTransform.identity
        while els.count == 1, let grp = els[0] as? [String: Any], grp["g"] != nil {
            guard grp["vp"] == nil, grp["cl"] == nil, grp["mk"] == nil, grp["fl"] == nil, grp["bl"] == nil, grp["iso"] == nil,
                  (grp["o"] as? NSNumber)?.doubleValue ?? 1 == 1, (grp["a"] as? [Any] ?? []).isEmpty,
                  let c = grp["c"] as? [Any] else { return nil }
            if let tf = grp["tf"] as? [String: Any] {
                let o = nums(tf["o"]), i = nums(tf["i"]), m = nums(tf["m"])
                guard o.count == 2, i.count == 6, m.count == 6, (tf["a"] as? [Any] ?? []).isEmpty else { return nil }
                let af = { (v: [Double]) in CGAffineTransform(a: v[0], b: v[1], c: v[2], d: v[3], tx: v[4], ty: v[5]) }
                // The pair's net map: about the origin, the individual
                // properties outside the list.
                let net = CGAffineTransform(translationX: -o[0], y: -o[1]).concatenating(af(m)).concatenating(af(i))
                    .concatenating(CGAffineTransform(translationX: o[0], y: o[1]))
                inner = net.concatenating(inner)
            }
            els = c
        }
        guard els.count == 1, let e = els[0] as? [String: Any], let g = SvgPaint.server(e["f"]), g["pt"] == nil,
              e["s"] == nil || e["s"] is NSNull, e["g"] == nil, e["tx"] == nil, e["tf"] == nil, e["inv"] == nil,
              e["cl"] == nil, e["mk"] == nil, e["fl"] == nil, e["pos"] == nil, e["po"] == nil, e["bl"] == nil, e["iso"] == nil,
              (e["a"] as? [Any] ?? []).isEmpty, (g["sp"] as? NSNumber)?.intValue ?? 0 == 0,
              let shapeRect = rect(e["p"]) else { return nil }
        // The mask's space: the masked element's user space through `t`,
        // which must keep axes (a scale and a move) for the layers to say it.
        let t: CGAffineTransform = inner.concatenating(tt.count == 6 ? CGAffineTransform(a: tt[0], b: tt[1], c: tt[2], d: tt[3], tx: tt[4], ty: tt[5]) : .identity)
        guard t.b == 0, t.c == 0, t.a != 0, t.d != 0 else { return nil }
        let gt = nums(g["t"])
        let gm: CGAffineTransform = gt.count == 6 ? CGAffineTransform(a: gt[0], b: gt[1], c: gt[2], d: gt[3], tx: gt[4], ty: gt[5]) : .identity
        guard gm.b == 0, gm.c == 0, gm.a != 0, gm.d != 0 else { return nil }
        let opacity = (e["o"] as? NSNumber)?.doubleValue ?? 1
        let luminance = ((spec["l"] as? NSNumber)?.intValue ?? 0) != 0
        // Stops as coverage, in order.
        var stops: [(Double, Double)] = []
        for case let s as [Any] in g["st"] as? [Any] ?? [] {
            guard s.count == 2, let off = (s[0] as? NSNumber)?.doubleValue, let c = rgba(s[1], dark: dark) else { return nil }
            let a = c[3] / 255 * opacity
            let cover = luminance ? min(a, (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]) / 255 * a) : a
            stops.append((min(max(off, stops.last?.0 ?? 0), 1), cover))
        }
        guard stops.count >= 2 else { return nil }
        let outer: CGAffineTransform = tt.count == 6 ? CGAffineTransform(a: tt[0], b: tt[1], c: tt[2], d: tt[3], tx: tt[4], ty: tt[5]) : .identity
        guard outer.b == 0, outer.c == 0 else { return nil }
        let region = CGRect(x: r[0], y: r[1], width: r[2], height: r[3]).applying(outer).standardized
        let box = shapeRect.applying(t).standardized
        guard region.width > 0, region.height > 0, box.width > 0, box.height > 0 else { return nil }
        let grad = CAGradientLayer()
        // Unit coordinates of the gradient layer (the rectangle's box).
        let unit = { (p: CGPoint) -> CGPoint in
            let q = p.applying(gm).applying(t)
            return CGPoint(x: (q.x - box.minX) / box.width, y: (q.y - box.minY) / box.height)
        }
        let lg = nums(g["lg"]), rg = nums(g["rg"])
        if lg.count == 4 {
            grad.type = .axial
            grad.startPoint = unit(CGPoint(x: lg[0], y: lg[1]))
            grad.endPoint = unit(CGPoint(x: lg[2], y: lg[3]))
        } else if rg.count == 6, rg[0] == rg[3], rg[1] == rg[4], rg[5] == 0, rg[2] > 0 {
            // A centred radial with no focal radius: an ellipse about its centre.
            grad.type = .radial
            let c = unit(CGPoint(x: rg[0], y: rg[1]))
            let edge = unit(CGPoint(x: rg[0] + rg[2], y: rg[1] + rg[2]))
            grad.startPoint = c
            grad.endPoint = edge
        } else {
            return nil
        }
        guard [grad.startPoint, grad.endPoint].allSatisfy({ $0.x.isFinite && $0.y.isFinite }) else { return nil }
        grad.colors = stops.map { CGColor(gray: 1, alpha: CGFloat($0.1)) }
        grad.locations = stops.map { NSNumber(value: $0.0) }
        let still: [String: CAAction] = ["contents": NSNull(), "bounds": NSNull(), "position": NSNull(), "colors": NSNull(),
                                         "locations": NSNull(), "startPoint": NSNull(), "endPoint": NSNull()]
        grad.actions = still
        grad.anchorPoint = .zero
        grad.bounds = CGRect(origin: .zero, size: box.size)
        grad.position = CGPoint(x: box.minX - region.minX, y: box.minY - region.minY)
        // The region clips it: nothing of the mask outside it.
        let container = CALayer()
        container.actions = still
        container.anchorPoint = .zero
        container.bounds = CGRect(origin: .zero, size: region.size)
        container.position = region.origin
        container.masksToBounds = true
        container.addSublayer(grad)
        return container
    }
}

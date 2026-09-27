// A path's markers (LLP 1065 D11), shared by UIKit and AppKit: the Rust host
// places them (`host/apple/src/vector.rs` writes the `markers` prop), and each
// is a clipping layer over its viewport holding a shape layer per path of the
// marker. Context paints take the path's own fill and stroke, moving ones
// included; a marker shows while the trim holds its vertex, as a dot does.
import CoreGraphics
import QuartzCore

final class VectorMarkers {
    private struct Shape {
        let path: CGPath?, fill: String, stroke: String, width: CGFloat
        let cap: CAShapeLayerLineCap, join: CAShapeLayerLineJoin, miter: CGFloat, evenOdd: Bool
    }
    private struct Instance {
        let def: Int, at: Double, outer: CGAffineTransform, clip: CGRect, fit: CGAffineTransform
    }
    private var text: String?
    private var total = 0.0
    private var defs: [[Shape]] = []
    private var placed: [(Instance, CALayer, [(StillShapeLayer, Shape)])] = []
    private var window = (start: 0.0, end: 1.0)

    /// Rebuild when the placement changed.
    func update(_ markers: String?, in host: CALayer) {
        guard markers != text else { return }
        text = markers
        for (_, layer, _) in placed { layer.removeFromSuperlayer() }
        placed = []; defs = []; total = 0
        for line in (markers ?? "").split(separator: "\n") {
            let body = line.dropFirst(2)
            switch line.first {
            case "L": total = Double(body) ?? 0
            case "D": defs.append([])
            case "S":
                let f = body.split(separator: "|", omittingEmptySubsequences: false).map(String.init)
                guard f.count == 8, !defs.isEmpty else { continue }
                defs[defs.count - 1].append(Shape(
                    path: VectorPath.path(f[0]), fill: f[1], stroke: f[2], width: CGFloat(Double(f[3]) ?? 1),
                    cap: f[4] == "round" ? .round : f[4] == "square" ? .square : .butt,
                    join: f[5] == "round" ? .round : f[5] == "bevel" ? .bevel : .miter,
                    miter: CGFloat(Double(f[6]) ?? 4), evenOdd: f[7] == "evenodd"))
            case "I":
                let n = body.split(separator: " ").compactMap { Double($0) }
                guard n.count == 16, Int(n[0]) < defs.count else { continue }
                let c = n.map { CGFloat($0) }
                let instance = Instance(def: Int(n[0]), at: n[1],
                    outer: CGAffineTransform(a: c[2], b: c[3], c: c[4], d: c[5], tx: c[6], ty: c[7]),
                    clip: CGRect(x: c[8], y: c[9], width: c[10], height: c[11]),
                    fit: CGAffineTransform(a: c[12], b: 0, c: 0, d: c[13], tx: c[14], ty: c[15]))
                let clip = StillShapeLayer() // a plain layer that never eases
                clip.anchorPoint = .zero
                clip.bounds = instance.clip
                clip.masksToBounds = true // a marker clips to its viewport
                let shapes = defs[instance.def].map { shape -> (StillShapeLayer, Shape) in
                    let layer = StillShapeLayer()
                    layer.anchorPoint = .zero
                    layer.path = shape.path
                    layer.setAffineTransform(instance.fit)
                    layer.lineWidth = shape.width; layer.lineCap = shape.cap; layer.lineJoin = shape.join
                    layer.miterLimit = shape.miter; layer.fillRule = shape.evenOdd ? .evenOdd : .nonZero
                    clip.addSublayer(layer)
                    return (layer, shape)
                }
                host.addSublayer(clip)
                placed.append((instance, clip, shapes))
            default: continue
            }
        }
        show()
    }

    /// Paint every shape: a keyword against the path's own paint, or a colour.
    func paint(fill: CGColor?, stroke: CGColor?, current: CGColor?, dark: Bool) {
        func color(_ p: String) -> CGColor? {
            switch p {
            case "none": return nil
            case "context-fill": return fill
            case "context-stroke": return stroke
            case "currentcolor": return current
            default:
                let pair = p.split(separator: "/")
                let c = (dark && pair.count == 2 ? pair[1] : pair[0]).split(separator: ",").compactMap { Double($0) }
                return c.count == 4 ? TextEngine.color(c).cgColor : nil
            }
        }
        for (_, _, shapes) in placed {
            for (layer, shape) in shapes { layer.fillColor = color(shape.fill); layer.strokeColor = color(shape.stroke) }
        }
    }

    /// Place every marker by the view box's fit into the box.
    func place(_ fit: CGAffineTransform) {
        for (instance, layer, _) in placed {
            let t = instance.outer.concatenating(fit)
            layer.position = instance.clip.origin.applying(t)
            layer.setAffineTransform(CGAffineTransform(a: t.a, b: t.b, c: t.c, d: t.d, tx: 0, ty: 0))
        }
    }

    /// The trim, as fractions: a marker shows while it holds its vertex.
    func present(start: Double? = nil, end: Double? = nil) {
        window = (start ?? window.start, end ?? window.end)
        show()
    }

    private func show() {
        let (s, e) = (window.start * total, window.end * total)
        for (instance, layer, _) in placed {
            layer.isHidden = !(s <= instance.at + 1e-9 && instance.at <= e + 1e-9 && window.start < window.end)
        }
    }
}

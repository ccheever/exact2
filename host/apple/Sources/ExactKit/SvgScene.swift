// @ref LLP 1055 D4, D7 — an `svg`'s scene as Core Animation layers, and CSS
// animations as `CAKeyframeAnimation`s. The Rust host sends the whole scene
// (host/apple/src/svg.rs): paths already flattened, paint resolved, dashes
// already scaled by `pathLength`, animations already lowered to key times,
// values and one cubic per segment. This file only builds layers from it, so
// iOS and macOS share it and no host parses SVG.
import CoreGraphics
import Foundation
import QuartzCore

/// No implicit animations: a model change is immediate, as in CSS.
private final class Still: NSObject, CAAction {
    static let shared = Still()
    func run(forKey event: String, object anObject: Any, arguments dict: [AnyHashable: Any]?) {}
}
private final class StillDelegate: NSObject, CALayerDelegate {
    static let shared = StillDelegate()
    func action(for layer: CALayer, forKey event: String) -> CAAction? { Still.shared }
}

private func still<L: CALayer>(_ layer: L) -> L { layer.delegate = StillDelegate.shared; return layer }

private func num(_ v: Any?) -> Double { (v as? NSNumber)?.doubleValue ?? 0 }
private func nums(_ v: Any?) -> [Double] { (v as? [Any])?.map(num) ?? [] }

/// A colour the host sent: `[r,g,b,a]` (0–255), or a `light-dark()` pair.
private func color(_ v: Any?, dark: Bool) -> CGColor? {
    guard let a = v as? [Any] else { return nil }
    let c: [Double]
    if a.count == 2, let pair = a[dark ? 1 : 0] as? [Any] { c = pair.map(num) } else { c = a.map(num) }
    guard c.count == 4 else { return nil }
    return CGColor(srgbRed: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
}

/// `[0,x,y, 1,x,y, 2,x1,y1,x2,y2,x,y, 3]`: move, line, cubic, close.
private func path(_ v: Any?) -> CGPath {
    let n = nums(v), p = CGMutablePath()
    var i = 0
    while i < n.count {
        switch Int(n[i]) {
        case 0 where i + 2 < n.count: p.move(to: CGPoint(x: n[i + 1], y: n[i + 2])); i += 3
        case 1 where i + 2 < n.count: p.addLine(to: CGPoint(x: n[i + 1], y: n[i + 2])); i += 3
        case 2 where i + 6 < n.count:
            p.addCurve(to: CGPoint(x: n[i + 5], y: n[i + 6]), control1: CGPoint(x: n[i + 1], y: n[i + 2]), control2: CGPoint(x: n[i + 3], y: n[i + 4])); i += 7
        case 3: p.closeSubpath(); i += 1
        default: return p
        }
    }
    return p
}

/// A circle about the origin from (r, 0), toward positive y (SVG 2 §10.3),
/// the same four cubics the kernel draws, so a radius animation interpolates
/// between circles.
private func circle(_ r: Double) -> CGPath {
    let k = r * 0.5522848, p = CGMutablePath()
    p.move(to: CGPoint(x: r, y: 0))
    p.addCurve(to: CGPoint(x: 0, y: r), control1: CGPoint(x: r, y: k), control2: CGPoint(x: k, y: r))
    p.addCurve(to: CGPoint(x: -r, y: 0), control1: CGPoint(x: -k, y: r), control2: CGPoint(x: -r, y: k))
    p.addCurve(to: CGPoint(x: 0, y: -r), control1: CGPoint(x: -r, y: -k), control2: CGPoint(x: -k, y: -r))
    p.addCurve(to: CGPoint(x: r, y: 0), control1: CGPoint(x: k, y: -r), control2: CGPoint(x: r, y: -k))
    p.closeSubpath()
    return p
}

/// CSS animations lowered to Core Animation (LLP 1055 D7). A spec's start is
/// on the runtime clock (ms / 1000 since `ExactEnv.t0`); a held one (authored
/// `paused`, or any under an agent-owned clock) is `speed = 0` at its local
/// time, so a screenshot and a `clock` seek are deterministic.
enum CssAnimations {
    /// Replace `layer`'s CSS animations with `specs`, keeping any whose spec
    /// is unchanged (a data tick must not restart a running pulse).
    static func apply(_ specs: [[String: Any]], to layer: CALayer, clock: Double?, installed: inout [String: String]) {
        var keep: Set<String> = []
        for spec in specs {
            guard let id = spec["id"] as? String else { continue }
            keep.insert(id)
            let signature = spec.keys.sorted().map { "\($0)=\(spec[$0]!)" }.joined(separator: ";") + (clock.map { "@\($0)" } ?? "")
            if installed[id] == signature { continue }
            installed[id] = signature
            layer.removeAnimation(forKey: id)
            if let animation = make(spec, layer: layer, clock: clock) { layer.add(animation, forKey: id) }
        }
        for id in installed.keys where !keep.contains(id) {
            layer.removeAnimation(forKey: id)
            installed.removeValue(forKey: id)
        }
    }

    static func make(_ spec: [String: Any], layer: CALayer, clock: Double?) -> CAAnimation? {
        let key = spec["k"] as? String ?? ""
        let times = nums(spec["t"]), values = nums(spec["v"])
        let duration = num(spec["d"]), repeatCount = num(spec["n"])
        guard duration > 0, repeatCount != 0, times.count == values.count, times.count >= 2 else { return nil }
        let a = CAKeyframeAnimation(keyPath: key == "r" ? "path" : key)
        a.keyTimes = times.map { NSNumber(value: $0) }
        a.values = key == "r" ? values.map { circle(max(0, $0)) } : values.map { NSNumber(value: $0) }
        a.timingFunctions = (spec["c"] as? [Any] ?? []).map { c in
            let p = nums(c).map(Float.init)
            return p.count == 4 ? CAMediaTimingFunction(controlPoints: p[0], p[1], p[2], p[3]) : CAMediaTimingFunction(name: .linear)
        }
        a.calculationMode = .linear
        a.duration = duration
        a.repeatCount = repeatCount < 0 ? .infinity : Float(repeatCount)
        let fill = Int(num(spec["fill"]))
        let backwards = fill == 2 || fill == 3
        a.fillMode = [CAMediaTimingFillMode.removed, .forwards, .backwards, .both][min(max(fill, 0), 3)]
        a.isRemovedOnCompletion = false
        let start = num(spec["s"]), delay = num(spec["dl"])
        let held = (spec["h"] as? NSNumber)?.doubleValue ?? clock.map { $0 / 1000 - start }
        if let local = held {
            let active = local - delay
            if active < 0 && !backwards { return nil }
            a.speed = 0
            // At or past a finite end CA wraps to the next cycle's start; CSS
            // holds the last frame (with a forwards fill), so stay a hair inside.
            let total = repeatCount < 0 ? Double.infinity : duration * repeatCount
            a.timeOffset = min(max(0, active), total - 1e-6)
            a.beginTime = 0
        } else {
            a.beginTime = layer.convertTime(ExactEnv.t0 + start + delay, from: nil)
        }
        return a
    }
}

/// One `svg` view's scene: a content-box layer whose sublayer transform is
/// the view box, a `CAShapeLayer` per shape and a `CALayer` per `g`.
final class SvgScene {
    let root = still(CALayer())
    private var layers: [Int: CALayer] = [:]
    private var installed: [Int: [String: String]] = [:]
    private var specs: [Int: [[String: Any]]] = [:]
    private var last: [String: Any] = [:]

    init() { root.masksToBounds = false }

    /// Build or update the layers from a scene; unchanged animations keep running.
    func apply(_ scene: [String: Any], dark: Bool, clock: Double?) {
        last = scene
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let box = nums(scene["box"])
        if box.count == 4 { root.frame = CGRect(x: box[0], y: box[1], width: box[2], height: box[3]) }
        let t = nums(scene["t"])
        root.isHidden = t.count != 6
        if t.count == 6 { root.sublayerTransform = CATransform3DMakeAffineTransform(CGAffineTransform(a: t[0], b: t[1], c: t[2], d: t[3], tx: t[4], ty: t[5])) }
        var alive: Set<Int> = []
        attach(scene["els"] as? [Any] ?? [], to: root, dark: dark, clock: clock, alive: &alive)
        for (id, layer) in layers where !alive.contains(id) {
            layer.removeAllAnimations(); layer.removeFromSuperlayer()
            layers.removeValue(forKey: id); installed.removeValue(forKey: id); specs.removeValue(forKey: id)
        }
    }

    /// Re-seek every animation to an agent-owned clock (or back to real time).
    func seek(clock: Double?) {
        for (id, list) in specs { if let layer = layers[id] { CssAnimations.apply(list, to: layer, clock: clock, installed: &installed[id, default: [:]]) } }
    }

    /// Everything off (the view is destroyed or parked).
    func reset() {
        for layer in layers.values { layer.removeAllAnimations() }
        root.sublayers?.forEach { $0.removeFromSuperlayer() }
        layers = [:]; installed = [:]; specs = [:]
    }

    private func attach(_ elements: [Any], to parent: CALayer, dark: Bool, clock: Double?, alive: inout Set<Int>) {
        var order: [CALayer] = []
        for case let e as [String: Any] in elements {
            let id = Int(num(e["id"]))
            alive.insert(id)
            let group = e["g"] != nil
            let layer: CALayer
            if let existing = layers[id], (existing is CAShapeLayer) != group { layer = existing } else {
                layers[id]?.removeFromSuperlayer()
                layer = group ? still(CALayer()) : still(CAShapeLayer())
                layers[id] = layer
            }
            layer.opacity = Float(num(e["o"]))
            if group {
                attach(e["c"] as? [Any] ?? [], to: layer, dark: dark, clock: clock, alive: &alive)
            } else if let shape = layer as? CAShapeLayer {
                shape.path = path(e["p"])
                let pos = nums(e["pos"])
                shape.position = pos.count == 2 ? CGPoint(x: pos[0], y: pos[1]) : .zero
                shape.fillColor = color(e["f"], dark: dark)
                shape.strokeColor = color(e["s"], dark: dark)
                shape.lineWidth = CGFloat(num(e["w"]))
                shape.lineCap = [CAShapeLayerLineCap.butt, .round, .square][min(Int(num(e["cap"])), 2)]
                shape.lineJoin = [CAShapeLayerLineJoin.miter, .round, .bevel][min(Int(num(e["join"])), 2)]
                shape.miterLimit = CGFloat(num(e["ml"]))
                shape.fillRule = num(e["rule"]) == 1 ? .evenOdd : .nonZero
                let dash = nums(e["dash"])
                shape.lineDashPattern = dash.isEmpty ? nil : dash.map { NSNumber(value: $0) }
                shape.lineDashPhase = CGFloat(num(e["ph"]))
            }
            let list = e["a"] as? [[String: Any]] ?? []
            specs[id] = list
            CssAnimations.apply(list, to: layer, clock: clock, installed: &installed[id, default: [:]])
            order.append(layer)
        }
        // Paint order is document order.
        if parent.sublayers?.map(ObjectIdentifier.init) != order.map(ObjectIdentifier.init) {
            for layer in order { layer.removeFromSuperlayer() }
            for layer in order { parent.addSublayer(layer) }
        }
    }
}

/// A presenter's scenes and box animations, by view id: the `svg` and
/// `animations` ops, the agent clock's re-seek, and cleanup when a view goes.
final class SvgHost {
    private var scenes: [UInt32: SvgScene] = [:]
    private var boxSpecs: [UInt32: (layer: CALayer, specs: [[String: Any]])] = [:]
    private var boxInstalled: [UInt32: [String: String]] = [:]
    private var seeked: Double?

    func scene(_ id: UInt32, _ payload: [String: Any], layer: CALayer?, dark: Bool, clock: Double?) {
        guard let layer else { return }
        let scene = scenes[id] ?? { let s = SvgScene(); scenes[id] = s; return s }()
        if scene.root.superlayer !== layer { layer.addSublayer(scene.root) }
        scene.apply(payload["scene"] as? [String: Any] ?? [:], dark: dark, clock: clock)
    }

    func animations(_ id: UInt32, _ payload: [String: Any], layer: CALayer?, clock: Double?) {
        guard let layer else { return }
        let specs = payload["specs"] as? [[String: Any]] ?? []
        boxSpecs[id] = specs.isEmpty ? nil : (layer, specs)
        CssAnimations.apply(specs, to: layer, clock: clock, installed: &boxInstalled[id, default: [:]])
    }

    /// An agent moved the clock: every held animation shows the new instant.
    func seek(clock: Double?) {
        guard clock != seeked else { return }
        seeked = clock
        for scene in scenes.values { scene.seek(clock: clock) }
        for (id, entry) in boxSpecs { CssAnimations.apply(entry.specs, to: entry.layer, clock: clock, installed: &boxInstalled[id, default: [:]]) }
    }

    func forget(_ id: UInt32) {
        if let scene = scenes.removeValue(forKey: id) { scene.reset(); scene.root.removeFromSuperlayer() }
        if let entry = boxSpecs.removeValue(forKey: id) { CssAnimations.apply([], to: entry.layer, clock: nil, installed: &boxInstalled[id, default: [:]]) }
        boxInstalled.removeValue(forKey: id)
    }
}

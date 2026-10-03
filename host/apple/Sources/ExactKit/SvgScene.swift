// @ref LLP 1055 D4, D7 — an `svg`'s scene as Core Animation layers, and CSS
// animations as `CAKeyframeAnimation`s. The Rust host sends the whole scene
// (host/apple/src/svg.rs): paths already flattened, paint resolved, dashes
// already scaled by `pathLength`, animations already lowered to key times,
// values and one cubic per segment. This file only builds layers from it, so
// iOS and macOS share it and no host parses SVG.
import CoreGraphics
import CoreText
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

/// A colour the host sent: `[r,g,b,a]` (0–255), a `light-dark()` pair, or a
/// colour in its own space, `{"cs": [{"s", "v"}…]}` (LLP 1100 D2).
private func color(_ v: Any?, dark: Bool) -> CGColor? {
    if let o = v as? [String: Any], let halves = o["cs"] as? [Any], !halves.isEmpty,
       let half = halves[dark && halves.count > 1 ? 1 : 0] as? [String: Any],
       let name = ["srgb": CGColorSpace.extendedSRGB, "display-p3": CGColorSpace.extendedDisplayP3,
                   "srgb-linear": CGColorSpace.extendedLinearSRGB][half["s"] as? String ?? ""],
       let space = CGColorSpace(name: name), case let v = nums(half["v"]), v.count == 4 {
        return CGColor(colorSpace: space, components: v.map { CGFloat($0) }).map(ColorRange.tagged)
    }
    guard let a = v as? [Any] else { return nil }
    let c: [Double]
    if a.count == 2, let pair = a[dark ? 1 : 0] as? [Any] { c = pair.map(num) } else { c = a.map(num) }
    guard c.count == 4 else { return nil }
    return CGColor(srgbRed: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
}

/// `[0,x,y, 1,x,y, 2,x1,y1,x2,y2,x,y, 3]`: move, line, cubic, close.
private func path(_ v: Any?) -> CGPath {
    if let prepared = v as? PreparedPath { return prepared.path }
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

/// The scene's path parser, for the owner's preparation (`SvgPrepare`).
enum SvgGeometry {
    static func path(_ v: Any?) -> CGPath { svgPath(v) }
}
func svgPath(_ v: Any?) -> CGPath { path(v) }

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

/// CSS animations lowered to Core Animation (LLP 1055 D7; a box's transform
/// key paths and background colour, LLP 1055.001). A spec's start is
/// on the runtime clock (ms / 1000 since `ExactEnv.t0`); a held one (authored
/// `paused`, or any under an agent-owned clock) is `speed = 0` at its local
/// time, so a screenshot and a `clock` seek are deterministic.
enum CssAnimations {
    /// Replace `layer`'s CSS animations with `specs`, keeping any whose spec
    /// is unchanged (a data tick must not restart a running pulse).
    static func apply(_ specs: [[String: Any]], to layer: CALayer, clock: Double?, installed: inout [String: String], offscreen: Bool = false) {
        var keep: Set<String> = []
        for spec in specs {
            guard let id = spec["id"] as? String else { continue }
            keep.insert(id)
            let prepared = spec["#"] as? PreparedAnimation
            let hash: Int
            if let prepared { hash = prepared.signature } else {
                var h = Hasher()
                digest(spec, into: &h)
                hash = h.finalize()
            }
            let signature = String(hash) + (clock.map { "@\($0)" } ?? "")
            if installed[id] == signature { continue }
            installed[id] = signature
            layer.removeAnimation(forKey: id)
            if let animation = make(spec, layer: layer, clock: clock, offscreen: offscreen) { layer.add(animation, forKey: id) }
        }
        for id in installed.keys where !keep.contains(id) {
            layer.removeAnimation(forKey: id)
            installed.removeValue(forKey: id)
        }
    }

    /// A spec's values, hashed: describing them as text was most of a
    /// scene's cost on a list's rows.
    /// A batch's numbers arrive as `NSNumber`: that case goes first, as a
    /// class cast, because `as Double` bridges and was most of a list
    /// row's animation cost.
    static func digest(_ v: Any?, into h: inout Hasher) {
        switch v {
        case let n as NSNumber: h.combine(n.doubleValue.bitPattern)
        case let n as Double: h.combine(n.bitPattern)
        case let s as String: h.combine(s)
        case let a as [Any]: h.combine(a.count); for x in a { digest(x, into: &h) }
        // A prepared path hashes as the numbers it was built from.
        case let p as PreparedPath: h.combine(p.digest)
        case let d as [String: Any]:
            // A spec's prepared parts ("#") are made from the rest.
            let keys = d.keys.filter { $0 != "#" }.sorted()
            h.combine(keys.count); for k in keys { h.combine(k); digest(d[k], into: &h) }
        case .none: h.combine(0 as UInt8)
        case let .some(other): h.combine(String(describing: other))
        }
    }

    /// A spec's keyframe values, key times and timing functions: plain
    /// values, built on the owner as the batch is decoded (`SvgPrepare`).
    static func lowered(_ spec: [String: Any]) -> (values: [Any], keyTimes: [NSNumber], timing: [CAMediaTimingFunction]?) {
        let key = spec["k"] as? String ?? ""
        let colors = key == "fillColor" || key == "strokeColor" || key == "backgroundColor"
        // A colour track's values are [r,g,b,a] bytes (LLP 1055.000 D6).
        let values: [Any] = colors
            ? (spec["v"] as? [Any] ?? []).compactMap { color($0, dark: false) }
            : nums(spec["v"]).map { key == "r" ? circle(max(0, $0)) as Any : NSNumber(value: $0) }
        // No curves: every interval is linear, Core Animation's default.
        var timing: [CAMediaTimingFunction]?
        if let curves = spec["c"] as? [Any], !curves.isEmpty {
            timing = curves.map { c in
                let p = nums(c).map(Float.init)
                return p.count == 4 ? CAMediaTimingFunction(controlPoints: p[0], p[1], p[2], p[3]) : CAMediaTimingFunction(name: .linear)
            }
        }
        return (values, nums(spec["t"]).map { NSNumber(value: $0) }, timing)
    }

    /// `offscreen`: for a tree `CARenderer` draws (a live filter picture),
    /// which plays running animations but not one held at `speed` 0: a
    /// held one runs from where it is held, and the picture is drawn at once.
    static func make(_ spec: [String: Any], layer: CALayer, clock: Double?, offscreen: Bool = false) -> CAAnimation? {
        let key = spec["k"] as? String ?? ""
        let (values, keyTimes, timing) = (spec["#"] as? PreparedAnimation).map { ($0.values, $0.keyTimes, $0.timing) } ?? lowered(spec)
        let duration = num(spec["d"]), repeatCount = num(spec["n"])
        guard duration > 0, repeatCount != 0, keyTimes.count == values.count, keyTimes.count >= 2 else { return nil }
        let a = CAKeyframeAnimation(keyPath: key == "r" ? "path" : key)
        a.keyTimes = keyTimes
        a.values = values
        if let timing { a.timingFunctions = timing }
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
            // At or past a finite end CA wraps to the next cycle's start; CSS
            // holds the last frame (with a forwards fill), so stay a hair inside.
            let total = repeatCount < 0 ? Double.infinity : duration * repeatCount
            let at = min(max(0, active), total - 1e-6)
            if offscreen {
                a.beginTime = layer.convertTime(CACurrentMediaTime(), from: nil) - at
            } else {
                a.speed = 0
                a.timeOffset = at
                a.beginTime = 0
            }
        } else {
            a.beginTime = layer.convertTime(ExactEnv.t0 + start + delay, from: nil)
        }
        return a
    }
}

private func affine(_ v: Any?) -> CGAffineTransform? {
    let t = nums(v)
    return t.count == 6 ? CGAffineTransform(a: t[0], b: t[1], c: t[2], d: t[3], tx: t[4], ty: t[5]) : nil
}

/// One `svg` view's scene: a content-box layer whose sublayer transform is
/// the view box, a `CAShapeLayer` per shape, a `CALayer` per `g` and nested
/// `svg`, and a pair of layers around an element with a transform (LLP
/// 1055.000 D5): the outer takes the individual properties, the inner the
/// `transform` list, both about the transform origin. Every container's
/// anchor is its bounds' origin, so a sublayer transform applies from there.
final class SvgScene {
    let root = still(CALayer())
    private var layers: [Int: CALayer] = [:]
    /// The transform pair around an element, by id.
    private var wrappers: [Int: (outer: CALayer, inner: CALayer)] = [:]
    /// The transform animations each wrapper plays, for the agent clock's re-seek.
    private var wrapSpecs: [Int: [[String: Any]]] = [:]
    private var wrapInstalled: [Int: [String: String]] = [:]
    private var installed: [Int: [String: String]] = [:]
    /// Each masked element's mask island, by the digest of what drew it.
    private var islands: [Int: (key: Int, layer: CALayer)] = [:]
    /// Each filtered element's picture, by the digest of what drew it.
    private var pictures: [Int: (key: Int, layer: CALayer)] = [:]
    private var specs: [Int: [[String: Any]]] = [:]
    /// The view's pixels per point, for gradients drawn as pixels.
    var scale: CGFloat = 2
    /// The presenter's fonts, for SVG text (LLP 1055.000 D11).
    var fonts: SvgText.Fonts?
    /// For hit testing (LLP 1055.000 D17): each element layer's node and
    /// parent element, whether it takes presses, whether hits pass it by.
    private var node: [ObjectIdentifier: (uid: Int, node: UInt32)] = [:]
    private var parentOf: [Int: Int] = [:]
    private var nodeOf: [Int: UInt32] = [:]
    private var pressable: Set<Int> = []
    private var passes: Set<Int> = []
    /// A system font where no presenter's fonts are known.
    static let systemFonts: SvgText.Fonts = { size, _, _, _ in CTFontCreateUIFontForLanguage(.system, size, nil) ?? CTFontCreateWithName("Helvetica" as CFString, size, nil) }

    /// The content box when the `svg` clips to it: an island over the cap
    /// renders only what can show there (`SvgIsland.extent`).
    private var clipped: CGRect?

    /// Drawn by a `CARenderer` (a live filter picture's sub-scene).
    var offscreen = false

    init() { root.masksToBounds = false; root.anchorPoint = .zero }

    #if os(iOS) || os(tvOS)
    /// Filtered pictures that follow a changing input on the GPU, by element.
    private var live: [Int: SvgFilterLive] = [:]
    #endif

    /// A shown filtered picture whose input changed: on iOS, the same layer
    /// redrawn on the GPU after this commit (`SvgFilterLive`); `nil` for a
    /// first picture, or a chain or host the GPU path does not take.
    private func follow(_ id: Int, _ fl: [String: Any], k: CGFloat, dark: Bool, clock: Double?) -> CALayer? {
        #if os(iOS) || os(tvOS)
        // A picture whose content animates follows it from its first frame:
        // its animations play in its sub-scene (`svg_lower::in_picture`).
        let animated = SvgFilterLive.animated(fl["c"] as Any)
        guard pictures[id] != nil || animated, SvgFilterLive.takes(fl),
              let e = SvgIsland.filterExtent(fl, k: k, seen: seen(fl)), e.gpu else { return nil }
        let shown = pictures[id]?.layer ?? {
            let l = CALayer()
            l.actions = ["contents": NSNull(), "bounds": NSNull(), "position": NSNull(), "contentsRect": NSNull()]
            l.anchorPoint = .zero
            return l
        }()
        let picture = live[id].flatMap { $0.layer === shown ? $0 : nil } ?? SvgFilterLive(layer: shown)
        live[id] = picture
        picture.update(els: fl["c"] as? [Any] ?? [], rect: e.rect, w: e.w, h: e.h, k: e.k, program: e.program,
                       dark: dark, fonts: fonts ?? SvgScene.systemFonts, clock: clock, now: pictures[id] == nil)
        return shown
        #else
        return nil
        #endif
    }

    private func forget(_ id: Int) {
        #if os(iOS) || os(tvOS)
        live.removeValue(forKey: id)?.stop()
        #endif
    }

    /// Lone drop shadows by element: the region's clip and the layer
    /// casting the shadow, which holds the element.
    private var shadows: [Int: (clip: CALayer, cast: CALayer)] = [:]

    /// A filter that is one `feDropShadow` (or CSS `drop-shadow()`) of the
    /// element, as Core Animation's shadow says it: `shadowRadius` is the
    /// Gaussian's σ, in the element's user space as the offset is. Its
    /// colour space (`color-interpolation-filters`) changes only how the
    /// element's antialiased edge blends over its own shadow.
    static func dropShadow(_ fl: [String: Any]) -> (region: CGRect, sigma: CGFloat, offset: CGSize, color: CGColor, opacity: Float)? {
        let p = nums(fl["p"])
        // region (4), count, then code, in, in2, subregion (4), linear, σx, σy, dx, dy, rgba.
        guard p.count == 5 + 8 + 8, p[4] == 1, p[5] == 6, p[6] == -1, p[13] == p[14], p[13] >= 0,
              p[15].isFinite, p[16].isFinite else { return nil }
        let region = CGRect(x: p[0], y: p[1], width: p[2], height: p[3])
        let sub = CGRect(x: p[8], y: p[9], width: p[10], height: p[11])
        guard region.width > 0, region.height > 0, sub.contains(region) || sub == region else { return nil }
        return (region, CGFloat(p[13]), CGSize(width: p[15], height: p[16]),
                CGColor(srgbRed: p[17], green: p[18], blue: p[19], alpha: 1), Float(min(max(p[20], 0), 1)))
    }

    /// Each shape's last drawn spec, hashed (with the appearance and the
    /// scale its parts are drawn at): a scene applied again redraws only the
    /// shapes that changed. A live filter picture's sub-scene is applied on
    /// every frame of the animation inside it, and redrawing its unchanged
    /// gradient card was most of that (467 ms/s on the M1 iPad Pro).
    private var drawn: [Int: Int] = [:]

    /// Whether shape `id` must be drawn from `e`: it changed, or is new.
    private func drew(_ id: Int, _ e: [String: Any], dark: Bool) -> Bool {
        var h = Hasher()
        for k in e.keys.sorted() where k != "a" { h.combine(k); CssAnimations.digest(e[k], into: &h) }
        h.combine(dark); h.combine(scale)
        let d = h.finalize()
        if drawn[id] == d { return false }
        drawn[id] = d
        return true
    }

    /// What of an island's user space can show: the clipping content box
    /// through the inverse of the island's `m` (user space to that box).
    private func seen(_ spec: [String: Any]) -> CGRect? {
        let m = nums(spec["m"])
        guard let clipped, m.count == 6, m[0] * m[3] - m[1] * m[2] != 0 else { return nil }
        return clipped.applying(CGAffineTransform(a: m[0], b: m[1], c: m[2], d: m[3], tx: m[4], ty: m[5]).inverted())
    }

    /// Build or update the layers from a scene; unchanged animations keep running.
    func apply(_ scene: [String: Any], dark: Bool, clock: Double?) {
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let box = nums(scene["box"])
        if box.count == 4 { root.bounds = CGRect(x: 0, y: 0, width: box[2], height: box[3]); root.position = CGPoint(x: box[0], y: box[1]) }
        clipped = scene["clip"] != nil && box.count == 4 ? CGRect(x: 0, y: 0, width: box[2], height: box[3]) : nil
        let t = nums(scene["t"])
        root.isHidden = t.count != 6
        if t.count == 6 { root.sublayerTransform = CATransform3DMakeAffineTransform(CGAffineTransform(a: t[0], b: t[1], c: t[2], d: t[3], tx: t[4], ty: t[5])) }
        var alive: Set<Int> = []
        attach(scene["els"] as? [Any] ?? [], to: root, dark: dark, clock: clock, alive: &alive)
        for (id, layer) in layers where !alive.contains(id) {
            layer.removeAllAnimations(); layer.removeFromSuperlayer()
            layers.removeValue(forKey: id); installed.removeValue(forKey: id); specs.removeValue(forKey: id); islands.removeValue(forKey: id); pictures.removeValue(forKey: id); forget(id); drawn.removeValue(forKey: id)
            if let pair = shadows.removeValue(forKey: id) { pair.clip.removeFromSuperlayer() }
            if let pair = wrappers.removeValue(forKey: id) { pair.outer.removeFromSuperlayer() }
            wrapSpecs.removeValue(forKey: id); wrapInstalled.removeValue(forKey: id)
            node.removeValue(forKey: ObjectIdentifier(layer)); parentOf.removeValue(forKey: id)
            nodeOf.removeValue(forKey: id); pressable.remove(id); passes.remove(id)
        }
    }

    /// The node a press at `point` (the host view's coordinates) goes to:
    /// the topmost element painted there (its fill, or its stroke's
    /// outline), then up to the nearest one with a press handler
    /// (LLP 1055.000 D17). `nil` when none takes it.
    func target(at point: CGPoint) -> UInt32? {
        guard let host = root.superlayer, !pressable.isEmpty else { return nil }
        var hit = self.hit(root, root.convert(point, from: host))
        while let uid = hit {
            if pressable.contains(uid) { return nodeOf[uid] }
            hit = parentOf[uid] ?? nil
        }
        return nil
    }

    /// The topmost element layer under `p` (in `layer`'s coordinates).
    private func hit(_ layer: CALayer, _ p: CGPoint) -> Int? {
        // Opacity does not change a hit (SVG 2 pointer-events); hiding does.
        if layer.isHidden { return nil }
        if let mask = layer.mask, !SvgScene.covers(mask, mask.convert(p, from: layer)) { return nil }
        if layer.masksToBounds, !layer.bounds.contains(p) { return nil }
        let mine = node[ObjectIdentifier(layer)]
        if let mine, passes.contains(mine.uid) { return nil }
        for sub in (layer.sublayers ?? []).reversed() where sub !== layer.mask {
            // A part layer (a gradient's, a text run's) hits as its element.
            if let found = hit(sub, sub.convert(p, from: layer)) { return found == -1 ? (mine?.uid ?? -1) : found }
        }
        if let shape = layer as? CAShapeLayer, SvgScene.paints(shape, at: p) {
            return mine?.uid ?? -1
        }
        return nil
    }

    /// Whether a shape layer paints `p`: its fill, or its stroke's outline
    /// (caps, joins and dashes as Core Graphics strokes them).
    private static func paints(_ shape: CAShapeLayer, at p: CGPoint) -> Bool {
        guard let path = shape.path else { return false }
        if shape.fillColor != nil, path.contains(p, using: shape.fillRule == .evenOdd ? .evenOdd : .winding) { return true }
        guard shape.strokeColor != nil, shape.lineWidth > 0 else { return false }
        var line = path
        if let dash = shape.lineDashPattern, !dash.isEmpty {
            line = path.copy(dashingWithPhase: shape.lineDashPhase, lengths: dash.map { CGFloat($0.doubleValue) })
        }
        let cap: CGLineCap = shape.lineCap == .round ? .round : shape.lineCap == .square ? .square : .butt
        let join: CGLineJoin = shape.lineJoin == .round ? .round : shape.lineJoin == .bevel ? .bevel : .miter
        return line.copy(strokingWithWidth: shape.lineWidth, lineCap: cap, lineJoin: join, miterLimit: shape.miterLimit).contains(p)
    }

    /// Whether a clip mask covers `p`: any of its opaque shapes, inside its
    /// own mask.
    private static func covers(_ mask: CALayer, _ p: CGPoint) -> Bool {
        if let inner = mask.mask, !covers(inner, inner.convert(p, from: mask)) { return false }
        return (mask.sublayers ?? []).contains { sub in
            guard let s = sub as? CAShapeLayer, let path = s.path else { return false }
            return path.contains(sub.convert(p, from: mask), using: s.fillRule == .evenOdd ? .evenOdd : .winding)
        }
    }

    /// Re-seek every animation to an agent-owned clock (or back to real time).
    func seek(clock: Double?, force: Bool = false) {
        if force { installed = [:]; wrapInstalled = [:] }
        for (id, list) in specs { if let layer = layers[id] { CssAnimations.apply(list, to: layer, clock: clock, installed: &installed[id, default: [:]], offscreen: offscreen) } }
        for (id, list) in wrapSpecs { if let outer = wrappers[id]?.outer { CssAnimations.apply(list, to: outer, clock: clock, installed: &wrapInstalled[id, default: [:]], offscreen: offscreen) } }
        #if os(iOS) || os(tvOS)
        for l in live.values { l.seek(clock) }
        #endif
    }

    /// Everything off (the view is destroyed or parked).
    func reset() {
        for layer in layers.values { layer.removeAllAnimations() }
        root.sublayers?.forEach { $0.removeFromSuperlayer() }
        for pair in wrappers.values { pair.outer.removeAllAnimations() }
        layers = [:]; installed = [:]; specs = [:]; wrappers = [:]; wrapSpecs = [:]; wrapInstalled = [:]; islands = [:]; pictures = [:]; drawn = [:]; shadows = [:]
        #if os(iOS) || os(tvOS)
        for l in live.values { l.stop() }
        live = [:]
        #endif
        node = [:]; parentOf = [:]; nodeOf = [:]; pressable = []; passes = []
    }

    /// The layer to place for an element: itself, or the outer of its
    /// transform pair, the element inside the inner.
    private func wrap(_ id: Int, _ layer: CALayer, _ tf: [String: Any]?, clock: Double?) -> CALayer {
        guard let tf else {
            if let pair = wrappers.removeValue(forKey: id) { pair.outer.removeFromSuperlayer() }
            wrapSpecs.removeValue(forKey: id); wrapInstalled.removeValue(forKey: id)
            return layer
        }
        let pair = wrappers[id] ?? { let p = (outer: still(CALayer()), inner: still(CALayer())); p.outer.addSublayer(p.inner); wrappers[id] = p; return p }()
        let o = nums(tf["o"])
        let origin = o.count == 2 ? CGPoint(x: o[0], y: o[1]) : .zero
        for l in [pair.outer, pair.inner] {
            l.bounds = CGRect(origin: origin, size: .zero)
            l.position = origin
        }
        pair.outer.setAffineTransform(affine(tf["i"]) ?? .identity)
        pair.inner.setAffineTransform(affine(tf["m"]) ?? .identity)
        if layer.superlayer !== pair.inner { layer.removeFromSuperlayer(); pair.inner.addSublayer(layer) }
        // The individual properties' lowered animations play on the outer
        // layer (LLP 1055.001, as a box's on iOS): each key path replaces
        // one component of its transform and keeps the others.
        let list = tf["a"] as? [[String: Any]] ?? []
        wrapSpecs[id] = list.isEmpty ? nil : list
        CssAnimations.apply(list, to: pair.outer, clock: clock, installed: &wrapInstalled[id, default: [:]], offscreen: offscreen)
        return pair.outer
    }

    private func attach(_ elements: [Any], to parent: CALayer, dark: Bool, clock: Double?, alive: inout Set<Int>, owner: Int? = nil) {
        var order: [CALayer] = []
        for case let e as [String: Any] in elements {
            let id = Int(num(e["id"]))
            alive.insert(id)
            parentOf[id] = owner
            nodeOf[id] = UInt32(num(e["n"]))
            if e["h"] != nil { pressable.insert(id) } else { pressable.remove(id) }
            if e["pn"] != nil { passes.insert(id) } else { passes.remove(id) }
            let text = e["tx"] as? [Any]
            let group = e["g"] != nil || text != nil
            let layer: CALayer
            if let existing = layers[id], (existing is CAShapeLayer) != group { layer = existing } else {
                layers[id]?.removeFromSuperlayer()
                layer = group ? still(CALayer()) : still(CAShapeLayer())
                layers[id] = layer
                drawn.removeValue(forKey: id)
            }
            layer.opacity = Float(num(e["o"]))
            if let text {
                // @ref LLP 1055.000 D11 — text: a layer of glyph outlines per run.
                SvgText.build(layer, chunks: text, dark: dark, fonts: fonts ?? SvgScene.systemFonts) { color($0, dark: $1) }
            } else if group {
                let vp = nums(e["vp"])
                if vp.count == 4 {
                    // A nested `svg`: its own viewport, clipped unless visible.
                    layer.anchorPoint = .zero
                    layer.bounds = CGRect(x: 0, y: 0, width: vp[2], height: vp[3])
                    layer.position = CGPoint(x: vp[0], y: vp[1])
                    layer.masksToBounds = num(e["clip"]) != 0
                    let view = affine(e["t"])
                    layer.isHidden = view == nil
                    layer.sublayerTransform = CATransform3DMakeAffineTransform(view ?? .identity)
                }
                attach(e["c"] as? [Any] ?? [], to: layer, dark: dark, clock: clock, alive: &alive, owner: id)
                // @ref LLP 1055.000 D14 — a filtered element's picture is an island.
                if let fl = e["fl"] as? [String: Any], let drop = SvgScene.dropShadow(fl) {
                    // A lone drop shadow is Core Animation's own shadow on
                    // the element, clipped to the filter region: no pixels
                    // of ours (F1's 300 shadowed POIs held a 21 MB island
                    // and 150 MB of Core Image intermediates at launch).
                    if let old = pictures.removeValue(forKey: id) { old.layer.removeFromSuperlayer(); forget(id) }
                    let pair = shadows[id] ?? { let p = (clip: still(CALayer()), cast: still(CALayer())); p.clip.addSublayer(p.cast); p.clip.anchorPoint = .zero; p.clip.masksToBounds = true; shadows[id] = p; return p }()
                    pair.clip.bounds = drop.region
                    pair.clip.position = drop.region.origin
                    pair.cast.shadowColor = drop.color
                    pair.cast.shadowOpacity = drop.opacity
                    pair.cast.shadowRadius = drop.sigma
                    pair.cast.shadowOffset = drop.offset
                    attach(fl["c"] as? [Any] ?? [], to: pair.cast, dark: dark, clock: clock, alive: &alive, owner: id)
                    if pair.clip.superlayer !== layer { layer.addSublayer(pair.clip) }
                } else if let fl = e["fl"] as? [String: Any] {
                    if let pair = shadows.removeValue(forKey: id) { pair.clip.removeFromSuperlayer() }
                    var h = Hasher()
                    CssAnimations.digest(fl, into: &h)
                    h.combine(scale); h.combine(dark)
                    let key = h.finalize()
                    let k = CGFloat(num(fl["k"])) * scale
                    let picture = pictures[id].flatMap { $0.key == key ? $0.layer : nil }
                        ?? follow(id, fl, k: k, dark: dark, clock: clock)
                        ?? SvgIsland.filter(fl, k: k, seen: seen(fl), dark: dark, fonts: fonts ?? SvgScene.systemFonts)
                    if pictures[id]?.layer !== picture { pictures[id]?.layer.removeFromSuperlayer() }
                    pictures[id] = (key, picture)
                    if picture.superlayer !== layer { layer.addSublayer(picture) }
                } else {
                    if let old = pictures.removeValue(forKey: id) { old.layer.removeFromSuperlayer(); forget(id) }
                    if let pair = shadows.removeValue(forKey: id) { pair.clip.removeFromSuperlayer() }
                }
            } else if let shape = layer as? CAShapeLayer, drew(id, e, dark: dark) {
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
                if SvgPaint.needsParts(e) {
                    // Gradients or a paint order: part layers paint it.
                    shape.fillColor = nil; shape.strokeColor = nil
                    SvgPaint.parts(shape, e, scale: scale, dark: dark, fonts: fonts ?? SvgScene.systemFonts) { color($0, dark: $1) }
                } else if shape.sublayers?.isEmpty == false {
                    shape.sublayers?.forEach { $0.removeFromSuperlayer() }
                }
            }
            if let shape = layer as? CAShapeLayer {
                // A non-scaling stroke: the path is in the content box's
                // space, and the layer undoes its parents' transforms.
                shape.setAffineTransform(affine(e["inv"]) ?? .identity)
            }
            // @ref LLP 1055.000 D10 — a clip is the layer's mask; a mask
            // is an island inside it.
            layer.mask = SvgPaint.clip(e["cl"]) { path($0) }
            if let mk = e["mk"] as? [String: Any] {
                var h = Hasher()
                CssAnimations.digest(mk, into: &h)
                h.combine(scale); h.combine(dark)
                let key = h.finalize()
                let m = islands[id].flatMap { $0.key == key ? $0.layer : nil }
                    ?? SvgIsland.mask(mk, k: CGFloat(num(mk["k"])) * scale, seen: seen(mk), dark: dark, fonts: fonts ?? SvgScene.systemFonts)
                islands[id] = (key, m)
                if var inner = layer.mask {
                    while let next = inner.mask { inner = next }
                    inner.mask = m
                } else {
                    layer.mask = m
                }
            } else {
                islands.removeValue(forKey: id)
            }
            node[ObjectIdentifier(layer)] = (id, UInt32(num(e["n"])))
            let list = e["a"] as? [[String: Any]] ?? []
            specs[id] = list
            CssAnimations.apply(list, to: layer, clock: clock, installed: &installed[id, default: [:]], offscreen: offscreen)
            let placed = wrap(id, layer, e["tf"] as? [String: Any], clock: clock)
            SvgIsland.blend(placed, mode: Int(num(e["bl"])), isolate: e["iso"] != nil, scale: scale)
            order.append(placed)
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
    /// Each view's last scene: a `light-dark()` paint is resolved when a
    /// scene is applied, so an appearance change applies it again
    /// (minesweeper F11, paint F10).
    private var payloads: [UInt32: [String: Any]] = [:]
    /// The session's fonts, for SVG text: set by the presenter.
    var fonts: SvgText.Fonts?
    private var boxSpecs: [UInt32: (layer: CALayer, specs: [[String: Any]])] = [:]
    private var boxInstalled: [UInt32: [String: String]] = [:]
    private var seeked: Double?

    /// A scene's root layer's name, by which its node re-asks its range (LLP 1100 D8).
    static let rootName = "exact-svg"

    func scene(_ id: UInt32, _ payload: [String: Any], layer: CALayer?, dark: Bool, clock: Double?, limit: String? = nil) {
        guard let layer else { return }
        let scene = scenes[id] ?? { let s = SvgScene(); scenes[id] = s; return s }()
        scene.root.name = Self.rootName
        if scene.root.superlayer !== layer { layer.addSublayer(scene.root) }
        scene.scale = max(1, layer.contentsScale)
        scene.fonts = fonts
        let spec = payload["scene"] as? [String: Any] ?? [:]
        payloads[id] = spec
        scene.apply(spec, dark: dark, clock: clock)
        scene.root.applyColorRange(limit: limit, deep: true)
    }

    /// `id`'s view changed appearance: its scene is applied again in the
    /// new one. Only what the appearance reaches redraws (`drew`, and the
    /// islands' and pictures' keys, hash it); animations keep running.
    func reappear(_ id: UInt32, dark: Bool, clock: Double?) {
        guard let scene = scenes[id], let spec = payloads[id] else { return }
        scene.apply(spec, dark: dark, clock: clock)
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

    /// The SVG element a press at `point` (in `id`'s view) goes to, if any
    /// takes it (LLP 1055.000 D17).
    func target(_ id: UInt32, at point: CGPoint) -> UInt32? { scenes[id]?.target(at: point) }

    func forget(_ id: UInt32) {
        if let scene = scenes.removeValue(forKey: id) { scene.reset(); scene.root.removeFromSuperlayer() }
        payloads.removeValue(forKey: id)
        if let entry = boxSpecs.removeValue(forKey: id) { CssAnimations.apply([], to: entry.layer, clock: nil, installed: &boxInstalled[id, default: [:]]) }
        boxInstalled.removeValue(forKey: id)
    }
}

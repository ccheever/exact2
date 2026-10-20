// What a batch's ops need that is a plain value, built on the owner thread
// as the batch is decoded (LLP 1072 stage 4): the SVG scene's paths and its
// animations' signatures and keyframe values, and a flat leaf's paint. Main
// applies them to layers; it no longer parses them from the batch's numbers.
import CoreGraphics
import Foundation
import QuartzCore

/// A path built off main, carried in an SVG element's payload under "p",
/// with the digest of the numbers it was built from: what the scene hashes
/// a shape's spec by (`SvgScene.drew`) in their place.
final class PreparedPath {
    let path: CGPath
    let digest: Int
    init(_ source: Any) {
        path = SvgGeometry.path(source)
        var h = Hasher()
        CssAnimations.digest(source, into: &h)
        digest = h.finalize()
    }
}

private func num(_ v: Any?) -> Double { (v as? NSNumber)?.doubleValue ?? 0 }

/// A CSS animation spec as `CssAnimations` plays it (LLP 1055 D7), read from
/// the batch's dictionary once, on the owner as the batch is decoded
/// (`SvgPrepare`), and kept for the agent clock's re-seek: kept as the
/// dictionary, each animated box held about 1 KB.
struct CssAnimation {
    let id: String
    /// The key path's name: a box's (`opacity`, `transform.scale`, …) or a
    /// shape's (`fillColor`, `path`, `r`).
    let key: String
    /// What `CssAnimations.apply` keeps an unchanged one by.
    let signature: Int
    /// The keyframe animation's values, key times and timing functions.
    let values: [Any], keyTimes: [NSNumber], timing: [CAMediaTimingFunction]?
    let duration: Double, repeatCount: Double, fill: Int, start: Double, delay: Double
    /// The local time an authored pause holds it at, in seconds.
    let held: Double?

    /// An SVG element's spec carries its reading under "#" (`SvgPrepare.spec`).
    init?(_ spec: [String: Any]) {
        if let read = spec["#"] as? CssAnimation { self = read; return }
        guard let id = spec["id"] as? String else { return nil }
        self.id = id
        key = spec["k"] as? String ?? ""
        var h = Hasher()
        CssAnimations.digest(spec, into: &h)
        signature = h.finalize()
        (values, keyTimes, timing) = CssAnimations.lowered(spec)
        duration = num(spec["d"]); repeatCount = num(spec["n"]); fill = Int(num(spec["fill"]))
        start = num(spec["s"]); delay = num(spec["dl"])
        held = (spec["h"] as? NSNumber)?.doubleValue
    }

    /// A list as an op or an element carries it: read, or dictionaries.
    static func list(_ value: Any?) -> [CssAnimation] {
        value as? [CssAnimation] ?? (value as? [[String: Any]] ?? []).compactMap(CssAnimation.init)
    }
}

extension FlatPaint {
    init?(_ style: NodeStyle) { self.init(style, colorChannels: { $0.channels(dark: $1) }) }
}

extension Batch {
    /// Build this batch's plain values; on the owner, as it is decoded.
    mutating func prepare() {
        for i in ops.indices {
            switch ops[i].op {
            case .create, .style:
                ops[i].flat = FlatPaint(ops[i].style)
            case .svg:
                if var scene = ops[i].payload["scene"] as? [String: Any] {
                    SvgPrepare.scene(&scene)
                    ops[i].payload["scene"] = scene
                }
            case .animations:
                if let specs = ops[i].payload["specs"] as? [[String: Any]] {
                    ops[i].payload["specs"] = specs.compactMap(CssAnimation.init)
                }
            default: break
            }
        }
    }
}

enum SvgPrepare {
    /// A scene's element tree, in place: each shape's path, each animation.
    static func scene(_ scene: inout [String: Any]) {
        if let elements = scene["els"] as? [Any] { scene["els"] = elements.map(element) }
    }

    static func element(_ value: Any) -> Any {
        guard var e = value as? [String: Any] else { return value }
        if let p = e["p"], !(p is PreparedPath) { e["p"] = PreparedPath(p) }
        if let children = e["c"] as? [Any] { e["c"] = children.map(element) }
        if let list = e["a"] as? [[String: Any]] { e["a"] = list.map(spec) }
        if var tf = e["tf"] as? [String: Any], let list = tf["a"] as? [[String: Any]] {
            tf["a"] = list.map(spec)
            e["tf"] = tf
        }
        return e
    }

    /// One CSS animation spec with its reading under "#": the scene's
    /// readers (`SvgFilterLive`) look at the element's dictionaries.
    static func spec(_ spec: [String: Any]) -> [String: Any] {
        guard spec["#"] == nil, let read = CssAnimation(spec) else { return spec }
        var out = spec
        out["#"] = read
        return out
    }
}

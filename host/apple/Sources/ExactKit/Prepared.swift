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

/// A CSS animation spec's lowered parts, carried under "#" in the spec: its
/// signature (what `CssAnimations.apply` keeps an unchanged one by), and the
/// keyframe animation's values, key times and timing functions.
final class PreparedAnimation {
    let signature: Int
    let values: [Any]
    let keyTimes: [NSNumber]
    let timing: [CAMediaTimingFunction]?
    init(signature: Int, values: [Any], keyTimes: [NSNumber], timing: [CAMediaTimingFunction]?) {
        self.signature = signature; self.values = values; self.keyTimes = keyTimes; self.timing = timing
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
                    ops[i].payload["specs"] = specs.map(SvgPrepare.spec)
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

    /// One CSS animation spec with its lowered parts under "#".
    static func spec(_ spec: [String: Any]) -> [String: Any] {
        guard spec["#"] == nil else { return spec }
        var out = spec
        var h = Hasher()
        CssAnimations.digest(spec, into: &h)
        let (values, keyTimes, timing) = CssAnimations.lowered(spec)
        out["#"] = PreparedAnimation(signature: h.finalize(), values: values, keyTimes: keyTimes, timing: timing)
        return out
    }
}

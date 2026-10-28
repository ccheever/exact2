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
    let track: LoweredTrack
    var values: NSArray { track.values }
    var keyTimes: [NSNumber] { track.keyTimes }
    var timing: [CAMediaTimingFunction]? { track.timing }
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
        track = LoweredTrack.shared(spec)
        duration = num(spec["d"]); repeatCount = num(spec["n"]); fill = Int(num(spec["fill"]))
        start = num(spec["s"]); delay = num(spec["dl"])
        held = (spec["h"] as? NSNumber)?.doubleValue
    }

    private init(_ base: CssAnimation, track: LoweredTrack, signature: Int) {
        (id, key, self.signature, self.track) = (base.id, base.key, signature, track)
        (duration, repeatCount, fill, start, delay, held) = (base.duration, base.repeatCount, base.fill, base.start, base.delay, base.held)
    }

    /// This spec with its colours as `grey` makes them, as a dimmed control's face plays its
    /// background (TintDimmingIOS.swift); a different spec to `CssAnimations.apply`
    func greyed(_ grey: (CGColor) -> CGColor) -> CssAnimation {
        let values = track.values.map { value -> Any in CFGetTypeID(value as CFTypeRef) == CGColor.typeID ? grey(value as! CGColor) : value }
        var h = Hasher()
        h.combine(signature); h.combine("greyed")
        return CssAnimation(self, track: LoweredTrack(values: NSArray(array: values), keyTimes: keyTimes, timing: timing), signature: h.finalize())
    }

    /// A list as an op or an element carries it: read, or dictionaries.
    static func list(_ value: Any?) -> [CssAnimation] {
        value as? [CssAnimation] ?? (value as? [[String: Any]] ?? []).compactMap(CssAnimation.init)
    }
}

/// A keyframe track lowered for Core Animation, shared by every animation
/// that plays it: boxes that play one rule from different delays (a step
/// row's lights) lowered it to equal arrays each, about 0.7 KB a box. The
/// values are an `NSArray`, which each `CAKeyframeAnimation` keeps as it is;
/// a Swift `[Any]` is bridged to a new array for each.
final class LoweredTrack {
    let values: NSArray, keyTimes: [NSNumber], timing: [CAMediaTimingFunction]?

    private init(_ spec: [String: Any]) {
        let (values, keyTimes, timing) = CssAnimations.lowered(spec)
        (self.values, self.keyTimes, self.timing) = (NSArray(array: values), keyTimes, timing)
    }

    /// A track of its own, not shared: a greyed copy of one (`CssAnimation.greyed`)
    init(values: NSArray, keyTimes: [NSNumber], timing: [CAMediaTimingFunction]?) {
        (self.values, self.keyTimes, self.timing) = (values, keyTimes, timing)
    }

    /// What lowering reads: the key path, values, key times and curves.
    private struct Key: Hashable {
        let parts: [Any?]
        let hash: Int
        init(_ spec: [String: Any]) {
            parts = [spec["k"], spec["v"], spec["t"], spec["c"]]
            var h = Hasher()
            for part in parts { CssAnimations.digest(part, into: &h) }
            hash = h.finalize()
        }
        func hash(into h: inout Hasher) { h.combine(hash) }
        static func == (a: Key, b: Key) -> Bool { a.hash == b.hash && zip(a.parts, b.parts).allSatisfy(same) }
    }
    private struct Held { weak var track: LoweredTrack? }
    private static let lock = NSLock()
    nonisolated(unsafe) private static var held: [Key: Held] = [:]
    /// Entries after the last sweep; the next comes at twice this.
    nonisolated(unsafe) private static var swept = 0

    /// The track `spec` lowers to: the one a live animation holds, or new.
    static func shared(_ spec: [String: Any]) -> LoweredTrack {
        let key = Key(spec)
        lock.lock(); defer { lock.unlock() }
        if let track = held[key]?.track { return track }
        let track = LoweredTrack(spec)
        held[key] = Held(track: track)
        if held.count > 2 * swept + 64 {
            held = held.filter { $0.value.track != nil }
            swept = held.count
        }
        return track
    }

    /// Equal as lowering reads them: numbers by value, strings, lists of them.
    private static func same(_ a: Any?, _ b: Any?) -> Bool {
        switch (a, b) {
        case (.none, .none): return true
        case let (x as NSNumber, y as NSNumber): return x.doubleValue.bitPattern == y.doubleValue.bitPattern
        case let (x as String, y as String): return x == y
        case let (x as [Any], y as [Any]): return x.count == y.count && zip(x, y).allSatisfy(same)
        default: return false
        }
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

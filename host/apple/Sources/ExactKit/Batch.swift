// @ref LLP 1044.000 §6 S8 — decode the existing Apple JSON wire once, before
// presentation. Style names/values still come from kernel/tables/schema.json;
// this is a consumer of that wire, not another declaration table.
import Foundation
import CoreGraphics

/// JSON values for style rows and the heterogeneous capability payloads. No
/// Objective-C containers or conditional bridges enter ordinary presentation.
package enum BatchValue: Equatable {
    case number(Double), string(String), profile(ProfileSpaces.Handle), bool(Bool), array([BatchValue]), object([String: BatchValue]), null

    package var number: Double? { if case .number(let n) = self { return n }; return nil }
    /// The value as text, for a cache key: different values never have the
    /// same text.
    var key: String {
        switch self {
        case .number(let n): return "\(n)"
        case .string(let s): return s.debugDescription
        case .profile(let p): return "@" + p.key.debugDescription
        case .bool(let b): return b ? "true" : "false"
        case .array(let a): return "[" + a.map(\.key).joined(separator: ",") + "]"
        case .object(let o): return "{" + o.keys.sorted().map { $0.debugDescription + ":" + o[$0]!.key }.joined(separator: ",") + "}"
        case .null: return "null"
        }
    }
    package var string: String? {
        if case .string(let s) = self { return s }
        if case .profile(let p) = self { return p.key }
        return nil
    }
    var profileSpace: CGColorSpace? { if case .profile(let p) = self { return p.space }; return nil }
    var array: [BatchValue]? { if case .array(let a) = self { return a }; return nil }
    var numbers: [Double]? {
        guard let a = array else { return nil }
        var result: [Double] = []; result.reserveCapacity(a.count)
        for value in a { guard let n = value.number else { return nil }; result.append(n) }
        return result
    }
    /// A colour row naming a platform colour (LLP 1095 D1): `{"sys", "c"}`.
    var isSystemColor: Bool {
        if case .object(let o) = self { return o["sys"]?.string != nil }
        return false
    }
    /// Whether a platform colour is anywhere in the value: a palette's, a
    /// shadow's.
    var containsSystemColor: Bool {
        switch self {
        case .object(let o): return o["sys"]?.string != nil || o.values.contains { $0.containsSystemColor }
        case .array(let a): return a.contains { $0.containsSystemColor }
        default: return false
        }
    }
    var isSchemeColor: Bool {
        if case .object(let o) = self { return o["sys"] != nil || o["cs"]?.array?.count == 2 }
        return array?.count == 2 && array?.first?.numbers?.count == 4 && array?.last?.numbers?.count == 4
    }
    package func channels(dark: Bool, contrast: Bool? = nil, elevated: Bool = false, tint: PlatformColor? = nil) -> [Double]? {
        if let c = numbers, c.count == 4 { return c }
        // @ref LLP 1095 D5 — a platform colour by name, its pair the fallback.
        if case .object(let o) = self, let name = o["sys"]?.string {
            return SystemColor.channels(name, dark: dark, contrast: contrast, elevated: elevated, tintColor: tint, fallback: o["c"]?.channels(dark: dark))
        }
        // A colour in its own space (LLP 1100 D2): its sRGB clip; a profile's
        // colour has none on the wire, so Core Graphics converts it (D3).
        if case .object(let o) = self, o["cs"] != nil {
            if let c = o["c"] { return c.channels(dark: dark) }
            guard let cg = cgColor(dark: dark), let srgb = CGColorSpace(name: CGColorSpace.sRGB),
                  let c = cg.converted(to: srgb, intent: .relativeColorimetric, options: nil)?.components, c.count == 4 else { return nil }
            return c.map { (Double($0) * 255).rounded() }
        }
        guard let a = array, a.count == 2, let c = a[dark ? 1 : 0].numbers, c.count == 4 else { return nil }
        return c
    }
    /// A colour in its own space (LLP 1100 D2): `{"cs": …, "c": …}`.
    var isWideColor: Bool {
        if case .object(let o) = self { return o["cs"] != nil }
        return false
    }
    /// Text's channels (LLP 1100 D2): the sRGB four, then for a colour in its
    /// own space its space (0 sRGB, 1 Display P3, 2 linear sRGB) and its four
    /// components, which `TextEngine.color` reads.
    func textChannels(dark: Bool, contrast: Bool? = nil, elevated: Bool = false, tint: PlatformColor? = nil) -> [Double]? {
        guard let c = channels(dark: dark, contrast: contrast, elevated: elevated, tint: tint) else { return nil }
        guard case .object(let o) = self, let halves = o["cs"]?.array, !halves.isEmpty,
              case .object(let half) = halves[dark && halves.count > 1 ? 1 : 0] else { return c }
        if let code = ["srgb": 0.0, "display-p3": 1, "srgb-linear": 2][half["s"]?.string ?? ""],
           let v = half["v"]?.numbers, v.count == 4 { return c + [code] + v }
        // A profile's colour: as extended sRGB (LLP 1100 D3).
        guard let cg = cgColor(dark: dark), let space = CGColorSpace(name: CGColorSpace.extendedSRGB),
              let e = cg.converted(to: space, intent: .relativeColorimetric, options: nil)?.components, e.count == 4 else { return c }
        return c + [0] + e.map { Double($0) }
    }
    /// The colour as Core Graphics draws it: in its own space, unclipped
    /// (LLP 1100 D2), else from its sRGB channels.
    func cgColor(dark: Bool, contrast: Bool? = nil, elevated: Bool = false, tint: PlatformColor? = nil) -> CGColor? {
        if case .object(let o) = self, let halves = o["cs"]?.array, !halves.isEmpty,
           case .object(let half) = halves[dark && halves.count > 1 ? 1 : 0],
           let name = half["s"]?.string, let v = half["v"]?.numbers {
            // @ref LLP 1100 D3
            if let space = half["s"]?.profileSpace ?? ProfileSpaces.space(name) {
                guard v.count == space.numberOfComponents + 1 else { return nil }
                guard let color = CGColor(colorSpace: space, components: v.map { CGFloat($0) }) else { return nil }
                // CGColor carries no rendering intent. Apply the authored ICC
                // transform once, before the color enters any paint path.
                if name.hasPrefix("icc-sha256:"), let target = CGColorSpace(name: CGColorSpace.extendedSRGB) {
                    return color.converted(to: target, intent: ProfileSpaces.intent(half["i"]?.string), options: nil).map(ColorRange.tagged)
                }
                return ColorRange.tagged(color)
            }
            guard v.count == 4, let spaceName = Self.wideSpaces[name], let space = CGColorSpace(name: spaceName) else { return nil }
            return CGColor(colorSpace: space, components: v.map { CGFloat($0) }).map(ColorRange.tagged)
        }
        guard let c = channels(dark: dark, contrast: contrast, elevated: elevated, tint: tint) else { return nil }
        return CGColor(srgbRed: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
    }
    /// The spaces a wide colour crosses the wire in, in their extended forms;
    /// Rust (`push_wide`) sends any other as extended linear sRGB.
    private static let wideSpaces: [String: CFString] = [
        "srgb": CGColorSpace.extendedSRGB, "display-p3": CGColorSpace.extendedDisplayP3,
        "srgb-linear": CGColorSpace.extendedLinearSRGB]
    /// Only capability/region/collection adapters still take heterogeneous data.
    var any: Any {
        switch self {
        case .number(let n): return NSNumber(value: n)
        case .string(let s): return s
        case .profile(let p): return p
        case .bool(let b): return b
        case .array(let a): return a.map(\.any)
        case .object(let o): return o.mapValues(\.any)
        case .null: return NSNull()
        }
    }
}

// Typed literals also make hand-authored style values usable by host embedders.
extension BatchValue: ExpressibleByIntegerLiteral, ExpressibleByFloatLiteral, ExpressibleByStringLiteral, ExpressibleByBooleanLiteral, ExpressibleByArrayLiteral, ExpressibleByDictionaryLiteral {
    package init(integerLiteral value: Int) { self = .number(Double(value)) }
    package init(floatLiteral value: Double) { self = .number(value) }
    package init(stringLiteral value: String) { self = .string(value) }
    package init(booleanLiteral value: Bool) { self = .bool(value) }
    package init(arrayLiteral elements: BatchValue...) { self = .array(elements) }
    package init(dictionaryLiteral elements: (String, BatchValue)...) { self = .object(Dictionary(uniqueKeysWithValues: elements)) }
}

package typealias NodeStyle = [String: BatchValue]

public struct BatchOp {
    package enum Kind: String {
        case create, props, style, children, paragraph, frame, content, present, roots, destroy
        case fieldContent, flow, surface, surfaceWork, command, hold, collections, region, router, title, language, unknown
        case auth // LLP 1069.006 D3: open or cancel an authentication session
        case svg, animations // LLP 1055 D4/D7: an `svg`'s scene; a view's CSS animations
        case canvas2d // LLP 1056 D7: a 2D canvas's stamped lists
        case heightDrag = "height-drag", transformDrag = "transform-drag", retireMotion = "retire-motion", reorder, exit
        case flight, land // LLP 1013.000 D4: a shared element's flight, and its end
        case rank // LLP 1083.000: twice the sibling paint rank
        case sticky // LLP 1083: a sticky box's constraint, or none
        case fragments // LLP 1093 D7: a box's column fragments or a container's columns
        case sound // LLP 1096 D8: the voice table's ops, and a boot's files
    }
    package let op: Kind
    var nodeID: UInt32?
    var id: UInt32 { nodeID ?? 0 }
    var kind = "view"
    var props: [String: String] = [:]
    var clear: [String] = []
    var style: NodeStyle = [:]
    var handlers: Set<String> = []
    var ids: [UInt32] = []
    var runs: [InlineText] = []
    var x = 0.0, y = 0.0, w = 0.0, h = 0.0
    var property = ""
    // Rare adapters retain their existing input shape. Common ops never build it.
    package var payload: [String: Any] = [:]
    /// A create or style op's flat-leaf paint, read on the owner (`prepare`).
    var flat: FlatPaint?

    init(op: Kind, nodeID: UInt32? = nil) {
        self.op = op; self.nodeID = nodeID
    }
}

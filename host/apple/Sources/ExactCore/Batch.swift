// @ref LLP 1044.000 §6 S8 — decode the existing Apple JSON wire once, before
// presentation. Style names/values still come from kernel/tables/schema.json;
// this is a consumer of that wire, not another declaration table.
import Foundation
import CoreGraphics

/// JSON values for style rows and the heterogeneous capability payloads. No
/// Objective-C containers or conditional bridges enter ordinary presentation.
public enum BatchValue: Equatable {
    case number(Double), string(String), profile(ProfileSpaces.Handle), calculation(CalculatedLength), bool(Bool), array([BatchValue]), object([String: BatchValue]), null

    public var number: Double? { if case .number(let n) = self { return n }; return nil }
    /// The value as text, for a cache key: different values never have the
    /// same text.
    public var key: String {
        switch self {
        case .number(let n): return "\(n)"
        case .string(let s): return s.debugDescription
        case .profile(let p): return "@" + p.key.debugDescription
        case .calculation(let c): return c.value.key
        case .bool(let b): return b ? "true" : "false"
        case .array(let a): return "[" + a.map(\.key).joined(separator: ",") + "]"
        case .object(let o): return "{" + o.keys.sorted().map { $0.debugDescription + ":" + o[$0]!.key }.joined(separator: ",") + "}"
        case .null: return "null"
        }
    }
    public var string: String? {
        if case .string(let s) = self { return s }
        if case .profile(let p) = self { return p.key }
        return nil
    }
    public var profileSpace: CGColorSpace? { if case .profile(let p) = self { return p.space }; return nil }
    public var array: [BatchValue]? { if case .array(let a) = self { return a }; return nil }
    /// Resolve points, mixed percentages, or a calculation with its saved
    /// environment against the host's basis (LLP 1001 §2).
    public func length(basis: CGFloat) -> CGFloat? {
        if let n = number { return CGFloat(n) }
        if case .calculation(let c) = self { return c.length(basis: basis) }
        guard case .object(let o) = self else { return nil }
        return CGFloat(o["pct"]?.number ?? 0) * basis / 100 + CGFloat(o["px"]?.number ?? 0)
    }
    public var numbers: [Double]? {
        guard let a = array else { return nil }
        var result: [Double] = []; result.reserveCapacity(a.count)
        for value in a { guard let n = value.number else { return nil }; result.append(n) }
        return result
    }
    /// A colour row naming a platform colour (LLP 1095 D1): `{"sys", "c"}`.
    public var isSystemColor: Bool {
        if case .object(let o) = self { return o["sys"]?.string != nil }
        return false
    }
    /// Whether a platform colour is anywhere in the value: a palette's, a
    /// shadow's.
    public var containsSystemColor: Bool {
        switch self {
        case .object(let o): return o["sys"]?.string != nil || o.values.contains { $0.containsSystemColor }
        case .array(let a): return a.contains { $0.containsSystemColor }
        default: return false
        }
    }
    /// A colour its author fixed: a literal, a `light-dark()` pair, one in
    /// its own space, or one from the app's asset catalogue (`named:`); not
    /// a system colour by name (a role, `AccentColor`, one a
    /// `-exact-platform-color()` names). What selected text keeps on the
    /// Mac, where a system colour's turns (LLP 1115.000 D7.9).
    public var fixesInk: Bool {
        guard case .object(let o) = self, let name = o["sys"]?.string else { return true }
        return name.hasPrefix("named:")
    }
    public var isSchemeColor: Bool {
        if case .object(let o) = self { return o["sys"] != nil || o["cs"]?.array?.count == 2 }
        return array?.count == 2 && array?.first?.numbers?.count == 4 && array?.last?.numbers?.count == 4
    }
    /// A colour in its own space (LLP 1100 D2): `{"cs": …, "c": …}`.
    public var isWideColor: Bool {
        if case .object(let o) = self { return o["cs"] != nil }
        return false
    }
    /// Only capability/region/collection adapters still take heterogeneous data.
    package var any: Any {
        switch self {
        case .number(let n): return NSNumber(value: n)
        case .string(let s): return s
        case .profile(let p): return p
        case .calculation(let c): return c.value.any
        case .bool(let b): return b
        case .array(let a): return a.map(\.any)
        case .object(let o): return o.mapValues(\.any)
        case .null: return NSNull()
        }
    }
}

// Typed literals also make hand-authored style values usable by host embedders.
extension BatchValue: ExpressibleByIntegerLiteral, ExpressibleByFloatLiteral, ExpressibleByStringLiteral, ExpressibleByBooleanLiteral, ExpressibleByArrayLiteral, ExpressibleByDictionaryLiteral {
    public init(integerLiteral value: Int) { self = .number(Double(value)) }
    public init(floatLiteral value: Double) { self = .number(value) }
    public init(stringLiteral value: String) { self = .string(value) }
    public init(booleanLiteral value: Bool) { self = .bool(value) }
    public init(arrayLiteral elements: BatchValue...) { self = .array(elements) }
    public init(dictionaryLiteral elements: (String, BatchValue)...) { self = .object(Dictionary(uniqueKeysWithValues: elements)) }
}

public typealias NodeStyle = [String: BatchValue]

public struct BatchOp {
    package enum Kind: String {
        case create, props, style, children, paragraph, frame, content, present, roots, destroy
        case fieldContent, flow, surface, surfaceWork, command, hold, collections, region, router, title, language, unknown
        case activity
        case auth // LLP 1069.006 D3: open or cancel an authentication session
        case svg, animations // LLP 1055 D4/D7: an `svg`'s scene; a view's CSS animations
        case canvas2d // LLP 1056 D7: a 2D canvas's stamped lists
        case heightDrag = "height-drag", transformDrag = "transform-drag", retireMotion = "retire-motion", reorder, exit
        case flight, land // LLP 1013.000 D4: a shared element's flight, and its end
        case rank // LLP 1083.000: twice the sibling paint rank
        case sticky // LLP 1083: a sticky box's constraint, or none
        case fragments // LLP 1093 D7: a box's column fragments or a container's columns
        case sound // LLP 1096 D8: the voice table's ops, and a boot's files
        case roles // LLP 1095 D4: the roles the app's theme replaces
        case midi // LLP 1125 D7: the MIDI queue's sends, retractions, inputs to open and a request
    }
    package let op: Kind
    package var nodeID: UInt32?
    package var id: UInt32 { nodeID ?? 0 }
    package var kind = "view"
    package var props: [String: String] = [:]
    package var clear: [String] = []
    package var style: NodeStyle = [:]
    package var handlers: Set<String> = []
    package var ids: [UInt32] = []
    package var runs: [InlineText] = []
    package var x = 0.0, y = 0.0, w = 0.0, h = 0.0
    package var property = ""
    // Rare adapters retain their existing input shape. Common ops never build it.
    package var payload: [String: Any] = [:]
    /// A create or style op's flat-leaf paint, read on the owner (`prepare`).
    package var flat: FlatPaint?

    package init(op: Kind, nodeID: UInt32? = nil) {
        self.op = op; self.nodeID = nodeID
    }
}

extension BatchValue {
    /// A headless snapshot preserves colour roles; validation uses their baked fallback.
    package func fallbackChannels(dark: Bool) -> [Double]? {
        if let c = numbers, c.count == 4 { return c }
        if case .object(let o) = self, let c = o["c"] { return c.fallbackChannels(dark: dark) }
        if let a = array, a.count == 2, let c = a[dark ? 1 : 0].numbers, c.count == 4 { return c }
        if case .object(let o) = self, let halves = o["cs"]?.array, !halves.isEmpty,
           case .object(let half) = halves[dark && halves.count > 1 ? 1 : 0],
           let name = half["s"]?.string, let v = half["v"]?.numbers,
           let space = half["s"]?.profileSpace ?? ProfileSpaces.space(name),
           v.count == space.numberOfComponents + 1,
           let color = CGColor(colorSpace: space, components: v.map { CGFloat($0) }),
           let srgb = CGColorSpace(name: CGColorSpace.sRGB),
           let c = color.converted(to: srgb, intent: ProfileSpaces.intent(half["i"]?.string), options: nil)?.components,
           c.count == 4 { return c.map { (Double($0) * 255).rounded() } }
        return nil
    }
}

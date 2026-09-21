// @ref LLP 1044.000 §6 S8 — decode the existing Apple JSON wire once, before
// presentation. Style names/values still come from kernel/tables/schema.json;
// this is a consumer of that wire, not another declaration table.
import Foundation

/// JSON values for style rows and the heterogeneous capability payloads. No
/// Objective-C containers or conditional bridges enter ordinary presentation.
enum BatchValue: Decodable {
    case number(Double), string(String), bool(Bool), array([BatchValue]), object([String: BatchValue]), null

    init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer()
        if value.decodeNil() { self = .null }
        else if let n = try? value.decode(Double.self) { self = .number(n) }
        else if let s = try? value.decode(String.self) { self = .string(s) }
        else if let b = try? value.decode(Bool.self) { self = .bool(b) }
        else if let a = try? value.decode([BatchValue].self) { self = .array(a) }
        else { self = .object(try value.decode([String: BatchValue].self)) }
    }
    var number: Double? { if case .number(let n) = self { return n }; return nil }
    var string: String? { if case .string(let s) = self { return s }; return nil }
    var array: [BatchValue]? { if case .array(let a) = self { return a }; return nil }
    var numbers: [Double]? {
        guard let a = array else { return nil }
        var result: [Double] = []; result.reserveCapacity(a.count)
        for value in a { guard let n = value.number else { return nil }; result.append(n) }
        return result
    }
    var isSchemeColor: Bool { array?.count == 2 && array?.first?.numbers?.count == 4 && array?.last?.numbers?.count == 4 }
    func channels(dark: Bool) -> [Double]? {
        if let c = numbers, c.count == 4 { return c }
        guard let a = array, a.count == 2, let c = a[dark ? 1 : 0].numbers, c.count == 4 else { return nil }
        return c
    }
    /// Only capability/region/collection adapters still take heterogeneous data.
    var any: Any {
        switch self {
        case .number(let n): return NSNumber(value: n)
        case .string(let s): return s
        case .bool(let b): return b
        case .array(let a): return a.map(\.any)
        case .object(let o): return o.mapValues(\.any)
        case .null: return NSNull()
        }
    }
}

// Typed literals also make hand-authored style values usable by host embedders.
extension BatchValue: ExpressibleByIntegerLiteral, ExpressibleByFloatLiteral, ExpressibleByStringLiteral, ExpressibleByBooleanLiteral, ExpressibleByArrayLiteral, ExpressibleByDictionaryLiteral {
    init(integerLiteral value: Int) { self = .number(Double(value)) }
    init(floatLiteral value: Double) { self = .number(value) }
    init(stringLiteral value: String) { self = .string(value) }
    init(booleanLiteral value: Bool) { self = .bool(value) }
    init(arrayLiteral elements: BatchValue...) { self = .array(elements) }
    init(dictionaryLiteral elements: (String, BatchValue)...) { self = .object(Dictionary(uniqueKeysWithValues: elements)) }
}

typealias NodeStyle = [String: BatchValue]

public struct BatchOp: Decodable {
    enum Kind: String {
        case create, props, style, children, paragraph, frame, content, present, roots, destroy
        case flow, surface, command, hold, collections, region, router, unknown
        case heightDrag = "height-drag", transformDrag = "transform-drag", retireMotion = "retire-motion"
    }
    let op: Kind
    let nodeID: UInt32?
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
    var payload: [String: Any] = [:]

    enum CodingKeys: String, CodingKey {
        case op, id, kind, props, set, clear, style, handlers, ids, runs, x, y, w, h, property
    }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        op = Kind(rawValue: try c.decode(String.self, forKey: .op)) ?? .unknown
        nodeID = try c.decodeIfPresent(UInt32.self, forKey: .id)
        switch op {
        case .create:
            kind = try c.decodeIfPresent(String.self, forKey: .kind) ?? "view"
            props = try c.decodeIfPresent([String: String].self, forKey: .props) ?? [:]
            style = try c.decodeIfPresent(NodeStyle.self, forKey: .style) ?? [:]
            handlers = try c.decodeIfPresent(Set<String>.self, forKey: .handlers) ?? []
        case .props:
            props = try c.decodeIfPresent([String: String].self, forKey: .set) ?? [:]
            clear = try c.decodeIfPresent([String].self, forKey: .clear) ?? []
        case .style: style = try c.decodeIfPresent(NodeStyle.self, forKey: .style) ?? [:]
        case .paragraph: runs = try c.decodeIfPresent([InlineText].self, forKey: .runs) ?? []
        case .children, .roots: ids = try c.decodeIfPresent([UInt32].self, forKey: .ids) ?? []
        case .frame, .content, .present:
            x = try c.decodeIfPresent(Double.self, forKey: .x) ?? 0
            y = try c.decodeIfPresent(Double.self, forKey: .y) ?? 0
            w = try c.decodeIfPresent(Double.self, forKey: .w) ?? 0
            h = try c.decodeIfPresent(Double.self, forKey: .h) ?? 0
            property = try c.decodeIfPresent(String.self, forKey: .property) ?? ""
        case .destroy: break
        default: payload = try [String: BatchValue](from: decoder).mapValues(\.any)
        }
    }
}

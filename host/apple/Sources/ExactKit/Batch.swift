// @ref LLP 1044.000 §6 S8 — decode the existing Apple JSON wire once, before
// presentation. Style names/values still come from kernel/tables/schema.json;
// this is a consumer of that wire, not another declaration table.
import Foundation

/// JSON values for style rows and the heterogeneous capability payloads. No
/// Objective-C containers or conditional bridges enter ordinary presentation.
enum BatchValue: Equatable {
    case number(Double), string(String), bool(Bool), array([BatchValue]), object([String: BatchValue]), null

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

public struct BatchOp {
    enum Kind: String {
        case create, props, style, children, paragraph, frame, content, present, roots, destroy
        case flow, surface, surfaceWork, command, hold, collections, region, router, title, language, unknown
        case auth // LLP 1069.006 D3: open or cancel an authentication session
        case svg, animations // LLP 1055 D4/D7: an `svg`'s scene; a view's CSS animations
        case canvas2d // LLP 1056 D7: a 2D canvas's stamped lists
        case heightDrag = "height-drag", transformDrag = "transform-drag", retireMotion = "retire-motion", reorder, exit
        case flight, land // LLP 1013.000 D4: a shared element's flight, and its end
        case sticky // LLP 1083: a sticky box's constraint, or none
    }
    let op: Kind
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
    var payload: [String: Any] = [:]
    /// A create or style op's flat-leaf paint, read on the owner (`prepare`).
    var flat: FlatPaint?

    init(op: Kind, nodeID: UInt32? = nil) {
        self.op = op; self.nodeID = nodeID
    }
}

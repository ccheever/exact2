import Foundation
import CoreText
// Frozen pre-S13 decoder: independent Foundation oracle, never linked into the app.
@testable import ExactKit
// @ref LLP 1044.000 §6 S8 — decode the existing Apple JSON wire once, before
// presentation. Style names/values still come from kernel/tables/schema.json;
// this is a consumer of that wire, not another declaration table.

/// JSON values for style rows and the heterogeneous capability payloads. No
/// Objective-C containers or conditional bridges enter ordinary presentation.
enum OracleBatchValue: Decodable {
    case number(Double), string(String), bool(Bool), array([OracleBatchValue]), object([String: OracleBatchValue]), null

    init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer()
        if value.decodeNil() { self = .null }
        else if let n = try? value.decode(Double.self) { self = .number(n) }
        else if let s = try? value.decode(String.self) { self = .string(s) }
        else if let b = try? value.decode(Bool.self) { self = .bool(b) }
        else if let a = try? value.decode([OracleBatchValue].self) { self = .array(a) }
        else { self = .object(try value.decode([String: OracleBatchValue].self)) }
    }
    var number: Double? { if case .number(let n) = self { return n }; return nil }
    var string: String? { if case .string(let s) = self { return s }; return nil }
    var array: [OracleBatchValue]? { if case .array(let a) = self { return a }; return nil }
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

typealias OracleNodeStyle = [String: OracleBatchValue]

struct OracleBatchOp: Decodable {
    enum Kind: String {
        case create, props, style, children, paragraph, frame, content, present, roots, destroy
        case flow, surface, command, hold, collections, region, router, title, language, unknown
        case svg, animations, canvas2d, reorder, exit, rank
        case heightDrag = "height-drag", transformDrag = "transform-drag", retireMotion = "retire-motion"
    }
    let op: Kind
    let nodeID: UInt32?
    var id: UInt32 { nodeID ?? 0 }
    var kind = "view"
    var props: [String: String] = [:]
    var clear: [String] = []
    var style: OracleNodeStyle = [:]
    var handlers: Set<String> = []
    var ids: [UInt32] = []
    var runs: [OracleInlineText] = []
    var x = 0.0, y = 0.0, w = 0.0, h = 0.0
    var property = ""
    // Rare adapters retain their existing input shape. Common ops never build it.
    var payload: [String: Any] = [:]

    enum CodingKeys: String, CodingKey {
        case op, id, kind, props, set, clear, style, handlers, ids, runs, x, y, w, h, property
    }
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        op = Kind(rawValue: try c.decode(String.self, forKey: .op)) ?? .unknown
        nodeID = try c.decodeIfPresent(UInt32.self, forKey: .id)
        switch op {
        case .create:
            kind = try c.decodeIfPresent(String.self, forKey: .kind) ?? "view"
            props = try c.decodeIfPresent([String: String].self, forKey: .props) ?? [:]
            style = try c.decodeIfPresent(OracleNodeStyle.self, forKey: .style) ?? [:]
            handlers = try c.decodeIfPresent(Set<String>.self, forKey: .handlers) ?? []
        case .props:
            props = try c.decodeIfPresent([String: String].self, forKey: .set) ?? [:]
            clear = try c.decodeIfPresent([String].self, forKey: .clear) ?? []
        case .style: style = try c.decodeIfPresent(OracleNodeStyle.self, forKey: .style) ?? [:]
        case .paragraph: runs = try c.decodeIfPresent([OracleInlineText].self, forKey: .runs) ?? []
        case .children, .roots: ids = try c.decodeIfPresent([UInt32].self, forKey: .ids) ?? []
        case .frame, .content, .present:
            x = try c.decodeIfPresent(Double.self, forKey: .x) ?? 0
            y = try c.decodeIfPresent(Double.self, forKey: .y) ?? 0
            w = try c.decodeIfPresent(Double.self, forKey: .w) ?? 0
            h = try c.decodeIfPresent(Double.self, forKey: .h) ?? 0
            property = try c.decodeIfPresent(String.self, forKey: .property) ?? ""
        case .destroy: break
        default: payload = try [String: OracleBatchValue](from: decoder).mapValues(\.any)
        }
    }
}
struct OracleBatch: Decodable {
    let ops: [OracleBatchOp]
    let timers: Bool
    let motion: Bool
    /// The runner's clock after the call, milliseconds (LLP 1012 `clock`).
    let clock: Double?
    let error: String?
    /// @ref LLP 1043.000 §3 D8 — absolute runner deadline, absent without timers.
    var timerDueMs: Double? = nil
    var pending = false
    enum CodingKeys: String, CodingKey {
        case ops, timers, motion, clock, error, pending
        case timerDueMs = "timer_due_ms"
    }
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        ops = try c.decodeIfPresent([OracleBatchOp].self, forKey: .ops) ?? []
        timers = try c.decodeIfPresent(Bool.self, forKey: .timers) ?? false
        motion = try c.decodeIfPresent(Bool.self, forKey: .motion) ?? false
        clock = try c.decodeIfPresent(Double.self, forKey: .clock)
        error = try c.decodeIfPresent(String.self, forKey: .error)
        timerDueMs = try c.decodeIfPresent(Double.self, forKey: .timerDueMs)
        pending = try c.decodeIfPresent(Bool.self, forKey: .pending) ?? false
    }
    init(ops: [OracleBatchOp], timers: Bool, motion: Bool, clock: Double?, error: String?, timerDueMs: Double? = nil, pending: Bool = false) {
        self.ops = ops; self.timers = timers; self.motion = motion; self.clock = clock
        self.error = error; self.timerDueMs = timerDueMs; self.pending = pending
    }
    static func decode(_ data: Data) -> OracleBatch {
        (try? JSONDecoder().decode(OracleBatch.self, from: data))
            ?? OracleBatch(ops: [], timers: false, motion: false, clock: nil, error: "unreadable batch")
    }

}

// @ref LLP 1044.000 §6 S1 — textual descendants are paragraph values.

struct OracleInlineText: Decodable {
    let id: UInt32
    let parent: UInt32
    let props: [String: String]
    private let lightRun: Run
    private let darkColor: [Double]?
    let hasSchemeColor: Bool
    let handlers: Set<String>
    let paints: Bool
    var range: NSRange = NSRange(location: 0, length: 0)

    enum CodingKeys: String, CodingKey { case id, parent, props, style, handlers, paint }
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(UInt32.self, forKey: .id)
        parent = try c.decode(UInt32.self, forKey: .parent)
        props = try c.decodeIfPresent([String: String].self, forKey: .props) ?? [:]
        let style = try c.decodeIfPresent(OracleInlineStyle.self, forKey: .style) ?? OracleInlineStyle()
        var value = style.run
        value.text = props["text"] ?? ""
        value.href = props["href"] ?? ""
        lightRun = value
        darkColor = style.color?.dark
        hasSchemeColor = style.color?.paired ?? false
        handlers = try c.decodeIfPresent(Set<String>.self, forKey: .handlers) ?? []
        paints = try c.decodeIfPresent(Bool.self, forKey: .paint) ?? false
    }

    var text: String { lightRun.text }
    func run(dark: Bool) -> Run {
        var value = lightRun
        if dark { value.color = darkColor }
        return value
    }

    static func run(_ text: String, style: OracleNodeStyle, href: String = "", dark: Bool) -> Run {
        func number(_ key: String, _ fallback: Double = 0) -> Double { style[key]?.number ?? fallback }
        let size = Float(number("font_size", 16))
        let height: CGFloat?
        if let ratio = style["line_height"]?.number { height = CGFloat(Float(ratio) * size) }
        else if let px = style["line_height"]?.string, px.hasSuffix("px"), let value = Float(px.dropLast(2)) { height = CGFloat(value) }
        else { height = nil }
        return Run(text: text, size: CGFloat(size), weight: Int(number("font_weight", 400)),
                   family: Int(number("font_family")), italic: style["font_style"]?.string == "italic",
                   lineHeight: height, letterSpacing: CGFloat(Float(number("letter_spacing"))),
                   color: style["text_color"]?.channels(dark: dark),
                   decoration: style["text_decoration_line"]?.string ?? "", href: href)
    }
}

// Paragraphs read only text rows, directly into their resolved representation.
// Decoding every style entry through a generic JSON value spends the saved apply
// time in speculative type probes and temporary dictionaries. No wire change.
private struct OracleInlineStyle: Decodable {
    var run = Run(text: "", size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
    var color: OracleInlineColor?
    init() {}
    enum CodingKeys: String, CodingKey {
        case font_size, font_weight, font_family, font_style, line_height, letter_spacing, text_color, text_decoration_line
    }
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let size = Float(try c.decodeIfPresent(Double.self, forKey: .font_size) ?? 16)
        run.size = CGFloat(size)
        run.weight = Int(try c.decodeIfPresent(Double.self, forKey: .font_weight) ?? 400)
        run.family = Int(try c.decodeIfPresent(Double.self, forKey: .font_family) ?? 0)
        run.italic = try c.decodeIfPresent(String.self, forKey: .font_style) == "italic"
        if let height = try c.decodeIfPresent(OracleInlineLineHeight.self, forKey: .line_height) {
            run.lineHeight = height.pixels.map(CGFloat.init) ?? height.ratio.map { CGFloat($0 * size) }
        }
        run.letterSpacing = CGFloat(Float(try c.decodeIfPresent(Double.self, forKey: .letter_spacing) ?? 0))
        run.decoration = try c.decodeIfPresent(String.self, forKey: .text_decoration_line) ?? ""
        color = try c.decodeIfPresent(OracleInlineColor.self, forKey: .text_color)
        run.color = color?.light
    }
}

private struct OracleInlineLineHeight: Decodable {
    var pixels: Float?
    var ratio: Float?
    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if let n = try? c.decode(Double.self) { ratio = Float(n) }
        else {
            let text = try c.decode(String.self)
            if text.hasSuffix("px") { pixels = Float(text.dropLast(2)) }
        }
    }
}

private struct OracleInlineColor: Decodable {
    let light: [Double]
    let dark: [Double]
    let paired: Bool
    init(from decoder: Decoder) throws {
        var c = try decoder.unkeyedContainer()
        paired = c.count == 2
        if paired {
            light = try c.decode([Double].self)
            dark = try c.decode([Double].self)
        } else {
            light = try (0..<4).map { _ in try c.decode(Double.self) }
            dark = light
        }
        guard c.isAtEnd, light.count == 4, dark.count == 4 else {
            throw DecodingError.dataCorruptedError(in: c, debugDescription: "expected RGBA or light/dark RGBA")
        }
    }
}

/// A batch as the library sends it: JSON ops through the same typed decoder,
/// for tests that state a batch as dictionaries.
func wireBatch(_ ops: [[String: Any]]) -> Batch {
    let data = try! JSONSerialization.data(withJSONObject: ["ops": ops])
    let batch = Batch.decode(data)
    precondition(batch.error == nil, "a test batch did not decode: \(ops)")
    return batch
}

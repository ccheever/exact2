// D8's JSON Schema subset on Apple (LLP 1133.016): the checks every target
// shares (web/schema.js says the same in JavaScript), and the schema
// Foundation Models constrains decoding with, built from it.
//
// A schema arrives as JSON text or as a value `JSONSerialization` made, so
// an object's keys have no order here: an instance is written in the order
// of its schema's `required`, which the bake writes in the shape's order.
import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

enum LanguageModelSchema {
    private static let keys: [String: [String]] = [
        "object": ["properties", "required"], "string": ["enum", "pattern"], "number": ["minimum", "maximum"],
        "integer": ["minimum", "maximum"], "boolean": [], "array": ["items", "minItems", "maxItems"],
    ]
    private static let any = ["type", "description", "$defs", "$schema", "title"]

    /// A JSON bool, not a number `JSONSerialization` made from 0 or 1.
    static func isBool(_ v: Any?) -> Bool {
        guard let v else { return false }
        return CFGetTypeID(v as CFTypeRef) == CFBooleanGetTypeID()
    }

    static func number(_ v: Any?) -> Double? {
        guard let n = v as? NSNumber, !isBool(n) else { return nil }
        return n.doubleValue
    }

    private static func resolve(_ ref: Any?, _ root: [String: Any]) -> [String: Any]? {
        guard let ref = ref as? String, ref.hasPrefix("#/$defs/") else { return nil }
        return (root["$defs"] as? [String: Any])?[String(ref.dropFirst(8))] as? [String: Any]
    }

    /// Why `schema` is outside the subset, or nil.
    static func refusal(_ schema: Any, root: [String: Any]? = nil, at: String = "") -> String? {
        let place = at.isEmpty ? "the schema" : at
        guard let s = schema as? [String: Any] else { return "\(place) is not an object" }
        let root = root ?? s
        if let ref = s["$ref"] { return resolve(ref, root) != nil ? nil : "\(at): `\(ref)` names nothing in `$defs`" }
        guard let type = s["type"] as? String, let allowed = keys[type] else { return "\(place): type `\(s["type"] ?? "none")` is outside D8's subset" }
        for k in s.keys.sorted() where !allowed.contains(k) && !any.contains(k) { return "\(place): `\(k)` is outside D8's subset" }
        for (name, d) in ((s["$defs"] as? [String: Any]) ?? [:]).sorted(by: { $0.key < $1.key }) {
            if let why = refusal(d, root: root, at: "$defs.\(name)") { return why }
        }
        if type == "object" {
            for (name, p) in ((s["properties"] as? [String: Any]) ?? [:]).sorted(by: { $0.key < $1.key }) {
                if let why = refusal(p, root: root, at: at.isEmpty ? name : "\(at).\(name)") { return why }
            }
        }
        if type == "array", let items = s["items"] { return refusal(items, root: root, at: "\(at)[]") }
        return nil
    }

    /// Why `v` does not satisfy `schema`, or nil.
    static func violation(_ schema: [String: Any], _ v: Any?, root: [String: Any]? = nil, at: String = "") -> String? {
        let root = root ?? schema
        if let ref = schema["$ref"] { return violation(resolve(ref, root) ?? [:], v, root: root, at: at) }
        let place = at.isEmpty ? "the value" : at
        switch schema["type"] as? String {
        case "object":
            guard let o = v as? [String: Any] else { return "\(place) is not an object" }
            for r in (schema["required"] as? [String]) ?? [] where o[r] == nil { return "\(place) has no `\(r)`" }
            let properties = (schema["properties"] as? [String: Any]) ?? [:]
            for (k, x) in o.sorted(by: { $0.key < $1.key }) {
                guard let p = properties[k] as? [String: Any] else { return "\(place) has `\(k)`, which the schema does not" }
                if let why = violation(p, x, root: root, at: at.isEmpty ? k : "\(at).\(k)") { return why }
            }
            return nil
        case "string":
            guard let s = v as? String, !(v is NSNumber) else { return "\(place) is not a string" }
            if let choices = schema["enum"] as? [String], !choices.contains(s) { return "\(place) is not one of \(choices.joined(separator: ", "))" }
            if let pattern = schema["pattern"] as? String, s.range(of: pattern, options: .regularExpression) == nil { return "\(place) does not match \(pattern)" }
            return nil
        case "number", "integer":
            let integer = schema["type"] as? String == "integer"
            guard let n = number(v), n.isFinite, !integer || n.rounded() == n else { return "\(place) is not \(integer ? "an integer" : "a number")" }
            if let min = number(schema["minimum"]), n < min { return "\(place) is under \(text(min))" }
            if let max = number(schema["maximum"]), n > max { return "\(place) is over \(text(max))" }
            return nil
        case "boolean":
            return isBool(v) ? nil : "\(place) is not a bool"
        case "array":
            guard let a = v as? [Any] else { return "\(place) is not an array" }
            if let min = number(schema["minItems"]), Double(a.count) < min { return "\(place) has fewer than \(text(min)) items" }
            if let max = number(schema["maxItems"]), Double(a.count) > max { return "\(place) has more than \(text(max)) items" }
            if let items = schema["items"] as? [String: Any] {
                for (i, x) in a.enumerated() { if let why = violation(items, x, root: root, at: "\(place)[\(i)]") { return why } }
            }
            return nil
        default:
            return nil
        }
    }

    /// §9's first instance as JSON text: each required property in order, a
    /// string's first `enum` value or `""`, a number's `minimum` or 0,
    /// `false`, and an array of `minItems` first instances.
    static func firstInstance(_ schema: [String: Any], root: [String: Any]? = nil) -> String {
        let root = root ?? schema
        if let ref = schema["$ref"] { return firstInstance(resolve(ref, root) ?? [:], root: root) }
        switch schema["type"] as? String {
        case "object":
            let properties = (schema["properties"] as? [String: Any]) ?? [:]
            let fields = ((schema["required"] as? [String]) ?? []).compactMap { k in
                (properties[k] as? [String: Any]).map { "\(quoted(k)):\(firstInstance($0, root: root))" }
            }
            return "{\(fields.joined(separator: ","))}"
        case "string": return quoted((schema["enum"] as? [String])?.first ?? "")
        case "number", "integer": return number(schema["minimum"]).map(text) ?? "0"
        case "boolean": return "false"
        case "array":
            let n = Int(number(schema["minItems"]) ?? 0), item = firstInstance((schema["items"] as? [String: Any]) ?? [:], root: root)
            return "[\(Array(repeating: item, count: max(0, n)).joined(separator: ","))]"
        default: return "null"
        }
    }

    /// A number as JavaScript writes it: no `.0` on an integer.
    static func text(_ n: Double) -> String {
        n.rounded() == n && abs(n) < 1e15 ? String(Int64(n)) : String(n)
    }

    /// A JSON string literal.
    static func quoted(_ s: String) -> String {
        json(s) ?? "\"\""
    }

    /// `v` as JSON text, slashes unescaped, as JavaScript writes it; keys sorted.
    static func json(_ v: Any) -> String? {
        guard let data = try? JSONSerialization.data(withJSONObject: v, options: [.fragmentsAllowed, .withoutEscapingSlashes, .sortedKeys]) else { return nil }
        return String(decoding: data, as: UTF8.self)
    }

    static func parse(_ text: String) -> Any? {
        try? JSONSerialization.jsonObject(with: Data(text.utf8), options: [.fragmentsAllowed])
    }
}

#if canImport(FoundationModels)
@available(iOS 26.0, macOS 26.0, *)
extension LanguageModelSchema {
    struct Unsupported: Error { let why: String }

    /// The `GenerationSchema` Foundation Models constrains decoding with, from
    /// a schema `refusal` passed (D8's table): every `$defs` entry a dependency.
    static func generation(_ schema: [String: Any], name: String) throws -> GenerationSchema {
        let defs = (schema["$defs"] as? [String: Any]) ?? [:]
        let dependencies = try defs.sorted(by: { $0.key < $1.key }).map { name, d in try dynamic(d as? [String: Any] ?? [:], name: name) }
        return try GenerationSchema(root: try dynamic(schema, name: name), dependencies: dependencies)
    }

    private static func dynamic(_ s: [String: Any], name: String) throws -> DynamicGenerationSchema {
        if let ref = s["$ref"] as? String { return DynamicGenerationSchema(referenceTo: String(ref.dropFirst(8))) }
        let description = s["description"] as? String
        switch s["type"] as? String {
        case "object":
            let properties = (s["properties"] as? [String: Any]) ?? [:], required = Set((s["required"] as? [String]) ?? [])
            // Required ones first, in their order, then the rest by name.
            let order = ((s["required"] as? [String]) ?? []).filter { properties[$0] != nil } + properties.keys.filter { !required.contains($0) }.sorted()
            let fields = try order.map { k -> DynamicGenerationSchema.Property in
                let p = properties[k] as? [String: Any] ?? [:]
                return DynamicGenerationSchema.Property(name: k, description: p["description"] as? String,
                                                        schema: try dynamic(p, name: "\(name)_\(k)"), isOptional: !required.contains(k))
            }
            return DynamicGenerationSchema(name: name, description: description, properties: fields)
        case "string":
            if let choices = s["enum"] as? [String] { return DynamicGenerationSchema(name: name, description: description, anyOf: choices) }
            if let pattern = s["pattern"] as? String {
                guard let regex = try? Regex(pattern) else { throw Unsupported(why: "`pattern` \(pattern) is not a regular expression Swift reads") }
                return DynamicGenerationSchema(type: String.self, guides: [.pattern(regex)])
            }
            return DynamicGenerationSchema(type: String.self)
        case "integer":
            var guides: [GenerationGuide<Int>] = []
            if let min = number(s["minimum"]) { guides.append(.minimum(Int(min))) }
            if let max = number(s["maximum"]) { guides.append(.maximum(Int(max))) }
            return DynamicGenerationSchema(type: Int.self, guides: guides)
        case "number":
            var guides: [GenerationGuide<Double>] = []
            if let min = number(s["minimum"]) { guides.append(.minimum(min)) }
            if let max = number(s["maximum"]) { guides.append(.maximum(max)) }
            return DynamicGenerationSchema(type: Double.self, guides: guides)
        case "boolean":
            return DynamicGenerationSchema(type: Bool.self)
        case "array":
            let items = try dynamic(s["items"] as? [String: Any] ?? ["type": "string"], name: "\(name)_item")
            return DynamicGenerationSchema(arrayOf: items, minimumElements: number(s["minItems"]).map { Int($0) },
                                           maximumElements: number(s["maxItems"]).map { Int($0) })
        default:
            throw Unsupported(why: "type `\(s["type"] ?? "none")` is outside D8's subset")
        }
    }
}
#endif

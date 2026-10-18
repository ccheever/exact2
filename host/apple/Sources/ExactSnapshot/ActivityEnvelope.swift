// @ref LLP 1126.001 WP8 — one Codable contract for every app and state shape.
import Foundation
#if os(iOS)
import ActivityKit
#endif

public indirect enum ActivityJSON: Codable, Hashable, Sendable {
    case null, bool(Bool), number(Double), string(String)
    case array([ActivityJSON]), object([String: ActivityJSON])

    public init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer()
        if value.decodeNil() { self = .null }
        else if let bool = try? value.decode(Bool.self) { self = .bool(bool) }
        else if let number = try? value.decode(Double.self), number.isFinite { self = .number(number) }
        else if let string = try? value.decode(String.self) { self = .string(string) }
        else if let array = try? value.decode([ActivityJSON].self) { self = .array(array) }
        else { self = .object(try value.decode([String: ActivityJSON].self)) }
    }

    public func encode(to encoder: Encoder) throws {
        var value = encoder.singleValueContainer()
        switch self {
        case .null: try value.encodeNil()
        case let .bool(v): try value.encode(v)
        case let .number(v): try value.encode(v)
        case let .string(v): try value.encode(v)
        case let .array(v): try value.encode(v)
        case let .object(v): try value.encode(v)
        }
    }

    // The same conservative UTF-8 JSON accounting as the bake's payload_bytes:
    // 32 bytes per finite number, six per control/line separator, two per slash.
    fileprivate func encodedSizeBound() throws -> Int {
        func string(_ value: String) -> Int {
            2 + value.unicodeScalars.reduce(0) { count, scalar in
                switch scalar.value {
                case 0...31, 0x2028, 0x2029: return count + 6
                case 34, 47, 92: return count + 2
                default: return count + String(scalar).utf8.count
                }
            }
        }
        switch self {
        case .null: return 4
        case let .bool(value): return value ? 4 : 5
        case let .number(value):
            guard value.isFinite else { throw ExactActivityAttributes.Failure(description: "activity state needs finite JSON numbers") }
            return 32
        case let .string(value): return string(value)
        case let .array(items):
            return try 2 + max(0, items.count - 1) + items.reduce(0) { try $0 + $1.encodedSizeBound() }
        case let .object(fields):
            return try 2 + max(0, fields.count - 1) + fields.reduce(0) { try $0 + string($1.key) + 1 + $1.value.encodedSizeBound() }
        }
    }

}

public struct ExactActivityAttributes: Codable, Hashable, Sendable {
    public struct ContentState: Codable, Hashable, Sendable {
        public var shape: String
        public var state: ActivityJSON
        public init(shape: String, state: ActivityJSON) { self.shape = shape; self.state = state }
    }

    public var kind: String
    public var generation: String
    public var shape: String
    public init(kind: String, generation: String, shape: String) {
        self.kind = kind; self.generation = generation; self.shape = shape
    }

    public struct Failure: Error, CustomStringConvertible {
        public let description: String
    }

    /// Called before a start, update or end send reaches ActivityKit.
    /// `isStale` remains ActivityKit's fact; a decode failure is the driver's outdated state.
    @discardableResult public func validate(_ content: ContentState) throws -> Int {
        func digest(_ value: String) -> Bool {
            value.utf8.count == 64 && value.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) }
        }
        guard digest(generation), digest(shape), content.shape == shape else {
            throw Failure(description: "activity generation or state shape digest does not match")
        }
        let attributes = ActivityJSON.object(["kind": .string(kind), "generation": .string(generation), "shape": .string(shape)])
        let state = ActivityJSON.object(["shape": .string(content.shape), "state": content.state])
        let bytes = try attributes.encodedSizeBound() + state.encodedSizeBound()
        guard bytes <= 4096 else {
            throw Failure(description: "attributes and state budget \(bytes) bytes (conservative JSON bound); the limit is 4096 bytes; shorten the state")
        }
        return bytes
    }
}

#if os(iOS)
extension ExactActivityAttributes: ActivityAttributes {}
#endif
